use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::binary::{find_binary, home_dir, run_with_timeout};

pub use crate::binary::{DiscoveredBinary, DiscoveryError};

const LOGIN_CHECK_TIMEOUT: Duration = Duration::from_secs(5);

pub fn find_claude_binary() -> Result<DiscoveredBinary, DiscoveryError> {
    find_binary("claude", candidate_paths())
}

/// Checks `claude auth status --json` for `"loggedIn": true`. Returns `false`
/// on any failure (not found, timeout, non-JSON output) so callers can treat
/// "unknown" the same as "not ready".
pub fn is_claude_logged_in(bin: &Path) -> bool {
    let Some(out) = run_with_timeout(bin, &["auth", "status", "--json"], LOGIN_CHECK_TIMEOUT)
    else {
        return false;
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str::<serde_json::Value>(&stdout)
        .ok()
        .and_then(|v| v.get("loggedIn").and_then(|b| b.as_bool()))
        .unwrap_or(false)
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
