use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

use tauri::{AppHandle, Emitter};

use crate::agent_provider::{self, TurnRequest};
use crate::commands::chat::build_system_prompt;
use crate::prompts::{self, WakeupArgs};
use crate::session::SessionManager;
use crate::stream_event::{StreamEvent, StreamState};

pub struct FireWakeupArgs<'a> {
    pub workspace_root: &'a Path,
    pub uuid: &'a str,
    pub exit_code: i32,
    pub tab_id: &'a str,
    pub session_uuid: &'a str,
    pub provider: &'a str,
    pub model: &'a str,
    pub effort: Option<&'a str>,
    pub wake_prompt: &'a str,
    pub app: &'a AppHandle,
    pub session_manager: &'a SessionManager,
}

pub fn fire_wakeup(args: FireWakeupArgs<'_>) {
    let FireWakeupArgs {
        workspace_root,
        uuid,
        exit_code,
        tab_id,
        session_uuid,
        provider,
        model,
        effort,
        wake_prompt,
        app,
        session_manager,
    } = args;

    let Some(home_data_dir) = crate::workspace::init::home_data_dir(workspace_root) else {
        tracing::warn!("cannot determine home directory for experiment wake-up");
        return;
    };

    let Some(agent) = agent_provider::provider(provider) else {
        tracing::error!(provider = %provider, "wake-up: unsupported provider");
        return;
    };

    let exp_dir = workspace_root
        .join(".ire/cache/experiments")
        .join(uuid);

    let stdout_tail = tail_file(&exp_dir.join("stdout.log"), 8192);
    let stderr_tail = tail_file(&exp_dir.join("stderr.log"), 8192);

    let message = prompts::experiment_wakeup(WakeupArgs {
        wake_prompt,
        uuid,
        exit_code,
        stdout_tail: &stdout_tail,
        stderr_tail: &stderr_tail,
    });

    let mcp_config = home_data_dir.join("mcp.json");
    let mcp_config = if mcp_config.exists() {
        Some(mcp_config)
    } else {
        None
    };
    let system_prompt = build_system_prompt(workspace_root);

    tracing::info!(uuid = %uuid, tab_id = %tab_id, provider = %provider, "firing experiment wake-up");

    let bin = match agent.discover() {
        Ok(b) => b.path,
        Err(e) => {
            tracing::error!(error = %e, provider = %provider, "wake-up: binary not found");
            return;
        }
    };

    let resume_id = match crate::db::models::get_chat_resume_id(&home_data_dir, session_uuid, provider) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, session_uuid = %session_uuid, provider = %provider, "wake-up: load resume id failed");
            None
        }
    };

    let mut cmd = agent.build_command(
        &bin,
        &TurnRequest {
            workspace: workspace_root,
            message: &message,
            model,
            effort,
            resume_id: resume_id.as_deref(),
            mcp_config: mcp_config.as_deref(),
            system_prompt: Some(&system_prompt),
        },
    );

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = %agent.normalize_spawn_error(&e), provider = %provider, "wake-up: failed to spawn agent");
            return;
        }
    };

    let pid = child.id();
    session_manager.set_pid(tab_id, pid);
    tracing::debug!(pid = pid, tab_id = %tab_id, provider = %provider, "wake-up agent spawned");

    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            tracing::error!("wake-up: no stdout");
            return;
        }
    };

    let mut state = StreamState::default();
    let tab_id_owned = tab_id.to_string();
    session_manager.set_agent_options(tab_id, session_uuid, provider, model, effort);
    let stream_id = format!("{tab_id}:{}", uuid::Uuid::new_v4());
    let mut event_id = 0_u64;

    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else { continue };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let mut emit_event = |event: StreamEvent| {
            if let StreamEvent::Init { ref session_id } = event {
                let _ = crate::db::models::update_chat_resume_id(
                    &home_data_dir,
                    session_uuid,
                    provider,
                    session_id,
                );
            }
            event_id += 1;
            let _ = app.emit(
                "chat-stream",
                serde_json::json!({
                    "tab_id": &tab_id_owned,
                    "stream_id": &stream_id,
                    "event_id": event_id,
                    "event": &event,
                }),
            );
        };
        agent.dispatch(&json, &mut state, &mut emit_event);
    }

    let _ = child.wait();
    session_manager.clear_pid(tab_id);

    if !state.emitted_done {
        event_id += 1;
        let _ = app.emit(
            "chat-stream",
            serde_json::json!({
                "tab_id": tab_id,
                "stream_id": &stream_id,
                "event_id": event_id,
                "event": &StreamEvent::Done,
            }),
        );
    }
}

fn tail_file(path: &Path, max_bytes: u64) -> String {
    let Ok(content) = fs::read(path) else {
        return String::new();
    };
    let len = content.len() as u64;
    let start = if len > max_bytes {
        (len - max_bytes) as usize
    } else {
        0
    };
    String::from_utf8_lossy(&content[start..]).into_owned()
}
