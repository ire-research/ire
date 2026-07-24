//! The OpenCode-transport turn runner: session ensure/create/reuse,
//! resume-id persistence, tab-route registration, and `prompt_async` dispatch.

use std::path::{Path, PathBuf};

use tauri::AppHandle;

use crate::agent_provider;
use crate::opencode::events::OpenCodeSessionState;
use crate::opencode::runtime::{emit_stream, OpenCodeRuntime, RuntimeInner, TabRoute};
use crate::session::{RunningTurn, SessionManager};
use crate::stream_event::StreamEvent;

pub struct SendArgs<'a> {
    pub tab_id: &'a str,
    pub session_uuid: &'a str,
    pub tab_label: &'a str,
    pub started_at: &'a str,
    pub model: &'a str,
    pub effort: Option<&'a str>,
    pub message: &'a str,
    pub system_prompt: Option<&'a str>,
}

fn mcp_config_path(home_data_dir: &Path) -> Option<PathBuf> {
    let p = home_data_dir.join("mcp.json");
    p.exists().then_some(p)
}

fn opencode_bin() -> Result<PathBuf, String> {
    agent_provider::provider("opencode")
        .ok_or("opencode provider not registered")?
        .discover()
        .map_err(|e| e.to_string())
        .map(|b| b.path)
}

fn persist_resume_id(
    home_data_dir: &Path,
    session_uuid: &str,
    tab_label: &str,
    model: &str,
    started_at: &str,
    session_id: &str,
) {
    if let Err(e) = crate::db::models::upsert_chat_resume_id(
        home_data_dir,
        session_uuid,
        tab_label,
        "opencode",
        model,
        started_at,
        session_id,
    ) {
        tracing::warn!(session_uuid = %session_uuid, error = %e, "persist opencode resume id failed");
    }
}

/// Starts (or continues) one OpenCode turn. Used for chat sends, resource
/// summaries, and experiment wake-ups alike.
pub async fn send(
    app: &AppHandle,
    runtime: &OpenCodeRuntime,
    session_manager: &SessionManager,
    workspace: &Path,
    home_data_dir: &Path,
    args: SendArgs<'_>,
) -> Result<(), String> {
    let bin = opencode_bin()?;
    let mcp_config = mcp_config_path(home_data_dir);

    let inner = runtime
        .ensure_started(app, session_manager, workspace, &bin, mcp_config.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    session_manager.set_agent_options(
        args.tab_id,
        args.session_uuid,
        "opencode",
        args.model,
        args.effort,
    );

    let existing = crate::db::models::get_chat_resume_id(home_data_dir, args.session_uuid, "opencode")
        .ok()
        .flatten();

    let session_id = match existing {
        Some(id) => id,
        None => {
            let s = inner.client.create_session().await.map_err(|e| e.to_string())?;
            persist_resume_id(
                home_data_dir,
                args.session_uuid,
                args.tab_label,
                args.model,
                args.started_at,
                &s.id,
            );
            s.id
        }
    };

    let stream_id = format!("{}:{}", args.tab_id, uuid::Uuid::new_v4());
    register_route(&inner, &session_id, &args, &stream_id).await;
    emit_stream(
        app,
        args.tab_id,
        &stream_id,
        1,
        &StreamEvent::Init { session_id: session_id.clone() },
    );
    session_manager.set_running_opencode(args.tab_id, session_id.clone());

    let accepted = inner
        .client
        .prompt_async(&session_id, args.model, args.effort, args.system_prompt, args.message)
        .await
        .map_err(|e| e.to_string())?;

    if accepted {
        return Ok(());
    }

    // Session id unknown to this server — start fresh and retry once.
    tracing::warn!(tab_id = %args.tab_id, session_id = %session_id, "opencode session not found on server, retrying with a fresh session");
    let route = { inner.sessions.lock().await.remove(&session_id) };
    let fresh = inner.client.create_session().await.map_err(|e| e.to_string())?;
    persist_resume_id(
        home_data_dir,
        args.session_uuid,
        args.tab_label,
        args.model,
        args.started_at,
        &fresh.id,
    );
    if let Some(route) = route {
        inner.sessions.lock().await.insert(fresh.id.clone(), route);
    }
    session_manager.set_running_opencode(args.tab_id, fresh.id.clone());

    let ok = inner
        .client
        .prompt_async(&fresh.id, args.model, args.effort, args.system_prompt, args.message)
        .await
        .map_err(|e| e.to_string())?;

    if !ok {
        emit_stream(
            app,
            args.tab_id,
            &stream_id,
            2,
            &StreamEvent::Error {
                message: "opencode: could not start a session on the server".to_string(),
            },
        );
        emit_stream(app, args.tab_id, &stream_id, 3, &StreamEvent::Done);
        session_manager.clear_running_if(args.tab_id, &RunningTurn::OpenCode { session_id: fresh.id });
        return Err("opencode: failed to start turn after retry".to_string());
    }
    Ok(())
}

async fn register_route(inner: &RuntimeInner, session_id: &str, args: &SendArgs<'_>, stream_id: &str) {
    inner.sessions.lock().await.insert(
        session_id.to_string(),
        TabRoute {
            tab_id: args.tab_id.to_string(),
            stream_id: stream_id.to_string(),
            event_id: 1,
            state: OpenCodeSessionState::default(),
        },
    );
}

/// One-shot title generation: disposable session, blocking `/message` call.
/// Still passes `mcp_config` to `ensure_started` — if this call is the one
/// that starts the shared server, it must start with the same MCP config a
/// concurrent chat send would use, even though the title request itself
/// sends no tools.
pub async fn generate_title(
    app: &AppHandle,
    runtime: &OpenCodeRuntime,
    session_manager: &SessionManager,
    workspace: &Path,
    home_data_dir: &Path,
    model: &str,
    prompt: &str,
) -> Result<String, String> {
    let bin = opencode_bin()?;
    let mcp_config = mcp_config_path(home_data_dir);
    let inner = runtime
        .ensure_started(app, session_manager, workspace, &bin, mcp_config.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    let session = inner.client.create_session().await.map_err(|e| e.to_string())?;
    let text = inner
        .client
        .send_message_blocking(&session.id, model, prompt)
        .await
        .map_err(|e| e.to_string());
    inner.client.delete_session(&session.id).await;
    text
}
