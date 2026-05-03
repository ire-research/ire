use std::io::{BufRead, BufReader};
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

    let pid_arc = Arc::clone(&session.running_pid);
    let sid_arc = Arc::clone(&session.session_id);

    tokio::task::spawn_blocking(move || {
        let mut cmd = build_command(&SpawnArgs {
            bin: &bin,
            workspace: &workspace_path,
            message: &message,
            mode: &mode,
            resume_id: resume_id.as_deref(),
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

        // Safety net: if the process exited without emitting a result/error line
        // (e.g. `--resume` failed, binary crashed), the frontend still needs to know
        // the stream is over. A duplicate Done is handled idempotently on the frontend.
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

#[cfg(unix)]
fn kill_process(pid: u32) {
    unsafe {
        libc::kill(pid as libc::pid_t, libc::SIGTERM);
    }
}

#[cfg(windows)]
fn kill_process(pid: u32) {
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .output();
}
