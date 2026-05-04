use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::Arc;

use tauri::{Emitter, State};

use crate::cc::discovery::find_claude_binary;
use crate::cc::session::ChatSession;
use crate::cc::spawn::{build_command, SpawnArgs};
use crate::cc::stream::{dispatch, StreamEvent, StreamState};
use crate::workspace::state::ActiveWorkspace;

#[tauri::command]
pub async fn chat_send(
    app_handle: tauri::AppHandle,
    active: State<'_, ActiveWorkspace>,
    session: State<'_, ChatSession>,
    message: String,
    mode: String,
) -> Result<(), String> {
    let workspace_path = {
        let guard = active.0.lock().map_err(|e| e.to_string())?;
        guard.as_ref().ok_or("no workspace open")?.state.path.clone()
    };

    let bin = find_claude_binary().map_err(|e| e.to_string())?.path;

    let resume_id = session
        .session_id
        .lock()
        .map_err(|e| e.to_string())?
        .clone();

    let mcp_config = {
        let p = workspace_path.join(".ire/mcp.json");
        if p.exists() { Some(p) } else { None }
    };

    let system_prompt = build_system_prompt(&workspace_path, &mode);

    let pid_arc = Arc::clone(&session.running_pid);
    let sid_arc = Arc::clone(&session.session_id);

    tokio::task::spawn_blocking(move || {
        let mut cmd = build_command(&SpawnArgs {
            bin: &bin,
            workspace: &workspace_path,
            message: &message,
            resume_id: resume_id.as_deref(),
            mcp_config: mcp_config.as_deref(),
            system_prompt: Some(&system_prompt),
        });

        let mut child = cmd.spawn().map_err(|e| e.to_string())?;
        *pid_arc.lock().unwrap() = Some(child.id());

        let stdout = child.stdout.take().ok_or("no stdout")?;
        let mut state = StreamState::default();

        for line in BufReader::new(stdout).lines() {
            if let Ok(line) = line {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                    dispatch(&json, &mut state, &mut |event| {
                        if let StreamEvent::Init { ref session_id } = event {
                            *sid_arc.lock().unwrap() = Some(session_id.clone());
                        }
                        let _ = app_handle.emit("chat-stream", &event);
                    });
                }
            }
        }

        let _ = child.wait();
        *pid_arc.lock().unwrap() = None;

        let _ = app_handle.emit("chat-stream", &StreamEvent::Done);
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| format!("task error: {e}"))?
}

#[tauri::command]
pub fn chat_cancel(session: State<'_, ChatSession>) -> Result<(), String> {
    let pid = *session.running_pid.lock().map_err(|e| e.to_string())?;
    if let Some(pid) = pid {
        kill_process(pid);
    }
    Ok(())
}

#[tauri::command]
pub fn chat_reset_session(session: State<'_, ChatSession>) -> Result<(), String> {
    *session.session_id.lock().map_err(|e| e.to_string())? = None;
    Ok(())
}

/// Compose the system prompt from wiki context files per §7.4.
fn build_system_prompt(workspace_root: &Path, mode: &str) -> String {
    let wiki_root = workspace_root.join(".ire/wiki");

    let preamble = if mode == "experiment" {
        "You are IRE's experiment-mode assistant. You have access to wiki, memory, pulse, and experiment MCP tools as well as Bash, Edit, Write, and Read. After every experiment, update the wiki and pulse."
    } else {
        "You are IRE's brainstorm-mode assistant. You have access to wiki, memory, and pulse MCP tools. Use them to maintain persistent project knowledge across sessions."
    };

    let mut parts = vec![preamble.to_string()];

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
            if let Ok(content) = fs::read_to_string(wiki_root.join("status/short-term").join(name)) {
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
