use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};

use anyhow::{anyhow, Context, Result};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::cc::session::SessionManager;
use crate::db::models as db;

pub fn start_experiment(
    params: &serde_json::Value,
    workspace_root: &Path,
    tab_id: String,
    session_id: String,
    session_manager: SessionManager,
    app: AppHandle,
) -> Result<serde_json::Value> {
    let name = params["name"]
        .as_str()
        .ok_or_else(|| anyhow!("missing name"))?
        .to_string();
    let plan_md = params["plan_md"]
        .as_str()
        .ok_or_else(|| anyhow!("missing plan_md"))?
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
    let log_dir = ire_dir.join("logs").join(&uuid);
    let exp_dir = ire_dir.join("experiments").join(&uuid);

    fs::create_dir_all(&log_dir).context("create log dir")?;
    fs::create_dir_all(&exp_dir).context("create experiments dir")?;
    fs::write(exp_dir.join("plan.md"), &plan_md).context("write plan.md")?;

    let stdout_file = File::create(log_dir.join("stdout.log")).context("create stdout.log")?;
    let stderr_file = File::create(log_dir.join("stderr.log")).context("create stderr.log")?;

    db::insert_experiment(
        &ire_dir,
        &uuid,
        &name,
        &command,
        &working_dir.to_string_lossy(),
        &wake_prompt,
        &session_id,
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
    // Bridge event: lets the frontend link this UUID to the pending ToolStart card.
    let _ = app.emit(
        "experiment-starting",
        serde_json::json!({ "tab_id": tab_id, "uuid": uuid }),
    );

    let uuid_m = uuid.clone();
    let root_m = workspace_root.to_path_buf();
    let app_m = app.clone();
    tauri::async_runtime::spawn(async move {
        let r = tokio::task::spawn_blocking(move || {
            monitor(
                child,
                uuid_m,
                root_m,
                tab_id,
                session_id,
                wake_prompt,
                app_m,
                session_manager,
            );
        })
        .await;
        if let Err(e) = r {
            tracing::error!(error = %e, "experiment monitor panicked");
        }
    });

    Ok(serde_json::json!({ "uuid": uuid, "status": "started" }))
}

// ── internal ──────────────────────────────────────────────────────────────────

fn spawn_detached(
    command: &str,
    working_dir: &Path,
    stdout: File,
    stderr: File,
) -> Result<Child> {
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

fn monitor(
    mut child: Child,
    uuid: String,
    workspace_root: PathBuf,
    tab_id: String,
    session_id: String,
    wake_prompt: String,
    app: AppHandle,
    session_manager: SessionManager,
) {
    let ire_dir = workspace_root.join(".ire");
    let log_dir = ire_dir.join("logs").join(&uuid);
    let mut stdout_pos = 0u64;
    let mut stderr_pos = 0u64;

    loop {
        emit_new_lines(&app, &uuid, &log_dir.join("stdout.log"), &mut stdout_pos, "stdout");
        emit_new_lines(&app, &uuid, &log_dir.join("stderr.log"), &mut stderr_pos, "stderr");

        match child.try_wait() {
            Ok(Some(status)) => {
                let exit_code = status.code().unwrap_or(-1);
                // Drain remaining log output.
                emit_new_lines(&app, &uuid, &log_dir.join("stdout.log"), &mut stdout_pos, "stdout");
                emit_new_lines(&app, &uuid, &log_dir.join("stderr.log"), &mut stderr_pos, "stderr");

                let status_str = if exit_code == 0 { "completed" } else { "failed" };
                db::update_experiment_completed(&ire_dir, &uuid, status_str, Some(exit_code)).ok();

                let _ = app.emit(
                    "experiment-status",
                    serde_json::json!({
                        "uuid": uuid,
                        "status": status_str,
                        "exit_code": exit_code,
                    }),
                );
                tracing::info!(uuid = %uuid, exit_code = exit_code, "experiment finished");

                super::wake::fire_wakeup(
                    &workspace_root,
                    &uuid,
                    exit_code,
                    &tab_id,
                    &session_id,
                    &wake_prompt,
                    &app,
                    &session_manager,
                );
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
                break;
            }
        }
    }
}

fn emit_new_lines(app: &AppHandle, uuid: &str, path: &Path, pos: &mut u64, stream: &str) {
    let Ok(mut file) = File::open(path) else { return };
    let Ok(_) = file.seek(SeekFrom::Start(*pos)) else { return };
    let mut buf = String::new();
    let Ok(n) = file.read_to_string(&mut buf) else { return };
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
