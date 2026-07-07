use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::binary::{find_binary, home_dir, run_with_timeout, DiscoveredBinary, DiscoveryError};

const LOGIN_CHECK_TIMEOUT: Duration = Duration::from_secs(5);

pub fn find_codex_binary() -> Result<DiscoveredBinary, DiscoveryError> {
    find_binary("codex", candidate_paths())
}

/// Checks `codex login status`, which exits 0 when logged in and non-zero
/// (printing "Not logged in") otherwise. Returns `false` on any failure
/// (not found, timeout) so callers can treat "unknown" as "not ready".
pub fn is_codex_logged_in(bin: &Path) -> bool {
    run_with_timeout(bin, &["login", "status"], LOGIN_CHECK_TIMEOUT)
        .is_some_and(|out| out.status.success())
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
