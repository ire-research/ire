use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind")]
pub enum StreamEvent {
    Init { session_id: String },
    TextDelta { text: String },
    ThinkingDelta { text: String },
    ToolStart { tool_id: String, tool_name: String, input_preview: Option<String>, input_full: Option<String> },
    ToolDone { tool_id: String, output_preview: Option<String>, output_full: Option<String> },
    Result { text: Option<String>, session_id: String },
    Error { message: String },
    Done,
}

#[derive(Default)]
pub struct StreamState {
    pub session_id: String,
    pub emitted_text: bool,
    emitted_text_chars_by_block: Vec<usize>,
    emitted_thinking_chars_by_block: Vec<usize>,
    emitted_tool_ids: Vec<String>,
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
            if !id.is_empty() {
                let (output_preview, output_full) = extract_tool_output(json);
                emit(StreamEvent::ToolDone { tool_id: id, output_preview, output_full });
            }
        }
        "result" => {
            let text = extract_result_text(json, state);
            emit(StreamEvent::Result {
                text,
                session_id: state.session_id.clone(),
            });
            emit(StreamEvent::Done);
        }
        "error" => {
            let msg = json["error"]["message"]
                .as_str()
                .or_else(|| json["message"].as_str())
                .unwrap_or("unknown error")
                .to_string();
            emit(StreamEvent::Error { message: msg });
            emit(StreamEvent::Done);
        }
        _ => {}
    }
}

// CC's stream-json format emits `{"type":"assistant","message":{...}}` snapshots.
// Each snapshot contains the full content array accumulated so far, so we track
// cursors to emit only the new portion as deltas.
fn dispatch_assistant<F: FnMut(StreamEvent)>(
    json: &Value,
    state: &mut StreamState,
    emit: &mut F,
) {
    let Some(arr) = json["message"]["content"].as_array() else { return };
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
                    let input_preview = extract_input_preview(&block["input"]);
                    let input_full = extract_input_full(&block["input"]);
                    emit(StreamEvent::ToolStart { tool_id: id, tool_name: name, input_preview, input_full });
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
    if input.is_null() { return None; }
    let s = serde_json::to_string_pretty(input).unwrap_or_default();
    if s.is_empty() || s == "null" || s == "{}" { return None; }
    Some(trunc_chars(&s, 10_000))
}

fn extract_input_preview(input: &Value) -> Option<String> {
    if input.is_null() { return None; }
    if let Some(obj) = input.as_object() {
        if obj.is_empty() { return None; }
        // Extract the most descriptive single-line value from known keys
        for key in &["path", "file_path", "url", "query", "from", "glob", "pattern", "command"] {
            if let Some(s) = obj.get(*key).and_then(|v| v.as_str()) {
                if !s.is_empty() { return Some(trunc_chars(s, 80)); }
            }
        }
        // Fall back to first non-empty string value
        for v in obj.values() {
            if let Some(s) = v.as_str() {
                if !s.is_empty() { return Some(trunc_chars(s, 80)); }
            }
        }
    }
    None
}

fn extract_tool_output(json: &Value) -> (Option<String>, Option<String>) {
    let content = if let Some(arr) = json["content"].as_array() {
        arr.iter()
            .filter_map(|item| item["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        json["content"].as_str().unwrap_or("").to_string()
    };

    if content.is_empty() { return (None, None); }

    let output_full = trunc_chars(&content, 10_000);
    let first_line = content.lines().next().unwrap_or(&content).trim();
    let output_preview = trunc_chars(first_line, 80);

    (Some(output_preview), Some(output_full))
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
                tool_id: "tool-1".into(),
                tool_name: "Read".into(),
                input_preview: Some("src/App.tsx".into()),
                input_full: Some("{\n  \"path\": \"src/App.tsx\"\n}".into()),
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
                    tool_id: "tool-1".into(),
                    tool_name: "Search".into(),
                    input_preview: Some("rust".into()),
                    input_full: Some("{\n  \"query\": \"rust\"\n}".into()),
                },
                StreamEvent::ThinkingDelta {
                    text: "done".into()
                },
            ]
        );
    }
}
