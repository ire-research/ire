use std::path::PathBuf;

use crate::binary::{find_binary, home_dir};

pub use crate::binary::{DiscoveredBinary, DiscoveryError};

pub fn find_claude_binary() -> Result<DiscoveredBinary, DiscoveryError> {
    find_binary("claude", candidate_paths())
}

fn candidate_paths() -> Vec<PathBuf> {
    let home = match home_dir() {
        Some(h) => h,
        None => return vec![],
    };
    let mut out: Vec<PathBuf> = Vec::new();

    #[cfg(unix)]
    {
        out.extend([
            home.join(".local/bin/claude"),
            home.join(".claude/local/claude"),
            home.join(".local/share/mise/shims/claude"),
            home.join(".asdf/shims/claude"),
            home.join(".npm-global/bin/claude"),
            home.join(".npm/bin/claude"),
            home.join(".linuxbrew/bin/claude"),
            PathBuf::from("/opt/homebrew/bin/claude"),
            PathBuf::from("/usr/local/bin/claude"),
        ]);
        if let Ok(entries) = std::fs::read_dir(home.join(".nvm/versions/node")) {
            for e in entries.flatten() {
                out.push(e.path().join("bin/claude"));
            }
        }
    }

    #[cfg(windows)]
    {
        out.extend([
            home.join("AppData/Roaming/npm/claude.cmd"),
            home.join("AppData/Local/pnpm/claude.cmd"),
            home.join("scoop/shims/claude.exe"),
        ]);
    }

    out
}
