use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

use tauri::{Emitter, Manager, State};

use crate::agent_provider::{self, TurnRequest, TurnTransport};
use crate::opencode::runtime::OpenCodeRuntime;
use crate::session::{RunningTurn, SessionManager};
use crate::stream_event::{StreamEvent, StreamState};
use crate::workspace::state::ActiveWorkspace;

fn trunc(s: &str) -> &str {
    let end = s.char_indices().nth(80).map(|(i, _)| i).unwrap_or(s.len());
    &s[..end]
}

#[derive(Clone, serde::Deserialize)]
pub struct ChatOptions {
    pub model: String,
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default)]
    pub effort: Option<String>,
}

fn default_provider() -> String {
    "claude".to_string()
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn chat_send(
    app_handle: tauri::AppHandle,
    active: State<'_, ActiveWorkspace>,
    session: State<'_, SessionManager>,
    tab_id: String,
    message: String,
    options: ChatOptions,
    session_uuid: String,
    tab_label: String,
    started_at: String,
) -> Result<(), String> {
    let workspace_path = {
        let guard = active.0.lock().map_err(|e| e.to_string())?;
        guard
            .as_ref()
            .ok_or("no workspace open")?
            .state
            .path
            .clone()
    };

    let provider = options.provider.clone();
    let agent = agent_provider::provider(&provider)
        .ok_or_else(|| format!("unsupported provider: {provider}"))?;

    let home_data_dir = crate::workspace::init::require_home_data_dir(&workspace_path)?;
    let system_prompt = build_system_prompt(&workspace_path);

    tracing::info!(
        tab_id = %tab_id,
        provider = %provider,
        model = %options.model,
        effort = ?options.effort,
        msg = %trunc(&message),
        "chat_send"
    );

    if agent.transport() == TurnTransport::OpenCodeServer {
        let runtime = app_handle.state::<OpenCodeRuntime>();
        return crate::opencode::turn::send(
            &app_handle,
            &runtime,
            &session,
            &workspace_path,
            &home_data_dir,
            crate::opencode::turn::SendArgs {
                tab_id: &tab_id,
                session_uuid: &session_uuid,
                tab_label: &tab_label,
                started_at: &started_at,
                model: &options.model,
                effort: options.effort.as_deref(),
                message: &message,
                system_prompt: Some(&system_prompt),
            },
        )
        .await;
    }

    let cli = agent_provider::cli_turn(&provider)
        .ok_or_else(|| format!("provider {provider} has no CLI turn support"))?;
    let bin = agent.discover().map_err(|e| e.to_string())?.path;
    let resume_id = match crate::db::models::get_chat_resume_id(&home_data_dir, &session_uuid, &provider) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(tab_id = %tab_id, session_uuid = %session_uuid, provider = %provider, error = %e, "load resume id failed");
            None
        }
    };

    let mcp_config = {
        let p = home_data_dir.join("mcp.json");
        if p.exists() {
            Some(p)
        } else {
            None
        }
    };

    // Clone the SessionManager handle (cheap Arc clone) so it can move into spawn_blocking.
    let session_clone = (*session).clone();
    let tab_id_outer = tab_id.clone();
    // Owned copies for the blocking closure that persists the resume id on Init.
    let home_data_dir_cl = home_data_dir.clone();
    let session_uuid_cl = session_uuid.clone();
    let tab_label_cl = tab_label.clone();
    let started_at_cl = started_at.clone();

    let result = tokio::task::spawn_blocking(move || {
        session_clone.set_agent_options(
            &tab_id,
            &session_uuid_cl,
            &provider,
            &options.model,
            options.effort.as_deref(),
        );

        let mut cmd = cli.build_command(
            &bin,
            &TurnRequest {
                workspace: &workspace_path,
                message: &message,
                model: &options.model,
                effort: options.effort.as_deref(),
                resume_id: resume_id.as_deref(),
                mcp_config: mcp_config.as_deref(),
                system_prompt: Some(&system_prompt),
            },
        );

        let mut child = cmd
            .spawn()
            .map_err(|e| cli.normalize_spawn_error(&e).to_string())?;
        let pid = child.id();
        let stream_id = format!("{tab_id}:{}", uuid::Uuid::new_v4());
        let mut event_id = 0_u64;
        session_clone.set_pid(&tab_id, pid);
        tracing::info!(tab_id = %tab_id, provider = %provider, pid = pid, resume = ?resume_id, "agent subprocess spawned");

        let stdout = child.stdout.take().ok_or("no stdout")?;
        let mut state = StreamState::default();

        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                let mut emit_event = |event: StreamEvent| {
                    if let StreamEvent::Init { ref session_id } = event {
                        if let Err(e) = crate::db::models::upsert_chat_resume_id(
                            &home_data_dir_cl,
                            &session_uuid_cl,
                            &tab_label_cl,
                            &provider,
                            &options.model,
                            &started_at_cl,
                            session_id,
                        ) {
                            tracing::warn!(tab_id = %tab_id, error = %e, "persist resume id failed");
                            let _ = app_handle.emit(
                                "error",
                                serde_json::json!({
                                    "scope": "resume id",
                                    "message": format!(
                                        "Couldn't save the resume id for \"{tab_label_cl}\" — reopening this chat will start a new conversation instead of continuing it. ({e})"
                                    ),
                                }),
                            );
                        }
                        tracing::debug!(tab_id = %tab_id, session_id = %session_id, "stream session init");
                    }
                    if let StreamEvent::Error { message: ref errmsg } = event {
                        tracing::warn!(tab_id = %tab_id, provider = %provider, error = %errmsg, "agent stream error");
                    }
                    event_id += 1;
                    let _ = app_handle.emit(
                        "chat-stream",
                        serde_json::json!({
                            "tab_id": &tab_id,
                            "stream_id": &stream_id,
                            "event_id": event_id,
                            "event": &event,
                        }),
                    );
                };
                cli.dispatch(&json, &mut state, &mut emit_event);
            }
        }

        let _ = child.wait();
        // Only clear if our PID is still current — fire_wakeup may have already
        // registered the wake-up CC subprocess under the same tab_id.
        session_clone.clear_running_if(&tab_id, &RunningTurn::Process(pid));
        tracing::info!(tab_id = %tab_id, "chat_send complete");

        if !state.emitted_done {
            event_id += 1;
            let _ = app_handle.emit(
                "chat-stream",
                serde_json::json!({
                    "tab_id": &tab_id,
                    "stream_id": &stream_id,
                    "event_id": event_id,
                    "event": &StreamEvent::Done,
                }),
            );
        }
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| format!("task error: {e}"))?;

    if let Err(ref e) = result {
        tracing::warn!(tab_id = %tab_id_outer, error = %e, "chat_send failed");
    }
    result
}

/// Generate a short descriptive title for a brand-new chat from its first user
/// message. One-shot: no system prompt, no MCP, no session resume, no tools.
#[tauri::command]
pub async fn generate_chat_title(
    app_handle: tauri::AppHandle,
    active: State<'_, ActiveWorkspace>,
    message: String,
    model: String,
    provider: String,
) -> Result<String, String> {
    let workspace_path = {
        let guard = active.0.lock().map_err(|e| e.to_string())?;
        guard
            .as_ref()
            .ok_or("no workspace open")?
            .state
            .path
            .clone()
    };

    let agent = agent_provider::provider(&provider)
        .ok_or_else(|| format!("unsupported provider: {provider}"))?;

    let prompt = crate::prompts::chat_title(&message);

    let title = if agent.transport() == TurnTransport::OpenCodeServer {
        let runtime = app_handle.state::<OpenCodeRuntime>();
        let session = app_handle.state::<SessionManager>();
        let home_data_dir = crate::workspace::init::require_home_data_dir(&workspace_path)?;
        crate::opencode::turn::generate_title(&app_handle, &runtime, &session, &workspace_path, &home_data_dir, &model, &prompt).await?
    } else {
        let cli = agent_provider::cli_turn(&provider)
            .ok_or_else(|| format!("provider {provider} has no CLI turn support"))?;
        let bin = agent.discover().map_err(|e| e.to_string())?.path;

        tokio::task::spawn_blocking(move || {
            // `title_request` gives the no-resume/no-MCP/no-system-prompt shape;
            // the frontend picks which lightweight model to use.
            let req = cli.title_request(&workspace_path, &prompt, &model);
            let mut cmd = cli.build_command(&bin, &req);

            let mut child = cmd
                .spawn()
                .map_err(|e| cli.normalize_spawn_error(&e).to_string())?;
            let stdout = child.stdout.take().ok_or("no stdout")?;
            let mut state = StreamState::default();
            let mut collected = String::new();

            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                    let mut collect = |event: StreamEvent| match event {
                        StreamEvent::TextDelta { text } => collected.push_str(&text),
                        StreamEvent::Result {
                            text: Some(text), ..
                        } => collected.push_str(&text),
                        _ => {}
                    };
                    cli.dispatch(&json, &mut state, &mut collect);
                }
            }
            let _ = child.wait();
            Ok::<String, String>(collected)
        })
        .await
        .map_err(|e| format!("task error: {e}"))??
    };

    let cleaned = clean_title(&title);
    if cleaned.is_empty() {
        return Err("empty title".to_string());
    }
    tracing::info!(title = %cleaned, "generate_chat_title");
    Ok(cleaned)
}

/// Normalise raw model output into a single-line, quote-free, length-capped title.
fn clean_title(raw: &str) -> String {
    let first_line = raw.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let trimmed = first_line
        .trim()
        .trim_matches(|c| c == '"' || c == '\'' || c == '`')
        .trim();
    trimmed.chars().take(60).collect()
}

#[tauri::command]
pub async fn chat_cancel(
    session: State<'_, SessionManager>,
    runtime: State<'_, OpenCodeRuntime>,
    tab_id: String,
) -> Result<(), String> {
    tracing::debug!(tab_id = %tab_id, "chat_cancel");
    session.cancel_ask(&tab_id);
    if let Some((running, provider)) = session.get_running_and_provider(&tab_id) {
        match running {
            RunningTurn::Process(pid) => {
                tracing::info!(tab_id = %tab_id, pid = pid, "cancelling agent subprocess");
                match provider.as_deref().and_then(agent_provider::cli_turn) {
                    Some(cli) => cli.cancel(pid),
                    None => kill_process(pid),
                }
            }
            RunningTurn::OpenCode { session_id } => {
                tracing::info!(tab_id = %tab_id, session_id = %session_id, "cancelling opencode session");
                if let Some(inner) = runtime.current().await {
                    if let Some(request_id) = session.take_pending_opencode_question(&tab_id) {
                        let _ = inner.client.reject_question(&request_id).await;
                    }
                    let _ = inner.client.abort_session(&session_id).await;
                }
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub fn chat_reset_session(
    session: State<'_, SessionManager>,
    tab_id: String,
) -> Result<(), String> {
    tracing::info!(tab_id = %tab_id, "chat_reset_session");
    session.cancel_ask(&tab_id);
    session.reset(&tab_id);
    Ok(())
}

/// Delivers the user's answers: wakes the MCP handler for Claude/Codex, or
/// posts directly to OpenCode's native question endpoint.
#[tauri::command]
pub async fn submit_ask_answer(
    session: State<'_, SessionManager>,
    runtime: State<'_, OpenCodeRuntime>,
    tab_id: String,
    answers: Vec<serde_json::Value>,
) -> Result<(), String> {
    tracing::debug!(tab_id = %tab_id, "submit_ask_answer");

    if let Some(request_id) = session.peek_pending_opencode_question(&tab_id) {
        let inner = runtime.current().await.ok_or("opencode server is not running")?;
        let normalized: Vec<Vec<String>> = answers.iter().map(normalize_opencode_answer).collect();
        return match inner.client.reply_question(&request_id, normalized).await {
            Ok(()) => {
                session.take_pending_opencode_question(&tab_id);
                Ok(())
            }
            // Keep the id pending on failure — OpenCode is still waiting
            // server-side, so the frontend can retry the same request.
            Err(e) => Err(e.to_string()),
        };
    }

    if session.submit_ask(&tab_id, answers) {
        Ok(())
    } else {
        Err("no pending question for this tab".to_string())
    }
}

/// OpenCode wants each answer as an array of selected option labels.
fn normalize_opencode_answer(value: &serde_json::Value) -> Vec<String> {
    match value {
        serde_json::Value::String(s) => vec![s.clone()],
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

/// Compose the always-injected system prompt: `_SYSTEM.md`, the focus, the
/// resources index, long-term memory, and the two most recent short-term notes.
/// Notes / ideas / experiments and individual resources are read on demand by
/// the agent via tools.
pub fn build_system_prompt(workspace_root: &Path) -> String {
    let ire_root = workspace_root.join(".ire");
    let store = crate::ire::IreStore::new(workspace_root.to_path_buf());

    let mut parts: Vec<String> = Vec::new();

    // Static IRE framework context — always first.
    if let Ok(content) = fs::read_to_string(ire_root.join("_SYSTEM.md")) {
        if !content.trim().is_empty() {
            parts.push(content);
        }
    }

    // Current focus (research question + this week), from ire.json.
    if let Ok(ire) = store.read_ire() {
        let focus = crate::ire::focus_prompt_block(&ire.focus);
        if !focus.is_empty() {
            parts.push(focus);
        }
    }

    if let Ok(content) = fs::read_to_string(store.resources_dir.join("_index.md")) {
        if !content.trim().is_empty() {
            parts.push(format!("### resources/_index.md\n\n{content}"));
        }
    }

    if let Ok(content) = fs::read_to_string(ire_root.join("long-term.md")) {
        if !content.trim().is_empty() {
            parts.push(format!("### long-term.md\n\n{content}"));
        }
    }

    // Inject the two most recent short-term day files.
    let short_term = ire_root.join("short-term");
    if let Ok(entries) = fs::read_dir(&short_term) {
        let mut names: Vec<String> = entries
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.ends_with(".md"))
            .collect();
        names.sort();
        names.reverse();
        for name in names.iter().take(2) {
            if let Ok(content) = fs::read_to_string(short_term.join(name)) {
                if !content.trim().is_empty() {
                    parts.push(format!("### short-term/{name}\n\n{content}"));
                }
            }
        }
    }

    parts.join("\n\n---\n\n")
}

#[cfg(unix)]
pub(crate) fn kill_process(pid: u32) {
    unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
}

#[cfg(windows)]
pub(crate) fn kill_process(pid: u32) {
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .output();
}

// Suppress dead-code lint for platforms where neither cfg applies.
#[cfg(not(any(unix, windows)))]
pub(crate) fn kill_process(_pid: u32) {}

#[cfg(test)]
mod tests {
    use super::{clean_title, normalize_opencode_answer};

    #[test]
    fn clean_title_strips_quotes_and_takes_first_line() {
        assert_eq!(
            clean_title("\"Quantum Error Correction\""),
            "Quantum Error Correction"
        );
        assert_eq!(
            clean_title("  Tuning a Sampler  \nextra"),
            "Tuning a Sampler"
        );
        assert_eq!(clean_title(""), "");
    }

    #[test]
    fn normalize_opencode_answer_wraps_single_string() {
        assert_eq!(
            normalize_opencode_answer(&serde_json::json!("Option A")),
            vec!["Option A".to_string()]
        );
    }

    #[test]
    fn normalize_opencode_answer_passes_through_array() {
        assert_eq!(
            normalize_opencode_answer(&serde_json::json!(["A", "B"])),
            vec!["A".to_string(), "B".to_string()]
        );
    }
}
