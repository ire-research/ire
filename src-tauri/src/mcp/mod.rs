pub mod config;
pub mod rpc;

use std::path::PathBuf;
use std::sync::Mutex;

pub struct McpHandle {
    pub task: tauri::async_runtime::JoinHandle<()>,
    pub socket_path: PathBuf,
}

impl Drop for McpHandle {
    fn drop(&mut self) {
        self.task.abort();
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

#[derive(Default)]
pub struct McpState(pub Mutex<Option<McpHandle>>);
