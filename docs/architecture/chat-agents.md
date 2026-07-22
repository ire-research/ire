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

Claude Code implements the patterns from [docs/blueprints/claude-code-wrapper.md](blueprints/claude-code-wrapper.md). Codex uses the same frontend event contract through a parallel backend module. `agent_provider::AgentProvider` is the trait that unifies both behind one interface, so call sites look an adapter up once (`agent_provider::provider("claude" | "codex")`) instead of branching on the provider string at every use. It doesn't change what either CLI is invoked with — `ClaudeCodeProvider`/`CodexProvider` are thin wrappers over the discovery/spawn/stream modules described below.

### The `AgentProvider` trait (`agent_provider.rs`)

```rust
pub trait AgentProvider: Send + Sync {
    fn id(&self) -> ToolProvider;                        // ToolProvider::Claude / ::Codex
    fn name(&self) -> &'static str;                       // "claude" / "codex"
    fn discover(&self) -> Result<DiscoveredBinary, DiscoveryError>;
    fn is_logged_in(&self, bin: &Path) -> bool;
    fn readiness(&self) -> BinaryStatus;                  // default: binary_status(name(), discover(), is_logged_in)
    fn build_command(&self, bin: &Path, req: &TurnRequest<'_>) -> Command;
    fn title_request<'a>(&self, workspace: &'a Path, prompt: &'a str, model: &'a str) -> TurnRequest<'a>;
    fn dispatch(&self, json: &Value, state: &mut StreamState, emit: &mut dyn FnMut(StreamEvent));
    fn cancel(&self, pid: u32);                           // default: kill_process(pid)
    fn normalize_spawn_error(&self, err: &std::io::Error) -> AgentError;
}
```

`TurnRequest { workspace, message, model, effort, resume_id, mcp_config, system_prompt }` is the provider-neutral shape of one turn. `build_command` is where `mcp_config`/`system_prompt` get injected into each CLI's own surface — a raw `--mcp-config <path>` / `--append-system-prompt <text>` flag pair for Claude, translated `-c mcp_servers.*` / `-c developer_instructions=...` flags for Codex (see Spawn below); `resume_id` selects resume vs. fresh-session invocation the same way. `title_request` takes `model` as a caller-supplied argument rather than deriving it — see below for why.

`ClaudeCodeProvider` and `CodexProvider` are zero-sized structs implementing the trait by delegating to `cc::discovery`/`cc::spawn`/`cc::stream` and `codex::discovery`/`codex::spawn`/`codex::stream` respectively — none of that logic is duplicated.

**One static registry is the single source of truth for which providers exist.** A private `agent_provider::REGISTRY: &[Registered]` pairs each wire-format name (`"claude"`/`"codex"`) with its `&'static dyn AgentProvider` and, if it has one, its `&'static dyn ModelCatalog`. Two public functions read it — `provider(name: &str) -> Option<&'static dyn AgentProvider>` (used everywhere a call site needs to resolve one named provider) and `all() -> impl Iterator<Item = (&'static dyn AgentProvider, Option<&'static dyn ModelCatalog>)>` (used by `list_agent_models`, see below, to enumerate every registered provider without needing its own copy of the name list). Adding a provider is one array entry in `REGISTRY`; nothing else needs updating to make it show up everywhere `provider()`/`all()` are consulted.

Call sites that used to branch on `provider == "codex"` now resolve the adapter once and call through it: `chat_send`/`generate_chat_title`/`resources::start_resource_summary`/`experiments::wake::fire_wakeup` (build command, spawn with `normalize_spawn_error` on failure, dispatch the stream), `chat_cancel` (reads the tab's pid and provider name in one lock acquisition via `SessionManager::get_pid_and_provider` — a single combined accessor, not two separate lookups, so the pair can't be read from two different points in time — and calls `.cancel(pid)`, falling back to the raw `kill_process` if the tab has no recorded provider), and the readiness checks in `setup_status` (`commands/workspace.rs`) and `get_system_metrics` (`commands/system.rs`), both now `XProvider.readiness()`. `fire_wakeup` in particular used to silently normalize any unrecognized provider string to `"claude"`; it now resolves through `agent_provider::provider(...)` and returns (logging an error) on an unknown name instead of guessing.

**Model catalog is a separate, optional capability — not on `AgentProvider`.** An earlier version of this trait had `models()`/`default_model()`/`lightweight_model()`/`effort_levels_for()` returning `&'static` data, which assumes every provider ships a fixed, compile-time-known model list. That's true for Claude and Codex but not universal — a provider fronting Ollama or a custom endpoint has a dynamic, user-configured catalog it may need a round-trip to resolve, or may fail to resolve at all. That capability now lives on its own trait:

```rust
pub trait ModelCatalog: Send + Sync {
    fn discover_models(&self) -> Result<Vec<ModelInfo>, AgentError>;  // ModelInfo { id, label, effort_levels }, all owned Strings
}
```

`ModelInfo` is owned (not `&'static`) for the same reason. `ClaudeCodeProvider`/`CodexProvider` implement `ModelCatalog` trivially (their catalog is static, wrapped in `Ok(...)`), but a provider that can't enumerate models still implements all of `AgentProvider` — catalog-less is a valid state, not an error (a `Registered` entry's `catalog` field is simply `None`). `commands/system::list_agent_models` is the IPC command exposing this: it calls `agent_provider::all()` and builds one `ProviderCapabilities` per pair, treating a missing catalog or a `discover_models` error the same way (empty `models: []`, `default_model: None`), and does **not** consume the frontend's own static `MODELS`/`*_EFFORT_LEVELS` tables in `src/state/chatOptions.ts` yet — the two must be kept in sync by hand until the frontend reads this command instead.

To wire in a third CLI alongside Claude Code and Codex, see [docs/blueprints/adding-a-provider.md](../blueprints/adding-a-provider.md).

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

### Session management

The durable resume id lives in the `chat_sessions` table (see **History persistence** above), keyed by `session_uuid` (the tab's `historySessionUuid`). `SessionManager` holds only transient in-process state for the current turn:

```rust
struct PerTabSession {
    session_uuid: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    effort: Option<String>,
    running_pid: Option<u32>,
    pending_ask: Option<oneshot::Sender<Vec<serde_json::Value>>>,
}

pub struct SessionManager(Arc<Mutex<HashMap<String, PerTabSession>>>);
```

`chat_send` receives `session_uuid`, `tab_label`, and `started_at` from the frontend. It reads the resume id from `chat_sessions` via `get_chat_resume_id(session_uuid, provider)` and passes `--resume <session_id>` for Claude or `codex exec resume <thread_id>` for Codex. On the first `Init` event of a turn it calls `upsert_chat_resume_id(...)`, persisting the resume id into the provider-specific column (`claude_session_id` / `codex_thread_id`) and creating the row if it does not exist yet. Because both columns persist, closing and reopening a workspace resumes the existing session, and toggling provider keeps each thread resumable.

`experiment.start` records the running turn's `tab_id`, `session_uuid`, `provider`, `model`, and optional `effort` (via `get_active_session`) before spawning the detached command. The wake-up (`fire_wakeup`) resolves the resume id from `chat_sessions` by `session_uuid` + provider, and persists any new resume id the wake turn emits via `update_chat_resume_id`. Resource-ingestion turns have no frontend history uuid, so they key their resume row by `tab_id`.

**`ask_user_question` handshake.** `pending_ask` carries the oneshot sender for a tab's in-flight `ask_user_question` MCP call. `register_ask(tab_id)` stores the sender and returns the receiver, which the MCP RPC handler (`mcp/rpc.rs`) blocks on with `blocking_recv()`. `submit_ask_answer(tab_id, answers)` (a Tauri command) calls `submit_ask`, which takes the sender and sends the answers, waking the blocked handler so it returns its `tool_result` within the same CC subprocess turn. `chat_cancel` and `chat_reset_session` both call `cancel_ask(tab_id)` to drop any pending sender, so a blocked handler returns an error instead of hanging if the user cancels or resets before answering.
