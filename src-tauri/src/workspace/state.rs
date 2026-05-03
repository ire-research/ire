use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;

use super::lock::WorkspaceLock;

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceState {
    pub path: PathBuf,
    pub name: String,
}

impl WorkspaceState {
    pub fn from_path(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("workspace")
            .to_string();
        Self { path, name }
    }
}

pub struct WorkspaceHandle {
    pub state: WorkspaceState,
    _lock: WorkspaceLock,
}

impl WorkspaceHandle {
    pub fn new(state: WorkspaceState, lock: WorkspaceLock) -> Self {
        Self { state, _lock: lock }
    }
}

#[derive(Default)]
pub struct ActiveWorkspace(pub Mutex<Option<WorkspaceHandle>>);
