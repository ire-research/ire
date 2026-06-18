use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

use tauri::{Emitter, State};

use crate::claude_code::discovery::find_claude_binary;
use crate::codex::discovery::find_codex_binary;
use crate::codex::spawn::{build_codex_command, CodexSpawnArgs};

fn trunc(s: &str) -> &str {
    let end = s.char_indices().nth(80).map(|(i, _)| i).unwrap_or(s.len());
    &s[..end]
}
use crate::claude_code::session::SessionManager;
use crate::claude_code::spawn::{build_command, SpawnArgs};
use crate::claude_code::stream::{self as cc_stream, StreamEvent, StreamState};
use crate::codex::stream as codex_stream;
use crate::workspace::state::ActiveWorkspace;

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
    if provider != "claude" && provider != "codex" {
        return Err(format!("unsupported provider: {provider}"));
    }

    let bin = if provider == "codex" {
        find_codex_binary().map_err(|e| e.to_string())?.path
    } else {
        find_claude_binary().map_err(|e| e.to_string())?.path
    };
    let ire_dir = workspace_path.join(".ire");
    let resume_id =
        crate::db::models::get_chat_resume_id(&ire_dir, &session_uuid, &provider).unwrap_or(None);

    let mcp_config = {
        let p = workspace_path.join(".ire/mcp.json");
        if p.exists() {
            Some(p)
        } else {
            None
        }
    };

    let system_prompt = build_system_prompt(&workspace_path);

    // Clone the SessionManager handle (cheap Arc clone) so it can move into spawn_blocking.
    let session_clone = (*session).clone();
    let tab_id_outer = tab_id.clone();
    // Owned copies for the blocking closure that persists the resume id on Init.
    let ire_dir_cl = ire_dir.clone();
    let session_uuid_cl = session_uuid.clone();
    let tab_label_cl = tab_label.clone();
    let started_at_cl = started_at.clone();

    tracing::info!(
        tab_id = %tab_id,
        provider = %provider,
        model = %options.model,
        effort = ?options.effort,
        msg = %trunc(&message),
        "chat_send"
    );

    let result = tokio::task::spawn_blocking(move || {
        session_clone.set_agent_options(
            &tab_id,
            &session_uuid_cl,
            &provider,
            &options.model,
            options.effort.as_deref(),
        );

        let mut cmd = if provider == "codex" {
            build_codex_command(&CodexSpawnArgs {
                bin: &bin,
                workspace: &workspace_path,
                message: &message,
                model: &options.model,
                reasoning_effort: options.effort.as_deref().unwrap_or("low"),
                system_prompt: Some(&system_prompt),
                mcp_config: mcp_config.as_deref(),
                resume_id: resume_id.as_deref(),
            })
        } else {
            build_command(&SpawnArgs {
                bin: &bin,
                workspace: &workspace_path,
                message: &message,
                resume_id: resume_id.as_deref(),
                mcp_config: mcp_config.as_deref(),
                system_prompt: Some(&system_prompt),
                model: &options.model,
                effort: options.effort.as_deref(),
            })
        };

        let mut child = cmd.spawn().map_err(|e| e.to_string())?;
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
                            &ire_dir_cl,
                            &session_uuid_cl,
                            &tab_label_cl,
                            &provider,
                            &options.model,
                            &started_at_cl,
                            session_id,
                        ) {
                            tracing::warn!(tab_id = %tab_id, error = %e, "persist resume id failed");
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
                if provider == "codex" {
                    codex_stream::dispatch(&json, &mut state, &mut emit_event);
                } else {
                    cc_stream::dispatch(&json, &mut state, &mut emit_event);
                }
            }
        }

        let _ = child.wait();
        // Only clear if our PID is still current — fire_wakeup may have already
        // registered the wake-up CC subprocess under the same tab_id.
        if session_clone.get_pid(&tab_id) == Some(pid) {
            session_clone.clear_pid(&tab_id);
        }
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
/// message. One-shot: spawns a fresh lightweight-model subprocess (no system
/// prompt, no MCP, no session resume, no tools) and returns the collected text.
#[tauri::command]
pub async fn generate_chat_title(
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

    if provider != "claude" && provider != "codex" {
        return Err(format!("unsupported provider: {provider}"));
    }

    let bin = if provider == "codex" {
        find_codex_binary().map_err(|e| e.to_string())?.path
    } else {
        find_claude_binary().map_err(|e| e.to_string())?.path
    };

    let prompt = crate::prompts::chat_title(&message);

    let title = tokio::task::spawn_blocking(move || {
        let mut cmd = if provider == "codex" {
            build_codex_command(&CodexSpawnArgs {
                bin: &bin,
                workspace: &workspace_path,
                message: &prompt,
                model: &model,
                reasoning_effort: "low",
                system_prompt: None,
                mcp_config: None,
                resume_id: None,
            })
        } else {
            build_command(&SpawnArgs {
                bin: &bin,
                workspace: &workspace_path,
                message: &prompt,
                resume_id: None,
                mcp_config: None,
                system_prompt: None,
                model: &model,
                effort: None,
            })
        };

        let mut child = cmd.spawn().map_err(|e| e.to_string())?;
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
                if provider == "codex" {
                    codex_stream::dispatch(&json, &mut state, &mut collect);
                } else {
                    cc_stream::dispatch(&json, &mut state, &mut collect);
                }
            }
        }
        let _ = child.wait();
        Ok::<String, String>(collected)
    })
    .await
    .map_err(|e| format!("task error: {e}"))??;

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
pub fn chat_cancel(session: State<'_, SessionManager>, tab_id: String) -> Result<(), String> {
    tracing::debug!(tab_id = %tab_id, "chat_cancel");
    session.cancel_ask(&tab_id);
    if let Some(pid) = session.get_pid(&tab_id) {
        tracing::info!(tab_id = %tab_id, pid = pid, "cancelling claude subprocess");
        kill_process(pid);
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

/// Deliver the user's answers for a pending `ask_user_question` MCP call. The
/// blocked MCP handler (in mcp/rpc.rs) is woken up and returns the answers as
/// the tool_result, letting the same subprocess continue.
#[tauri::command]
pub fn submit_ask_answer(
    session: State<'_, SessionManager>,
    tab_id: String,
    answers: Vec<serde_json::Value>,
) -> Result<(), String> {
    tracing::debug!(tab_id = %tab_id, "submit_ask_answer");
    if session.submit_ask(&tab_id, answers) {
        Ok(())
    } else {
        Err("no pending question for this tab".to_string())
    }
}

/// Compose the system prompt from wiki context files per §7.4.
pub fn build_system_prompt(workspace_root: &Path) -> String {
    let ire_root = workspace_root.join(".ire");
    let wiki_root = workspace_root.join(".ire/wiki");

    let mut parts: Vec<String> = Vec::new();

    // Static IRE framework context — always first.
    if let Ok(content) = fs::read_to_string(ire_root.join("_SYSTEM.md")) {
        if !content.trim().is_empty() {
            parts.push(content);
        }
    }

    for rel in &["_index.md", "pulse.json", "long-term.md"] {
        if let Ok(content) = fs::read_to_string(wiki_root.join(rel)) {
            if !content.trim().is_empty() {
                parts.push(format!("### {rel}\n\n{content}"));
            }
        }
    }

    // Inject the two most recent short-term day files.
    if let Ok(entries) = fs::read_dir(wiki_root.join("short-term")) {
        let mut names: Vec<String> = entries
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.ends_with(".md"))
            .collect();
        names.sort();
        names.reverse();
        for name in names.iter().take(2) {
            if let Ok(content) = fs::read_to_string(wiki_root.join("short-term").join(name)) {
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
    use super::clean_title;

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
}
