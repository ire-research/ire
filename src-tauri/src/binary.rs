use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;
use thiserror::Error;

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
