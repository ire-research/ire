use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::mpsc;
use std::time::Duration;

use serde::Serialize;
use thiserror::Error;

/// Runs `bin` with `args`, giving up (and leaving the child to finish in the
/// background) if it doesn't complete within `timeout`. Used for login-status
/// checks, which must never hang `setup_status`.
pub fn run_with_timeout(bin: &Path, args: &[&str], timeout: Duration) -> Option<Output> {
    let child = Command::new(bin)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .stdin(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });
    rx.recv_timeout(timeout).ok()?.ok()
}

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("binary not found in PATH, login shell, or known install locations")]
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

/// Tri-state readiness for an agent binary: installed and authenticated,
/// installed but not logged in, or not found at all.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BinaryStatus {
    Ready {
        path: PathBuf,
        version: Option<String>,
    },
    LoggedOut {
        path: PathBuf,
        version: Option<String>,
    },
    Missing,
}

pub fn binary_status(
    name: &str,
    result: Result<DiscoveredBinary, DiscoveryError>,
    is_logged_in: impl FnOnce(&Path) -> bool,
) -> BinaryStatus {
    match result {
        Ok(b) => {
            if is_logged_in(&b.path) {
                tracing::debug!(binary = name, path = ?b.path, version = ?b.version, "binary ready");
                BinaryStatus::Ready {
                    path: b.path,
                    version: b.version,
                }
            } else {
                tracing::debug!(binary = name, path = ?b.path, "binary found but not logged in");
                BinaryStatus::LoggedOut {
                    path: b.path,
                    version: b.version,
                }
            }
        }
        Err(DiscoveryError::NotFound) => {
            tracing::debug!(binary = name, "binary not found");
            BinaryStatus::Missing
        }
        Err(DiscoveryError::NotExecutable(_)) => BinaryStatus::Missing,
        Err(DiscoveryError::Io(e)) => {
            tracing::warn!(binary = name, error = %e, "binary discovery io error");
            BinaryStatus::Missing
        }
    }
}

pub fn find_binary(
    name: &str,
    candidates: Vec<PathBuf>,
) -> Result<DiscoveredBinary, DiscoveryError> {
    if let Some(path) = which(name) {
        return finalize(path);
    }
    if let Some(path) = login_shell_lookup(name) {
        return finalize(path);
    }
    for cand in candidates {
        if cand.is_file() {
            if !is_executable(&cand) {
                continue;
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
fn login_shell_lookup(name: &str) -> Option<PathBuf> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let lookup = format!("command -v {name}");
    let out = Command::new(&shell).args(["-lc", &lookup]).output().ok()?;
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
fn login_shell_lookup(_name: &str) -> Option<PathBuf> {
    None
}

#[cfg(unix)]
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

#[cfg(windows)]
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(PathBuf::from)
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
