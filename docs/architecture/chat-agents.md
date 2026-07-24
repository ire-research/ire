# Chat & Agent Layer

Covers the ingestion pipelines, the chat system (send flow, experiment lifecycle, multi-tab), and the agent subprocess layer (binary discovery, spawn, JSONL parsers, session management).

---

## Pipelines

### Notes ingestion

```
User edits the notes pane (raw text)
  → blur or Ctrl+Enter after content changed
  → save_notes(content) Tauri command
    → Rust patches the `notes` field in .ire/ire.json (atomic, under IRE_LOCK)
  → `workspace-event notes-changed { content }` is emitted
```

No agent turn is triggered. The agent reads `notes` (via `ire.read`) only if the user explicitly requests it; it is never injected into the system prompt by default.

### Ideas ingestion

Ideas are stored in the `ire.json` `ideas` array (ordered `{ text }[]`) through `save_ideas`. Clicking Add in `IdeasPane` opens an inline draft card; pressing Enter prepends a new `{ text }` entry. The trash icon removes the entry; drag-to-reorder reorders the array; React keys by index. Each action writes the full array via `save_ideas`. No agent turn is triggered.

### Resource ingestion

```
User queues one or more URLs/files → Ingest
  → submit_resources(sources, options: { provider, model, effort }) Tauri command
  → Rust:
      1. URL resources: fetch URL with reqwest (20 s timeout, follow redirects)
      2. URL arXiv shortcut: if URL is arxiv.org/abs/<id> or arxiv.org/pdf/<id>,
         fetch arxiv.org/e-print/<id> instead and extract LaTeX from the
         tarball (gzip + tar). Falls back to PDF on failure.
      3. URL content-type extraction:
           pdf  → pdf-extract crate → plain text
           html → readability extraction → plain text
      4. Local file extraction, limited to .txt, .md, .pdf, .docx:
           .txt/.md → UTF-8 text
           .pdf     → pdf-extract crate → plain text
           .docx    → unzip Office package and extract text from word/document.xml
      5. if any source fails, abort the whole job and return `source N: <error>`;
         no cache file, DB row, or resource tab is created
      6. write extracted text to cache:
           single URL resources use .ire/cache/<sha256(url)>.txt
           single local files use .ire/cache/<sha256(file bytes)>.txt
           multi-source jobs use .ire/cache/<batch_sha>/source-001.txt, source-002.txt, ...
      7. insert one resource row in SQLite (status=pending_summary, source_type='url',
         'local_file', or 'batch'); if an unindexed row with the same resource ID
         already exists, refresh its source refs from the current request
      8. open a new resource chat tab (see Multi-tab chat below), defaulted to "Ingest"
      9. kick an agent turn in that tab using the composer-selected provider/model/effort
```

The selected agent writes the draft markdown file and streams one short confirmation into the resource tab. When the turn ends, **Confirm** and **Discard** buttons appear.

In-flight ingestion state (the source refs for a transient resource id) lives in an in-memory `InflightResources` registry in app state, not in the DB.

**Confirm** (`confirm_resource`): reads `.ire/cache/<resource_id>_draft.md`, injects the `sources:` frontmatter from the registry entry, writes it to `resources/<slug>.md` through `IreStore::write_resource` (which regenerates `resources/_index.md` and emits `resource-changed`), removes the cache + draft, and drops the registry entry. Frontmatter: `title`, `sources`, `updated`, and `TL;DR`. No git commit is created.

**Discard** (`discard_resource`): for an in-flight id, drops the registry entry and deletes `.ire/cache/<id>.txt` / `.ire/cache/<id>/` + the draft, closing the tab. For a confirmed resource (the id is its `resources/<slug>.md` path), deletes the file, regenerates the index, and emits `resource-deleted`.

`resource.add` (MCP) is a simulated ingestion: the agent supplies the markdown directly (no fetch), it registers the same in-flight entry, writes the draft, and emits `tab-created` with `resource_status: "ready"` so the Approve/Discard bar appears immediately.

---

## Chat

IRE uses a **single unified agent surface** in the central pane. The user selects Claude Code or Codex via the model picker. The selected agent receives the same composed wiki system prompt and IRE MCP server configuration; frontend stream handling is shared through a common `StreamEvent` shape. The experiment workflow instructions are part of `.ire/_SYSTEM.md` (see [wiki-memory.md — context injection rules](wiki-memory.md#context-injection-rules)).

### Send message flow

```
User types in central pane → Send
  → chat_send({ message, options: { provider, model, effort } }) Tauri command
  → Rust spawns one of:
      claude -p "<message>"
        --model <model>
        [--effort <low|medium|high|xhigh|max>]
        --output-format stream-json --verbose --include-partial-messages
        --mcp-config ~/.ire/workspaces/<id>/mcp.json
        --append-system-prompt "<composed per context injection rules>"
        --resume <session_id_if_any>
      codex exec --json
        -m <model>
        -c model_reasoning_effort=<low|medium|high|xhigh>
        -c developer_instructions="<composed per context injection rules>"
        -c mcp_servers.ire.command="<ire-binary>" -c mcp_servers.ire.args=["--mcp-stdio"]
        -C <workspace>
        --dangerously-bypass-approvals-and-sandbox
        -- "<message>"
      codex exec resume [OPTIONS] <thread_id> -- "<message>"
  → Rust parses JSONL line-by-line, emits chat-stream events
  → on `Init`: capture provider-scoped session/thread id
  → on `Result`/`Done`: turn complete, frontend re-enables input
```

**Auto-title (first message of a new chat tab):** When the user sends the first message in a brand-new chat tab whose label is still the default `"Chat"`, `ChatPane.handleSend` fires `generate_chat_title({ message, model, provider })` in the background. The model is the smallest of the selected provider (`claude-haiku-4-5-20251001` / `gpt-5.4-mini`). The Rust command spawns a one-shot subprocess with **no** system prompt, MCP config, session resume, or effort, and returns a cleaned single-line title. On resolve the frontend calls `renameTab`, which types over the old label (40 ms/char typewriter). Any failure is silent — the label stays `"Chat"`.

### Experiment lifecycle (the wake-up pattern)

```
T0  User asks: "Run an ablation over learning rates [1e-3, 1e-4, 1e-5]."
T1  CC plans, gets agreement, then calls MCP tool experiment.start({
       name: "lr-ablation",
       command: "python scripts/ablate_lr.py --output runs/lr_ablation",
       working_dir: "<project root>",
       wake_prompt: "Experiment lr-ablation finished. Read its result.md and
                     logs, then update the wiki and pulse."
    })
T2  MCP server forwards to IRE Rust backend:
       - inserts experiment row (status=running, uuid, start_time, command, …)
       - spawns the command as a DETACHED process group:
           Command::new("sh").args(["-c", &command])
                 .current_dir(working_dir)
                 .stdin(Stdio::null())
                 .process_group(0)             // setsid
                 .env_remove("CLAUDECODE")
                 .spawn()
       - returns { uuid, status: "started" } to CC
T3  CC's response to the user: "Started experiment <uuid>; I'll come back
    when it's done." Then this agent turn ENDS naturally.
T4  Backend monitor task waits on the child PID (off-thread). Frontend
    receives experiment-status events (started, log lines, …) and renders
    a live tail.
T5  Process exits. Backend:
       - updates the DB row (status, exit_code, end_time) and mirrors it
         into ire.json (`upsert_experiment`)
       - reads tail of stdout/stderr (last N kB) from .ire/cache/experiments/<uuid>/
       - composes wake-up message and spawns the same provider with its
         resume id and that message
T6  The agent reads result files, uses memory.write_short_term for daily
    notes, memory.write_long_term for durable conclusions, and ire.read +
    ire.edit to update focus/notes/ideas if the research direction changed.
```

**Subtleties:**
- The user can keep using the chat pane during T3–T5. The next user message and the wake-up share the same provider-scoped session id; whichever arrives first runs first. We serialise agent turns: only one subprocess per session at a time. If a user message arrives while a wake-up is running, it queues; if a wake-up fires while the user is mid-turn, it queues. The pending-queue is shown in the UI ("1 wake-up pending").
- `process_group(0)` (Linux/macOS) ensures killing IRE doesn't kill running experiments. On Windows we use `CREATE_NEW_PROCESS_GROUP`.
- Logs are streamed to disk; the UI tails them via `experiment-log-line` events.

### Cancellation

- **User cancels agent turn**: kill the running subprocess; emit `chat-cancelled`. Session id is retained.
- **User resets session**: starting a fresh session means a new chat tab, which gets a new `historySessionUuid` (hence a new `chat_sessions` row with no resume id). `chat_reset_session(tab_id)` clears the tab's transient `SessionManager` state; it is not currently wired to a frontend control.
- **User cancels experiment**: SIGTERM the process group; on next monitor tick, mark `status=cancelled` and fire the wake-up with that fact.

### Multi-tab chat

IRE supports multiple independent chat tabs in the central pane.

**Tab types**

| Type | Created by | Closeable | Description |
|---|---|---|---|
| Main | On workspace open (id `"main"`) | No (pinned) | The primary research conversation. |
| Chat | User clicks + button | Yes | Fresh provider-scoped agent session. |
| Resource | Backend resource ingestion | Auto-closes on Confirm/Discard | Dedicated to reviewing a single resource summary. |
| Preview | User clicks a resource in the Resources list | Yes | Renders the resource's wiki file with edit/preview toggle + Submit. |
| Experiment | User clicks an experiment in ExperimentsSection | Yes | Full view: metadata grid + live log tail. |

**Session isolation.** Each tab carries its own `tab_id`. The backend `SessionManager` maintains a `HashMap<tab_id, PerTabSession>` of transient per-turn state (`session_uuid`, `provider`, `model`, `effort`, `running_pid`, `pending_ask`); the durable resume id lives in `chat_sessions`.

**Event routing.** `chat-stream` events are wrapped as `{ tab_id, stream_id, event_id, event }` before being emitted. The frontend maintains a single global listener, routes each event to the correct tab's message list using `tab_id`, and ignores any already-seen `{tab_id, stream_id, event_id}` delivery.

**History persistence.** `chat_sessions` is the single durable store for chat content **and** resume ids. Chat tabs carry optional `historySessionUuid`, `historyStartedAt`, and `agentOptions`; the `tauri-plugin-store` workspace state persists only this small UI metadata (tab id/label/kind/pinned/options + `panel_layout` and `active_tab_id`) — **not** the `messages` array. The frontend creates the history UUID/start time on first send, passes them to `chat_send` (so the backend can upsert the resume id) and to `chat_history_save`. Messages are written to `chat_sessions` on `Done`/`Error`/close and on a ~1 s debounce while streaming (crash safety). On workspace open, each tab's messages are hydrated from `chat_sessions` by `historySessionUuid`. The History menu lists rows with `message_count > 0`, filtering out any UUID currently open as a chat tab; restoring a row binds a new tab to it (no delete), preserving its resume id.

**IPC changes from single-session baseline**

| Command / Event | Old signature | New signature |
|---|---|---|
| `chat_send` | `{ mode, message }` | `{ tab_id, message, options, session_uuid, tab_label, started_at }` |
| `chat_cancel` | `{}` | `{ tab_id }` |
| `chat_reset_session` | `{}` | `{ tab_id }` |
| `submit_ask_answer` | — (new) | `{ tab_id, answers }` |
| `chat-stream` event | `StreamEvent` | `{ tab_id, stream_id, event_id, event: StreamEvent }` |
| `tab-created` event | — | `{ tab_id, label, kind, resource_id?, resource_status?, agent_options? }` |

---

## Agent Subprocess Layer

Claude Code implements the patterns from [docs/blueprints/claude-code-wrapper.md](blueprints/claude-code-wrapper.md). Codex uses the same frontend event contract through a parallel backend module. Both run one turn as a local CLI subprocess emitting JSONL on stdout. OpenCode instead runs against a long-lived `opencode serve` HTTP/SSE process IRE itself starts and owns — see [OpenCode Server Transport](#opencode-server-transport) below. `agent_provider::AgentProvider` is the trait every provider implements (identity, discovery, readiness, which transport it uses); `agent_provider::CliTurn` is a separate, optional capability implemented only by the two CLI-subprocess providers, so call sites resolve an adapter once (`agent_provider::provider("claude" | "codex" | "opencode")`, plus `agent_provider::cli_turn(name)` for the CLI-specific surface) instead of branching on the provider string at every use.

### The `AgentProvider` and `CliTurn` traits (`agent_provider.rs`)

```rust
pub enum TurnTransport { CliSubprocess, OpenCodeServer }

pub trait AgentProvider: Send + Sync {
    fn id(&self) -> ToolProvider;                        // ToolProvider::Claude / ::Codex / ::Opencode
    fn name(&self) -> &'static str;                       // "claude" / "codex" / "opencode"
    fn discover(&self) -> Result<DiscoveredBinary, DiscoveryError>;
    fn is_logged_in(&self, bin: &Path) -> bool;
    fn readiness(&self) -> BinaryStatus;                  // default: binary_status(name(), discover(), is_logged_in)
    fn transport(&self) -> TurnTransport;                 // default: CliSubprocess; OpenCode overrides to OpenCodeServer
}

// Implemented only by CliSubprocess-transport providers (Claude, Codex).
pub trait CliTurn: Send + Sync {
    fn build_command(&self, bin: &Path, req: &TurnRequest<'_>) -> Command;
    fn title_request<'a>(&self, workspace: &'a Path, prompt: &'a str, model: &'a str) -> TurnRequest<'a>;
    fn dispatch(&self, json: &Value, state: &mut StreamState, emit: &mut dyn FnMut(StreamEvent));
    fn cancel(&self, pid: u32);                           // default: kill_process(pid)
    fn normalize_spawn_error(&self, err: &std::io::Error) -> AgentError;
}
```

`TurnRequest { workspace, message, model, effort, resume_id, mcp_config, system_prompt }` is the CLI-subprocess shape of one turn (`CliTurn` only — OpenCode's server transport has its own request shape in `opencode::turn`/`opencode::client`, since it POSTs JSON over HTTP rather than building a `Command`). `build_command` is where `mcp_config`/`system_prompt` get injected into each CLI's own surface — a raw `--mcp-config <path>` / `--append-system-prompt <text>` flag pair for Claude, translated `-c mcp_servers.*` / `-c developer_instructions=...` flags for Codex (see Spawn below); `resume_id` selects resume vs. fresh-session invocation the same way. `title_request` takes `model` as a caller-supplied argument rather than deriving it — see below for why.

`ClaudeCodeProvider` and `CodexProvider` are zero-sized structs implementing both traits by delegating to `cc::discovery`/`cc::spawn`/`cc::stream` and `codex::discovery`/`codex::spawn`/`codex::stream` respectively — none of that logic is duplicated. `OpenCodeProvider` implements only `AgentProvider` (`transport()` returns `OpenCodeServer`); it does not implement `CliTurn` at all, rather than faking a `Command` it doesn't have.

**One static registry is the single source of truth for which providers exist.** A private `agent_provider::REGISTRY: &[Registered]` pairs each wire-format name (`"claude"`/`"codex"`/`"opencode"`) with its `&'static dyn AgentProvider` and, if it has them, its `&'static dyn ModelCatalog` and `&'static dyn CliTurn`. Three public functions read it — `provider(name) -> Option<&'static dyn AgentProvider>`, `cli_turn(name) -> Option<&'static dyn CliTurn>` (`None` for OpenCode), and `all() -> impl Iterator<Item = (&'static dyn AgentProvider, Option<&'static dyn ModelCatalog>)>` (used by `list_agent_models`, see below). Adding a provider is one array entry in `REGISTRY`; nothing else needs updating to make it show up everywhere `provider()`/`cli_turn()`/`all()` are consulted.

Call sites branch once on `agent.transport()`: `TurnTransport::CliSubprocess` keeps the exact call chain `chat_send`/`generate_chat_title`/`resources::start_resource_summary`/`experiments::wake::fire_wakeup` always used — resolve `cli_turn(name)`, build the command, spawn with `normalize_spawn_error` on failure, dispatch the stream on a `spawn_blocking` thread. `TurnTransport::OpenCodeServer` instead awaits `opencode::turn::send`/`opencode::turn::generate_title` directly (both are already `async`, no blocking thread needed — the "process" is a persistent server, not spawned per turn). `chat_cancel` reads the tab's running-turn handle and provider name in one lock acquisition via `SessionManager::get_running_and_provider`, then dispatches on the handle's kind: `RunningTurn::Process(pid)` calls `cli_turn(provider).cancel(pid)` (falling back to raw `kill_process` if unresolvable); `RunningTurn::OpenCode { session_id }` calls `POST /session/:id/abort` through the running `OpenCodeRuntime`. The readiness checks in `setup_status` (`commands/workspace.rs`) and `get_system_metrics` (`commands/system.rs`) stay `XProvider.readiness()`, unaffected by transport.

**Model catalog is a separate, optional capability — not on `AgentProvider`.** An earlier version of this trait had `models()`/`default_model()`/`lightweight_model()`/`effort_levels_for()` returning `&'static` data, which assumes every provider ships a fixed, compile-time-known model list. That's true for Claude and Codex but not universal — a provider fronting Ollama or a custom endpoint has a dynamic, user-configured catalog it may need a round-trip to resolve, or may fail to resolve at all. That capability now lives on its own trait:

```rust
pub trait ModelCatalog: Send + Sync {
    fn discover_models(&self) -> Result<Vec<ModelInfo>, AgentError>;  // ModelInfo { id, label, effort_levels }, all owned Strings
}
```

`ModelInfo` is owned (not `&'static`) for the same reason. `ClaudeCodeProvider`/`CodexProvider` implement `ModelCatalog` trivially (their catalog is static, wrapped in `Ok(...)`); a provider that can't enumerate models this way still implements all of `AgentProvider` — catalog-less is a valid state, not an error (a `Registered` entry's `catalog` field is simply `None`). **OpenCode's `Registered` entry has `catalog: None` too**, but for a different reason than "can't enumerate": its catalog is real and dynamic, but resolving it needs an `async` round-trip to a live `opencode serve` process (`GET /provider`), which doesn't fit `ModelCatalog::discover_models`'s synchronous signature. `list_agent_models` (below) fetches it separately instead of forcing the trait to accommodate one provider's async, stateful special case.

`commands/system::list_agent_models` is the IPC command exposing this: for Claude/Codex it calls `agent_provider::all()` (filtering out OpenCode) and builds one `ProviderCapabilities { provider, default_model, catalog }` per pair via the synchronous `ModelCatalog` path; for OpenCode it separately calls `OpenCodeRuntime::ensure_started` (starting the server if this is the first OpenCode use this session) then `OpenCodeClient::list_models` (`GET /provider`, flattened across every provider OpenCode knows about — including ones the user hasn't authenticated, mirroring the old `opencode models` CLI catalog), building the same `ProviderCapabilities` shape by hand. Either path's `catalog: ModelCatalogStatus` is:

```rust
pub enum ModelCatalogStatus {
    Available { models: Vec<ModelInfo> },
    Error { message: String },
}
```

A provider with no `ModelCatalog` at all, and one whose catalog resolved successfully to an empty list, both report `Available { models: [] }` — "nothing to offer right now" (e.g. a local Ollama install with no models pulled), which isn't an error. A discovery failure (including "OpenCode server failed to start" or "no workspace open") reports `Error { message }` instead of being folded into the same empty list. `default_model` is `None` whenever `catalog` is `Error` or an empty `Available`. This does **not** consume the frontend's own static `MODELS`/`*_EFFORT_LEVELS` tables in `src/state/chatOptions.ts` yet — the two must be kept in sync by hand until the frontend reads this command instead.

To wire in a fourth CLI-subprocess provider alongside Claude Code and Codex, see [docs/blueprints/adding-a-provider.md](../blueprints/adding-a-provider.md) (a server-backed provider like OpenCode instead follows the pattern in `opencode/`, not that blueprint).

### Binary discovery (`binary`, `cc::discovery`, `codex::discovery`)

`find_claude_binary()` and `find_codex_binary()` share `binary::find_binary()`, which tries, in order:
1. `which <name>` on the current process PATH.
2. `$SHELL -lc "command -v <name>"` to load nvm/asdf/mise shims.
3. Provider-specific candidate paths.

Claude candidates include `.local/bin/claude`, `.claude/local/claude`, mise/asdf shims, npm locations, Homebrew locations, and all `$HOME/.nvm/versions/node/*/bin/claude`. Codex candidates include npm locations, Homebrew locations, all `$HOME/.nvm/versions/node/*/bin/codex`, and `%APPDATA%\npm\codex.cmd` on Windows.

Returns `Result<DiscoveredBinary, DiscoveryError>` with three error variants: `NotFound`, `NotExecutable`, `Io`.

Discovery alone only proves the binary is installed, not that it's authenticated. `binary::binary_status(name, result, is_logged_in)` folds a `DiscoveredBinary` result and a login check into one `BinaryStatus` (`Ready { path, version }`, `LoggedOut { path, version }`, or `Missing`), used by both `setup_status` and `get_system_metrics`. The login check runs the binary with a 5s timeout via `binary::run_with_timeout` (spawns the child, waits for output on a background thread, and gives up — leaving the child to finish on its own — if the timeout elapses): `is_claude_logged_in` parses `claude auth status --json`'s `loggedIn` field, and `is_codex_logged_in` treats a zero exit from `codex login status` as logged in. Any failure (not found, timeout, malformed output) is treated as not logged in.

### Spawn (`cc::spawn`, `codex::spawn`)

Claude Code spawn non-negotiables:
- `.env_remove("CLAUDECODE")` — prevent nested-session refusal.
- `.stdin(Stdio::null())` — don't hang waiting for stdin.
- `.current_dir(workspace_root)` — relative paths resolve correctly.

Always pair `--output-format stream-json` with `--verbose --include-partial-messages`.

`--disallowedTools AskUserQuestion` is always passed: the built-in `AskUserQuestion` tool can't be answered in one-shot `-p` mode (no stdin to carry the `tool_result` back to a pending `tool_use`). IRE's `mcp__ire__ask_user_question` MCP tool replaces it, answered synchronously within the same subprocess via the MCP backend socket (see `ask_user_question` handshake above and [mcp.md](mcp.md)).

Codex spawn uses `codex exec`, `--json`, `-m <model>`, `--dangerously-bypass-approvals-and-sandbox`, and `-c model_reasoning_effort=<low|medium|high|xhigh>`. Fresh turns pass `-C <workspace>`; resumed turns run with `Command::current_dir(workspace_root)` because `codex exec resume` does not accept `-C`. The prompt is passed after a `--` separator so messages beginning with `-` are not parsed as Codex CLI flags. `~/.ire/workspaces/<id>/mcp.json` is translated into Codex config flags such as `-c mcp_servers.ire.command="<ire-binary>"`, `-c mcp_servers.ire.args=["--mcp-stdio"]`, and `-c mcp_servers.ire.env.IRE_WORKSPACE="..."`.

**Codex PATH fix.** The `codex` npm package installs a `#!/usr/bin/env node` JS wrapper that locates and spawns the platform-native binary. Unlike `claude` (which ships a self-contained native executable), `codex` requires `node` to be in `PATH`. When the Tauri app is launched from Finder (production `.dmg`), the inherited `PATH` is minimal (`/usr/bin:/bin:/usr/sbin:/sbin`) and does not include nvm/asdf/mise directories, so `#!/usr/bin/env node` fails with `env: node: No such file or directory` and the subprocess exits immediately with no output. `build_codex_command` works around this by prepending the codex binary's parent directory to `PATH` — since nvm co-locates `node` and `codex` in the same `bin/` directory, this guarantees `node` is reachable. Any future agent binary that is a script requiring a runtime interpreter (node, python, etc.) will need the same treatment.

### JSONL parsers (`cc::stream`, `codex::stream`)

Reads stdout line-by-line on a `spawn_blocking` thread; deserialises each line into `serde_json::Value`; dispatches provider-specific JSONL into typed `StreamEvent`s emitted to the frontend on the `chat-stream` channel. Each emitted payload includes `stream_id = "{tab_id}:{stream_uuid}"` and a per-process monotonic `event_id`.

`StreamEvent`, `StreamState`, and `AskQuestion`/`AskQuestionOption` are defined once in the top-level `stream_event` module (not in `cc::stream`) since both parsers — and `AgentProvider::dispatch` — share them; `cc::stream`/`codex::stream` each own only their `dispatch()` function and provider-specific parsing helpers.

```rust
#[serde(tag = "kind")]
enum StreamEvent {
    Init { session_id: String },
    TextDelta { text: String },
    ThinkingDelta { text: String },
    ToolStart { tool: ToolCall },
    ToolDone { tool_id: String, output: Option<ToolIo>, status: ToolStatus, meta: Value },
    AskUserQuestion { tool_id: String, questions: Vec<AskQuestion> },
    Result { text: Option<String>, session_id: String },
    Error { message: String },
    Done,
}
```

`ToolCall` is the provider-neutral tool-card contract defined in `tool_cards.rs`: `{ tool_id, provider, kind, raw_name, title, input, output, status, meta }`. `kind` is one of `command`, `file_read`, `file_write`, `file_edit`, `file_search`, `web_fetch`, `ire_read`, `ire_edit`, `resource_add`, `memory_write`, `experiment_start`, `experiment_status`, `experiment_tail_logs`, or `other`.

Claude and Codex both normalize native tool records into `ToolCall` before emitting `ToolStart`. Claude maps built-ins such as `Bash`, `Read`, `Write`, `Edit`/`MultiEdit`, `Grep`/`Glob`/`LS`, and `WebFetch`; MCP names such as `ire__ire.read` normalize through the `ire.read`/`ire.edit`, `resource.add`, `memory.*`, and `experiment.*` mapping.

`AskUserQuestion` is emitted when CC calls IRE's `mcp__ire__ask_user_question` tool (the built-in `AskUserQuestion` tool is passed via `--disallowedTools`, see below). The parser intercepts that `tool_use` block, parses its `questions[]` payload, and tracks the tool id so the matching `tool_result` is suppressed. The frontend renders an `AskQuestionCard` and, on submit, calls `submit_ask_answer(tab_id, answers)` to deliver the answers back to the blocked MCP call in the same subprocess turn.

### OpenCode Server Transport (`opencode/`)

OpenCode does not use a CLI subprocess: IRE starts and owns one private `opencode serve` HTTP/SSE process per open workspace, and talks to it with `reqwest` (no `@opencode-ai/sdk` — that's a TypeScript client for the same HTTP API IRE's Rust backend calls directly). The CLI transport (`opencode run --format json`) can't drive interactive OpenCode tools well: it only emits tool JSON after the tool finishes, whereas the server's SSE stream emits incremental part/question/permission/session events. Full design rationale in `docs/opencode-server-integration.md`; this section documents the implementation as built.

**Runtime ownership (`opencode::runtime::OpenCodeRuntime`).** Tauri-managed state holding at most one running server, matched to the single `ActiveWorkspace`. Starts **lazily** on first OpenCode use (first `chat_send`, resource summary, wake-up, title generation, or catalog fetch with `provider: "opencode"`) rather than eagerly on workspace open — most workspaces never touch OpenCode, so there's no reason to pay a process-spawn+health-check cost on every open. `ensure_started(app, session_manager, workspace, bin, mcp_config)`:
1. Spawns `opencode serve --hostname 127.0.0.1 --port 0` with `current_dir(workspace)` and `OPENCODE_CONFIG_CONTENT` set to the overlay from `opencode::config::server_config` (below). `--port 0` lets the OS pick a free port; the actual bound address is parsed from the one plain line the process prints to stdout on startup, `opencode server listening on http://HOST:PORT` (structured logs go to a log file by default, not stdout/stderr, so this is the only stdout output to expect).
2. Polls `GET /global/health` until it reports healthy (bounded timeout).
3. Spawns one long-lived task that opens `GET /event` (SSE) and holds it for the runtime's whole lifetime — **not per-turn** — reconnecting with bounded exponential backoff on disconnect.

Bound to `127.0.0.1` only; no mDNS, no CORS. No password is set (loopback-only is considered safe for v1; `opencode serve` warns about this on stderr, which IRE discards). `stop()` (called from `close_workspace`, see [workspace.md](workspace.md#close)) aborts every OpenCode session IRE registered via `POST /session/:id/abort`, aborts the SSE task, and kills the server process. It never attaches to a server the user started themselves via the OpenCode TUI — IRE only ever talks to a process it spawned itself.

**Server config overlay (`opencode::config::server_config`).** Builds the `OPENCODE_CONFIG_CONTENT` JSON once, at server startup — never writes `opencode.json`/`opencode.jsonc` into the user's workspace. Two keys:
- `permission: "allow"` — equivalent to the old CLI transport's `--auto`: the server has no TTY to prompt on, and there's no permission-approval UI yet (a deliberate v1 scope cut, not an oversight). Because every action is pre-approved, OpenCode never emits a `permission.asked` event to reply to.
- `mcp` — IRE's own `mcp.json` translated into OpenCode's `McpLocalConfig` shape (same translation the old CLI transport used), with `IRE_MCP_EXCLUDE_ASK=1` added to the translated server's `environment` — see [mcp.md](mcp.md#tool-catalog) for why.

Unlike the old CLI transport, there is **no per-turn system prompt baked into server config** (no `agent.ire` block, no `--agent` flag): IRE's composed system prompt is instead passed per-message via `prompt_async`'s `system` field (see Turn lifecycle below), so a stale workspace context can never become persistent server configuration, and ordinary turns run OpenCode's own default agent.

**Turn lifecycle (`opencode::turn::send`).** The single entry point every OpenCode turn type goes through — chat send, resource summary, and experiment wake-up all call this with different `tab_id`/`session_uuid`/prompt; only title generation (below) is different enough to bypass it.
1. `ensure_started` (returns immediately if already running for this workspace).
2. Resolve the OpenCode session id: read `chat_resume_ids(session_uuid, "opencode")`; if present, reuse it; otherwise `POST /session` and persist the new id via the same `chat_resume_ids` table every other provider uses (no schema change).
3. Register a `TabRoute { tab_id, stream_id, event_id, state }` in the runtime's `session_id -> TabRoute` map (keyed by OpenCode session id, not tab id — the SSE task looks up incoming events by `sessionID`) and emit `StreamEvent::Init` immediately (event_id 1), before the prompt is even sent — same timing as the CLI transport, which also emits `Init` as soon as a session id is known, not once the turn is confirmed to succeed.
4. Record `RunningTurn::OpenCode { session_id }` in `SessionManager` (see Session management below).
5. `POST /session/:id/prompt_async` with `{ model: { providerID, modelID }, variant: effort, system: system_prompt, parts: [{ type: "text", text: message }] }`. Returns `204` immediately — the reply streams over the runtime's already-open SSE connection, not this response.
6. If the server reports the session id unknown (`404` — e.g. deleted, or from an incompatible version after a restart), create one fresh session, move the same `TabRoute` (same `stream_id`, so no duplicate `Init`) to the new id, persist the new resume id, and retry once. A second failure surfaces `StreamEvent::Error` + `Done` and returns an error rather than silently sending into an unrelated session.

**Event normalization (`opencode::events`).** The runtime's SSE task parses every `/event` payload (`{ id, type, properties }`) via `parse_event`, acting only on the subset of OpenCode's ~90 event types IRE's UI needs — everything else (`session.updated`, `message.updated`, `session.diff`, `plugin.added`, the experimental `session.next.*` granular stream, ...) is ignored:

| OpenCode event | IRE action |
|---|---|
| `message.part.updated` (`part.type == "text"` / `"reasoning"`) | Suffix-delta against `OpenCodeSessionState`'s per-part-id length tracking (OpenCode resends the *full* part text on every update, not a delta) → `TextDelta` / `ThinkingDelta` |
| `message.part.updated` (`part.type == "tool"`) | `part.state.status` transitions `pending`/`running` → `ToolStart` (deduped per `callID` via `OpenCodeSessionState.tool_started`); `completed`/`error` → `ToolDone` (deduped via `tool_done`; emits a synthetic `ToolStart` first if a fast tool jumped straight to `completed` without an observed pending/running update) |
| `session.idle` | `Result { text: None }` then `Done`; clears the tab's `RunningTurn` (guarded — only if it still matches the session id, same "still current" guard the CLI transport uses for pids) |
| `session.error` | `Error { message }` (extracted from the error union's `data.message`/`message`/`name`) then `Done`; same `RunningTurn` clear |
| `question.asked` | See Native questions below |

Turn completion is driven by `session.idle`/`session.error`, not inferred from a `step-finish` part reason like the CLI transport has to (the server has an explicit terminal event; the CLI's JSONL stream doesn't).

**Native questions, not IRE's MCP question tool.** OpenCode has a built-in `question` tool; its `question.asked` SSE event carries `requestID`/`sessionID` and blocks server-side until answered. IRE's system prompt directs OpenCode to use it instead of `ask_user_question` (which is hidden from OpenCode's MCP catalog entirely — see [mcp.md](mcp.md#tool-catalog)) because the MCP-based flow finds the answering tab by scanning for *any* running subprocess, which can't disambiguate multiple concurrent OpenCode sessions; the native event's `sessionID` can. On `question.asked`, the SSE handler stores `request_id` via `SessionManager::set_pending_opencode_question(tab_id, request_id)` and emits the same `StreamEvent::AskUserQuestion` the frontend already renders (`tool_id` is the OpenCode `requestID`). `submit_ask_answer` checks for a pending OpenCode question first: if present, it normalizes each `AskAnswer` (`string | string[]`) into OpenCode's expected `string[]` (array of selected option labels) and `POST`s `/question/:requestID/reply`, instead of waking the MCP oneshot channel used for Claude/Codex.

**Title generation (`opencode::turn::generate_title`)** does not go through the turn-lifecycle SSE machinery above — a disposable one-shot session has no tab/SSE routing worth setting up. It creates a session, calls the *blocking* `POST /session/:id/message` (returns the complete reply directly instead of streaming), collects the text parts, and deletes the session. Mirrors `CliTurn::title_request`'s shape: no MCP config, no system prompt, no resume.

**Dynamic model catalog.** No `opencode models` CLI parsing. `OpenCodeClient::list_models` calls `GET /provider` and flattens every model across every provider OpenCode knows about (`<providerID>/<modelID>` id, `Model.name` as label, `Model.variants` keys as `effort_levels`) — including providers the user hasn't authenticated, matching the old CLI catalog's behavior. See "Model catalog" above for how `list_agent_models` fetches this (it needs a live server, so it can't go through the synchronous `ModelCatalog` trait the other two providers use).

### Session management

The durable resume id lives in `chat_resume_ids(session_uuid, provider, resume_id)` (`UNIQUE(session_uuid, provider)`), not a fixed per-provider column — this is what lets a third (and fourth) provider register without a schema change. `SessionManager` holds only transient in-process state for the current turn, generalized over transport via `RunningTurn`:

```rust
enum RunningTurn {
    Process(u32),                    // Claude/Codex: the subprocess pid
    OpenCode { session_id: String }, // OpenCode: no pid, just its own session id
}

struct PerTabSession {
    session_uuid: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    effort: Option<String>,
    running: Option<RunningTurn>,
    pending_ask: Option<oneshot::Sender<Vec<serde_json::Value>>>,
    pending_opencode_question: Option<String>,  // requestID of an in-flight native `question.asked`
}

pub struct SessionManager(Arc<Mutex<HashMap<String, PerTabSession>>>);
```

`chat_send` receives `session_uuid`, `tab_label`, and `started_at` from the frontend. It reads the resume id via `get_chat_resume_id(session_uuid, provider)` and passes `--resume <session_id>` for Claude, `codex exec resume <thread_id>` for Codex, or (for OpenCode) reuses the OpenCode session id directly — see [OpenCode Server Transport](#opencode-server-transport) above. On the first `Init` event of a turn (CLI transport) or immediately on session resolution (OpenCode transport) it calls `upsert_chat_resume_id(...)`, creating the `chat_sessions`/`chat_resume_ids` rows if they don't exist yet. Because the row is keyed by `(session_uuid, provider)`, closing and reopening a workspace resumes the existing session for whichever provider the tab last used, and toggling provider keeps each provider's thread independently resumable.

`experiment.start` records the running turn's `tab_id`, `session_uuid`, `provider`, `model`, and optional `effort` (via `get_active_session`, which matches the first tab with *any* `RunningTurn` — `Process` or `OpenCode`, any provider can kick off an experiment) before spawning the detached command. The wake-up (`fire_wakeup`) resolves the resume id from `chat_resume_ids` by `session_uuid` + provider, and persists any new resume id the wake turn emits (`update_chat_resume_id` for the CLI path; the same `opencode::turn::send` path as any other OpenCode turn otherwise). Resource-ingestion turns have no frontend history uuid, so they key their resume row by `tab_id`.

**`ask_user_question` handshake (Claude/Codex only).** `pending_ask` carries the oneshot sender for a tab's in-flight `ask_user_question` MCP call. The handler resolves the calling tab via `SessionManager::get_active_process_session` — a `Process`-only variant of `get_active_session`, since only a `Process`-transport agent can ever call this tool (it's hidden from OpenCode's catalog) and a broader match risks picking a concurrently-running OpenCode tab instead of the Claude/Codex tab that actually asked. `register_ask(tab_id)` stores the sender and returns the receiver, which the MCP RPC handler (`mcp/rpc.rs`) blocks on with `blocking_recv()`. `submit_ask_answer(tab_id, answers)` (a Tauri command) calls `submit_ask`, which takes the sender and sends the answers, waking the blocked handler so it returns its `tool_result` within the same CC subprocess turn. `chat_cancel` and `chat_reset_session` both call `cancel_ask(tab_id)` to drop any pending sender (and, for OpenCode, any `pending_opencode_question`), so a blocked handler returns an error instead of hanging if the user cancels or resets before answering. `submit_ask_answer` checks `pending_opencode_question` *before* falling back to this MCP path — see [OpenCode Server Transport](#opencode-server-transport) "Native questions" above for the OpenCode-specific reply flow.
