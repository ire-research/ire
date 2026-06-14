use serde::Serialize;
use serde_json::Value;

use crate::tool_cards::{build_tool_call, text_output, ToolCall, ToolIo, ToolProvider, ToolStatus};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AskQuestionOption {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AskQuestion {
    pub header: String,
    pub question: String,
    pub multi_select: bool,
    pub options: Vec<AskQuestionOption>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind")]
pub enum StreamEvent {
    Init {
        session_id: String,
    },
    TextDelta {
        text: String,
    },
    ThinkingDelta {
        text: String,
    },
    ToolStart {
        tool: ToolCall,
    },
    ToolDone {
        tool_id: String,
        output: Option<ToolIo>,
        status: ToolStatus,
        meta: Value,
    },
    AskUserQuestion {
        tool_id: String,
        questions: Vec<AskQuestion>,
    },
    Result {
        text: Option<String>,
        session_id: String,
    },
    Error {
        message: String,
    },
    Done,
}

#[derive(Default)]
pub struct StreamState {
    pub session_id: String,
    pub emitted_text: bool,
    pub emitted_done: bool,
    pub emitted_codex_agent_item_ids: Vec<String>,
    claude_message_id: Option<String>,
    emitted_text_chars_by_block: Vec<usize>,
    emitted_thinking_chars_by_block: Vec<usize>,
    emitted_tool_ids: Vec<String>,
    ask_tool_ids: Vec<String>,
}

pub fn dispatch<F: FnMut(StreamEvent)>(json: &Value, state: &mut StreamState, emit: &mut F) {
    match json["type"].as_str().unwrap_or("") {
        "system" if json["subtype"].as_str() == Some("init") => {
            let sid = json["session_id"].as_str().unwrap_or("").to_string();
            state.session_id = sid.clone();
            emit(StreamEvent::Init { session_id: sid });
        }
        "assistant" => dispatch_assistant(json, state, emit),
        "tool_result" => {
            let id = json["tool_use_id"].as_str().unwrap_or("").to_string();
            if !id.is_empty() && !state.ask_tool_ids.contains(&id) {
                let (output_preview, output_full, status) = extract_tool_output(json);
                emit(StreamEvent::ToolDone {
                    tool_id: id,
                    output: text_output(output_preview, output_full),
                    status,
                    meta: Value::Object(Default::default()),
                });
            }
        }
        "result" => {
            let text = extract_result_text(json, state);
            emit(StreamEvent::Result {
                text,
                session_id: state.session_id.clone(),
            });
            state.emitted_done = true;
            emit(StreamEvent::Done);
        }
        "error" => {
            let msg = json["error"]["message"]
                .as_str()
                .or_else(|| json["message"].as_str())
                .unwrap_or("unknown error")
                .to_string();
            emit(StreamEvent::Error { message: msg });
            state.emitted_done = true;
            emit(StreamEvent::Done);
        }
        _ => {}
    }
}

// CC's stream-json format emits `{"type":"assistant","message":{...}}` snapshots.
// Text/thinking blocks in the same message can be repeated as they grow, so we
// track cursors to emit only the new portion. Claude starts a new message after
// tool execution, and those message-local block indexes start over at zero.
fn dispatch_assistant<F: FnMut(StreamEvent)>(json: &Value, state: &mut StreamState, emit: &mut F) {
    if let Some(message_id) = json["message"]["id"].as_str() {
        if state.claude_message_id.as_deref() != Some(message_id) {
            state.claude_message_id = Some(message_id.to_string());
            state.emitted_text_chars_by_block.clear();
            state.emitted_thinking_chars_by_block.clear();
        }
    }

    let Some(arr) = json["message"]["content"].as_array() else {
        return;
    };
    for (index, block) in arr.iter().enumerate() {
        match block["type"].as_str() {
            Some("text") => {
                let full = block["text"].as_str().unwrap_or("");
                let n = full.chars().count();
                let previous = advance_cursor(&mut state.emitted_text_chars_by_block, index, n);
                if n > previous {
                    let delta: String = full.chars().skip(previous).collect();
                    state.emitted_text = true;
                    emit(StreamEvent::TextDelta { text: delta });
                }
            }
            Some("thinking") => {
                let full = block["thinking"].as_str().unwrap_or("");
                let n = full.chars().count();
                let previous = advance_cursor(&mut state.emitted_thinking_chars_by_block, index, n);
                if n > previous {
                    let delta: String = full.chars().skip(previous).collect();
                    emit(StreamEvent::ThinkingDelta { text: delta });
                }
            }
            Some("tool_use") => {
                let id = block["id"].as_str().unwrap_or("").to_string();
                if !id.is_empty() && !state.emitted_tool_ids.contains(&id) {
                    let name = block["name"].as_str().unwrap_or("").to_string();
                    state.emitted_tool_ids.push(id.clone());

                    if is_ask_user_question(&name) {
                        if let Some(questions) = parse_ask_questions(&block["input"]) {
                            state.ask_tool_ids.push(id.clone());
                            emit(StreamEvent::AskUserQuestion {
                                tool_id: id,
                                questions,
                            });
                            continue;
                        }
                    }

                    let input_preview = extract_input_preview(&block["input"]);
                    let input_full = extract_input_full(&block["input"]);
                    let tool = build_tool_call(
                        ToolProvider::Claude,
                        id,
                        name,
                        &block["input"],
                        input_preview,
                        input_full,
                    );
                    emit(StreamEvent::ToolStart { tool });
                }
            }
            _ => {}
        }
    }
}

fn advance_cursor(cursors: &mut Vec<usize>, index: usize, next: usize) -> usize {
    if cursors.len() <= index {
        cursors.resize(index + 1, 0);
    }
    let previous = cursors[index];
    if next > previous {
        cursors[index] = next;
    }
    previous
}

fn extract_result_text(json: &Value, state: &StreamState) -> Option<String> {
    // Avoid duplicating text already streamed via TextDelta
    if state.emitted_text {
        return None;
    }
    json["result"].as_str().map(|s| s.to_string())
}

fn extract_input_full(input: &Value) -> Option<String> {
    if input.is_null() {
        return None;
    }
    let s = serde_json::to_string_pretty(input).unwrap_or_default();
    if s.is_empty() || s == "null" || s == "{}" {
        return None;
    }
    Some(trunc_chars(&s, 10_000))
}

fn extract_input_preview(input: &Value) -> Option<String> {
    if input.is_null() {
        return None;
    }
    if let Some(obj) = input.as_object() {
        if obj.is_empty() {
            return None;
        }
        // Extract the most descriptive single-line value from known keys
        for key in &[
            "path",
            "file_path",
            "url",
            "query",
            "from",
            "glob",
            "pattern",
            "command",
        ] {
            if let Some(s) = obj.get(*key).and_then(|v| v.as_str()) {
                if !s.is_empty() {
                    return Some(trunc_chars(s, 80));
                }
            }
        }
        // Fall back to first non-empty string value
        for v in obj.values() {
            if let Some(s) = v.as_str() {
                if !s.is_empty() {
                    return Some(trunc_chars(s, 80));
                }
            }
        }
    }
    None
}

fn extract_tool_output(json: &Value) -> (Option<String>, Option<String>, ToolStatus) {
    let content = if let Some(arr) = json["content"].as_array() {
        arr.iter()
            .filter_map(|item| item["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        json["content"].as_str().unwrap_or("").to_string()
    };

    let status = if json["is_error"].as_bool().unwrap_or(false) {
        ToolStatus::Failed
    } else {
        ToolStatus::Completed
    };

    if content.is_empty() {
        return (None, None, status);
    }

    let output_full = trunc_chars(&content, 10_000);
    let first_line = content.lines().next().unwrap_or(&content).trim();
    let output_preview = trunc_chars(first_line, 80);

    (Some(output_preview), Some(output_full), status)
}

fn is_ask_user_question(name: &str) -> bool {
    let bare = name.rsplit("__").next().unwrap_or(name);
    bare == "AskUserQuestion"
}

fn parse_ask_questions(input: &Value) -> Option<Vec<AskQuestion>> {
    let arr = input.get("questions")?.as_array()?;
    let mut out = Vec::with_capacity(arr.len());
    for q in arr {
        let header = q.get("header")?.as_str()?.to_string();
        let question = q.get("question")?.as_str()?.to_string();
        let multi_select = q
            .get("multiSelect")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let options = q
            .get("options")?
            .as_array()?
            .iter()
            .filter_map(|o| {
                let label = o.get("label")?.as_str()?.to_string();
                let description = o
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                Some(AskQuestionOption { label, description })
            })
            .collect::<Vec<_>>();
        if options.is_empty() {
            continue;
        }
        out.push(AskQuestion {
            header,
            question,
            multi_select,
            options,
        });
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn trunc_chars(s: &str, max: usize) -> String {
    let count = s.chars().count();
    if count <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect::<String>() + "…"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool_cards::{ToolFormat, ToolKind, ToolProvider};
    use serde_json::json;

    fn collect_assistant_events(content: Value, state: &mut StreamState) -> Vec<StreamEvent> {
        let mut events = Vec::new();
        let json = json!({
            "type": "assistant",
            "message": { "content": content },
        });
        dispatch(&json, state, &mut |event| events.push(event));
        events
    }

    fn collect_assistant_message_events(
        message_id: &str,
        content: Value,
        state: &mut StreamState,
    ) -> Vec<StreamEvent> {
        let mut events = Vec::new();
        let json = json!({
            "type": "assistant",
            "message": { "id": message_id, "content": content },
        });
        dispatch(&json, state, &mut |event| events.push(event));
        events
    }

    #[test]
    fn emits_text_tool_text_in_content_order() {
        let mut state = StreamState::default();

        let first = collect_assistant_events(
            json!([
                { "type": "text", "text": "A" },
            ]),
            &mut state,
        );
        assert_eq!(first, vec![StreamEvent::TextDelta { text: "A".into() }]);

        let second = collect_assistant_events(
            json!([
                { "type": "text", "text": "A" },
                { "type": "tool_use", "id": "tool-1", "name": "Read", "input": { "path": "src/App.tsx" } },
            ]),
            &mut state,
        );
        assert_eq!(
            second,
            vec![StreamEvent::ToolStart {
                tool: crate::tool_cards::ToolCall {
                    tool_id: "tool-1".into(),
                    provider: ToolProvider::Claude,
                    kind: ToolKind::FileRead,
                    raw_name: "Read".into(),
                    title: "Read file".into(),
                    input: crate::tool_cards::ToolIo {
                        preview: Some("src/App.tsx".into()),
                        full: Some("{\n  \"path\": \"src/App.tsx\"\n}".into()),
                        format: ToolFormat::Json,
                    },
                    output: None,
                    status: ToolStatus::Running,
                    meta: json!({
                        "path": "src/App.tsx",
                        "paths": ["src/App.tsx"],
                    }),
                },
            }]
        );

        let third = collect_assistant_events(
            json!([
                { "type": "text", "text": "A" },
                { "type": "tool_use", "id": "tool-1", "name": "Read", "input": { "path": "src/App.tsx" } },
                { "type": "text", "text": "B" },
            ]),
            &mut state,
        );
        assert_eq!(third, vec![StreamEvent::TextDelta { text: "B".into() }]);
    }

    #[test]
    fn resets_text_cursors_for_new_claude_message_after_tool_use() {
        let mut state = StreamState::default();

        let first = collect_assistant_message_events(
            "msg_1",
            json!([
                { "type": "text", "text": "hello, message 1" },
            ]),
            &mut state,
        );
        assert_eq!(
            first,
            vec![StreamEvent::TextDelta {
                text: "hello, message 1".into()
            }]
        );

        let tool = collect_assistant_message_events(
            "msg_1",
            json!([
                { "type": "tool_use", "id": "tool-1", "name": "Bash", "input": { "command": "printf 'tool, message 2'" } },
            ]),
            &mut state,
        );
        assert!(matches!(tool.as_slice(), [StreamEvent::ToolStart { .. }]));

        let final_text = collect_assistant_message_events(
            "msg_2",
            json!([
                { "type": "text", "text": "done, message 3" },
            ]),
            &mut state,
        );
        assert_eq!(
            final_text,
            vec![StreamEvent::TextDelta {
                text: "done, message 3".into()
            }]
        );
    }

    #[test]
    fn emits_ask_user_question_for_builtin_tool() {
        let mut state = StreamState::default();
        let events = collect_assistant_events(
            json!([
                {
                    "type": "tool_use",
                    "id": "tool-ask-1",
                    "name": "AskUserQuestion",
                    "input": {
                        "questions": [
                            {
                                "header": "Lib",
                                "question": "Which date library?",
                                "multiSelect": false,
                                "options": [
                                    { "label": "date-fns", "description": "Tree-shakable" },
                                    { "label": "Day.js" }
                                ]
                            }
                        ]
                    }
                }
            ]),
            &mut state,
        );

        assert_eq!(
            events,
            vec![StreamEvent::AskUserQuestion {
                tool_id: "tool-ask-1".into(),
                questions: vec![AskQuestion {
                    header: "Lib".into(),
                    question: "Which date library?".into(),
                    multi_select: false,
                    options: vec![
                        AskQuestionOption {
                            label: "date-fns".into(),
                            description: Some("Tree-shakable".into())
                        },
                        AskQuestionOption {
                            label: "Day.js".into(),
                            description: None
                        },
                    ],
                }],
            }]
        );
        assert!(state.ask_tool_ids.contains(&"tool-ask-1".to_string()));
    }

    #[test]
    fn suppresses_tool_result_for_ask_user_question() {
        let mut state = StreamState::default();
        state.ask_tool_ids.push("tool-ask-1".into());

        let mut events = Vec::new();
        let json = json!({
            "type": "tool_result",
            "tool_use_id": "tool-ask-1",
            "content": "anything",
        });
        dispatch(&json, &mut state, &mut |event| events.push(event));
        assert!(events.is_empty(), "ask tool_result should be suppressed");
    }

    #[test]
    fn emits_thinking_text_tool_thinking_in_content_order() {
        let mut state = StreamState::default();

        let first = collect_assistant_events(
            json!([
                { "type": "thinking", "thinking": "plan" },
                { "type": "text", "text": "A" },
                { "type": "tool_use", "id": "tool-1", "name": "Search", "input": { "query": "rust" } },
                { "type": "thinking", "thinking": "done" },
            ]),
            &mut state,
        );

        assert_eq!(
            first,
            vec![
                StreamEvent::ThinkingDelta {
                    text: "plan".into()
                },
                StreamEvent::TextDelta { text: "A".into() },
                StreamEvent::ToolStart {
                    tool: crate::tool_cards::ToolCall {
                        tool_id: "tool-1".into(),
                        provider: ToolProvider::Claude,
                        kind: ToolKind::Other,
                        raw_name: "Search".into(),
                        title: "Tool".into(),
                        input: crate::tool_cards::ToolIo {
                            preview: Some("rust".into()),
                            full: Some("{\n  \"query\": \"rust\"\n}".into()),
                            format: ToolFormat::Json,
                        },
                        output: None,
                        status: ToolStatus::Running,
                        meta: json!({}),
                    },
                },
                StreamEvent::ThinkingDelta {
                    text: "done".into()
                },
            ]
        );
    }

    #[test]
    fn maps_claude_builtin_tools_to_canonical_kinds() {
        let cases = [
            (
                "Bash",
                json!({ "command": "npm test" }),
                ToolKind::Command,
                "Command",
                Some("npm test"),
            ),
            (
                "Read",
                json!({ "path": "src/App.tsx" }),
                ToolKind::FileRead,
                "Read file",
                Some("src/App.tsx"),
            ),
            (
                "Edit",
                json!({ "file_path": "src/App.tsx", "old_string": "a", "new_string": "b" }),
                ToolKind::FileEdit,
                "Edit file",
                Some("src/App.tsx"),
            ),
            (
                "mcp__ire__experiment__start",
                json!({ "name": "smoke" }),
                ToolKind::ExperimentStart,
                "Start experiment",
                Some("smoke"),
            ),
        ];

        for (name, input, kind, title, preview) in cases {
            let mut state = StreamState::default();
            let events = collect_assistant_events(
                json!([
                    { "type": "tool_use", "id": format!("tool-{name}"), "name": name, "input": input },
                ]),
                &mut state,
            );

            let StreamEvent::ToolStart { tool } = &events[0] else {
                panic!("expected ToolStart");
            };
            assert_eq!(tool.provider, ToolProvider::Claude);
            assert_eq!(tool.kind, kind);
            assert_eq!(tool.raw_name, name);
            assert_eq!(tool.title, title);
            assert_eq!(tool.input.preview.as_deref(), preview);
            assert_eq!(tool.input.format, ToolFormat::Json);
            assert_eq!(tool.status, ToolStatus::Running);
        }
    }

    #[test]
    fn maps_unknown_claude_tools_to_other_with_raw_details() {
        let mut state = StreamState::default();
        let events = collect_assistant_events(
            json!([
                { "type": "tool_use", "id": "tool-unknown", "name": "Mystery", "input": { "value": "raw" } },
            ]),
            &mut state,
        );

        let StreamEvent::ToolStart { tool } = &events[0] else {
            panic!("expected ToolStart");
        };
        assert_eq!(tool.kind, ToolKind::Other);
        assert_eq!(tool.raw_name, "Mystery");
        assert_eq!(tool.input.preview.as_deref(), Some("raw"));
        assert_eq!(
            tool.input.full.as_deref(),
            Some("{\n  \"value\": \"raw\"\n}")
        );
    }

    #[test]
    fn emits_tool_done_with_canonical_output_and_status() {
        let mut state = StreamState::default();
        let mut events = Vec::new();
        let json = json!({
            "type": "tool_result",
            "tool_use_id": "tool-1",
            "content": "first line\nsecond line",
        });

        dispatch(&json, &mut state, &mut |event| events.push(event));

        assert_eq!(
            events,
            vec![StreamEvent::ToolDone {
                tool_id: "tool-1".into(),
                output: Some(crate::tool_cards::ToolIo {
                    preview: Some("first line".into()),
                    full: Some("first line\nsecond line".into()),
                    format: ToolFormat::Text,
                }),
                status: ToolStatus::Completed,
                meta: json!({}),
            }]
        );
    }

    #[test]
    fn tracks_terminal_done_for_result() {
        let mut state = StreamState::default();
        let mut events = Vec::new();
        let json = json!({
            "type": "result",
            "result": "done",
        });

        dispatch(&json, &mut state, &mut |event| events.push(event));

        assert!(state.emitted_done);
        assert!(matches!(events.last(), Some(StreamEvent::Done)));
    }
}
