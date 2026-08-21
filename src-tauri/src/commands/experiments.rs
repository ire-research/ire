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
        guard
            .as_ref()
            .ok_or("no workspace open")?
            .state
            .path
            .clone()
    };
    let home_data_dir = crate::workspace::init::require_home_data_dir(&workspace_path)?;
    crate::experiments::list(&workspace_path, &home_data_dir, limit.unwrap_or(50))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn experiment_logs(
    active: State<'_, ActiveWorkspace>,
    uuid: String,
    kb: Option<u64>,
) -> Result<LogsResult, String> {
    let workspace_path = {
        let guard = active.0.lock().map_err(|e| e.to_string())?;
        guard
            .as_ref()
            .ok_or("no workspace open")?
            .state
            .path
            .clone()
    };
    let log_dir = workspace_path.join(".ire/cache/experiments").join(&uuid);
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
        guard
            .as_ref()
            .ok_or("no workspace open")?
            .state
            .path
            .clone()
    };
    let home_data_dir = crate::workspace::init::require_home_data_dir(&workspace_path)?;

    let row = crate::experiments::row_or_record(&workspace_path, &home_data_dir, &uuid)
        .ok_or_else(|| format!("experiment {uuid} not found"))?;

    // A run with no local row was started on another machine: nothing here to
    // signal, but the record can still be marked.
    if let Some(pid) = row.pid {
        kill_process_group(pid as u32);
    }

    crate::experiments::transition(
        &app,
        &workspace_path,
        &home_data_dir,
        &uuid,
        "cancelled",
        None,
    );
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
        guard
            .as_ref()
            .ok_or("no workspace open")?
            .state
            .path
            .clone()
    };
    let home_data_dir = crate::workspace::init::require_home_data_dir(&workspace_path)?;
    let row = db::get_experiment(&home_data_dir, &uuid)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("experiment {uuid} not found"))?;
    if row.status == "running" || row.status == "starting" {
        return Err(format!("experiment {uuid} is still {}", row.status));
    }

    let log_dir = workspace_path.join(".ire/cache/experiments").join(&uuid);
    if log_dir.exists() {
        fs::remove_dir_all(&log_dir).map_err(|e| e.to_string())?;
    }
    if let Ok(Some(dir)) = db::get_experiment_record_dir(&home_data_dir, &uuid) {
        crate::experiments::record::remove(&workspace_path, &dir);
    }
    db::delete_experiment(&home_data_dir, &uuid).map_err(|e| e.to_string())?;
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
        guard
            .as_ref()
            .ok_or("no workspace open")?
            .state
            .path
            .clone()
    };
    let home_data_dir = crate::workspace::init::require_home_data_dir(&workspace_path)?;
    db::rename_experiment(&home_data_dir, &uuid, &name).map_err(|e| e.to_string())?;
    // The record is the source of truth for the title, so failing to write it
    // is a failed rename — not a warning behind a silent no-op.
    let dir = crate::experiments::record_dir(&workspace_path, &home_data_dir, &uuid)
        .ok_or_else(|| format!("experiment {uuid} not found"))?;
    crate::experiments::record::set_title(&workspace_path, &dir, &name)
        .map_err(|e| e.to_string())?;
    crate::experiments::emit_changed(&app, &workspace_path, &home_data_dir, &uuid);
    Ok(())
}

fn read_tail(path: &std::path::Path, max_bytes: u64) -> String {
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
