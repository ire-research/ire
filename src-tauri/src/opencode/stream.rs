use serde_json::Value;

use crate::stream_event::{StreamEvent, StreamState};
use crate::tool_cards::{build_tool_call, text_output, ToolProvider, ToolStatus};

/// Dispatches one `opencode run --format json` JSONL line.
///
/// Every event carries a top-level `sessionID`, including the very first one
/// of a brand-new (non-resumed) turn — confirmed empirically, there is no
/// separate "session started" event type like Claude/Codex have. `Init` is
/// therefore emitted once, off whichever event arrives first.
///
/// `step_finish.part.reason` is the turn-continuation signal: `"stop"` means
/// the turn is done, `"tool-calls"` means more steps follow (the model made
/// tool calls and will keep going) — confirmed by capturing a real multi-step
/// transcript. Any other/unrecognized reason is treated as "not done yet"
/// rather than risk ending a turn early; a genuine failure surfaces as its
/// own top-level `"error"` event instead (confirmed shape:
/// `{"type":"error","error":{"name","data":{"message"}}}`).
///
/// TODO(opencode-mcp-tools): tool calls routed through an MCP server (e.g.
/// IRE's own `ask_user_question`) haven't been verified against a live
/// MCP round-trip yet — `part.tool`'s exact naming convention for
/// MCP-sourced tools is unconfirmed, so there's no `AskUserQuestion`
/// suppression here yet (contrast `claude_code::stream`'s handling of the
/// same tool). Regular non-MCP tools (bash, file edits, etc.) are unaffected.
pub fn dispatch<F: FnMut(StreamEvent)>(json: &Value, state: &mut StreamState, emit: &mut F) {
    if state.session_id.is_empty() {
        if let Some(sid) = json["sessionID"].as_str() {
            if !sid.is_empty() {
                state.session_id = sid.to_string();
                emit(StreamEvent::Init {
                    session_id: sid.to_string(),
                });
            }
        }
    }

    match json["type"].as_str().unwrap_or("") {
        "text" => {
            if let Some(text) = json["part"]["text"].as_str() {
                if !text.is_empty() {
                    state.emitted_text = true;
                    emit(StreamEvent::TextDelta {
                        text: text.to_string(),
                    });
                }
            }
        }
        "tool_use" => {
            let part = &json["part"];
            let Some(tool_id) = part["callID"].as_str().filter(|s| !s.is_empty()) else {
                return;
            };
            let raw_name = part["tool"].as_str().unwrap_or("tool").to_string();
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
                    raw_name,
                    part,
                    input_preview,
                    input_full,
                ),
            });

            // Every capture so far arrives with `state.status` already
            // "completed" — opencode's --format json doesn't appear to emit
            // a separate pending/running tool event, so Start+Done fire
            // together rather than waiting for a distinct completion event.
            let status = if part["state"]["status"].as_str() == Some("error") {
                ToolStatus::Failed
            } else {
                ToolStatus::Completed
            };
            let output_text = part["state"]["output"].as_str().map(|s| s.to_string());
            let output_preview = output_text
                .as_deref()
                .and_then(|s| s.lines().next())
                .map(|s| trunc_chars(s.trim(), 80));
            let output_full = output_text.map(|s| trunc_chars(&s, 10_000));

            emit(StreamEvent::ToolDone {
                tool_id: tool_id.to_string(),
                output: text_output(output_preview, output_full),
                status,
                meta: Value::Object(Default::default()),
            });
        }
        "step_finish" => {
            if json["part"]["reason"].as_str() == Some("stop") {
                emit(StreamEvent::Result {
                    text: None,
                    session_id: state.session_id.clone(),
                });
                state.emitted_done = true;
                emit(StreamEvent::Done);
            }
        }
        "error" => {
            let msg = json["error"]["data"]["message"]
                .as_str()
                .or_else(|| json["error"]["message"].as_str())
                .unwrap_or("unknown error")
                .to_string();
            emit(StreamEvent::Error { message: msg });
            state.emitted_done = true;
            emit(StreamEvent::Done);
        }
        _ => {}
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

    fn run(events: &[Value]) -> Vec<StreamEvent> {
        let mut state = StreamState::default();
        let mut out = Vec::new();
        for e in events {
            dispatch(e, &mut state, &mut |ev| out.push(ev));
        }
        out
    }

    #[test]
    fn emits_init_once_from_first_events_session_id() {
        let events = vec![json!({
            "type": "step_start",
            "sessionID": "ses_abc",
            "part": {"type": "step-start"}
        })];
        let out = run(&events);
        assert_eq!(
            out,
            vec![StreamEvent::Init {
                session_id: "ses_abc".to_string()
            }]
        );
    }

    #[test]
    fn emits_text_delta_for_each_text_part() {
        let events = vec![
            json!({"type": "step_start", "sessionID": "ses_1", "part": {}}),
            json!({"type": "text", "sessionID": "ses_1", "part": {"text": "pong"}}),
        ];
        let out = run(&events);
        assert_eq!(
            out,
            vec![
                StreamEvent::Init { session_id: "ses_1".to_string() },
                StreamEvent::TextDelta { text: "pong".to_string() },
            ]
        );
    }

    #[test]
    fn tool_use_emits_start_then_done_from_one_event() {
        let events = vec![
            json!({"type": "step_start", "sessionID": "ses_1", "part": {}}),
            json!({
                "type": "tool_use",
                "sessionID": "ses_1",
                "part": {
                    "type": "tool",
                    "tool": "bash",
                    "callID": "call_1",
                    "state": {
                        "status": "completed",
                        "input": {"command": "ls"},
                        "output": "a.txt\nb.txt\n"
                    }
                }
            }),
        ];
        let out = run(&events);
        assert_eq!(out.len(), 3);
        match &out[1] {
            StreamEvent::ToolStart { tool } => {
                assert_eq!(tool.tool_id, "call_1");
                assert_eq!(tool.raw_name, "bash");
                assert_eq!(tool.provider, ToolProvider::Opencode);
            }
            other => panic!("expected ToolStart, got {other:?}"),
        }
        match &out[2] {
            StreamEvent::ToolDone { tool_id, status, output, .. } => {
                assert_eq!(tool_id, "call_1");
                assert_eq!(*status, ToolStatus::Completed);
                assert_eq!(output.as_ref().unwrap().preview.as_deref(), Some("a.txt"));
            }
            other => panic!("expected ToolDone, got {other:?}"),
        }
    }

    #[test]
    fn step_finish_with_tool_calls_reason_does_not_end_turn() {
        let events = vec![
            json!({"type": "step_start", "sessionID": "ses_1", "part": {}}),
            json!({"type": "step_finish", "sessionID": "ses_1", "part": {"reason": "tool-calls"}}),
        ];
        let out = run(&events);
        assert_eq!(out, vec![StreamEvent::Init { session_id: "ses_1".to_string() }]);
    }

    #[test]
    fn step_finish_with_stop_reason_ends_turn() {
        let events = vec![
            json!({"type": "step_start", "sessionID": "ses_1", "part": {}}),
            json!({"type": "step_finish", "sessionID": "ses_1", "part": {"reason": "stop"}}),
        ];
        let out = run(&events);
        assert_eq!(
            out,
            vec![
                StreamEvent::Init { session_id: "ses_1".to_string() },
                StreamEvent::Result { text: None, session_id: "ses_1".to_string() },
                StreamEvent::Done,
            ]
        );
    }

    #[test]
    fn emits_error_and_done_for_error_event() {
        let events = vec![json!({
            "type": "error",
            "sessionID": "ses_1",
            "error": {"name": "UnknownError", "data": {"message": "boom", "ref": "err_1"}}
        })];
        let out = run(&events);
        assert_eq!(
            out,
            vec![
                StreamEvent::Init { session_id: "ses_1".to_string() },
                StreamEvent::Error { message: "boom".to_string() },
                StreamEvent::Done,
            ]
        );
    }
}
