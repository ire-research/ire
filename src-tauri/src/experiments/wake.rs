use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

use tauri::{AppHandle, Emitter};

use crate::cc::discovery::find_claude_binary;
use crate::cc::session::SessionManager;
use crate::cc::spawn::{build_command, SpawnArgs};
use crate::cc::stream::{dispatch, StreamEvent, StreamState};
use crate::commands::chat::build_system_prompt;
use crate::prompts::{self, WakeupArgs};

pub fn fire_wakeup(
    workspace_root: &Path,
    uuid: &str,
    exit_code: i32,
    tab_id: &str,
    session_id: &str,
    wake_prompt: &str,
    app: &AppHandle,
    session_manager: &SessionManager,
) {
    let ire_dir = workspace_root.join(".ire");
    let log_dir = ire_dir.join("logs").join(uuid);
    let plan_path = format!(".ire/experiments/{uuid}/plan.md");

    let stdout_tail = tail_file(&log_dir.join("stdout.log"), 8192);
    let stderr_tail = tail_file(&log_dir.join("stderr.log"), 8192);

    let message = prompts::experiment_wakeup(WakeupArgs {
        wake_prompt,
        uuid,
        exit_code,
        plan_path: &plan_path,
        stdout_tail: &stdout_tail,
        stderr_tail: &stderr_tail,
    });

    tracing::info!(uuid = %uuid, tab_id = %tab_id, "firing experiment wake-up");

    let bin = match find_claude_binary() {
        Ok(b) => b.path,
        Err(e) => {
            tracing::error!(error = %e, "wake-up: claude binary not found");
            return;
        }
    };

    let mcp_config = workspace_root.join(".ire/mcp.json");
    let mcp_config = if mcp_config.exists() { Some(mcp_config) } else { None };
    let system_prompt = build_system_prompt(workspace_root);

    let resume_id = Some(session_id.to_string());
    let mut cmd = build_command(&SpawnArgs {
        bin: &bin,
        workspace: workspace_root,
        message: &message,
        resume_id: resume_id.as_deref(),
        mcp_config: mcp_config.as_deref(),
        system_prompt: Some(&system_prompt),
        model: "claude-haiku-4-5-20251001",
        effort: "high",
    });

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = %e, "wake-up: failed to spawn claude");
            return;
        }
    };

    let pid = child.id();
    session_manager.set_pid(tab_id, pid);
    tracing::debug!(pid = pid, tab_id = %tab_id, "wake-up claude spawned");

    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            tracing::error!("wake-up: no stdout");
            return;
        }
    };

    let mut state = StreamState::default();
    let tab_id_owned = tab_id.to_string();

    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else { continue };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
        dispatch(&json, &mut state, &mut |event| {
            if let StreamEvent::Init { ref session_id } = event {
                session_manager.set_session_id(&tab_id_owned, session_id.clone());
            }
            let _ = app.emit(
                "chat-stream",
                serde_json::json!({ "tab_id": &tab_id_owned, "event": &event }),
            );
        });
    }

    let _ = child.wait();
    session_manager.clear_pid(tab_id);

    let _ = app.emit(
        "chat-stream",
        serde_json::json!({ "tab_id": tab_id, "event": &StreamEvent::Done }),
    );
}

fn tail_file(path: &Path, max_bytes: u64) -> String {
    let Ok(content) = fs::read(path) else { return String::new() };
    let len = content.len() as u64;
    let start = if len > max_bytes { (len - max_bytes) as usize } else { 0 };
    String::from_utf8_lossy(&content[start..]).into_owned()
}
