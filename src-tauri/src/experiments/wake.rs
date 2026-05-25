use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

use tauri::{AppHandle, Emitter};

use crate::cc::discovery::find_claude_binary;
use crate::cc::session::SessionManager;
use crate::cc::spawn::{build_command, SpawnArgs};
use crate::cc::stream::{self as cc_stream, StreamEvent, StreamState};
use crate::codex::discovery::find_codex_binary;
use crate::codex::spawn::{build_codex_command, CodexSpawnArgs};
use crate::codex::stream as codex_stream;
use crate::commands::chat::build_system_prompt;
use crate::prompts::{self, WakeupArgs};

pub struct FireWakeupArgs<'a> {
    pub workspace_root: &'a Path,
    pub uuid: &'a str,
    pub exit_code: i32,
    pub tab_id: &'a str,
    pub session_id: &'a str,
    pub provider: &'a str,
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
        session_id,
        provider,
        wake_prompt,
        app,
        session_manager,
    } = args;

    let ire_dir = workspace_root.join(".ire");
    let exp_dir = ire_dir.join("wiki/experiments").join(uuid);

    let stdout_tail = tail_file(&exp_dir.join("stdout.log"), 8192);
    let stderr_tail = tail_file(&exp_dir.join("stderr.log"), 8192);

    let message = prompts::experiment_wakeup(WakeupArgs {
        wake_prompt,
        uuid,
        exit_code,
        stdout_tail: &stdout_tail,
        stderr_tail: &stderr_tail,
    });

    let mcp_config = workspace_root.join(".ire/mcp.json");
    let mcp_config = if mcp_config.exists() {
        Some(mcp_config)
    } else {
        None
    };
    let system_prompt = build_system_prompt(workspace_root);

    tracing::info!(uuid = %uuid, tab_id = %tab_id, provider = %provider, "firing experiment wake-up");

    let provider = if provider == "codex" {
        "codex"
    } else {
        "claude"
    };
    let bin = match provider {
        "codex" => match find_codex_binary() {
            Ok(b) => b.path,
            Err(e) => {
                tracing::error!(error = %e, "wake-up: codex binary not found");
                return;
            }
        },
        _ => match find_claude_binary() {
            Ok(b) => b.path,
            Err(e) => {
                tracing::error!(error = %e, "wake-up: claude binary not found");
                return;
            }
        },
    };

    let resume_id = Some(session_id.to_string());
    let mut cmd = match provider {
        "codex" => build_codex_command(&CodexSpawnArgs {
            bin: &bin,
            workspace: workspace_root,
            message: &message,
            model: "gpt-5.3-codex",
            reasoning_effort: "high",
            system_prompt: Some(&system_prompt),
            mcp_config: mcp_config.as_deref(),
            resume_id: resume_id.as_deref(),
        }),
        _ => build_command(&SpawnArgs {
            bin: &bin,
            workspace: workspace_root,
            message: &message,
            resume_id: resume_id.as_deref(),
            mcp_config: mcp_config.as_deref(),
            system_prompt: Some(&system_prompt),
            model: "claude-haiku-4-5-20251001",
            effort: "high",
        }),
    };

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = %e, provider = %provider, "wake-up: failed to spawn agent");
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
    let provider_owned = provider.to_string();

    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else { continue };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let mut emit_event = |event: StreamEvent| {
            if let StreamEvent::Init { ref session_id } = event {
                session_manager.set_session_id_for_provider(
                    &tab_id_owned,
                    &provider_owned,
                    session_id.clone(),
                );
            }
            let _ = app.emit(
                "chat-stream",
                serde_json::json!({ "tab_id": &tab_id_owned, "event": &event }),
            );
        };
        if provider_owned == "codex" {
            codex_stream::dispatch(&json, &mut state, &mut emit_event);
        } else {
            cc_stream::dispatch(&json, &mut state, &mut emit_event);
        }
    }

    let _ = child.wait();
    session_manager.clear_pid(tab_id);

    let _ = app.emit(
        "chat-stream",
        serde_json::json!({ "tab_id": tab_id, "event": &StreamEvent::Done }),
    );
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
