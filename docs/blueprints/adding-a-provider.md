# Adding a new `AgentProvider`

Walkthrough for wiring in a third agent CLI (Claude Code and Codex are the
two existing ones) alongside the current pair, without touching call sites
in `commands/chat.rs`, `commands/system.rs`, or `commands/workspace.rs`. See
[docs/architecture/chat-agents.md — the `AgentProvider` trait](../architecture/chat-agents.md#the-agentprovider-trait-agent_providerrs)
for what the trait looks like and how it's already wired in; this file is
the steps to extend it.

## 1. Add the provider's own discovery / spawn / stream modules

Mirror `claude-code/` or `codex/` — three files, same shapes as the existing
ones:

- **`discovery.rs`** — `find_<name>_binary() -> Result<DiscoveredBinary, DiscoveryError>` via `binary::find_binary(name, candidate_paths())`, plus `is_<name>_logged_in(bin: &Path) -> bool` via `binary::run_with_timeout`.
- **`spawn.rs`** — a `<Name>SpawnArgs<'a>` struct and `build_<name>_command(&<Name>SpawnArgs) -> Command`. This is where the CLI's actual flags live: how it takes a message, model, effort, resume id, MCP config, and system prompt. Look at `codex::spawn::inject_mcp_servers` for the pattern to follow if the new CLI needs config translated rather than passed as a raw file path.
- **`stream.rs`** — `dispatch<F: FnMut(StreamEvent)>(json: &Value, state: &mut StreamState, emit: &mut F)`, reusing `StreamEvent`/`StreamState` from `claude_code::stream` (don't define a new event enum) and `tool_cards::build_tool_call` to normalize tool calls into `ToolCall`.

Write the same kind of arg-shape and dispatch unit tests the existing
modules have (`claude-code/spawn.rs`, `codex/stream.rs`) — these are what
actually pin down the CLI flags/JSON shape, independent of the trait.

## 2. Implement `AgentProvider`

In `agent_provider.rs`, add a zero-sized struct and delegate every method to
the modules from step 1:

```rust
pub struct NewCliProvider;

impl AgentProvider for NewCliProvider {
    fn id(&self) -> ToolProvider { ToolProvider::NewCli } // add the variant in tool_cards.rs
    fn name(&self) -> &'static str { "new_cli" }           // the wire string
    fn discover(&self) -> Result<DiscoveredBinary, DiscoveryError> {
        crate::new_cli::discovery::find_new_cli_binary()
    }
    fn is_logged_in(&self, bin: &Path) -> bool {
        crate::new_cli::discovery::is_new_cli_logged_in(bin)
    }
    fn build_command(&self, bin: &Path, req: &TurnRequest<'_>) -> Command {
        crate::new_cli::spawn::build_new_cli_command(&crate::new_cli::spawn::NewCliSpawnArgs {
            bin, workspace: req.workspace, message: req.message, model: req.model,
            effort: req.effort, resume_id: req.resume_id,
            mcp_config: req.mcp_config, system_prompt: req.system_prompt,
        })
    }
    fn dispatch(&self, json: &Value, state: &mut StreamState, emit: &mut dyn FnMut(StreamEvent)) {
        crate::new_cli::stream::dispatch(json, state, &mut |event| emit(event));
    }
    // normalize_spawn_error / cancel: only override if the CLI has a failure
    // mode the default (pass the io::Error through / SIGTERM the pid) can't
    // describe usefully — see CodexProvider's missing-node-in-PATH override.
}
```

`dispatch`'s `emit: &mut dyn FnMut(StreamEvent)` parameter, not a generic
`impl FnMut`, is required for the trait to stay object-safe (`&dyn
AgentProvider`) — wrap the call as `&mut |event| emit(event)`, don't try to
pass `emit` straight through to a `dispatch<F: FnMut(StreamEvent)>` function,
which won't type-check against an unsized `F`.

Note `AgentProvider` has no model-catalog methods — see step 3.

## 3. Implement `ModelCatalog`, if the CLI has one to expose

This is optional and separate from `AgentProvider`. Implement it if models
can be enumerated at all, even fallibly:

```rust
impl ModelCatalog for NewCliProvider {
    fn discover_models(&self) -> Result<Vec<ModelInfo>, AgentError> {
        // Static list, like Claude/Codex: wrap it in Ok(...).
        // Dynamic backend (e.g. querying an Ollama endpoint, reading a
        // config file the user points at a custom endpoint): do the
        // round-trip here and return Err(AgentError { message }) on failure
        // — a catalog that can't resolve right now is not the same as the
        // provider being unavailable, so don't fold this into `readiness`.
        Ok(vec![
            ModelInfo { id: "...".into(), label: "...".into(), effort_levels: vec![] },
        ])
    }
}
```

If the CLI has no fixed catalog and no way to enumerate one at all (e.g. it
just accepts whatever model string the user types, sourced from wherever
its own config lives), skip this impl entirely. `model_catalog(name)`
returning `None` for it is a legitimate, handled state — `list_agent_models`
already treats "no catalog" and "catalog resolution failed" the same way
(empty `models`, `default_model: None`), not as an error.

## 4. Register it in the lookups

```rust
pub fn provider(name: &str) -> Option<&'static dyn AgentProvider> {
    match name {
        "claude" => Some(&ClaudeCodeProvider),
        "codex" => Some(&CodexProvider),
        "new_cli" => Some(&NewCliProvider),
        _ => None,
    }
}

pub fn model_catalog(name: &str) -> Option<&'static dyn ModelCatalog> {
    match name {
        "claude" => Some(&ClaudeCodeProvider),
        "codex" => Some(&CodexProvider),
        "new_cli" => Some(&NewCliProvider),  // omit this arm if step 3 was skipped
        _ => None,
    }
}
```

Also add `"new_cli"` to `PROVIDER_NAMES` in `commands/system.rs` so
`list_agent_models` picks it up. Together with the two match arms above,
this is what makes `chat_send`, `generate_chat_title`, `chat_cancel`,
`setup_status`, and `get_system_metrics` pick the new provider up — none of
those call sites need editing.

## 5. Wire up the frontend

- Add entries to `MODELS` in `src/state/chatOptions.ts` (`id`, `label`,
  `provider`) — by hand, matching whatever `discover_models()` returns (or a
  fixed list, if step 3 was skipped). See
  [chat-agents.md](../architecture/chat-agents.md#the-agentprovider-trait-agent_providerrs):
  `list_agent_models` is not consumed by the frontend yet, so this is not
  read from one source today.
- Add an effort-levels table if the new CLI's rule isn't uniform (see
  `effortLevelsForModel`), and a case to `lightweightModelForProvider`.
- Add the provider to `useChatOptions`'s `availableProviders` default and
  wherever the model picker enumerates `Provider` (`"claude" | "codex"`
  becomes a three-way union).

## 6. Test it end-to-end

Follow the pattern in `agent_provider.rs`'s own test module: arg-shape
assertions on `build_command`'s output (`--resume`/`exec resume` equivalent,
model/effort flags), a `dispatch` test per JSONL shape the CLI emits, and
`normalize_spawn_error` for any special-cased failure text. Then run the
app in dev mode (`npm run dev:tauri`) and send one message through the new
provider — `chat_send`'s tracing (`tracing::info!(... provider = %provider
...)`) will confirm which code path actually ran.
