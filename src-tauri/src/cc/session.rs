use std::collections::HashMap;
use std::sync::{Arc, Mutex};

struct PerTabSession {
    session_id: Option<String>,
    running_pid: Option<u32>,
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
    pub fn get_session_id(&self, tab_id: &str) -> Option<String> {
        self.0.lock().unwrap().get(tab_id)?.session_id.clone()
    }

    pub fn set_session_id(&self, tab_id: &str, sid: String) {
        let mut map = self.0.lock().unwrap();
        map.entry(tab_id.to_string())
            .or_insert_with(|| PerTabSession { session_id: None, running_pid: None })
            .session_id = Some(sid);
    }

    pub fn get_pid(&self, tab_id: &str) -> Option<u32> {
        self.0.lock().unwrap().get(tab_id)?.running_pid
    }

    pub fn set_pid(&self, tab_id: &str, pid: u32) {
        let mut map = self.0.lock().unwrap();
        map.entry(tab_id.to_string())
            .or_insert_with(|| PerTabSession { session_id: None, running_pid: None })
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
        }
    }
}
