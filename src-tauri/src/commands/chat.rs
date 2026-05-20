use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

use tauri::{Emitter, State};

use crate::cc::discovery::find_claude_binary;

fn trunc(s: &str) -> &str {
    let end = s.char_indices().nth(80).map(|(i, _)| i).unwrap_or(s.len());
    &s[..end]
}
use crate::cc::session::SessionManager;
use crate::cc::spawn::{build_command, SpawnArgs};
use crate::cc::stream::{dispatch, StreamEvent, StreamState};
use crate::workspace::state::ActiveWorkspace;

#[derive(serde::Deserialize)]
pub struct ChatOptions {
    pub model: String,
    pub effort: String,
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
        guard.as_ref().ok_or("no workspace open")?.state.path.clone()
    };

    let bin = find_claude_binary().map_err(|e| e.to_string())?.path;
    let resume_id = session.get_session_id(&tab_id);

    let mcp_config = {
        let p = workspace_path.join(".ire/mcp.json");
        if p.exists() { Some(p) } else { None }
    };

    let system_prompt = build_system_prompt(&workspace_path);

    // Clone the SessionManager handle (cheap Arc clone) so it can move into spawn_blocking.
    let session_clone = (*session).clone();
    let tab_id_outer = tab_id.clone();

    tracing::info!(tab_id = %tab_id, model = %options.model, effort = %options.effort, msg = %trunc(&message), "chat_send");

    let result = tokio::task::spawn_blocking(move || {
        let mut cmd = build_command(&SpawnArgs {
            bin: &bin,
            workspace: &workspace_path,
            message: &message,
            resume_id: resume_id.as_deref(),
            mcp_config: mcp_config.as_deref(),
            system_prompt: Some(&system_prompt),
            model: &options.model,
            effort: &options.effort,
        });

        let mut child = cmd.spawn().map_err(|e| e.to_string())?;
        let pid = child.id();
        session_clone.set_pid(&tab_id, pid);
        tracing::info!(tab_id = %tab_id, pid = pid, resume = ?resume_id, "claude subprocess spawned");

        let stdout = child.stdout.take().ok_or("no stdout")?;
        let mut state = StreamState::default();

        for line in BufReader::new(stdout).lines() {
            if let Ok(line) = line {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                    dispatch(&json, &mut state, &mut |event| {
                        if let StreamEvent::Init { ref session_id } = event {
                            session_clone.set_session_id(&tab_id, session_id.clone());
                            tracing::debug!(tab_id = %tab_id, session_id = %session_id, "stream session init");
                        }
                        if let StreamEvent::Error { message: ref errmsg } = event {
                            tracing::warn!(tab_id = %tab_id, error = %errmsg, "claude stream error");
                        }
                        let _ = app_handle.emit(
                            "chat-stream",
                            serde_json::json!({ "tab_id": &tab_id, "event": &event }),
                        );
                    });
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
pub fn chat_reset_session(session: State<'_, SessionManager>, tab_id: String) -> Result<(), String> {
    tracing::info!(tab_id = %tab_id, "chat_reset_session");
    session.reset(&tab_id);
    Ok(())
}

/// Compose the system prompt from wiki context files per §7.4.
pub fn build_system_prompt(workspace_root: &Path) -> String {
    let wiki_root = workspace_root.join(".ire/wiki");

    let mut parts: Vec<String> = Vec::new();

    // Static IRE framework context — always first.
    if let Ok(content) = fs::read_to_string(wiki_root.join("_SYSTEM.md")) {
        if !content.trim().is_empty() {
            parts.push(content);
        }
    }

    for rel in &[
        "_schema.md",
        "_index.md",
        "status/pulse.md",
        "status/long-term.md",
        "status/failures.md",
    ] {
        if let Ok(content) = fs::read_to_string(wiki_root.join(rel)) {
            if !content.trim().is_empty() {
                parts.push(format!("### {rel}\n\n{content}"));
            }
        }
    }

    // Inject the two most recent short-term day files.
    if let Ok(entries) = fs::read_dir(wiki_root.join("status/short-term")) {
        let mut names: Vec<String> = entries
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.ends_with(".md"))
            .collect();
        names.sort();
        names.reverse();
        for name in names.iter().take(2) {
            if let Ok(content) =
                fs::read_to_string(wiki_root.join("status/short-term").join(name))
            {
                if !content.trim().is_empty() {
                    parts.push(format!("### status/short-term/{name}\n\n{content}"));
                }
            }
        }
    }

    parts.join("\n\n---\n\n")
}

#[cfg(unix)]
fn kill_process(pid: u32) {
    unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
}

#[cfg(windows)]
fn kill_process(pid: u32) {
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .output();
}

// Suppress dead-code lint for platforms where neither cfg applies.
#[cfg(not(any(unix, windows)))]
fn kill_process(_pid: u32) {}
