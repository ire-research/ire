use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, State};

use crate::binary::{binary_status, BinaryStatus};
use crate::claude_code::discovery::{find_claude_binary, is_claude_logged_in};
use crate::claude_code::session::SessionManager;
use crate::codex::discovery::{find_codex_binary, is_codex_logged_in};
use crate::db::schema;
use crate::events::{self, EventSource};
use crate::mcp::{McpHandle, McpState};
use crate::user_config::{self, UserConfig};
use crate::workspace::init as ws_init;
use crate::workspace::lock::{LockError, WorkspaceLock};
use crate::workspace::state::{ActiveWorkspace, WorkspaceHandle, WorkspaceState};

#[derive(Debug, Serialize)]
pub struct SetupStatus {
    pub claude_binary: BinaryStatus,
    pub codex_binary: BinaryStatus,
}

#[tauri::command]
pub fn setup_status() -> SetupStatus {
    tracing::debug!("setup_status");
    let claude_binary = binary_status("claude", find_claude_binary(), is_claude_logged_in);
    let codex_binary = binary_status("codex", find_codex_binary(), is_codex_logged_in);
    SetupStatus {
        claude_binary,
        codex_binary,
    }
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
    if ws_init::validate_existing(&path).is_err() {
        ws_init::initialize(&path).map_err(|e| e.to_string())?;
    } else {
        ws_init::ensure_git(&path).map_err(|e| e.to_string())?;
    }
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
pub fn close_workspace(
    active: State<'_, ActiveWorkspace>,
    mcp: State<'_, McpState>,
    session: State<'_, SessionManager>,
) -> Result<(), String> {
    tracing::info!("close_workspace");
    // Stop MCP server first (Drop impl aborts task + removes socket file).
    mcp.0.lock().map_err(|e| e.to_string())?.take();

    // Terminate any in-flight CC subprocesses so their late chat-stream events
    // don't leak into the next workspace (the frontend listener is global).
    for pid in session.drain() {
        tracing::info!(
            pid = pid,
            "terminating stale CC subprocess on workspace close"
        );
        crate::commands::chat::kill_process(pid);
    }

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
    let home_data_dir = ws_init::require_home_data_dir(&path)?;
    std::fs::create_dir_all(&home_data_dir).map_err(|e| format!("create home data dir: {e}"))?;

    let lock = WorkspaceLock::acquire(&home_data_dir).map_err(|e| match e {
        LockError::AlreadyHeld { pid } => {
            format!("workspace is already open (process {pid})")
        }
        LockError::Io(io) => io.to_string(),
    })?;

    schema::run(&home_data_dir).map_err(|e| e.to_string())?;

    // Start the MCP RPC server and write the mcp.json config for CC.
    let socket = crate::mcp::rpc::socket_path(&home_data_dir);
    let task = crate::mcp::rpc::start(socket.clone(), path.clone(), session_manager, app.clone());
    crate::mcp::config::write_mcp_config(&home_data_dir, &path, &socket).map_err(|e| e.to_string())?;
    *mcp.0.lock().map_err(|e| e.to_string())? = Some(McpHandle {
        task,
        socket_path: socket,
    });

    let state = WorkspaceState::from_path(path.clone(), home_data_dir);
    *active.0.lock().map_err(|e| e.to_string())? = Some(WorkspaceHandle::new(state.clone(), lock));

    // Initial-state burst: every panel listener sees its data through the same
    // workspace-event channel that carries live mutations. The `Hydrate` source
    // tag lets side-effect listeners (panel flashes, etc.) skip these events.
    emit_initial_state(&app, &path);

    Ok(state)
}

fn emit_initial_state(app: &AppHandle, workspace_root: &Path) {
    let store = crate::ire::IreStore::new(workspace_root.to_path_buf());
    let ire = store.read_ire().unwrap_or_default();

    events::emit_notes_changed(app, EventSource::Hydrate, &ire.notes);
    events::emit_focus_changed(
        app,
        EventSource::Hydrate,
        &ire.focus.research_question,
        &ire.focus.this_week,
    );
    let ideas = serde_json::to_value(&ire.ideas).unwrap_or_else(|_| json!([]));
    events::emit_ideas_changed(app, EventSource::Hydrate, &ideas);

    // Resources are file-based: scan resources/*.md and emit one event each.
    for resource in store.list_resources() {
        events::emit_resource_changed(app, EventSource::Hydrate, &resource);
    }

    // Experiments come from the git-tracked ire.json display record. The
    // operational tab linkage is re-established live via events, so tab_id is
    // empty on hydrate.
    for exp in ire.experiments {
        let payload = json!({
            "uuid": exp.uuid,
            "name": exp.name,
            "command": exp.command,
            "status": exp.status,
            "exit_code": exp.exit_code,
            "started_at": exp.started_at,
            "ended_at": exp.ended_at,
            "tab_id": "",
        });
        events::emit_experiment_changed(app, EventSource::Hydrate, &payload);
    }
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
pub fn read_user_config() -> UserConfig {
    user_config::read()
}

#[tauri::command]
pub fn save_user_config(config: UserConfig) -> Result<(), String> {
    let was_enabled = user_config::analytics_enabled();
    user_config::write(&config).map_err(|e| e.to_string())?;
    // `setup()` only fires `app_launched` for sessions that were already opted
    // in at native startup, so the session in which the user answers the
    // consent modal would otherwise never send it. Fire it here instead, once,
    // right at the moment consent flips on.
    if !cfg!(debug_assertions) && !was_enabled && config.analytics_enabled == Some(true) {
        crate::analytics::track_app_launched(user_config::analytics_id());
    }
    Ok(())
}
