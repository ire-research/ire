//! Provider-neutral adapter interface over the agent subprocess layer.
//!
//! `claude_code` and `codex` each implement discovery, spawn-arg building, and
//! JSONL stream dispatch independently (see docs/architecture/chat-agents.md).
//! `AgentProvider` is the single trait that names those responsibilities so a
//! caller can hold `&dyn AgentProvider` instead of branching on a provider
//! string. It is additive: existing call sites keep working unchanged.

use std::path::Path;
use std::process::Command;

use serde::Serialize;
use serde_json::Value;

use crate::binary::{binary_status, BinaryStatus, DiscoveredBinary, DiscoveryError};
use crate::claude_code::stream::{StreamEvent, StreamState};
use crate::tool_cards::ToolProvider;

/// Everything needed to spawn one agent turn, independent of provider.
/// `build_command` is where IRE MCP config and the composed system prompt
/// get injected into the provider's own command-line surface (a raw file
/// flag for Claude, translated `-c mcp_servers.*` / `-c developer_instructions`
/// flags for Codex).
pub struct TurnRequest<'a> {
    pub workspace: &'a Path,
    pub message: &'a str,
    pub model: &'a str,
    pub effort: Option<&'a str>,
    pub resume_id: Option<&'a str>,
    pub mcp_config: Option<&'a Path>,
    pub system_prompt: Option<&'a str>,
}

/// One selectable model plus the effort levels valid for it (`[]` means the
/// model doesn't take an effort flag at all, e.g. Claude's Haiku).
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ModelInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub effort_levels: &'static [&'static str],
}

/// A spawn-time failure, normalized to one message regardless of which
/// provider's raw `io::Error` produced it.
#[derive(Debug, Serialize)]
pub struct AgentError {
    pub message: String,
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// A provider-scoped adapter over one agent CLI (Claude Code, Codex, ...).
/// Object-safe so callers can hold `&'static dyn AgentProvider` / look one up
/// by wire name instead of branching on `provider == "codex"`.
pub trait AgentProvider: Send + Sync {
    /// Stable identity used in tool-card provenance (`ToolCall::provider`).
    fn id(&self) -> ToolProvider;

    /// The wire string used in IPC options, DB columns, and session lookups
    /// (`"claude"` / `"codex"`).
    fn name(&self) -> &'static str;

    // --- discovery and readiness -------------------------------------------

    fn discover(&self) -> Result<DiscoveredBinary, DiscoveryError>;

    fn is_logged_in(&self, bin: &Path) -> bool;

    /// Folds discovery + login check into the tri-state `BinaryStatus` used
    /// by `setup_status` / `get_system_metrics`.
    fn readiness(&self) -> BinaryStatus {
        binary_status(self.name(), self.discover(), |bin| self.is_logged_in(bin))
    }

    // --- available models and capability metadata --------------------------

    /// Selectable models in display order. First entry is the default.
    fn models(&self) -> &'static [ModelInfo];

    /// Smallest/cheapest model, used for background chat-title generation.
    fn lightweight_model(&self) -> &'static str;

    fn default_model(&self) -> &'static str {
        self.models().first().map(|m| m.id).unwrap_or_default()
    }

    /// Effort levels valid for `model`, or `[]` if it takes no effort flag.
    fn effort_levels_for(&self, model: &str) -> &'static [&'static str] {
        self.models()
            .iter()
            .find(|m| m.id == model)
            .map(|m| m.effort_levels)
            .unwrap_or(&[])
    }

    // --- command/session creation and resume --------------------------------

    /// Builds the subprocess command for one turn. `req.resume_id` selects
    /// resume vs. fresh-session invocation; `req.mcp_config` / `req.system_prompt`
    /// are injected in whatever form the underlying CLI expects.
    fn build_command(&self, bin: &Path, req: &TurnRequest<'_>) -> Command;

    /// Builds a title-generation turn: the lightweight model, no system
    /// prompt, no MCP config, no session resume.
    fn title_request<'a>(&self, workspace: &'a Path, prompt: &'a str) -> TurnRequest<'a> {
        TurnRequest {
            workspace,
            message: prompt,
            model: self.lightweight_model(),
            effort: None,
            resume_id: None,
            mcp_config: None,
            system_prompt: None,
        }
    }

    // --- stream-event normalization ------------------------------------------

    /// Parses one JSONL line into zero or more `StreamEvent`s via `emit`.
    /// Shares `StreamState`/`StreamEvent` across providers (defined in
    /// `claude_code::stream`) so the frontend handles one event shape.
    fn dispatch(&self, json: &Value, state: &mut StreamState, emit: &mut dyn FnMut(StreamEvent));

    // --- cancellation --------------------------------------------------------

    /// Terminates a running turn's subprocess by pid. Default is SIGTERM
    /// (Unix) / `taskkill` (Windows); override if a provider ever needs to
    /// kill a process group or child tree instead of a single pid.
    fn cancel(&self, pid: u32) {
        crate::commands::chat::kill_process(pid);
    }

    // --- error normalization --------------------------------------------------

    /// Maps a raw spawn/IO failure into a provider-neutral `AgentError`.
    /// Providers may recognize their own known failure text (e.g. Codex's
    /// missing-node-in-PATH case) and return a clearer message.
    fn normalize_spawn_error(&self, err: &std::io::Error) -> AgentError {
        AgentError {
            message: err.to_string(),
        }
    }
}

/// Looks up the adapter for a wire-format provider name (`"claude"` /
/// `"codex"`), the same strings used in `ChatOptions::provider` and the
/// `chat_sessions` resume columns.
pub fn provider(name: &str) -> Option<&'static dyn AgentProvider> {
    match name {
        "claude" => Some(&ClaudeCodeProvider),
        "codex" => Some(&CodexProvider),
        _ => None,
    }
}

/// `effort_levels` mirror `src/state/chatOptions.ts` (`CLAUDE_EFFORT_LEVELS`,
/// `effortLevelsForModel`): Haiku takes no effort flag, Opus gets the full
/// range, Sonnet/Fable drop `xhigh`.
const CLAUDE_MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "claude-sonnet-5",
        label: "Sonnet 5",
        effort_levels: &["low", "medium", "high", "max"],
    },
    ModelInfo {
        id: "claude-opus-4-8",
        label: "Opus 4.8",
        effort_levels: &["low", "medium", "high", "xhigh", "max"],
    },
    ModelInfo {
        id: "claude-fable-5",
        label: "Fable 5",
        effort_levels: &["low", "medium", "high", "max"],
    },
    ModelInfo {
        id: "claude-haiku-4-5-20251001",
        label: "Haiku 4.5",
        effort_levels: &[],
    },
];

pub struct ClaudeCodeProvider;

impl AgentProvider for ClaudeCodeProvider {
    fn id(&self) -> ToolProvider {
        ToolProvider::Claude
    }

    fn name(&self) -> &'static str {
        "claude"
    }

    fn discover(&self) -> Result<DiscoveredBinary, DiscoveryError> {
        crate::claude_code::discovery::find_claude_binary()
    }

    fn is_logged_in(&self, bin: &Path) -> bool {
        crate::claude_code::discovery::is_claude_logged_in(bin)
    }

    fn models(&self) -> &'static [ModelInfo] {
        CLAUDE_MODELS
    }

    fn lightweight_model(&self) -> &'static str {
        "claude-haiku-4-5-20251001"
    }

    fn build_command(&self, bin: &Path, req: &TurnRequest<'_>) -> Command {
        crate::claude_code::spawn::build_command(&crate::claude_code::spawn::SpawnArgs {
            bin,
            workspace: req.workspace,
            message: req.message,
            resume_id: req.resume_id,
            mcp_config: req.mcp_config,
            system_prompt: req.system_prompt,
            model: req.model,
            effort: req.effort,
        })
    }

    fn dispatch(&self, json: &Value, state: &mut StreamState, emit: &mut dyn FnMut(StreamEvent)) {
        crate::claude_code::stream::dispatch(json, state, &mut |event| emit(event));
    }
}

/// Codex takes the same effort range on every model, including the mini
/// variant (`src/state/chatOptions.ts` `CODEX_EFFORT_LEVELS`).
const CODEX_MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "gpt-5.5",
        label: "GPT-5.5",
        effort_levels: &["low", "medium", "high", "xhigh"],
    },
    ModelInfo {
        id: "gpt-5.4",
        label: "GPT-5.4",
        effort_levels: &["low", "medium", "high", "xhigh"],
    },
    ModelInfo {
        id: "gpt-5.4-mini",
        label: "GPT-5.4-Mini",
        effort_levels: &["low", "medium", "high", "xhigh"],
    },
    ModelInfo {
        id: "gpt-5.3-codex",
        label: "GPT-5.3-Codex",
        effort_levels: &["low", "medium", "high", "xhigh"],
    },
];

pub struct CodexProvider;

impl AgentProvider for CodexProvider {
    fn id(&self) -> ToolProvider {
        ToolProvider::Codex
    }

    fn name(&self) -> &'static str {
        "codex"
    }

    fn discover(&self) -> Result<DiscoveredBinary, DiscoveryError> {
        crate::codex::discovery::find_codex_binary()
    }

    fn is_logged_in(&self, bin: &Path) -> bool {
        crate::codex::discovery::is_codex_logged_in(bin)
    }

    fn models(&self) -> &'static [ModelInfo] {
        CODEX_MODELS
    }

    fn lightweight_model(&self) -> &'static str {
        "gpt-5.4-mini"
    }

    fn build_command(&self, bin: &Path, req: &TurnRequest<'_>) -> Command {
        crate::codex::spawn::build_codex_command(&crate::codex::spawn::CodexSpawnArgs {
            bin,
            workspace: req.workspace,
            message: req.message,
            model: req.model,
            reasoning_effort: req.effort.unwrap_or("low"),
            system_prompt: req.system_prompt,
            mcp_config: req.mcp_config,
            resume_id: req.resume_id,
        })
    }

    fn dispatch(&self, json: &Value, state: &mut StreamState, emit: &mut dyn FnMut(StreamEvent)) {
        crate::codex::stream::dispatch(json, state, &mut |event| emit(event));
    }

    fn normalize_spawn_error(&self, err: &std::io::Error) -> AgentError {
        // The codex npm package is a `#!/usr/bin/env node` script; a minimal
        // inherited PATH (e.g. launched from Finder) can't find `node`. See
        // `codex::spawn::build_codex_command`'s PATH prepend workaround, which
        // this only catches when it wasn't enough (`bin` had no parent, or
        // `node` is missing system-wide).
        if err.kind() == std::io::ErrorKind::NotFound {
            AgentError {
                message: "codex could not find a node runtime in PATH to run its script entry point"
                    .to_string(),
            }
        } else {
            AgentError {
                message: err.to_string(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn command_args(cmd: &Command) -> Vec<String> {
        cmd.get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn claude_effort_levels_match_frontend_rules() {
        let p = ClaudeCodeProvider;
        assert_eq!(
            p.effort_levels_for("claude-haiku-4-5-20251001"),
            &[] as &[&str]
        );
        assert_eq!(
            p.effort_levels_for("claude-opus-4-8"),
            &["low", "medium", "high", "xhigh", "max"]
        );
        assert_eq!(
            p.effort_levels_for("claude-sonnet-5"),
            &["low", "medium", "high", "max"]
        );
        assert_eq!(p.effort_levels_for("not-a-model"), &[] as &[&str]);
    }

    #[test]
    fn codex_effort_levels_are_uniform_across_models() {
        let p = CodexProvider;
        for model in ["gpt-5.5", "gpt-5.4", "gpt-5.4-mini", "gpt-5.3-codex"] {
            assert_eq!(
                p.effort_levels_for(model),
                &["low", "medium", "high", "xhigh"]
            );
        }
    }

    #[test]
    fn default_model_is_first_entry() {
        assert_eq!(ClaudeCodeProvider.default_model(), "claude-sonnet-5");
        assert_eq!(CodexProvider.default_model(), "gpt-5.5");
    }

    #[test]
    fn title_request_has_no_resume_mcp_or_system_prompt() {
        let workspace = Path::new("/tmp/workspace");
        let req = ClaudeCodeProvider.title_request(workspace, "name this chat");
        assert_eq!(req.model, "claude-haiku-4-5-20251001");
        assert!(req.resume_id.is_none());
        assert!(req.mcp_config.is_none());
        assert!(req.system_prompt.is_none());
        assert_eq!(req.message, "name this chat");
    }

    #[test]
    fn claude_build_command_injects_resume_and_model() {
        let req = TurnRequest {
            workspace: Path::new("/tmp/workspace"),
            message: "hello",
            model: "claude-sonnet-5",
            effort: Some("high"),
            resume_id: Some("session-123"),
            mcp_config: None,
            system_prompt: None,
        };
        let cmd = ClaudeCodeProvider.build_command(Path::new("claude"), &req);
        let args = command_args(&cmd);
        assert!(args.windows(2).any(|w| w == ["--resume", "session-123"]));
        assert!(args.windows(2).any(|w| w == ["--model", "claude-sonnet-5"]));
        assert!(args.windows(2).any(|w| w == ["--effort", "high"]));
    }

    #[test]
    fn codex_build_command_uses_resume_subcommand() {
        let req = TurnRequest {
            workspace: Path::new("/tmp/workspace"),
            message: "continue",
            model: "gpt-5.3-codex",
            effort: Some("medium"),
            resume_id: Some("thread-123"),
            mcp_config: None,
            system_prompt: None,
        };
        let cmd = CodexProvider.build_command(Path::new("codex"), &req);
        let args = command_args(&cmd);
        assert_eq!(args[0], "exec");
        assert_eq!(args[1], "resume");
        assert!(args.iter().any(|a| a == "thread-123"));
    }

    #[test]
    fn claude_dispatch_delegates_to_claude_code_stream() {
        let json = serde_json::json!({
            "type": "assistant",
            "message": { "content": [{ "type": "text", "text": "hi" }] },
        });
        let mut state = StreamState::default();
        let mut events = Vec::new();
        ClaudeCodeProvider.dispatch(&json, &mut state, &mut |e| events.push(e));
        assert_eq!(events, vec![StreamEvent::TextDelta { text: "hi".into() }]);
    }

    #[test]
    fn provider_lookup_resolves_wire_names() {
        assert_eq!(provider("claude").unwrap().id(), ToolProvider::Claude);
        assert_eq!(provider("codex").unwrap().id(), ToolProvider::Codex);
        assert!(provider("gemini").is_none());
    }

    #[test]
    fn codex_normalizes_missing_node_error() {
        let err = std::io::Error::from(std::io::ErrorKind::NotFound);
        let msg = CodexProvider.normalize_spawn_error(&err).to_string();
        assert!(msg.contains("node runtime"), "got: {msg}");
    }

    #[test]
    fn claude_normalize_spawn_error_passes_through_message() {
        let err = std::io::Error::other("boom");
        let msg = ClaudeCodeProvider.normalize_spawn_error(&err).to_string();
        assert_eq!(msg, "boom");
    }

    #[test]
    fn codex_dispatch_delegates_to_codex_stream() {
        let json = serde_json::json!({
            "type": "thread.started",
            "thread_id": "thread-abc",
        });
        let mut state = StreamState::default();
        let mut events = Vec::new();
        CodexProvider.dispatch(&json, &mut state, &mut |e| events.push(e));
        assert_eq!(
            events,
            vec![StreamEvent::Init {
                session_id: "thread-abc".into()
            }]
        );
    }
}
