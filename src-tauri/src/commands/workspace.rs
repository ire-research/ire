use std::path::PathBuf;

use serde::Serialize;
use tauri::State;

use crate::cc::discovery::{find_claude_binary, DiscoveryError};
use crate::db::migrations;
use crate::workspace::init as ws_init;
use crate::workspace::lock::{LockError, WorkspaceLock};
use crate::workspace::state::{ActiveWorkspace, WorkspaceHandle, WorkspaceState};

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BinaryStatus {
    Found { path: PathBuf, version: Option<String> },
    Missing,
}

#[derive(Debug, Serialize)]
pub struct SetupStatus {
    pub binary: BinaryStatus,
}

#[tauri::command]
pub fn setup_status() -> SetupStatus {
    let binary = match find_claude_binary() {
        Ok(b) => BinaryStatus::Found {
            path: b.path,
            version: b.version,
        },
        Err(DiscoveryError::NotFound) => BinaryStatus::Missing,
        Err(DiscoveryError::NotExecutable(_)) => BinaryStatus::Missing,
        Err(DiscoveryError::Io(e)) => {
            tracing::warn!(error = %e, "binary discovery io error");
            BinaryStatus::Missing
        }
    };
    SetupStatus { binary }
}

#[tauri::command]
pub fn open_workspace(
    path: String,
    active: State<'_, ActiveWorkspace>,
) -> Result<WorkspaceState, String> {
    let path = PathBuf::from(path);
    ws_init::validate_existing(&path).map_err(|e| e.to_string())?;
    attach(&active, path)
}

#[tauri::command]
pub fn init_workspace(
    path: String,
    active: State<'_, ActiveWorkspace>,
) -> Result<WorkspaceState, String> {
    let path = PathBuf::from(path);
    ws_init::initialize(&path).map_err(|e| e.to_string())?;
    attach(&active, path)
}

#[tauri::command]
pub fn close_workspace(active: State<'_, ActiveWorkspace>) -> Result<(), String> {
    let mut guard = active.0.lock().map_err(|e| e.to_string())?;
    let prev = guard.take();
    drop(guard);
    if let Some(h) = prev {
        tracing::info!(path = ?h.state.path, "closed workspace");
    }
    Ok(())
}

fn attach(
    active: &State<'_, ActiveWorkspace>,
    path: PathBuf,
) -> Result<WorkspaceState, String> {
    let ire = ws_init::ire_dir(&path);
    let lock = WorkspaceLock::acquire(&ire).map_err(|e| match e {
        LockError::AlreadyHeld { pid } => {
            format!("workspace is already open (process {pid})")
        }
        LockError::Io(io) => io.to_string(),
    })?;
    migrations::run(&ire).map_err(|e| e.to_string())?;
    let state = WorkspaceState::from_path(path);
    let mut guard = active.0.lock().map_err(|e| e.to_string())?;
    *guard = Some(WorkspaceHandle::new(state.clone(), lock));
    Ok(state)
}
