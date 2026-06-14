use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

const SESSIONS_FILE: &str = "sessions.json";

fn sessions_path(ire_dir: &Path) -> PathBuf {
    ire_dir.join(SESSIONS_FILE)
}

#[derive(Serialize, Deserialize)]
struct PersistedSession {
    session_id: String,
    session_provider: String,
}

struct PerTabSession {
    session_id: Option<String>,
    session_provider: Option<String>,
    model: Option<String>,
    effort: Option<String>,
    running_pid: Option<u32>,
}

pub struct ActiveSession {
    pub tab_id: String,
    pub session_id: String,
    pub provider: String,
    pub model: String,
    pub effort: Option<String>,
}

/// Holds per-tab CC session state. Clone is cheap (Arc clone).
#[derive(Clone)]
pub struct SessionManager(Arc<Mutex<HashMap<String, PerTabSession>>>);

impl Default for SessionManager {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(HashMap::new())))
    }
}

impl SessionManager {
    pub fn get_session_id_for_provider(&self, tab_id: &str, provider: &str) -> Option<String> {
        let guard = self.0.lock().unwrap();
        let session = guard.get(tab_id)?;
        if session.session_provider.as_deref().unwrap_or("claude") == provider {
            session.session_id.clone()
        } else {
            None
        }
    }

    pub fn set_session_id_for_provider(&self, tab_id: &str, provider: &str, sid: String) {
        let mut map = self.0.lock().unwrap();
        let session = map
            .entry(tab_id.to_string())
            .or_insert_with(|| PerTabSession {
                session_id: None,
                session_provider: None,
                model: None,
                effort: None,
                running_pid: None,
            });
        session.session_id = Some(sid);
        session.session_provider = Some(provider.to_string());
    }

    pub fn set_agent_options(
        &self,
        tab_id: &str,
        provider: &str,
        model: &str,
        effort: Option<&str>,
    ) {
        let mut map = self.0.lock().unwrap();
        let session = map
            .entry(tab_id.to_string())
            .or_insert_with(|| PerTabSession {
                session_id: None,
                session_provider: None,
                model: None,
                effort: None,
                running_pid: None,
            });
        if session.session_provider.as_deref() != Some(provider) {
            session.session_id = None;
            session.session_provider = Some(provider.to_string());
        }
        session.model = Some(model.to_string());
        session.effort = effort.map(str::to_string);
    }

    pub fn get_pid(&self, tab_id: &str) -> Option<u32> {
        self.0.lock().unwrap().get(tab_id)?.running_pid
    }

    pub fn set_pid(&self, tab_id: &str, pid: u32) {
        let mut map = self.0.lock().unwrap();
        map.entry(tab_id.to_string())
            .or_insert_with(|| PerTabSession {
                session_id: None,
                session_provider: None,
                model: None,
                effort: None,
                running_pid: None,
            })
            .running_pid = Some(pid);
    }

    pub fn clear_pid(&self, tab_id: &str) {
        if let Some(s) = self.0.lock().unwrap().get_mut(tab_id) {
            s.running_pid = None;
        }
    }

    pub fn reset(&self, tab_id: &str) {
        if let Some(s) = self.0.lock().unwrap().get_mut(tab_id) {
            s.session_id = None;
            s.session_provider = None;
            s.model = None;
            s.effort = None;
        }
    }

    /// Returns the first tab with a running agent subprocess.
    pub fn get_active_session(&self) -> Option<ActiveSession> {
        let guard = self.0.lock().unwrap();
        guard
            .iter()
            .find(|(_, s)| s.running_pid.is_some())
            .and_then(|(tab_id, s)| {
                let sid = s.session_id.as_ref()?;
                let provider = s.session_provider.clone()?;
                let model = s.model.clone()?;
                Some(ActiveSession {
                    tab_id: tab_id.clone(),
                    session_id: sid.clone(),
                    provider,
                    model,
                    effort: s.effort.clone(),
                })
            })
    }

    /// Restore each tab's `session_id`/`session_provider` from
    /// `.ire/sessions.json`, written by `persist_session`. Called once on
    /// workspace attach so CC `--resume` keeps working across backend
    /// restarts (the in-memory map is otherwise lost on restart).
    pub fn restore_from_disk(&self, ire_dir: &Path) {
        let Ok(content) = fs::read_to_string(sessions_path(ire_dir)) else {
            return;
        };
        let Ok(sessions) = serde_json::from_str::<HashMap<String, PersistedSession>>(&content)
        else {
            return;
        };
        for (tab_id, s) in sessions {
            self.set_session_id_for_provider(&tab_id, &s.session_provider, s.session_id);
        }
    }

    /// Persist this tab's current `session_id`/`session_provider` to
    /// `.ire/sessions.json` so it survives a backend restart.
    pub fn persist_session(&self, ire_dir: &Path, tab_id: &str) {
        let (session_id, session_provider) = {
            let guard = self.0.lock().unwrap();
            match guard.get(tab_id) {
                Some(PerTabSession {
                    session_id: Some(sid),
                    session_provider: Some(provider),
                    ..
                }) => (sid.clone(), provider.clone()),
                _ => return,
            }
        };

        let path = sessions_path(ire_dir);
        let mut sessions: HashMap<String, PersistedSession> = fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        sessions.insert(
            tab_id.to_string(),
            PersistedSession {
                session_id,
                session_provider,
            },
        );
        if let Ok(json) = serde_json::to_string_pretty(&sessions) {
            let _ = crate::wiki::store::atomic_write(&path, &json);
        }
    }

    /// Remove this tab's persisted session so the next `chat_send` starts a
    /// fresh CC session without `--resume`.
    pub fn clear_persisted_session(&self, ire_dir: &Path, tab_id: &str) {
        let path = sessions_path(ire_dir);
        let Ok(content) = fs::read_to_string(&path) else {
            return;
        };
        let Ok(mut sessions) = serde_json::from_str::<HashMap<String, PersistedSession>>(&content)
        else {
            return;
        };
        if sessions.remove(tab_id).is_some() {
            if let Ok(json) = serde_json::to_string_pretty(&sessions) {
                let _ = crate::wiki::store::atomic_write(&path, &json);
            }
        }
    }

    /// Drop all per-tab state and return the PIDs that were running. Used on
    /// workspace close to terminate stragglers; their late chat-stream events
    /// would otherwise leak into the next workspace because the frontend
    /// listener is global.
    pub fn drain(&self) -> Vec<u32> {
        let mut map = self.0.lock().unwrap();
        let pids = map.values().filter_map(|s| s.running_pid).collect();
        map.clear();
        pids
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persist_and_restore_round_trip_across_managers() {
        let dir = tempfile::tempdir().unwrap();

        let sm = SessionManager::default();
        sm.set_session_id_for_provider("main", "claude", "sess-1".to_string());
        sm.persist_session(dir.path(), "main");

        let restored = SessionManager::default();
        restored.restore_from_disk(dir.path());
        assert_eq!(
            restored.get_session_id_for_provider("main", "claude"),
            Some("sess-1".to_string())
        );
    }

    #[test]
    fn clear_persisted_session_removes_only_that_tab() {
        let dir = tempfile::tempdir().unwrap();

        let sm = SessionManager::default();
        sm.set_session_id_for_provider("main", "claude", "sess-1".to_string());
        sm.persist_session(dir.path(), "main");
        sm.set_session_id_for_provider("other", "claude", "sess-2".to_string());
        sm.persist_session(dir.path(), "other");

        sm.clear_persisted_session(dir.path(), "main");

        let restored = SessionManager::default();
        restored.restore_from_disk(dir.path());
        assert_eq!(restored.get_session_id_for_provider("main", "claude"), None);
        assert_eq!(
            restored.get_session_id_for_provider("other", "claude"),
            Some("sess-2".to_string())
        );
    }
}
