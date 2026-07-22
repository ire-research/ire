use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolProvider {
    Claude,
    Codex,
    Opencode,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    Command,
    FileRead,
    FileWrite,
    FileEdit,
    FileSearch,
    WebFetch,
    IreRead,
    IreEdit,
    ResourceAdd,
    MemoryWrite,
    ExperimentStart,
    ExperimentStatus,
    ExperimentTailLogs,
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToolIo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full: Option<String>,
    pub format: ToolFormat,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToolCall {
    pub tool_id: String,
    pub provider: ToolProvider,
    pub kind: ToolKind,
    pub raw_name: String,
    pub title: String,
    pub input: ToolIo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<ToolIo>,
    pub status: ToolStatus,
    pub meta: Value,
}

pub fn build_tool_call(
    provider: ToolProvider,
    tool_id: String,
    raw_name: String,
    input_value: &Value,
    input_preview: Option<String>,
    input_full: Option<String>,
) -> ToolCall {
    let normalized = normalize_tool_name(&raw_name);
    let kind = tool_kind(&normalized);
    let title = tool_title(&kind).to_string();
    let meta = extract_meta(&kind, input_value);
    ToolCall {
        tool_id,
        provider,
        kind,
        raw_name,
        title,
        input: ToolIo {
            preview: input_preview,
            full: input_full,
            format: ToolFormat::Json,
        },
        output: None,
        status: ToolStatus::Running,
        meta,
    }
}

pub fn text_output(preview: Option<String>, full: Option<String>) -> Option<ToolIo> {
    if preview.is_none() && full.is_none() {
        return None;
    }
    Some(ToolIo {
        preview,
        full,
        format: ToolFormat::Text,
    })
}

pub fn normalize_tool_name(name: &str) -> String {
    let parts = name.split("__").collect::<Vec<_>>();
    if parts.len() > 2 && parts[0] == "mcp" {
        return parts[2..].join(".").replace('_', ".");
    }
    if parts.len() > 2 {
        return parts[1..].join(".").replace('_', ".");
    }
    if parts.len() == 2 {
        return parts[1].replace('_', ".");
    }
    name.to_string()
}

pub fn tool_kind(normalized_name: &str) -> ToolKind {
    match normalized_name.to_ascii_lowercase().as_str() {
        "bash" | "command_execution" | "commandexecution" => ToolKind::Command,
        "read" => ToolKind::FileRead,
        "write" => ToolKind::FileWrite,
        "edit" | "multiedit" | "file_change" | "filechange" => ToolKind::FileEdit,
        "grep" | "glob" | "ls" => ToolKind::FileSearch,
        "webfetch" => ToolKind::WebFetch,
        "ire.read" => ToolKind::IreRead,
        "ire.edit" => ToolKind::IreEdit,
        "resource.add" => ToolKind::ResourceAdd,
        "memory.write.long.term"
        | "memory.write_long_term"
        | "memory.write.short.term"
        | "memory.write_short_term" => ToolKind::MemoryWrite,
        "experiment.start" => ToolKind::ExperimentStart,
        "experiment.status" => ToolKind::ExperimentStatus,
        "experiment.tail.logs" | "experiment.tail_logs" => ToolKind::ExperimentTailLogs,
        _ => ToolKind::Other,
    }
}

pub fn tool_title(kind: &ToolKind) -> &'static str {
    match kind {
        ToolKind::Command => "Command",
        ToolKind::FileRead => "Read file",
        ToolKind::FileWrite => "Write file",
        ToolKind::FileEdit => "Edit file",
        ToolKind::FileSearch => "Search files",
        ToolKind::WebFetch => "Fetch web",
        ToolKind::IreRead => "Read ire.json",
        ToolKind::IreEdit => "Edit ire.json",
        ToolKind::ResourceAdd => "Add resource",
        ToolKind::MemoryWrite => "Write memory",
        ToolKind::ExperimentStart => "Start experiment",
        ToolKind::ExperimentStatus => "Experiment status",
        ToolKind::ExperimentTailLogs => "Experiment logs",
        ToolKind::Other => "Tool",
    }
}

pub fn extract_meta(kind: &ToolKind, input: &Value) -> Value {
    let mut meta = Map::new();
    // mcp_tool_call uses "arguments"; dynamic_tool_call uses "input"
    let detail = input
        .get("arguments")
        .or_else(|| input.get("input"))
        .filter(|v| v.is_object())
        .unwrap_or(input);
    if let Some(command) = input
        .get("command")
        .and_then(Value::as_str)
        .or_else(|| detail.get("command").and_then(Value::as_str))
    {
        meta.insert("command".into(), Value::String(command.to_string()));
    }
    if let Some(path) = first_path(input).or_else(|| first_path(detail)) {
        meta.insert("path".into(), Value::String(path.to_string()));
    }
    let mut paths = collect_paths(input);
    for path in collect_paths(detail) {
        if !paths.iter().any(|p| p == &path) {
            paths.push(path);
        }
    }
    if !paths.is_empty() {
        meta.insert(
            "paths".into(),
            Value::Array(paths.into_iter().map(Value::String).collect()),
        );
    }
    if matches!(kind, ToolKind::ExperimentStart) {
        if let Some(name) = input
            .get("name")
            .and_then(Value::as_str)
            .or_else(|| detail.get("name").and_then(Value::as_str))
        {
            meta.insert("name".into(), Value::String(name.to_string()));
        }
    }
    Value::Object(meta)
}

fn first_path(value: &Value) -> Option<&str> {
    for key in ["path", "file_path", "filename", "from", "to"] {
        if let Some(s) = value.get(key).and_then(Value::as_str) {
            if !s.is_empty() {
                return Some(s);
            }
        }
    }
    if let Some(changes) = value.get("changes").and_then(Value::as_array) {
        for change in changes {
            if let Some(s) = change.get("path").and_then(Value::as_str) {
                if !s.is_empty() {
                    return Some(s);
                }
            }
        }
    }
    None
}

fn collect_paths(value: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for key in ["path", "file_path", "filename", "from", "to"] {
        if let Some(s) = value.get(key).and_then(Value::as_str) {
            if !s.is_empty() {
                out.push(s.to_string());
            }
        }
    }
    if let Some(changes) = value.get("changes").and_then(Value::as_array) {
        for change in changes {
            if let Some(s) = change.get("path").and_then(Value::as_str) {
                if !s.is_empty() && !out.iter().any(|p| p == s) {
                    out.push(s.to_string());
                }
            }
        }
    }
    out
}
