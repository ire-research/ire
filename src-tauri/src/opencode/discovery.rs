use std::path::{Path, PathBuf};

use crate::binary::{find_binary, home_dir, DiscoveredBinary, DiscoveryError};

pub fn find_opencode_binary() -> Result<DiscoveredBinary, DiscoveryError> {
    find_binary("opencode", candidate_paths())
}

/// Whether at least one provider has credentials configured in OpenCode's own
/// auth store. Reads `auth.json` directly rather than shelling out to
/// `opencode providers list` — that command only prints decorated box-art to
/// a terminal, not machine-parseable output, but its own displayed path
/// (`~/.local/share/opencode/auth.json`) is a plain JSON object keyed by
/// provider id. Missing/unreadable/empty all mean "no credentials", not an
/// error — same "unknown counts as not ready" convention as the other
/// providers' login checks.
pub fn is_opencode_logged_in(_bin: &Path) -> bool {
    let Some(path) = auth_path() else {
        return false;
    };
    let Ok(content) = std::fs::read_to_string(path) else {
        return false;
    };
    serde_json::from_str::<serde_json::Value>(&content)
        .ok()
        .and_then(|v| v.as_object().map(|o| !o.is_empty()))
        .unwrap_or(false)
}

fn auth_path() -> Option<PathBuf> {
    #[cfg(unix)]
    {
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| home_dir().map(|h| h.join(".local/share")))?;
        Some(base.join("opencode").join("auth.json"))
    }
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA")
            .map(|p| PathBuf::from(p).join("opencode").join("auth.json"))
    }
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
            PathBuf::from("/opt/homebrew/bin/opencode"),
            PathBuf::from("/usr/local/bin/opencode"),
            home.join(".opencode/bin/opencode"),
            home.join(".local/bin/opencode"),
        ]);
    }

    #[cfg(windows)]
    {
        out.push(home.join("AppData/Roaming/npm/opencode.cmd"));
    }

    out
}
