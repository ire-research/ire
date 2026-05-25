use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

use tauri::{Emitter, State};

use crate::cc::discovery::find_claude_binary;
use crate::codex::discovery::find_codex_binary;
use crate::codex::spawn::{build_codex_command, CodexSpawnArgs};

fn trunc(s: &str) -> &str {
    let end = s.char_indices().nth(80).map(|(i, _)| i).unwrap_or(s.len());
    &s[..end]
}
use crate::cc::session::SessionManager;
use crate::cc::spawn::{build_command, SpawnArgs};
use crate::cc::stream::{self as cc_stream, StreamEvent, StreamState};
use crate::codex::stream as codex_stream;
use crate::workspace::state::ActiveWorkspace;

#[derive(serde::Deserialize)]
pub struct ChatOptions {
    pub model: String,
    #[serde(default = "default_provider")]
    pub provider: String,
    pub effort: String,
}

fn default_provider() -> String {
    "claude".to_string()
}

#[tauri::command]
pub async fn chat_send(
    app_handle: tauri::AppHandle,
    active: State<'_, ActiveWorkspace>,
    session: State<'_, SessionManager>,
    tab_id: String,
    message: String,
    options: ChatOptions,
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
    let resume_id = session.get_session_id_for_provider(&tab_id, &provider);

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

    tracing::info!(
        tab_id = %tab_id,
        provider = %provider,
        model = %options.model,
        effort = %options.effort,
        msg = %trunc(&message),
        "chat_send"
    );

    let result = tokio::task::spawn_blocking(move || {
        let mut cmd = if provider == "codex" {
            build_codex_command(&CodexSpawnArgs {
                bin: &bin,
                workspace: &workspace_path,
                message: &message,
                model: &options.model,
                reasoning_effort: &options.effort,
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
                effort: &options.effort,
            })
        };

        let mut child = cmd.spawn().map_err(|e| e.to_string())?;
        let pid = child.id();
        session_clone.set_pid(&tab_id, pid);
        tracing::info!(tab_id = %tab_id, provider = %provider, pid = pid, resume = ?resume_id, "agent subprocess spawned");

        let stdout = child.stdout.take().ok_or("no stdout")?;
        let mut state = StreamState::default();

        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                let mut emit_event = |event: StreamEvent| {
                    if let StreamEvent::Init { ref session_id } = event {
                        session_clone.set_session_id_for_provider(
                            &tab_id,
                            &provider,
                            session_id.clone(),
                        );
                        tracing::debug!(tab_id = %tab_id, session_id = %session_id, "stream session init");
                    }
                    if let StreamEvent::Error { message: ref errmsg } = event {
                        tracing::warn!(tab_id = %tab_id, provider = %provider, error = %errmsg, "agent stream error");
                    }
                    let _ = app_handle.emit(
                        "chat-stream",
                        serde_json::json!({ "tab_id": &tab_id, "event": &event }),
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

        let _ = app_handle.emit(
            "chat-stream",
            serde_json::json!({ "tab_id": &tab_id, "event": &StreamEvent::Done }),
        );
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| format!("task error: {e}"))?;

    if let Err(ref e) = result {
        tracing::warn!(tab_id = %tab_id_outer, error = %e, "chat_send failed");
    }
    result
}

#[tauri::command]
pub fn chat_cancel(session: State<'_, SessionManager>, tab_id: String) -> Result<(), String> {
    tracing::debug!(tab_id = %tab_id, "chat_cancel");
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
    session.reset(&tab_id);
    Ok(())
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
