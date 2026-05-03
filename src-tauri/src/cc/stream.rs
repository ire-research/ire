use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum StreamEvent {
    Init { session_id: String },
    TextDelta { text: String },
    ThinkingDelta { text: String },
    ToolStart { tool_id: String, tool_name: String, input_preview: Option<String> },
    ToolInputDelta { tool_id: String, partial_json: String },
    ToolDone { tool_id: String, output_preview: Option<String> },
    Result { text: Option<String>, session_id: String },
    Error { message: String },
    Done,
}

#[derive(Default)]
pub struct StreamState {
    pub session_id: String,
    pub current_tool_id: Option<String>,
    pub emitted_text: bool,
}

pub fn dispatch<F: FnMut(StreamEvent)>(json: &Value, state: &mut StreamState, emit: &mut F) {
    match json["type"].as_str().unwrap_or("") {
        "system" if json["subtype"].as_str() == Some("init") => {
            let sid = json["session_id"].as_str().unwrap_or("").to_string();
            state.session_id = sid.clone();
            emit(StreamEvent::Init { session_id: sid });
        }
        "stream_event" => dispatch_stream_event(json, state, emit),
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

fn dispatch_stream_event<F: FnMut(StreamEvent)>(
    json: &Value,
    state: &mut StreamState,
    emit: &mut F,
) {
    let event = &json["stream_event"];
    match event["type"].as_str().unwrap_or("") {
        "content_block_start" => {
            let block = &event["content_block"];
            if block["type"].as_str() == Some("tool_use") {
                let id = block["id"].as_str().unwrap_or("").to_string();
                let name = block["name"].as_str().unwrap_or("").to_string();
                state.current_tool_id = Some(id.clone());
                emit(StreamEvent::ToolStart {
                    tool_id: id,
                    tool_name: name,
                    input_preview: None,
                });
            }
        }
        "content_block_delta" => {
            let delta = &event["delta"];
            match delta["type"].as_str() {
                Some("text_delta") => {
                    let text = delta["text"].as_str().unwrap_or("").to_string();
                    state.emitted_text = true;
                    emit(StreamEvent::TextDelta { text });
                }
                Some("thinking_delta") => {
                    let text = delta["thinking"].as_str().unwrap_or("").to_string();
                    emit(StreamEvent::ThinkingDelta { text });
                }
                Some("input_json_delta") => {
                    if let Some(id) = &state.current_tool_id {
                        let partial = delta["partial_json"].as_str().unwrap_or("").to_string();
                        emit(StreamEvent::ToolInputDelta {
                            tool_id: id.clone(),
                            partial_json: partial,
                        });
                    }
                }
                _ => {}
            }
        }
        "content_block_stop" => {
            if let Some(id) = state.current_tool_id.take() {
                emit(StreamEvent::ToolDone { tool_id: id, output_preview: None });
            }
        }
        _ => {}
    }
}

fn extract_result_text(json: &Value, state: &StreamState) -> Option<String> {
    // Avoid duplicating text already streamed via TextDelta
    if state.emitted_text {
        return None;
    }
    json["result"].as_str().map(|s| s.to_string())
}
