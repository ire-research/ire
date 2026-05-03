use std::sync::{Arc, Mutex};

pub struct ChatSession {
    pub session_id: Arc<Mutex<Option<String>>>,
    pub running_pid: Arc<Mutex<Option<u32>>>,
}

impl Default for ChatSession {
    fn default() -> Self {
        Self {
            session_id: Arc::new(Mutex::new(None)),
            running_pid: Arc::new(Mutex::new(None)),
        }
    }
}
