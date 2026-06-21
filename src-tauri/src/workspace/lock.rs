use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum LockError {
    #[error("workspace already open in process {pid}")]
    AlreadyHeld { pid: u32 },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// PID-based lock file at `.ire/.lock`. Released on drop.
#[derive(Debug)]
pub struct WorkspaceLock {
    path: PathBuf,
}

impl WorkspaceLock {
    pub fn acquire(home_data_dir: &Path) -> Result<Self, LockError> {
        let path = home_data_dir.join(".lock");
        let our_pid = std::process::id();

        loop {
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    file.write_all(our_pid.to_string().as_bytes())?;
                    file.sync_all()?;
                    tracing::info!(?path, pid = our_pid, "acquired workspace lock");
                    return Ok(Self { path });
                }
                Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                    let mut existing = String::new();
                    OpenOptions::new()
                        .read(true)
                        .open(&path)?
                        .read_to_string(&mut existing)?;
                    let existing_pid: u32 = existing.trim().parse().unwrap_or(0);
                    if existing_pid != 0 && pid_alive(existing_pid) {
                        return Err(LockError::AlreadyHeld { pid: existing_pid });
                    }
                    tracing::warn!(stale_pid = existing_pid, "reclaiming stale workspace lock");
                    fs::remove_file(&path)?;
                    continue;
                }
                Err(e) => return Err(LockError::Io(e)),
            }
        }
    }
}

impl Drop for WorkspaceLock {
    fn drop(&mut self) {
        if let Err(e) = fs::remove_file(&self.path) {
            tracing::warn!(error = %e, "failed to release workspace lock");
        }
    }
}

#[cfg(unix)]
fn pid_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    // SAFETY: kill(pid, 0) only checks signal-deliverability; no signal sent.
    let res = unsafe { libc::kill(pid as libc::pid_t, 0) };
    if res == 0 {
        return true;
    }
    let err = std::io::Error::last_os_error();
    err.raw_os_error() == Some(libc::EPERM)
}

#[cfg(windows)]
fn pid_alive(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, FALSE};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, STILL_ACTIVE,
    };
    if pid == 0 {
        return false;
    }
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
        if handle == 0 {
            return false;
        }
        let mut code: u32 = 0;
        let alive = GetExitCodeProcess(handle, &mut code) != 0 && code as i32 == STILL_ACTIVE;
        CloseHandle(handle);
        alive
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acquire_then_drop_releases() {
        let tmp = tempfile::tempdir().unwrap();
        {
            let _lock = WorkspaceLock::acquire(tmp.path()).unwrap();
            assert!(tmp.path().join(".lock").exists());
        }
        assert!(!tmp.path().join(".lock").exists());
    }

    #[test]
    fn second_acquire_in_same_process_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let _first = WorkspaceLock::acquire(tmp.path()).unwrap();
        let err = WorkspaceLock::acquire(tmp.path()).unwrap_err();
        assert!(matches!(err, LockError::AlreadyHeld { .. }));
    }

    #[test]
    fn stale_lock_is_reclaimed() {
        let tmp = tempfile::tempdir().unwrap();
        let lock_path = tmp.path().join(".lock");
        // PID 0 is never alive; acts as a stale entry.
        std::fs::write(&lock_path, "0").unwrap();
        let _lock = WorkspaceLock::acquire(tmp.path()).unwrap();
        let contents = std::fs::read_to_string(&lock_path).unwrap();
        assert_eq!(contents, std::process::id().to_string());
    }
}
