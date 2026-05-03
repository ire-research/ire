use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("claude binary not found in PATH, login shell, or known install locations")]
    NotFound,
    #[error("found {0} but it is not executable")]
    NotExecutable(PathBuf),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscoveredBinary {
    pub path: PathBuf,
    pub version: Option<String>,
}

pub fn find_claude_binary() -> Result<DiscoveredBinary, DiscoveryError> {
    if let Some(path) = which("claude") {
        return finalize(path);
    }
    if let Some(path) = login_shell_lookup() {
        return finalize(path);
    }
    for cand in candidate_paths() {
        if cand.is_file() {
            if !is_executable(&cand) {
                return Err(DiscoveryError::NotExecutable(cand));
            }
            return finalize(cand);
        }
    }
    Err(DiscoveryError::NotFound)
}

fn finalize(path: PathBuf) -> Result<DiscoveredBinary, DiscoveryError> {
    if !is_executable(&path) {
        return Err(DiscoveryError::NotExecutable(path));
    }
    let version = read_version(&path);
    Ok(DiscoveredBinary { path, version })
}

fn read_version(bin: &Path) -> Option<String> {
    let out = Command::new(bin).arg("--version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn which(name: &str) -> Option<PathBuf> {
    #[cfg(windows)]
    let cmd = "where";
    #[cfg(not(windows))]
    let cmd = "which";

    let out = Command::new(cmd).arg(name).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    let first = s.lines().next()?.trim();
    if first.is_empty() {
        None
    } else {
        Some(PathBuf::from(first))
    }
}

#[cfg(unix)]
fn login_shell_lookup() -> Option<PathBuf> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let out = Command::new(&shell)
        .args(["-lc", "command -v claude"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let line = String::from_utf8_lossy(&out.stdout);
    let trimmed = line.lines().next()?.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

#[cfg(windows)]
fn login_shell_lookup() -> Option<PathBuf> {
    None
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

fn home_dir() -> Option<PathBuf> {
    #[cfg(unix)]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }
}

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata()
        .map(|m| m.is_file() && (m.permissions().mode() & 0o111) != 0)
        .unwrap_or(false)
}

#[cfg(windows)]
fn is_executable(p: &Path) -> bool {
    p.is_file()
}
