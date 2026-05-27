use std::path::PathBuf;

use crate::binary::{find_binary, home_dir, DiscoveredBinary, DiscoveryError};

pub fn find_codex_binary() -> Result<DiscoveredBinary, DiscoveryError> {
    find_binary("codex", candidate_paths())
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
            home.join(".npm-global/bin/codex"),
            home.join(".npm/bin/codex"),
            PathBuf::from("/opt/homebrew/bin/codex"),
            PathBuf::from("/usr/local/bin/codex"),
        ]);
        if let Ok(entries) = std::fs::read_dir(home.join(".nvm/versions/node")) {
            for e in entries.flatten() {
                out.push(e.path().join("bin/codex"));
            }
        }
    }

    #[cfg(windows)]
    {
        out.push(home.join("AppData/Roaming/npm/codex.cmd"));
    }

    out
}
