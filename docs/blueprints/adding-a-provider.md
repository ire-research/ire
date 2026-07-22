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
    fn models(&self) -> &'static [ModelInfo] { NEW_CLI_MODELS }
    fn lightweight_model(&self) -> &'static str { "..." }
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

`ModelInfo { id, label, effort_levels }` entries should match whatever the
frontend's model picker will offer for this provider (see step 4).

`dispatch`'s `emit: &mut dyn FnMut(StreamEvent)` parameter, not a generic
`impl FnMut`, is required for the trait to stay object-safe (`&dyn
AgentProvider`) — wrap the call as `&mut |event| emit(event)`, don't try to
pass `emit` straight through to a `dispatch<F: FnMut(StreamEvent)>` function,
which won't type-check against an unsized `F`.

## 3. Register it in the lookup

```rust
pub fn provider(name: &str) -> Option<&'static dyn AgentProvider> {
    match name {
        "claude" => Some(&ClaudeCodeProvider),
        "codex" => Some(&CodexProvider),
        "new_cli" => Some(&NewCliProvider),
        _ => None,
    }
}
```

This one line is what makes `chat_send`, `generate_chat_title`,
`chat_cancel`, `setup_status`, `get_system_metrics`, and
`list_agent_models` all pick the new provider up — none of them need
editing.

## 4. Wire up the frontend

- Add entries to `MODELS` in `src/state/chatOptions.ts` (`id`, `label`,
  `provider`), matching the `ModelInfo` list from step 2.
- Add an effort-levels table if the new CLI's rule isn't uniform (see
  `effortLevelsForModel`), and a case to `lightweightModelForProvider`.
- Add the provider to `useChatOptions`'s `availableProviders` default and
  wherever the model picker enumerates `Provider` (`"claude" | "codex"`
  becomes a three-way union).

The frontend tables and the Rust `ModelInfo` tables are **not** read from
one source today — `commands/system::list_agent_models` exposes the Rust
side over IPC but nothing consumes it yet (see
[chat-agents.md](../architecture/chat-agents.md#the-agentprovider-trait-agent_providerrs)).
Keep both in sync by hand until that's wired up.

## 5. Test it end-to-end

Follow the pattern in `agent_provider.rs`'s own test module: arg-shape
assertions on `build_command`'s output (`--resume`/`exec resume` equivalent,
model/effort flags), a `dispatch` test per JSONL shape the CLI emits, and
`normalize_spawn_error` for any special-cased failure text. Then run the
app in dev mode (`npm run dev:tauri`) and send one message through the new
provider — `chat_send`'s tracing (`tracing::info!(... provider = %provider
...)`) will confirm which code path actually ran.
