//! Provider-neutral stream-event contract shared by every `AgentProvider`'s
//! JSONL dispatcher. Defined once here so Claude Code's and Codex's parsers
//! (`claude_code::stream::dispatch`, `codex::stream::dispatch`) emit the same
//! shape, and the frontend can handle it through a single `chat-stream`
//! listener regardless of which provider is running.

use serde::Serialize;
use serde_json::Value;

use crate::tool_cards::{ToolCall, ToolIo, ToolStatus};

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

/// Mutable parse state threaded through one turn's `dispatch` calls. Most
/// fields are `pub(crate)` rather than private: `claude_code::stream` and
/// `codex::stream` are sibling modules that each update their own bookkeeping
/// fields directly as they parse their provider's JSONL shape.
#[derive(Default)]
pub struct StreamState {
    pub session_id: String,
    pub emitted_text: bool,
    pub emitted_done: bool,
    pub emitted_codex_agent_item_ids: Vec<String>,
    pub(crate) claude_message_id: Option<String>,
    pub(crate) emitted_text_chars_by_block: Vec<usize>,
    pub(crate) emitted_thinking_chars_by_block: Vec<usize>,
    pub(crate) emitted_tool_ids: Vec<String>,
    pub(crate) ask_tool_ids: Vec<String>,
}
