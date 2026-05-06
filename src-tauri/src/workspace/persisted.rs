use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

const FILE: &str = "workspace.json";

/// On-disk UI/session state for a workspace. Forward-compatible: unknown fields
/// are dropped; missing fields default. Bump `version` on schema changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedWorkspace {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(default)]
    pub panel_layout: Option<serde_json::Value>,
    #[serde(default)]
    pub last_opened: Option<String>,
}

impl Default for PersistedWorkspace {
    fn default() -> Self {
        Self {
            version: default_version(),
            theme: None,
            panel_layout: None,
            last_opened: None,
        }
    }
}

fn default_version() -> u32 {
    1
}

pub fn read(ire_dir: &Path) -> PersistedWorkspace {
    let path = ire_dir.join(FILE);
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn write(ire_dir: &Path, state: &PersistedWorkspace) -> Result<()> {
    let path = ire_dir.join(FILE);
    let json = serde_json::to_string_pretty(state)?;
    crate::wiki::store::atomic_write(&path, &json)
}
