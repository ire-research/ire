use std::path::PathBuf;

use serde::Serialize;
use tauri::State;

use crate::cc::discovery::{find_claude_binary, DiscoveryError};
use crate::cc::session::SessionManager;
use crate::db::migrations;
use crate::mcp::{McpHandle, McpState};
use crate::user_config::{self, UserConfig};
use crate::workspace::init as ws_init;
use crate::workspace::lock::{LockError, WorkspaceLock};
use crate::workspace::persisted::{self, PersistedWorkspace};
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
    tracing::debug!("setup_status");
    let binary = match find_claude_binary() {
        Ok(b) => {
            tracing::debug!(path = ?b.path, version = ?b.version, "claude binary found");
            BinaryStatus::Found { path: b.path, version: b.version }
        }
        Err(DiscoveryError::NotFound) => {
            tracing::debug!("claude binary not found");
            BinaryStatus::Missing
        }
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
    mcp: State<'_, McpState>,
    session: State<'_, SessionManager>,
    app: tauri::AppHandle,
) -> Result<WorkspaceState, String> {
    tracing::info!(path = %path, "open_workspace");
    let path = PathBuf::from(path);
    ws_init::validate_existing(&path).map_err(|e| e.to_string())?;
    let sm = (*session).clone();
    let result = attach(&active, &mcp, sm, path.clone(), app);
    match &result {
        Ok(s) => {
            tracing::info!(name = %s.name, path = ?s.path, "workspace opened");
            user_config::push_recent(&path).ok();
        }
        Err(e) => tracing::warn!(error = %e, "open_workspace failed"),
    }
    result
}

#[tauri::command]
pub fn init_workspace(
    path: String,
    active: State<'_, ActiveWorkspace>,
    mcp: State<'_, McpState>,
    session: State<'_, SessionManager>,
    app: tauri::AppHandle,
) -> Result<WorkspaceState, String> {
    tracing::info!(path = %path, "init_workspace");
    let path = PathBuf::from(path);
    ws_init::initialize(&path).map_err(|e| e.to_string())?;
    let sm = (*session).clone();
    let result = attach(&active, &mcp, sm, path.clone(), app);
    match &result {
        Ok(s) => {
            tracing::info!(name = %s.name, path = ?s.path, "workspace initialized");
            user_config::push_recent(&path).ok();
        }
        Err(e) => tracing::warn!(error = %e, "init_workspace failed"),
    }
    result
}

#[tauri::command]
pub fn close_workspace(
    active: State<'_, ActiveWorkspace>,
    mcp: State<'_, McpState>,
) -> Result<(), String> {
    tracing::info!("close_workspace");
    // Stop MCP server first (Drop impl aborts task + removes socket file).
    mcp.0.lock().map_err(|e| e.to_string())?.take();

    let mut guard = active.0.lock().map_err(|e| e.to_string())?;
    let prev = guard.take();
    drop(guard);
    if let Some(h) = prev {
        tracing::info!(path = ?h.state.path, "workspace closed");
    } else {
        tracing::debug!("close_workspace: no workspace was open");
    }
    Ok(())
}

fn attach(
    active: &State<'_, ActiveWorkspace>,
    mcp: &State<'_, McpState>,
    session_manager: SessionManager,
    path: PathBuf,
    app: tauri::AppHandle,
) -> Result<WorkspaceState, String> {
    let ire = ws_init::ire_dir(&path);

    let lock = WorkspaceLock::acquire(&ire).map_err(|e| match e {
        LockError::AlreadyHeld { pid } => {
            format!("workspace is already open (process {pid})")
        }
        LockError::Io(io) => io.to_string(),
    })?;

    migrations::run(&ire).map_err(|e| e.to_string())?;

    // Start the MCP RPC server and write the mcp.json config for CC.
    let socket = crate::mcp::rpc::socket_path(&ire);
    let task = crate::mcp::rpc::start(socket.clone(), path.clone(), session_manager, app);
    crate::mcp::config::write_mcp_config(&ire, &socket).map_err(|e| e.to_string())?;
    *mcp.0.lock().map_err(|e| e.to_string())? = Some(McpHandle { task, socket_path: socket });

    let state = WorkspaceState::from_path(path);
    *active.0.lock().map_err(|e| e.to_string())? = Some(WorkspaceHandle::new(state.clone(), lock));
    Ok(state)
}

#[tauri::command]
pub fn open_in_vscode(path: String) -> Result<(), String> {
    tracing::info!(path = %path, "open_in_vscode");
    std::process::Command::new("code")
        .args(["--new-window", "--force", &path])
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn read_workspace_state(
    active: State<'_, ActiveWorkspace>,
) -> Result<PersistedWorkspace, String> {
    let guard = active.0.lock().map_err(|e| e.to_string())?;
    let handle = guard.as_ref().ok_or("no workspace open")?;
    let ire = ws_init::ire_dir(&handle.state.path);
    Ok(persisted::read(&ire))
}

#[tauri::command]
pub fn save_workspace_state(
    state: PersistedWorkspace,
    active: State<'_, ActiveWorkspace>,
) -> Result<(), String> {
    let guard = active.0.lock().map_err(|e| e.to_string())?;
    let handle = guard.as_ref().ok_or("no workspace open")?;
    let ire = ws_init::ire_dir(&handle.state.path);
    persisted::write(&ire, &state).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn read_user_config() -> UserConfig {
    user_config::read()
}

#[tauri::command]
pub fn save_user_config(config: UserConfig) -> Result<(), String> {
    user_config::write(&config).map_err(|e| e.to_string())
}
