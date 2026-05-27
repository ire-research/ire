use serde_json::Value;

use crate::cc::stream::{StreamEvent, StreamState};
use crate::tool_cards::{build_tool_call, text_output, ToolProvider, ToolStatus};

pub fn dispatch<F: FnMut(StreamEvent)>(json: &Value, state: &mut StreamState, emit: &mut F) {
    match json["type"].as_str().unwrap_or("") {
        "thread.started" => {
            let sid = json["thread_id"].as_str().unwrap_or("").to_string();
            state.session_id = sid.clone();
            emit(StreamEvent::Init { session_id: sid });
        }
        "item.agentMessage.delta" => {
            if let Some(delta) = json["delta"].as_str() {
                if !delta.is_empty() {
                    if let Some(id) = codex_item_id(json) {
                        mark_codex_agent_item_emitted(state, id);
                    }
                    state.emitted_text = true;
                    emit(StreamEvent::TextDelta {
                        text: delta.to_string(),
                    });
                }
            }
        }
        "item.reasoning.textDelta" => {
            if let Some(delta) = json["delta"].as_str() {
                if !delta.is_empty() {
                    emit(StreamEvent::ThinkingDelta {
                        text: delta.to_string(),
                    });
                }
            }
        }
        "item.started" => {
            let item = &json["item"];
            if let Some(t) = item["type"].as_str().filter(|t| is_tool_item_type(t)) {
                let id = item["id"].as_str().unwrap_or("").to_string();
                if id.is_empty() {
                    return;
                }
                let raw_name = codex_tool_name(item, t);
                let input_preview = extract_input_preview(item);
                let input_full = extract_input_full(item);
                emit(StreamEvent::ToolStart {
                    tool: build_tool_call(
                        ToolProvider::Codex,
                        id,
                        raw_name,
                        item,
                        input_preview,
                        input_full,
                    ),
                });
            }
        }
        "item.completed" => {
            let item = &json["item"];
            match item["type"].as_str() {
                Some("agent_message" | "agentMessage") => {
                    let id = item["id"].as_str().unwrap_or("");
                    if !id.is_empty() && state.emitted_codex_agent_item_ids.iter().any(|v| v == id)
                    {
                        return;
                    }
                    if let Some(text) = item["text"].as_str() {
                        if !text.is_empty() {
                            if !id.is_empty() {
                                mark_codex_agent_item_emitted(state, id);
                            }
                            state.emitted_text = true;
                            emit(StreamEvent::TextDelta {
                                text: text.to_string(),
                            });
                        }
                    }
                }
                Some(t) if is_tool_item_type(t) => {
                    let id = item["id"].as_str().unwrap_or("").to_string();
                    if id.is_empty() {
                        return;
                    }
                    let output = if t == "mcp_tool_call" {
                        extract_mcp_result_text(item)
                    } else {
                        item["output"]
                            .as_str()
                            .or_else(|| item["aggregated_output"].as_str())
                            .map(|s| s.to_string())
                    };
                    let output_preview = output
                        .as_deref()
                        .and_then(|s| s.lines().next())
                        .map(|s| trunc_chars(s.trim(), 80));
                    let output_full = output.map(|s| trunc_chars(&s, 10_000));
                    let status = if (t == "mcp_tool_call" && item["error"].is_string())
                        || item["exit_code"].as_i64().is_some_and(|code| code != 0)
                    {
                        ToolStatus::Failed
                    } else {
                        ToolStatus::Completed
                    };
                    emit(StreamEvent::ToolDone {
                        tool_id: id,
                        output: text_output(output_preview, output_full),
                        status,
                        meta: Value::Object(Default::default()),
                    });
                }
                _ => {}
            }
        }
        "turn.completed" => {
            emit(StreamEvent::Result {
                text: None,
                session_id: state.session_id.clone(),
            });
            state.emitted_done = true;
            emit(StreamEvent::Done);
        }
        "error" => {
            let msg = json["message"]
                .as_str()
                .unwrap_or("unknown error")
                .to_string();
            emit(StreamEvent::Error { message: msg });
            state.emitted_done = true;
            emit(StreamEvent::Done);
        }
        _ => {}
    }
}

fn is_tool_item_type(t: &str) -> bool {
    matches!(
        t,
        "command_execution"
            | "commandExecution"
            | "file_change"
            | "fileChange"
            | "dynamic_tool_call"
            | "dynamicToolCall"
            | "mcp_tool_call"
    )
}

fn codex_item_id(json: &Value) -> Option<&str> {
    json["item_id"]
        .as_str()
        .or_else(|| json["itemId"].as_str())
        .or_else(|| json["id"].as_str())
        .or_else(|| json["item"]["id"].as_str())
}

fn mark_codex_agent_item_emitted(state: &mut StreamState, id: &str) {
    if !state
        .emitted_codex_agent_item_ids
        .iter()
        .any(|existing| existing == id)
    {
        state.emitted_codex_agent_item_ids.push(id.to_string());
    }
}

fn codex_tool_name(item: &Value, item_type: &str) -> String {
    if matches!(item_type, "dynamic_tool_call" | "dynamicToolCall") {
        for path in [
            &["name"][..],
            &["tool_name"][..],
            &["toolName"][..],
            &["function", "name"][..],
            &["call", "name"][..],
        ] {
            if let Some(s) = get_path(item, path).and_then(Value::as_str) {
                if !s.is_empty() {
                    return s.to_string();
                }
            }
        }
    }
    if item_type == "mcp_tool_call" {
        let server = item["server"].as_str().unwrap_or("");
        let tool = item["tool"].as_str().unwrap_or("");
        if !server.is_empty() && !tool.is_empty() {
            return format!("{}__{}", server, tool);
        }
        if !tool.is_empty() {
            return tool.to_string();
        }
    }
    item_type.to_string()
}

fn get_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn extract_input_preview(item: &Value) -> Option<String> {
    for key in &["command", "path", "file_path", "filename"] {
        if let Some(s) = item.get(*key).and_then(|v| v.as_str()) {
            if !s.is_empty() {
                return Some(trunc_chars(s, 80));
            }
        }
    }

    // mcp_tool_call uses "arguments" instead of "input"
    for args_key in &["arguments", "input"] {
        if let Some(args) = item.get(*args_key) {
            for key in &[
                "path",
                "file_path",
                "filename",
                "url",
                "query",
                "from",
                "glob",
                "pattern",
                "command",
            ] {
                if let Some(s) = args.get(*key).and_then(|v| v.as_str()) {
                    if !s.is_empty() {
                        return Some(trunc_chars(s, 80));
                    }
                }
            }
            if let Some(preview) = first_string(args).map(|s| trunc_chars(s, 80)) {
                return Some(preview);
            }
        }
    }

    for key in &["name", "tool_name"] {
        if let Some(s) = item.get(*key).and_then(|v| v.as_str()) {
            if !s.is_empty() {
                return Some(trunc_chars(s, 80));
            }
        }
    }

    None
}

fn extract_input_full(item: &Value) -> Option<String> {
    let obj = item.as_object()?;
    let mut out = serde_json::Map::new();

    for (key, value) in obj {
        if matches!(
            key.as_str(),
            "id" | "type"
                | "status"
                | "exit_code"
                | "output"
                | "aggregated_output"
                | "result"
                | "error"
        ) || is_empty_value(value)
        {
            continue;
        }
        out.insert(key.clone(), value.clone());
    }

    if out.is_empty() {
        return None;
    }

    serde_json::to_string_pretty(&Value::Object(out))
        .ok()
        .map(|s| trunc_chars(&s, 10_000))
}

/// Extracts the text payload from an `mcp_tool_call` item's `result.content` array.
fn extract_mcp_result_text(item: &Value) -> Option<String> {
    let content = item["result"]["content"].as_array()?;
    let parts: Vec<&str> = content.iter().filter_map(|c| c["text"].as_str()).collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

fn first_string(value: &Value) -> Option<&str> {
    if let Some(s) = value.as_str() {
        if !s.is_empty() {
            return Some(s);
        }
    }

    if let Some(obj) = value.as_object() {
        for value in obj.values() {
            if let Some(s) = first_string(value) {
                return Some(s);
            }
        }
    }

    if let Some(arr) = value.as_array() {
        for value in arr {
            if let Some(s) = first_string(value) {
                return Some(s);
            }
        }
    }

    None
}

fn is_empty_value(value: &Value) -> bool {
    value.is_null()
        || value.as_str() == Some("")
        || value.as_array().is_some_and(|arr| arr.is_empty())
        || value.as_object().is_some_and(|obj| obj.is_empty())
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
    use crate::tool_cards::{ToolFormat, ToolIo, ToolKind, ToolProvider};
    use serde_json::json;

    fn collect(json: Value, state: &mut StreamState) -> Vec<StreamEvent> {
        let mut events = Vec::new();
        dispatch(&json, state, &mut |event| events.push(event));
        events
    }

    #[test]
    fn emits_text_from_completed_agent_message() {
        let mut state = StreamState::default();
        let events = collect(
            json!({
                "type": "item.completed",
                "item": {
                    "id": "item_0",
                    "type": "agent_message",
                    "text": "OK"
                }
            }),
            &mut state,
        );

        assert_eq!(events, vec![StreamEvent::TextDelta { text: "OK".into() }]);
    }

    #[test]
    fn emits_multiple_completed_agent_messages_in_one_turn() {
        let mut state = StreamState::default();

        let first = collect(
            json!({
                "type": "item.completed",
                "item": {
                    "id": "item_0",
                    "type": "agent_message",
                    "text": "hello, message 1"
                }
            }),
            &mut state,
        );
        assert_eq!(
            first,
            vec![StreamEvent::TextDelta {
                text: "hello, message 1".into()
            }]
        );

        let second = collect(
            json!({
                "type": "item.completed",
                "item": {
                    "id": "item_2",
                    "type": "agent_message",
                    "text": "done, message 3"
                }
            }),
            &mut state,
        );
        assert_eq!(
            second,
            vec![StreamEvent::TextDelta {
                text: "done, message 3".into()
            }]
        );
    }

    #[test]
    fn suppresses_completed_agent_message_after_delta_for_same_item() {
        let mut state = StreamState::default();

        let delta = collect(
            json!({
                "type": "item.agentMessage.delta",
                "item_id": "item_0",
                "delta": "streamed"
            }),
            &mut state,
        );
        assert_eq!(
            delta,
            vec![StreamEvent::TextDelta {
                text: "streamed".into()
            }]
        );

        let completed = collect(
            json!({
                "type": "item.completed",
                "item": {
                    "id": "item_0",
                    "type": "agent_message",
                    "text": "streamed"
                }
            }),
            &mut state,
        );
        assert!(completed.is_empty());
    }

    #[test]
    fn emits_command_execution_tool_events() {
        let mut state = StreamState::default();

        let started = collect(
            json!({
                "type": "item.started",
                "item": {
                    "id": "item_0",
                    "type": "command_execution",
                    "command": "/bin/zsh -lc 'printf tool-ok'",
                    "aggregated_output": "",
                    "exit_code": null,
                    "status": "in_progress"
                }
            }),
            &mut state,
        );
        assert_eq!(
            started,
            vec![StreamEvent::ToolStart {
                tool: crate::tool_cards::ToolCall {
                    tool_id: "item_0".into(),
                    provider: ToolProvider::Codex,
                    kind: ToolKind::Command,
                    raw_name: "command_execution".into(),
                    title: "Command".into(),
                    input: ToolIo {
                        preview: Some("/bin/zsh -lc 'printf tool-ok'".into()),
                        full: Some("{\n  \"command\": \"/bin/zsh -lc 'printf tool-ok'\"\n}".into()),
                        format: ToolFormat::Json,
                    },
                    output: None,
                    status: ToolStatus::Running,
                    meta: json!({
                        "command": "/bin/zsh -lc 'printf tool-ok'",
                    }),
                },
            }]
        );

        let completed = collect(
            json!({
                "type": "item.completed",
                "item": {
                    "id": "item_0",
                    "type": "command_execution",
                    "command": "/bin/zsh -lc 'printf tool-ok'",
                    "aggregated_output": "tool-ok",
                    "exit_code": 0,
                    "status": "completed"
                }
            }),
            &mut state,
        );
        assert_eq!(
            completed,
            vec![StreamEvent::ToolDone {
                tool_id: "item_0".into(),
                output: Some(ToolIo {
                    preview: Some("tool-ok".into()),
                    full: Some("tool-ok".into()),
                    format: ToolFormat::Text,
                }),
                status: ToolStatus::Completed,
                meta: json!({}),
            }]
        );
    }

    #[test]
    fn emits_file_change_input_details_when_codex_provides_them() {
        let mut state = StreamState::default();
        let events = collect(
            json!({
                "type": "item.started",
                "item": {
                    "id": "item_1",
                    "type": "file_change",
                    "path": "src/App.tsx",
                    "changes": [
                        { "kind": "add", "content": "export const ok = true;" }
                    ],
                    "status": "in_progress"
                }
            }),
            &mut state,
        );

        assert_eq!(
            events,
            vec![StreamEvent::ToolStart {
                tool: crate::tool_cards::ToolCall {
                    tool_id: "item_1".into(),
                    provider: ToolProvider::Codex,
                    kind: ToolKind::FileEdit,
                    raw_name: "file_change".into(),
                    title: "Edit file".into(),
                    input: ToolIo {
                        preview: Some("src/App.tsx".into()),
                        full: Some(
                            "{\n  \"changes\": [\n    {\n      \"content\": \"export const ok = true;\",\n      \"kind\": \"add\"\n    }\n  ],\n  \"path\": \"src/App.tsx\"\n}".into()
                        ),
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
    }

    #[test]
    fn maps_codex_dynamic_tools_to_canonical_kinds() {
        let mut state = StreamState::default();
        let events = collect(
            json!({
                "type": "item.started",
                "item": {
                    "id": "item_2",
                    "type": "dynamic_tool_call",
                    "name": "mcp__ire__wiki__write",
                    "input": { "path": "docs/Page.md", "content": "ok" },
                    "status": "in_progress"
                }
            }),
            &mut state,
        );

        let StreamEvent::ToolStart { tool } = &events[0] else {
            panic!("expected ToolStart");
        };
        assert_eq!(tool.provider, ToolProvider::Codex);
        assert_eq!(tool.kind, ToolKind::WikiWrite);
        assert_eq!(tool.raw_name, "mcp__ire__wiki__write");
        assert_eq!(tool.title, "Write wiki");
        assert_eq!(tool.input.preview.as_deref(), Some("docs/Page.md"));
        assert_eq!(tool.input.format, ToolFormat::Json);
    }

    #[test]
    fn maps_unknown_codex_tools_to_other_with_raw_details() {
        let mut state = StreamState::default();
        let events = collect(
            json!({
                "type": "item.started",
                "item": {
                    "id": "item_3",
                    "type": "dynamic_tool_call",
                    "name": "custom.lookup",
                    "input": { "query": "needle" },
                    "status": "in_progress"
                }
            }),
            &mut state,
        );

        let StreamEvent::ToolStart { tool } = &events[0] else {
            panic!("expected ToolStart");
        };
        assert_eq!(tool.kind, ToolKind::Other);
        assert_eq!(tool.raw_name, "custom.lookup");
        assert_eq!(tool.input.preview.as_deref(), Some("needle"));
        assert_eq!(tool.input.full.as_deref(), Some("{\n  \"input\": {\n    \"query\": \"needle\"\n  },\n  \"name\": \"custom.lookup\"\n}"));
    }

    #[test]
    fn emits_mcp_tool_call_start_and_done() {
        let mut state = StreamState::default();

        let started = collect(
            json!({
                "type": "item.started",
                "item": {
                    "id": "item_0",
                    "type": "mcp_tool_call",
                    "server": "ire",
                    "tool": "wiki.list",
                    "arguments": {},
                    "result": null,
                    "error": null,
                    "status": "in_progress"
                }
            }),
            &mut state,
        );

        let StreamEvent::ToolStart { tool } = &started[0] else {
            panic!("expected ToolStart");
        };
        assert_eq!(tool.provider, ToolProvider::Codex);
        assert_eq!(tool.raw_name, "ire__wiki.list");
        assert_eq!(tool.kind, ToolKind::WikiRead);
        assert_eq!(tool.title, "Read wiki");
        assert_eq!(tool.status, ToolStatus::Running);

        let completed = collect(
            json!({
                "type": "item.completed",
                "item": {
                    "id": "item_0",
                    "type": "mcp_tool_call",
                    "server": "ire",
                    "tool": "wiki.list",
                    "arguments": {},
                    "result": {
                        "content": [{ "type": "text", "text": "README.md\nNOTES.md" }],
                        "structured_content": null
                    },
                    "error": null,
                    "status": "completed"
                }
            }),
            &mut state,
        );

        assert_eq!(
            completed,
            vec![StreamEvent::ToolDone {
                tool_id: "item_0".into(),
                output: Some(ToolIo {
                    preview: Some("README.md".into()),
                    full: Some("README.md\nNOTES.md".into()),
                    format: ToolFormat::Text,
                }),
                status: ToolStatus::Completed,
                meta: json!({}),
            }]
        );
    }

    #[test]
    fn mcp_tool_call_error_field_sets_failed_status() {
        let mut state = StreamState::default();
        let events = collect(
            json!({
                "type": "item.completed",
                "item": {
                    "id": "item_1",
                    "type": "mcp_tool_call",
                    "server": "ire",
                    "tool": "wiki.read",
                    "arguments": { "path": "missing.md" },
                    "result": null,
                    "error": "file not found",
                    "status": "completed"
                }
            }),
            &mut state,
        );

        let StreamEvent::ToolDone { status, .. } = &events[0] else {
            panic!("expected ToolDone");
        };
        assert_eq!(*status, ToolStatus::Failed);
    }

    #[test]
    fn tracks_terminal_done_for_turn_completed() {
        let mut state = StreamState::default();
        let events = collect(
            json!({
                "type": "turn.completed"
            }),
            &mut state,
        );

        assert!(state.emitted_done);
        assert!(matches!(events.last(), Some(StreamEvent::Done)));
    }
}
