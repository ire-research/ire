//! Parses `opencode serve`'s `/event` SSE stream into IRE's `StreamEvent`.

use std::collections::HashSet;

use serde_json::Value;

use crate::stream_event::StreamEvent;
use crate::tool_cards::{build_tool_call, text_output, ToolProvider, ToolStatus};

/// One parsed `/event` payload, reduced to the subset IRE acts on.
/// Everything else is `Other`.
#[derive(Debug, PartialEq)]
pub enum OpenCodeEvent {
    SessionIdle {
        session_id: String,
    },
    SessionError {
        session_id: Option<String>,
        message: String,
    },
    /// Used only to tell an assistant message id apart from a user one.
    MessageUpdated {
        session_id: String,
        message_id: String,
        role: String,
    },
    MessagePartUpdated {
        session_id: String,
        part: Value,
    },
    QuestionAsked {
        session_id: String,
        request_id: String,
        questions: Vec<Value>,
    },
    Other,
}

pub fn parse_event(raw: &Value) -> OpenCodeEvent {
    let props = &raw["properties"];
    match raw["type"].as_str().unwrap_or("") {
        "session.idle" => OpenCodeEvent::SessionIdle {
            session_id: props["sessionID"].as_str().unwrap_or_default().to_string(),
        },
        "session.error" => OpenCodeEvent::SessionError {
            session_id: props["sessionID"].as_str().map(str::to_string),
            message: extract_error_message(&props["error"]),
        },
        "message.updated" => OpenCodeEvent::MessageUpdated {
            session_id: props["sessionID"].as_str().unwrap_or_default().to_string(),
            message_id: props["info"]["id"].as_str().unwrap_or_default().to_string(),
            role: props["info"]["role"].as_str().unwrap_or_default().to_string(),
        },
        "message.part.updated" => OpenCodeEvent::MessagePartUpdated {
            session_id: props["sessionID"].as_str().unwrap_or_default().to_string(),
            part: props["part"].clone(),
        },
        "question.asked" => OpenCodeEvent::QuestionAsked {
            session_id: props["sessionID"].as_str().unwrap_or_default().to_string(),
            request_id: props["id"].as_str().unwrap_or_default().to_string(),
            questions: props["questions"].as_array().cloned().unwrap_or_default(),
        },
        _ => OpenCodeEvent::Other,
    }
}

/// `error` is a tagged union; try `data.message`, `message`, then `name`.
fn extract_error_message(error: &Value) -> String {
    error["data"]["message"]
        .as_str()
        .or_else(|| error["message"].as_str())
        .map(str::to_string)
        .unwrap_or_else(|| error["name"].as_str().unwrap_or("unknown error").to_string())
}

/// Per-session bookkeeping: turns "full part on every update" SSE payloads
/// into suffix deltas and dedupes tool start/done events.
#[derive(Default)]
pub struct OpenCodeSessionState {
    text_len_by_part: std::collections::HashMap<String, usize>,
    reasoning_len_by_part: std::collections::HashMap<String, usize>,
    tool_started: HashSet<String>,
    tool_done: HashSet<String>,
    /// Message ids confirmed (by role, not by arrival order) as the
    /// assistant's — an allow-list so the user's own echoed prompt is
    /// never misattributed.
    assistant_message_ids: HashSet<String>,
    /// Message ids confirmed as *not* the assistant's, so a role lookup
    /// isn't repeated for every part update on the same message.
    other_message_ids: HashSet<String>,
}

impl OpenCodeSessionState {
    pub fn mark_assistant_message(&mut self, message_id: String) {
        self.assistant_message_ids.insert(message_id);
    }

    pub fn mark_other_message(&mut self, message_id: String) {
        self.other_message_ids.insert(message_id);
    }

    /// Whether this message's role has already been resolved either way.
    pub fn role_known(&self, message_id: &str) -> bool {
        self.assistant_message_ids.contains(message_id) || self.other_message_ids.contains(message_id)
    }
}

/// Normalizes one `message.part.updated` part. Parts not (yet) known to
/// belong to the assistant's message are dropped.
pub fn normalize_part(part: &Value, state: &mut OpenCodeSessionState, emit: &mut dyn FnMut(StreamEvent)) {
    let Some(message_id) = part["messageID"].as_str() else { return };
    if !state.assistant_message_ids.contains(message_id) {
        return;
    }
    match part["type"].as_str().unwrap_or("") {
        "text" => emit_suffix_delta(part, &mut state.text_len_by_part, emit, |text| {
            StreamEvent::TextDelta { text }
        }),
        "reasoning" => emit_suffix_delta(part, &mut state.reasoning_len_by_part, emit, |text| {
            StreamEvent::ThinkingDelta { text }
        }),
        "tool" => normalize_tool_part(part, state, emit),
        _ => {}
    }
}

fn emit_suffix_delta(
    part: &Value,
    seen_by_part: &mut std::collections::HashMap<String, usize>,
    emit: &mut dyn FnMut(StreamEvent),
    to_event: impl FnOnce(String) -> StreamEvent,
) {
    let Some(id) = part["id"].as_str() else { return };
    let text = part["text"].as_str().unwrap_or_default();
    let seen = seen_by_part.entry(id.to_string()).or_insert(0);
    let total = text.chars().count();
    if total <= *seen {
        return;
    }
    let suffix: String = text.chars().skip(*seen).collect();
    *seen = total;
    if !suffix.is_empty() {
        emit(to_event(suffix));
    }
}

fn normalize_tool_part(part: &Value, state: &mut OpenCodeSessionState, emit: &mut dyn FnMut(StreamEvent)) {
    let Some(tool_id) = part["callID"].as_str().filter(|s| !s.is_empty()) else {
        return;
    };
    let status = part["state"]["status"].as_str().unwrap_or("");
    let raw_name = || part["tool"].as_str().unwrap_or("tool").to_string();

    let started_now = matches!(status, "pending" | "running" | "completed" | "error")
        && state.tool_started.insert(tool_id.to_string());
    if started_now {
        let input = &part["state"]["input"];
        let input_preview = first_string_field(input).map(|s| trunc_chars(s, 80));
        let input_full = serde_json::to_string_pretty(input)
            .ok()
            .filter(|_| !input.is_null())
            .map(|s| trunc_chars(&s, 10_000));
        emit(StreamEvent::ToolStart {
            tool: build_tool_call(
                ToolProvider::Opencode,
                tool_id.to_string(),
                raw_name(),
                part,
                input_preview,
                input_full,
            ),
        });
    }

    if matches!(status, "completed" | "error") && state.tool_done.insert(tool_id.to_string()) {
        let tool_status = if status == "error" {
            ToolStatus::Failed
        } else {
            ToolStatus::Completed
        };
        let output_text = if status == "error" {
            part["state"]["error"].as_str().map(str::to_string)
        } else {
            part["state"]["output"].as_str().map(str::to_string)
        };
        let output_preview = output_text
            .as_deref()
            .and_then(|s| s.lines().next())
            .map(|s| trunc_chars(s.trim(), 80));
        let output_full = output_text.map(|s| trunc_chars(&s, 10_000));

        emit(StreamEvent::ToolDone {
            tool_id: tool_id.to_string(),
            output: text_output(output_preview, output_full),
            status: tool_status,
            meta: Value::Object(Default::default()),
        });
    }
}

fn first_string_field(value: &Value) -> Option<&str> {
    for key in ["command", "path", "file_path", "filename", "pattern", "query", "url"] {
        if let Some(s) = value.get(key).and_then(Value::as_str) {
            if !s.is_empty() {
                return Some(s);
            }
        }
    }
    None
}

fn trunc_chars(s: &str, max: usize) -> String {
    let count = s.chars().count();
    if count <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect::<String>() + "..."
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Default message id, pre-confirmed as the assistant's.
    const ASSISTANT_MSG: &str = "msg_assistant";

    fn assistant_state() -> OpenCodeSessionState {
        let mut state = OpenCodeSessionState::default();
        state.mark_assistant_message(ASSISTANT_MSG.to_string());
        state
    }

    fn run_part(part: &Value) -> Vec<StreamEvent> {
        let mut state = assistant_state();
        let mut out = Vec::new();
        normalize_part(part, &mut state, &mut |e| out.push(e));
        out
    }

    #[test]
    fn parses_session_idle() {
        let raw = json!({"type": "session.idle", "properties": {"sessionID": "ses_1"}});
        assert_eq!(
            parse_event(&raw),
            OpenCodeEvent::SessionIdle { session_id: "ses_1".to_string() }
        );
    }

    #[test]
    fn parses_session_error_message_from_data() {
        let raw = json!({
            "type": "session.error",
            "properties": {"sessionID": "ses_1", "error": {"name": "APIError", "data": {"message": "No provider available"}}}
        });
        assert_eq!(
            parse_event(&raw),
            OpenCodeEvent::SessionError {
                session_id: Some("ses_1".to_string()),
                message: "No provider available".to_string()
            }
        );
    }

    #[test]
    fn parses_question_asked() {
        let raw = json!({
            "type": "question.asked",
            "properties": {"id": "que_1", "sessionID": "ses_1", "questions": [{"question": "which?", "header": "h", "options": []}]}
        });
        match parse_event(&raw) {
            OpenCodeEvent::QuestionAsked { session_id, request_id, questions } => {
                assert_eq!(session_id, "ses_1");
                assert_eq!(request_id, "que_1");
                assert_eq!(questions.len(), 1);
            }
            other => panic!("expected QuestionAsked, got {other:?}"),
        }
    }

    #[test]
    fn unrecognized_event_is_other() {
        let raw = json!({"type": "plugin.added", "properties": {}});
        assert_eq!(parse_event(&raw), OpenCodeEvent::Other);
    }

    #[test]
    fn text_part_emits_suffix_delta_across_updates() {
        let mut state = assistant_state();
        let mut out = Vec::new();
        normalize_part(
            &json!({"type": "text", "id": "prt_1", "messageID": ASSISTANT_MSG, "text": "Hello"}),
            &mut state,
            &mut |e| out.push(e),
        );
        normalize_part(
            &json!({"type": "text", "id": "prt_1", "messageID": ASSISTANT_MSG, "text": "Hello, world"}),
            &mut state,
            &mut |e| out.push(e),
        );
        assert_eq!(
            out,
            vec![
                StreamEvent::TextDelta { text: "Hello".to_string() },
                StreamEvent::TextDelta { text: ", world".to_string() },
            ]
        );
    }

    #[test]
    fn reasoning_part_emits_thinking_delta() {
        let out = run_part(&json!({"type": "reasoning", "id": "prt_1", "messageID": ASSISTANT_MSG, "text": "thinking..."}));
        assert_eq!(out, vec![StreamEvent::ThinkingDelta { text: "thinking...".to_string() }]);
    }

    #[test]
    fn tool_part_pending_then_completed_emits_start_once_then_done() {
        let mut state = assistant_state();
        let mut out = Vec::new();
        normalize_part(
            &json!({"type": "tool", "callID": "call_1", "messageID": ASSISTANT_MSG, "tool": "bash", "state": {"status": "pending", "input": {"command": "ls"}}}),
            &mut state,
            &mut |e| out.push(e),
        );
        normalize_part(
            &json!({"type": "tool", "callID": "call_1", "messageID": ASSISTANT_MSG, "tool": "bash", "state": {"status": "running", "input": {"command": "ls"}}}),
            &mut state,
            &mut |e| out.push(e),
        );
        normalize_part(
            &json!({"type": "tool", "callID": "call_1", "messageID": ASSISTANT_MSG, "tool": "bash", "state": {"status": "completed", "input": {"command": "ls"}, "output": "a.txt\n"}}),
            &mut state,
            &mut |e| out.push(e),
        );
        assert_eq!(out.len(), 2);
        assert!(matches!(out[0], StreamEvent::ToolStart { .. }));
        match &out[1] {
            StreamEvent::ToolDone { tool_id, status, output, .. } => {
                assert_eq!(tool_id, "call_1");
                assert_eq!(*status, ToolStatus::Completed);
                assert_eq!(output.as_ref().unwrap().preview.as_deref(), Some("a.txt"));
            }
            other => panic!("expected ToolDone, got {other:?}"),
        }
    }

    #[test]
    fn tool_part_jumping_straight_to_error_emits_start_and_done() {
        let out = run_part(&json!({
            "type": "tool", "callID": "call_2", "messageID": ASSISTANT_MSG, "tool": "bash",
            "state": {"status": "error", "input": {"command": "false"}, "error": "exit 1"}
        }));
        assert_eq!(out.len(), 2);
        assert!(matches!(out[0], StreamEvent::ToolStart { .. }));
        match &out[1] {
            StreamEvent::ToolDone { status, .. } => assert_eq!(*status, ToolStatus::Failed),
            other => panic!("expected ToolDone, got {other:?}"),
        }
    }

    #[test]
    fn duplicate_sse_update_does_not_duplicate_events() {
        let mut state = assistant_state();
        let mut out = Vec::new();
        let completed = json!({
            "type": "tool", "callID": "call_3", "messageID": ASSISTANT_MSG, "tool": "bash",
            "state": {"status": "completed", "input": {}, "output": "ok"}
        });
        normalize_part(&completed, &mut state, &mut |e| out.push(e));
        normalize_part(&completed, &mut state, &mut |e| out.push(e));
        assert_eq!(out.len(), 2, "second identical update must be a no-op");
    }

    #[test]
    fn parses_message_updated_role_and_id() {
        let raw = json!({
            "type": "message.updated",
            "properties": {"sessionID": "ses_1", "info": {"id": "msg_1", "role": "assistant"}}
        });
        assert_eq!(
            parse_event(&raw),
            OpenCodeEvent::MessageUpdated {
                session_id: "ses_1".to_string(),
                message_id: "msg_1".to_string(),
                role: "assistant".to_string(),
            }
        );
    }

    /// Regression test: OpenCode echoes the user's own prompt as a part
    /// update too, and it must not leak into the assistant's reply.
    #[test]
    fn text_part_on_unmarked_message_id_is_dropped() {
        let mut state = OpenCodeSessionState::default();
        let mut out = Vec::new();

        normalize_part(
            &json!({"type": "text", "id": "prt_user", "messageID": "msg_user", "text": "hello there"}),
            &mut state,
            &mut |e| out.push(e),
        );
        assert!(out.is_empty(), "user's own echoed text must not become a StreamEvent");

        state.mark_assistant_message("msg_assistant".to_string());
        normalize_part(
            &json!({"type": "text", "id": "prt_reply", "messageID": "msg_assistant", "text": "hi!"}),
            &mut state,
            &mut |e| out.push(e),
        );
        assert_eq!(out, vec![StreamEvent::TextDelta { text: "hi!".to_string() }]);
    }
}
