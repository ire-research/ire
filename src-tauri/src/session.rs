use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::oneshot;

/// Transient, in-process state for the current turn of a tab. The durable resume
/// id lives in the `chat_sessions` table (keyed by `session_uuid`); this holds
/// only what the experiment wake-up flow needs to re-attach to a live turn.
#[derive(Default)]
struct PerTabSession {
    session_uuid: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    effort: Option<String>,
    running_pid: Option<u32>,
    pending_ask: Option<oneshot::Sender<Vec<serde_json::Value>>>,
}

pub struct ActiveSession {
    pub tab_id: String,
    pub session_uuid: String,
    pub provider: String,
    pub model: String,
    pub effort: Option<String>,
}

/// Holds per-tab agent session state (any `AgentProvider`). Clone is cheap (Arc clone).
#[derive(Clone)]
pub struct SessionManager(Arc<Mutex<HashMap<String, PerTabSession>>>);

impl Default for SessionManager {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(HashMap::new())))
    }
}

impl SessionManager {
    /// Record the transient state for the turn starting on `tab_id`: the session
    /// uuid it belongs to and the agent options in use. The wake-up flow reads
    /// these back via `get_active_session`.
    pub fn set_agent_options(
        &self,
        tab_id: &str,
        session_uuid: &str,
        provider: &str,
        model: &str,
        effort: Option<&str>,
    ) {
        let mut map = self.0.lock().unwrap();
        let session = map.entry(tab_id.to_string()).or_default();
        session.session_uuid = Some(session_uuid.to_string());
        session.provider = Some(provider.to_string());
        session.model = Some(model.to_string());
        session.effort = effort.map(str::to_string);
    }

    pub fn get_pid(&self, tab_id: &str) -> Option<u32> {
        self.0.lock().unwrap().get(tab_id)?.running_pid
    }

    /// The wire-format provider name (`"claude"` / `"codex"`) the current
    /// turn on `tab_id` is running, if known. Used by `chat_cancel` to route
    /// through the right `AgentProvider::cancel`.
    pub fn get_provider(&self, tab_id: &str) -> Option<String> {
        self.0.lock().unwrap().get(tab_id)?.provider.clone()
    }

    pub fn set_pid(&self, tab_id: &str, pid: u32) {
        let mut map = self.0.lock().unwrap();
        map.entry(tab_id.to_string()).or_default().running_pid = Some(pid);
    }

    pub fn clear_pid(&self, tab_id: &str) {
        if let Some(s) = self.0.lock().unwrap().get_mut(tab_id) {
            s.running_pid = None;
        }
    }

    pub fn reset(&self, tab_id: &str) {
        if let Some(s) = self.0.lock().unwrap().get_mut(tab_id) {
            s.session_uuid = None;
            s.provider = None;
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
                let session_uuid = s.session_uuid.clone()?;
                let provider = s.provider.clone()?;
                let model = s.model.clone()?;
                Some(ActiveSession {
                    tab_id: tab_id.clone(),
                    session_uuid,
                    provider,
                    model,
                    effort: s.effort.clone(),
                })
            })
    }

    /// Register a pending `ask_user_question` for `tab_id` and return a
    /// receiver that resolves once the user submits their answers via
    /// `submit_ask`. Called from the MCP RPC handler, which blocks on it.
    pub fn register_ask(&self, tab_id: &str) -> oneshot::Receiver<Vec<serde_json::Value>> {
        let (tx, rx) = oneshot::channel();
        let mut map = self.0.lock().unwrap();
        map.entry(tab_id.to_string()).or_default().pending_ask = Some(tx);
        rx
    }

    /// Deliver the user's answers to the pending `ask_user_question` for
    /// `tab_id`, if any. Returns `false` if there was nothing pending.
    pub fn submit_ask(&self, tab_id: &str, answers: Vec<serde_json::Value>) -> bool {
        let mut map = self.0.lock().unwrap();
        match map.get_mut(tab_id).and_then(|s| s.pending_ask.take()) {
            Some(tx) => tx.send(answers).is_ok(),
            None => false,
        }
    }

    /// Drop a pending `ask_user_question` without answering it, so the
    /// blocked MCP handler returns an error instead of hanging forever.
    pub fn cancel_ask(&self, tab_id: &str) {
        if let Some(s) = self.0.lock().unwrap().get_mut(tab_id) {
            s.pending_ask = None;
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
