use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::oneshot;

/// A turn currently running for a tab: a subprocess pid, or (for OpenCode)
/// its session id — no pid to signal, cancelled via `POST /session/:id/abort`.
#[derive(Debug, Clone, PartialEq)]
pub enum RunningTurn {
    Process(u32),
    OpenCode { session_id: String },
}

/// Transient, in-process state for the current turn of a tab.
#[derive(Default)]
struct PerTabSession {
    session_uuid: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    effort: Option<String>,
    running: Option<RunningTurn>,
    pending_ask: Option<oneshot::Sender<Vec<serde_json::Value>>>,
    /// A native OpenCode `question.asked` awaiting a reply; distinct from
    /// `pending_ask` (Claude/Codex's MCP `ask_user_question`).
    pending_opencode_question: Option<String>,
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
    /// Records the transient state for the turn starting on `tab_id`.
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

    /// Atomically reads the running turn handle and its provider under one
    /// lock, so `chat_cancel` never sees them from two different moments.
    pub fn get_running_and_provider(&self, tab_id: &str) -> Option<(RunningTurn, Option<String>)> {
        let map = self.0.lock().unwrap();
        let session = map.get(tab_id)?;
        let running = session.running.clone()?;
        Some((running, session.provider.clone()))
    }

    pub fn set_pid(&self, tab_id: &str, pid: u32) {
        let mut map = self.0.lock().unwrap();
        map.entry(tab_id.to_string()).or_default().running = Some(RunningTurn::Process(pid));
    }

    pub fn set_running_opencode(&self, tab_id: &str, session_id: String) {
        let mut map = self.0.lock().unwrap();
        map.entry(tab_id.to_string()).or_default().running =
            Some(RunningTurn::OpenCode { session_id });
    }

    /// Clears the running turn only if it still matches `expected` (a newer
    /// turn may have already been registered under the same `tab_id`).
    pub fn clear_running_if(&self, tab_id: &str, expected: &RunningTurn) {
        if let Some(s) = self.0.lock().unwrap().get_mut(tab_id) {
            if s.running.as_ref() == Some(expected) {
                s.running = None;
            }
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

    /// First tab with a running turn, any transport. Used by `experiment.start`.
    pub fn get_active_session(&self) -> Option<ActiveSession> {
        self.find_active_session(|s| s.running.is_some())
    }

    /// First tab with a running `Process` turn specifically — narrower than
    /// `get_active_session` so a concurrent OpenCode tab can't steal it.
    pub fn get_active_process_session(&self) -> Option<ActiveSession> {
        self.find_active_session(|s| matches!(s.running, Some(RunningTurn::Process(_))))
    }

    fn find_active_session(&self, matches: impl Fn(&PerTabSession) -> bool) -> Option<ActiveSession> {
        let guard = self.0.lock().unwrap();
        guard
            .iter()
            .find(|(_, s)| matches(s))
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

    /// Registers a pending `ask_user_question` and returns its receiver.
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
            s.pending_opencode_question = None;
        }
    }

    /// Record the `requestID` of a native OpenCode `question.asked` event
    /// awaiting a reply for `tab_id`.
    pub fn set_pending_opencode_question(&self, tab_id: &str, request_id: String) {
        let mut map = self.0.lock().unwrap();
        map.entry(tab_id.to_string()).or_default().pending_opencode_question = Some(request_id);
    }

    /// Take (clear) the pending OpenCode question request id for `tab_id`,
    /// if any.
    pub fn take_pending_opencode_question(&self, tab_id: &str) -> Option<String> {
        self.0
            .lock()
            .unwrap()
            .get_mut(tab_id)?
            .pending_opencode_question
            .take()
    }

    /// Read (without clearing) the pending OpenCode question request id for
    /// `tab_id`, if any — used to keep the id around until a reply attempt
    /// actually succeeds, so a failed reply can be retried.
    pub fn peek_pending_opencode_question(&self, tab_id: &str) -> Option<String> {
        self.0
            .lock()
            .unwrap()
            .get(tab_id)?
            .pending_opencode_question
            .clone()
    }

    /// Drops all per-tab state and returns the running `Process` pids
    /// (OpenCode turns are cleaned up separately, via `OpenCodeRuntime`).
    pub fn drain(&self) -> Vec<u32> {
        let mut map = self.0.lock().unwrap();
        let pids = map
            .values()
            .filter_map(|s| match &s.running {
                Some(RunningTurn::Process(pid)) => Some(*pid),
                _ => None,
            })
            .collect();
        map.clear();
        pids
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_active_session_matches_opencode_turns_too() {
        let sm = SessionManager::default();
        sm.set_agent_options("tab-1", "uuid-1", "opencode", "anthropic/claude-opus-4-8", None);
        sm.set_running_opencode("tab-1", "ses_abc".to_string());

        let active = sm.get_active_session().expect("expected an active session");
        assert_eq!(active.tab_id, "tab-1");
        assert_eq!(active.provider, "opencode");
    }

    #[test]
    fn get_active_process_session_skips_opencode_and_finds_the_process_tab() {
        let sm = SessionManager::default();
        // An OpenCode tab is running concurrently...
        sm.set_agent_options("tab-opencode", "uuid-1", "opencode", "anthropic/claude-opus-4-8", None);
        sm.set_running_opencode("tab-opencode", "ses_abc".to_string());
        // ...alongside a Claude tab that's the one actually asking a question.
        sm.set_agent_options("tab-claude", "uuid-2", "claude", "claude-sonnet-5", None);
        sm.set_pid("tab-claude", 4242);

        let active = sm
            .get_active_process_session()
            .expect("expected the process tab, not the opencode one");
        assert_eq!(active.tab_id, "tab-claude");
        assert_eq!(active.provider, "claude");
    }

    #[test]
    fn get_active_process_session_is_none_when_only_opencode_is_running() {
        let sm = SessionManager::default();
        sm.set_agent_options("tab-1", "uuid-1", "opencode", "anthropic/claude-opus-4-8", None);
        sm.set_running_opencode("tab-1", "ses_abc".to_string());

        assert!(sm.get_active_process_session().is_none());
    }

    #[test]
    fn clear_running_if_only_clears_matching_handle() {
        let sm = SessionManager::default();
        sm.set_pid("tab-1", 111);
        sm.clear_running_if("tab-1", &RunningTurn::Process(222));
        assert_eq!(
            sm.get_running_and_provider("tab-1").map(|(r, _)| r),
            Some(RunningTurn::Process(111)),
            "mismatched pid must not clear"
        );

        sm.clear_running_if("tab-1", &RunningTurn::Process(111));
        assert!(sm.get_running_and_provider("tab-1").is_none());
    }
}
