use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserConfig {
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(default)]
    pub recent_workspaces: Vec<String>,
}

fn config_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("ire").join("config.json"))
}

pub fn read() -> UserConfig {
    let mut config = config_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    if remove_missing_recent_workspaces(&mut config) {
        let _ = write(&config);
    }
    config
}

pub fn write(config: &UserConfig) -> Result<()> {
    let path = config_path().ok_or_else(|| anyhow::anyhow!("cannot determine config path"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(config)?;
    std::fs::write(&path, json.as_bytes())?;
    Ok(())
}

pub fn push_recent(workspace_path: &Path) -> Result<()> {
    let mut config = read();
    let s = workspace_path.to_string_lossy().to_string();
    config.recent_workspaces.retain(|p| p != &s);
    config.recent_workspaces.insert(0, s);
    config.recent_workspaces.truncate(10);
    write(&config)
}

fn remove_missing_recent_workspaces(config: &mut UserConfig) -> bool {
    let before = config.recent_workspaces.len();
    config.recent_workspaces.retain(|p| Path::new(p).is_dir());
    before != config.recent_workspaces.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_missing_recent_workspaces_keeps_existing_directories_only() {
        let tmp = tempfile::tempdir().unwrap();
        let existing = tmp.path().join("workspace");
        std::fs::create_dir(&existing).unwrap();
        let missing = tmp.path().join("missing");
        let file = tmp.path().join("file");
        std::fs::write(&file, "").unwrap();

        let mut config = UserConfig {
            theme: None,
            recent_workspaces: vec![
                existing.to_string_lossy().to_string(),
                missing.to_string_lossy().to_string(),
                file.to_string_lossy().to_string(),
            ],
        };

        assert!(remove_missing_recent_workspaces(&mut config));
        assert_eq!(config.recent_workspaces, vec![existing.to_string_lossy().to_string()]);
    }
}
