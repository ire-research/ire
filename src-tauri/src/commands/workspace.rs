use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, State};

use crate::claude_code::discovery::{find_claude_binary, DiscoveryError};
use crate::claude_code::session::SessionManager;
use crate::codex::discovery::find_codex_binary;
use crate::db::{migrations, models};
use crate::events::{self, EventSource};
use crate::mcp::{McpHandle, McpState};
use crate::user_config::{self, UserConfig};
use crate::workspace::init as ws_init;
use crate::workspace::lock::{LockError, WorkspaceLock};
use crate::workspace::persisted::{self, PersistedWorkspace};
use crate::workspace::state::{ActiveWorkspace, WorkspaceHandle, WorkspaceState};

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BinaryStatus {
    Found {
        path: PathBuf,
        version: Option<String>,
    },
    Missing,
}

#[derive(Debug, Serialize)]
pub struct SetupStatus {
    pub binary: BinaryStatus,
    pub codex_binary: BinaryStatus,
}

#[tauri::command]
pub fn setup_status() -> SetupStatus {
    tracing::debug!("setup_status");
    let binary = binary_status("claude", find_claude_binary());
    let codex_binary = binary_status("codex", find_codex_binary());
    SetupStatus {
        binary,
        codex_binary,
    }
}

fn binary_status(
    name: &str,
    result: Result<crate::binary::DiscoveredBinary, DiscoveryError>,
) -> BinaryStatus {
    match result {
        Ok(b) => {
            tracing::debug!(binary = name, path = ?b.path, version = ?b.version, "binary found");
            BinaryStatus::Found {
                path: b.path,
                version: b.version,
            }
        }
        Err(DiscoveryError::NotFound) => {
            tracing::debug!(binary = name, "binary not found");
            BinaryStatus::Missing
        }
        Err(DiscoveryError::NotExecutable(_)) => BinaryStatus::Missing,
        Err(DiscoveryError::Io(e)) => {
            tracing::warn!(binary = name, error = %e, "binary discovery io error");
            BinaryStatus::Missing
        }
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
    ws_init::validate_existing(&path).map_err(|e| e.to_string())?;
    ws_init::ensure_git(&path).map_err(|e| e.to_string())?;
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
    let ire = ws_init::ire_dir(&path);

    let lock = WorkspaceLock::acquire(&ire).map_err(|e| match e {
        LockError::AlreadyHeld { pid } => {
            format!("workspace is already open (process {pid})")
        }
        LockError::Io(io) => io.to_string(),
    })?;

    migrations::run(&ire).map_err(|e| e.to_string())?;

    // Restore per-tab CC session ids saved on previous Init events, so
    // `--resume` keeps working across backend restarts.
    session_manager.restore_from_disk(&ire);

    // Start the MCP RPC server and write the mcp.json config for CC.
    let socket = crate::mcp::rpc::socket_path(&ire);
    let task = crate::mcp::rpc::start(socket.clone(), path.clone(), session_manager, app.clone());
    crate::mcp::config::write_mcp_config(&ire, &socket).map_err(|e| e.to_string())?;
    *mcp.0.lock().map_err(|e| e.to_string())? = Some(McpHandle {
        task,
        socket_path: socket,
    });

    let state = WorkspaceState::from_path(path.clone());
    *active.0.lock().map_err(|e| e.to_string())? = Some(WorkspaceHandle::new(state.clone(), lock));

    // Initial-state burst: every panel listener sees its data through the same
    // workspace-event channel that carries live mutations. The `Hydrate` source
    // tag lets side-effect listeners (panel flashes, etc.) skip these events.
    emit_initial_state(&app, &path);

    Ok(state)
}

fn emit_initial_state(app: &AppHandle, workspace_root: &Path) {
    let wiki_root = workspace_root.join(".ire/wiki");
    let ire_dir = workspace_root.join(".ire");

    let store = crate::wiki::WikiStore::new(workspace_root.to_path_buf());
    let pulse = crate::commands::wiki::read_pulse_content(&store).unwrap_or(
        crate::commands::wiki::PulseContent {
            research_question: String::new(),
            this_week: String::new(),
        },
    );
    events::emit_pulse_changed(
        app,
        EventSource::Hydrate,
        &pulse.research_question,
        &pulse.this_week,
    );

    let notes = fs::read_to_string(wiki_root.join("notes.md")).unwrap_or_default();
    events::emit_notes_changed(app, EventSource::Hydrate, &notes);

    // Ideas: only emit if the file parses as a JSON array. A schema mismatch
    // skips this one variant rather than blanking the whole panel.
    if let Ok(raw) = fs::read_to_string(wiki_root.join("ideas.json")) {
        match serde_json::from_str::<serde_json::Value>(&raw) {
            Ok(parsed) if parsed.is_array() => {
                events::emit_ideas_changed(app, EventSource::Hydrate, &parsed);
            }
            Ok(_) => tracing::warn!("emit_initial_state: ideas.json is not a JSON array"),
            Err(e) => tracing::warn!(error = %e, "emit_initial_state: ideas.json parse failed"),
        }
    }

    match models::list_resources(&ire_dir) {
        Ok(rows) => {
            for r in rows {
                let source_label = r.source_label.clone().unwrap_or_else(|| r.url.clone());
                let payload = json!({
                    "resource_id": r.url_sha256,
                    "url": r.url,
                    "source_type": r.source_type,
                    "source_label": source_label,
                    "title": r.title,
                    "wiki_path": r.wiki_path,
                });
                events::emit_resource_changed(app, EventSource::Hydrate, &payload);
            }
        }
        Err(e) => tracing::warn!(error = %e, "emit_initial_state: list_resources failed"),
    }

    match models::list_experiments(&ire_dir, 50) {
        Ok(rows) => {
            for row in rows {
                events::emit_experiment_changed(app, EventSource::Hydrate, &row);
            }
        }
        Err(e) => tracing::warn!(error = %e, "emit_initial_state: list_experiments failed"),
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
