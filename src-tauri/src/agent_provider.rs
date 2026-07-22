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
use crate::stream_event::{StreamEvent, StreamState};
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
/// model doesn't take an effort flag at all, e.g. Claude's Haiku). Owned
/// rather than `'static`: a provider backed by a dynamic/user-configured
/// backend (e.g. Ollama or a custom endpoint) discovers this at runtime,
/// it isn't a fixed compile-time list.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub label: String,
    pub effort_levels: Vec<String>,
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

/// The result of resolving one provider's model catalog — kept distinct
/// from an empty list so callers (the frontend model picker) can tell "no
/// usable models configured" apart from "catalog discovery failed" and show
/// the right thing for each: `Available { models: [] }` means the provider
/// resolved cleanly but has nothing to offer yet (e.g. no models pulled
/// into a local Ollama install), while `Error` carries an actionable reason
/// (e.g. the backend was unreachable). A provider with no `ModelCatalog`
/// implementation at all is reported as `Available { models: [] }` too —
/// from the caller's perspective there's simply nothing to enumerate,
/// which isn't an error either.
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ModelCatalogStatus {
    Available { models: Vec<ModelInfo> },
    Error { message: String },
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
    /// by `setup_status` / `get_system_metrics`. This default assumes
    /// "usable" means "logged in" — a credential-free provider (a
    /// local/Ollama-backed backend, a custom endpoint with no OAuth step)
    /// should override `readiness()` directly instead, so it can report
    /// itself ready once its binary/backend is reachable without a login
    /// concept at all.
    fn readiness(&self) -> BinaryStatus {
        binary_status(self.name(), self.discover(), |bin| self.is_logged_in(bin))
    }

    // --- command/session creation and resume --------------------------------

    /// Builds the subprocess command for one turn. `req.resume_id` selects
    /// resume vs. fresh-session invocation; `req.mcp_config` / `req.system_prompt`
    /// are injected in whatever form the underlying CLI expects.
    fn build_command(&self, bin: &Path, req: &TurnRequest<'_>) -> Command;

    /// Builds a title-generation turn: no system prompt, no MCP config, no
    /// session resume. `model` is the caller's choice (typically its
    /// lightest/cheapest one) — the trait doesn't assume a fixed catalog it
    /// could pick a default from; see `ModelCatalog` for providers that can
    /// enumerate one.
    fn title_request<'a>(&self, workspace: &'a Path, prompt: &'a str, model: &'a str) -> TurnRequest<'a> {
        TurnRequest {
            workspace,
            message: prompt,
            model,
            effort: None,
            resume_id: None,
            mcp_config: None,
            system_prompt: None,
        }
    }

    // --- stream-event normalization ------------------------------------------

    /// Parses one JSONL line into zero or more `StreamEvent`s via `emit`.
    /// Every provider shares the same `StreamState`/`StreamEvent` (defined in
    /// `stream_event`) so the frontend handles one event shape.
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

/// One entry in the provider registry: the wire-format name paired with its
/// `AgentProvider` and (if it has one) its `ModelCatalog`. This is the single
/// place that knows which providers exist — `provider()` and `all()` both
/// read from it, so registering a provider here is enough for every caller
/// (including `commands/system::list_agent_models`) to pick it up; nothing
/// else needs its own copy of the provider-name list.
struct Registered {
    name: &'static str,
    agent: &'static dyn AgentProvider,
    catalog: Option<&'static dyn ModelCatalog>,
}

static REGISTRY: &[Registered] = &[
    Registered {
        name: "claude",
        agent: &ClaudeCodeProvider,
        catalog: Some(&ClaudeCodeProvider),
    },
    Registered {
        name: "codex",
        agent: &CodexProvider,
        catalog: Some(&CodexProvider),
    },
];

/// Looks up the adapter for a wire-format provider name (`"claude"` /
/// `"codex"`), the same strings used in `ChatOptions::provider` and the
/// `chat_sessions` resume columns.
pub fn provider(name: &str) -> Option<&'static dyn AgentProvider> {
    REGISTRY.iter().find(|r| r.name == name).map(|r| r.agent)
}

/// Every registered provider, paired with its model catalog if it has one.
/// Used by `list_agent_models` so the known-provider set has exactly one
/// owner instead of a separately maintained name list.
pub fn all() -> impl Iterator<Item = (&'static dyn AgentProvider, Option<&'static dyn ModelCatalog>)> {
    REGISTRY.iter().map(|r| (r.agent, r.catalog))
}

/// Optional capability, separate from `AgentProvider`: a provider whose
/// available models can be enumerated. Claude and Codex both ship a fixed
/// model list, but that's not universal — a provider fronting Ollama or a
/// custom endpoint (e.g. a future OpenCode adapter) has a dynamic,
/// user-configured catalog it may need a network/process round-trip to
/// resolve, or may not be able to resolve at all. Keeping this off the core
/// `AgentProvider` trait means a provider that can't enumerate models still
/// implements the rest of the interface fully.
pub trait ModelCatalog: Send + Sync {
    /// Lists currently available models, in display order. Fallible
    /// independent of `AgentProvider::discover`/`readiness` — a provider's
    /// binary can be installed and ready while its model catalog still
    /// fails to resolve (e.g. its backend is unreachable).
    fn discover_models(&self) -> Result<Vec<ModelInfo>, AgentError>;
}


struct StaticModel {
    id: &'static str,
    label: &'static str,
    effort_levels: &'static [&'static str],
}

impl From<&StaticModel> for ModelInfo {
    fn from(m: &StaticModel) -> Self {
        ModelInfo {
            id: m.id.to_string(),
            label: m.label.to_string(),
            effort_levels: m.effort_levels.iter().map(|s| s.to_string()).collect(),
        }
    }
}

/// `effort_levels` mirror `src/state/chatOptions.ts` (`CLAUDE_EFFORT_LEVELS`,
/// `effortLevelsForModel`): Haiku takes no effort flag, Opus gets the full
/// range, Sonnet/Fable drop `xhigh`.
const CLAUDE_MODELS: &[StaticModel] = &[
    StaticModel {
        id: "claude-sonnet-5",
        label: "Sonnet 5",
        effort_levels: &["low", "medium", "high", "max"],
    },
    StaticModel {
        id: "claude-opus-4-8",
        label: "Opus 4.8",
        effort_levels: &["low", "medium", "high", "xhigh", "max"],
    },
    StaticModel {
        id: "claude-fable-5",
        label: "Fable 5",
        effort_levels: &["low", "medium", "high", "max"],
    },
    StaticModel {
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

impl ModelCatalog for ClaudeCodeProvider {
    fn discover_models(&self) -> Result<Vec<ModelInfo>, AgentError> {
        Ok(CLAUDE_MODELS.iter().map(ModelInfo::from).collect())
    }
}

/// Codex takes the same effort range on every model, including the mini
/// variant (`src/state/chatOptions.ts` `CODEX_EFFORT_LEVELS`).
const CODEX_MODELS: &[StaticModel] = &[
    StaticModel {
        id: "gpt-5.5",
        label: "GPT-5.5",
        effort_levels: &["low", "medium", "high", "xhigh"],
    },
    StaticModel {
        id: "gpt-5.4",
        label: "GPT-5.4",
        effort_levels: &["low", "medium", "high", "xhigh"],
    },
    StaticModel {
        id: "gpt-5.4-mini",
        label: "GPT-5.4-Mini",
        effort_levels: &["low", "medium", "high", "xhigh"],
    },
    StaticModel {
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
        // `Command::spawn()` returns `NotFound` when the OS can't exec the
        // path it was given — i.e. the discovered `bin` itself has gone
        // missing (uninstalled, moved, an nvm alias switch) between
        // `discover()` and this call, not a missing `node` runtime: a
        // `#!/usr/bin/env node` script's shebang is resolved by the kernel
        // as part of that same exec, so a missing `node` only shows up
        // later as a nonzero exit status from the running process, never as
        // an `io::Error` here.
        if err.kind() == std::io::ErrorKind::NotFound {
            AgentError {
                message: "codex binary not found — it may have been moved or uninstalled since it was last detected"
                    .to_string(),
            }
        } else {
            AgentError {
                message: err.to_string(),
            }
        }
    }
}

impl ModelCatalog for CodexProvider {
    fn discover_models(&self) -> Result<Vec<ModelInfo>, AgentError> {
        Ok(CODEX_MODELS.iter().map(ModelInfo::from).collect())
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

    fn effort_levels_for<'a>(models: &'a [ModelInfo], id: &str) -> &'a [String] {
        models
            .iter()
            .find(|m| m.id == id)
            .map(|m| m.effort_levels.as_slice())
            .unwrap_or(&[])
    }

    #[test]
    fn claude_effort_levels_match_frontend_rules() {
        let models = ClaudeCodeProvider.discover_models().unwrap();
        assert_eq!(
            effort_levels_for(&models, "claude-haiku-4-5-20251001"),
            &[] as &[String]
        );
        assert_eq!(
            effort_levels_for(&models, "claude-opus-4-8"),
            ["low", "medium", "high", "xhigh", "max"]
        );
        assert_eq!(
            effort_levels_for(&models, "claude-sonnet-5"),
            ["low", "medium", "high", "max"]
        );
        assert_eq!(effort_levels_for(&models, "not-a-model"), &[] as &[String]);
    }

    #[test]
    fn codex_effort_levels_are_uniform_across_models() {
        let models = CodexProvider.discover_models().unwrap();
        for id in ["gpt-5.5", "gpt-5.4", "gpt-5.4-mini", "gpt-5.3-codex"] {
            assert_eq!(
                effort_levels_for(&models, id),
                ["low", "medium", "high", "xhigh"]
            );
        }
    }

    #[test]
    fn discover_models_first_entry_is_the_display_default() {
        assert_eq!(
            ClaudeCodeProvider.discover_models().unwrap()[0].id,
            "claude-sonnet-5"
        );
        assert_eq!(CodexProvider.discover_models().unwrap()[0].id, "gpt-5.5");
    }

    #[test]
    fn all_returns_every_registered_provider_paired_with_its_catalog() {
        let all: Vec<_> = all().collect();
        assert_eq!(all.len(), 2);
        assert!(all
            .iter()
            .all(|(_, catalog)| catalog.is_some()));
        let names: Vec<&str> = all.iter().map(|(agent, _)| agent.name()).collect();
        assert!(names.contains(&"claude"));
        assert!(names.contains(&"codex"));
    }

    #[test]
    fn title_request_has_no_resume_mcp_or_system_prompt() {
        let workspace = Path::new("/tmp/workspace");
        let req = ClaudeCodeProvider.title_request(workspace, "name this chat", "claude-haiku-4-5-20251001");
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
    fn codex_normalizes_missing_binary_error() {
        let err = std::io::Error::from(std::io::ErrorKind::NotFound);
        let msg = CodexProvider.normalize_spawn_error(&err).to_string();
        assert!(msg.contains("codex binary not found"), "got: {msg}");
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
