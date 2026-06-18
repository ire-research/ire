use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};

use anyhow::{anyhow, Context, Result};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::claude_code::session::{ActiveSession, SessionManager};
use crate::db::models as db;
use crate::events;

pub fn start_experiment(
    params: &serde_json::Value,
    workspace_root: &Path,
    active_session: ActiveSession,
    session_manager: SessionManager,
    app: AppHandle,
) -> Result<serde_json::Value> {
    let ActiveSession {
        tab_id,
        session_uuid,
        provider,
        model,
        effort,
    } = active_session;

    let name = params["name"]
        .as_str()
        .ok_or_else(|| anyhow!("missing name"))?
        .to_string();
    let command = params["command"]
        .as_str()
        .ok_or_else(|| anyhow!("missing command"))?
        .to_string();
    let working_dir = params["working_dir"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace_root.to_path_buf());
    let wake_prompt = params["wake_prompt"]
        .as_str()
        .ok_or_else(|| anyhow!("missing wake_prompt"))?
        .to_string();

    let uuid = Uuid::new_v4().to_string();
    let ire_dir = workspace_root.join(".ire");
    let exp_dir = ire_dir.join("wiki/experiments").join(&uuid);

    fs::create_dir_all(&exp_dir).context("create experiments dir")?;

    let stdout_file = File::create(exp_dir.join("stdout.log")).context("create stdout.log")?;
    let stderr_file = File::create(exp_dir.join("stderr.log")).context("create stderr.log")?;

    db::insert_experiment(
        &ire_dir,
        &uuid,
        &name,
        &command,
        &working_dir.to_string_lossy(),
        &wake_prompt,
        &session_uuid,
        &tab_id,
    )?;

    let child = spawn_detached(&command, &working_dir, stdout_file, stderr_file)?;
    let pid = child.id();
    tracing::info!(uuid = %uuid, pid = pid, name = %name, "experiment spawned");

    db::update_experiment_pid(&ire_dir, &uuid, pid).ok();

    let _ = app.emit(
        "experiment-status",
        serde_json::json!({ "uuid": uuid, "status": "running" }),
    );
    if let Ok(Some(row)) = db::get_experiment(&ire_dir, &uuid) {
        events::emit_experiment_changed(&app, events::EventSource::Mutation, &row);
    }
    // Bridge event: lets the frontend link this UUID to the pending ToolStart card.
    let _ = app.emit(
        "experiment-starting",
        serde_json::json!({ "tab_id": tab_id, "uuid": uuid, "pid": pid }),
    );

    let monitor_args = MonitorArgs {
        uuid: uuid.clone(),
        workspace_root: workspace_root.to_path_buf(),
        tab_id,
        session_uuid,
        provider,
        model,
        effort,
        wake_prompt,
        app: app.clone(),
        session_manager,
    };
    tauri::async_runtime::spawn(async move {
        let r = tokio::task::spawn_blocking(move || {
            monitor(child, monitor_args);
        })
        .await;
        if let Err(e) = r {
            tracing::error!(error = %e, "experiment monitor panicked");
        }
    });

    Ok(serde_json::json!({ "uuid": uuid, "status": "started" }))
}

// ── internal ──────────────────────────────────────────────────────────────────

struct MonitorArgs {
    uuid: String,
    workspace_root: PathBuf,
    tab_id: String,
    session_uuid: String,
    provider: String,
    model: String,
    effort: Option<String>,
    wake_prompt: String,
    app: AppHandle,
    session_manager: SessionManager,
}

fn spawn_detached(command: &str, working_dir: &Path, stdout: File, stderr: File) -> Result<Child> {
    use std::process::Command;

    let mut cmd = Command::new("sh");
    cmd.args(["-c", command])
        .current_dir(working_dir)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .env_remove("CLAUDECODE");

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    cmd.spawn().context("spawn experiment subprocess")
}

fn monitor(mut child: Child, args: MonitorArgs) {
    let MonitorArgs {
        uuid,
        workspace_root,
        tab_id,
        session_uuid,
        provider,
        model,
        effort,
        wake_prompt,
        app,
        session_manager,
    } = args;

    let ire_dir = workspace_root.join(".ire");
    let exp_dir = ire_dir.join("wiki/experiments").join(&uuid);
    let mut stdout_pos = 0u64;
    let mut stderr_pos = 0u64;

    loop {
        emit_new_lines(
            &app,
            &uuid,
            &exp_dir.join("stdout.log"),
            &mut stdout_pos,
            "stdout",
        );
        emit_new_lines(
            &app,
            &uuid,
            &exp_dir.join("stderr.log"),
            &mut stderr_pos,
            "stderr",
        );

        match child.try_wait() {
            Ok(Some(status)) => {
                let exit_code = status.code().unwrap_or(-1);
                // Drain remaining log output.
                emit_new_lines(
                    &app,
                    &uuid,
                    &exp_dir.join("stdout.log"),
                    &mut stdout_pos,
                    "stdout",
                );
                emit_new_lines(
                    &app,
                    &uuid,
                    &exp_dir.join("stderr.log"),
                    &mut stderr_pos,
                    "stderr",
                );

                let status_str = if exit_code == 0 {
                    "completed"
                } else {
                    "failed"
                };
                db::update_experiment_completed(&ire_dir, &uuid, status_str, Some(exit_code)).ok();

                let _ = app.emit(
                    "experiment-status",
                    serde_json::json!({
                        "uuid": uuid,
                        "status": status_str,
                        "exit_code": exit_code,
                    }),
                );
                if let Ok(Some(row)) = db::get_experiment(&ire_dir, &uuid) {
                    events::emit_experiment_changed(&app, events::EventSource::Mutation, &row);
                }
                tracing::info!(uuid = %uuid, exit_code = exit_code, "experiment finished");

                super::wake::fire_wakeup(super::wake::FireWakeupArgs {
                    workspace_root: &workspace_root,
                    uuid: &uuid,
                    exit_code,
                    tab_id: &tab_id,
                    session_uuid: &session_uuid,
                    provider: &provider,
                    model: &model,
                    effort: effort.as_deref(),
                    wake_prompt: &wake_prompt,
                    app: &app,
                    session_manager: &session_manager,
                });
                break;
            }
            Ok(None) => {
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
            Err(e) => {
                tracing::error!(error = %e, uuid = %uuid, "experiment wait error");
                db::update_experiment_completed(&ire_dir, &uuid, "failed", Some(-1)).ok();
                let _ = app.emit(
                    "experiment-status",
                    serde_json::json!({ "uuid": uuid, "status": "failed" }),
                );
                if let Ok(Some(row)) = db::get_experiment(&ire_dir, &uuid) {
                    events::emit_experiment_changed(&app, events::EventSource::Mutation, &row);
                }
                break;
            }
        }
    }
}

fn emit_new_lines(app: &AppHandle, uuid: &str, path: &Path, pos: &mut u64, stream: &str) {
    let Ok(mut file) = File::open(path) else {
        return;
    };
    let Ok(_) = file.seek(SeekFrom::Start(*pos)) else {
        return;
    };
    let mut buf = String::new();
    let Ok(n) = file.read_to_string(&mut buf) else {
        return;
    };
    if n == 0 {
        return;
    }
    *pos += n as u64;
    for line in buf.lines() {
        let _ = app.emit(
            "experiment-log-line",
            serde_json::json!({ "uuid": uuid, "stream": stream, "line": line }),
        );
    }
}
