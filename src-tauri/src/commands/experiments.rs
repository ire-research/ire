use std::fs;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::db::models::{self as db, ExperimentRow};
use crate::events;
use crate::workspace::state::ActiveWorkspace;

#[derive(Debug, Serialize)]
pub struct LogsResult {
    pub stdout: String,
    pub stderr: String,
}

#[tauri::command]
pub fn experiment_list(
    active: State<'_, ActiveWorkspace>,
    limit: Option<usize>,
) -> Result<Vec<ExperimentRow>, String> {
    let workspace_path = {
        let guard = active.0.lock().map_err(|e| e.to_string())?;
        guard.as_ref().ok_or("no workspace open")?.state.path.clone()
    };
    let ire_dir = workspace_path.join(".ire");
    db::list_experiments(&ire_dir, limit.unwrap_or(50)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn experiment_logs(
    active: State<'_, ActiveWorkspace>,
    uuid: String,
    kb: Option<u64>,
) -> Result<LogsResult, String> {
    let workspace_path = {
        let guard = active.0.lock().map_err(|e| e.to_string())?;
        guard.as_ref().ok_or("no workspace open")?.state.path.clone()
    };
    let log_dir = workspace_path.join(".ire/wiki/experiments").join(&uuid);
    let max_bytes = kb.unwrap_or(64) * 1024;

    Ok(LogsResult {
        stdout: read_tail(&log_dir.join("stdout.log"), max_bytes),
        stderr: read_tail(&log_dir.join("stderr.log"), max_bytes),
    })
}

#[tauri::command]
pub fn experiment_cancel(
    app: AppHandle,
    active: State<'_, ActiveWorkspace>,
    uuid: String,
) -> Result<(), String> {
    let workspace_path = {
        let guard = active.0.lock().map_err(|e| e.to_string())?;
        guard.as_ref().ok_or("no workspace open")?.state.path.clone()
    };
    let ire_dir = workspace_path.join(".ire");

    let row = db::get_experiment(&ire_dir, &uuid)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("experiment {uuid} not found"))?;

    if let Some(pid) = row.pid {
        kill_process_group(pid as u32);
    }

    db::update_experiment_completed(&ire_dir, &uuid, "cancelled", None)
        .map_err(|e| e.to_string())?;

    if let Ok(Some(row)) = db::get_experiment(&ire_dir, &uuid) {
        events::emit_experiment_changed(&app, events::EventSource::Mutation, &row);
    }
    Ok(())
}

#[tauri::command]
pub fn experiment_delete(
    app: AppHandle,
    active: State<'_, ActiveWorkspace>,
    uuid: String,
) -> Result<(), String> {
    let workspace_path = {
        let guard = active.0.lock().map_err(|e| e.to_string())?;
        guard.as_ref().ok_or("no workspace open")?.state.path.clone()
    };
    let ire_dir = workspace_path.join(".ire");
    let row = db::get_experiment(&ire_dir, &uuid)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("experiment {uuid} not found"))?;
    if row.status == "running" || row.status == "starting" {
        return Err(format!("experiment {uuid} is still {}", row.status));
    }

    for dir in [ire_dir.join("wiki/experiments").join(&uuid)] {
        if dir.exists() {
            fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
        }
    }
    db::delete_experiment(&ire_dir, &uuid).map_err(|e| e.to_string())?;
    events::emit_experiment_deleted(&app, &uuid);
    Ok(())
}

#[tauri::command]
pub fn experiment_rename(
    app: AppHandle,
    active: State<'_, ActiveWorkspace>,
    uuid: String,
    name: String,
) -> Result<(), String> {
    let workspace_path = {
        let guard = active.0.lock().map_err(|e| e.to_string())?;
        guard.as_ref().ok_or("no workspace open")?.state.path.clone()
    };
    let ire_dir = workspace_path.join(".ire");
    db::rename_experiment(&ire_dir, &uuid, &name).map_err(|e| e.to_string())?;
    if let Ok(Some(row)) = db::get_experiment(&ire_dir, &uuid) {
        events::emit_experiment_changed(&app, events::EventSource::Mutation, &row);
    }
    Ok(())
}

fn read_tail(path: &std::path::Path, max_bytes: u64) -> String {
    let Ok(content) = fs::read(path) else { return String::new() };
    let len = content.len() as u64;
    let start = if len > max_bytes { (len - max_bytes) as usize } else { 0 };
    String::from_utf8_lossy(&content[start..]).into_owned()
}

#[cfg(unix)]
fn kill_process_group(pid: u32) {
    unsafe { libc::killpg(pid as libc::pid_t, libc::SIGTERM) };
}

#[cfg(not(unix))]
fn kill_process_group(pid: u32) {
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/T", "/PID", &pid.to_string()])
        .output();
}
