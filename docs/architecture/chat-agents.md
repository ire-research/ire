# Chat & Agent Layer

Covers the ingestion pipelines, the chat system (send flow, experiment lifecycle, multi-tab), and the agent subprocess layer (binary discovery, spawn, JSONL parsers, session management).

---

## Pipelines

### Notes ingestion

```
User edits the notes pane (raw text)
  → blur or Ctrl+Enter after content changed
  → save_notes(content) Tauri command
    → Rust writes content to .ire/wiki/notes.md (atomic)
  → `workspace-event notes-changed { content }` is emitted
```

No agent turn is triggered. The selected agent reads `notes.md` only if the user explicitly requests it; it is never injected into the system prompt by default.

### Ideas ingestion

Ideas are stored directly in `wiki/ideas.json` through `read_ideas` / `save_ideas_json`. Clicking Add in `IdeasPane` opens an inline draft card; pressing Enter writes a new `{ id, text, trashed: false, order }` entry and reorders active ideas. Clicking the trash icon soft-deletes by setting `trashed: true`; trashed ideas remain in JSON but are hidden from the pane. Drag-to-reorder rewrites active `order` values. No agent turn is triggered.

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

**Confirm**: reads `.ire/cache/<resource_id>_draft.md`, normalizes its `sources:` frontmatter from the stored DB source refs, writes it to `resources/<slug>.md` through `WikiStore`, then removes the cache. Frontmatter: `title`, `sources`, `updated`, and `TL;DR`. Body starts with a `#` heading matching the title, then the summary. The written file is indexed in SQLite by matching every stored source ref in `sources`, but no git commit is created.

**Discard**: deletes `.ire/cache/<sha256>.txt` or `.ire/cache/<batch_sha>/`, marks the DB row `status=rejected`, closes the tab immediately. No wiki file is written.

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
        --mcp-config .ire/mcp.json
        --append-system-prompt "<composed per context injection rules>"
        --resume <session_id_if_any>
      codex exec --json
        -m <model>
        -c model_reasoning_effort=<low|medium|high|xhigh>
        -c developer_instructions="<composed per context injection rules>"
        -c mcp_servers.ire.command="node" -c mcp_servers.ire.args=[...]
        -C <workspace>
        --dangerously-bypass-approvals-and-sandbox
        -- "<message>"
      codex exec resume [OPTIONS] <thread_id> -- "<message>"
  → Rust parses JSONL line-by-line, emits chat-stream events
  → on `Init`: capture provider-scoped session/thread id
  → on `Result`/`Done`: turn complete, frontend re-enables input
```

**Auto-title (first message of a new chat tab):** When the user sends the first message in a brand-new chat tab whose label is still the default `"Chat"`, `ChatPane.handleSend` fires `generate_chat_title({ message, model, provider })` in the background. The model is the smallest of the selected provider (`claude-haiku-4-5-20251001` / `gpt-5.2`). The Rust command spawns a one-shot subprocess with **no** system prompt, MCP config, session resume, or effort, and returns a cleaned single-line title. On resolve the frontend calls `renameTab`, which types over the old label (40 ms/char typewriter). Any failure is silent — the label stays `"Chat"`.

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
       - updates DB row (status, exit_code, end_time)
       - reads tail of stdout/stderr (last N kB)
       - composes wake-up message and spawns the same provider with its
         resume id and that message
T6  The agent reads result files, calls wiki.write for any new findings,
    memory.write_short_term for daily notes, memory.write_long_term for
    durable conclusions, and pulse.update if the research question changed.
```

**Subtleties:**
- The user can keep using the chat pane during T3–T5. The next user message and the wake-up share the same provider-scoped session id; whichever arrives first runs first. We serialise agent turns: only one subprocess per session at a time. If a user message arrives while a wake-up is running, it queues; if a wake-up fires while the user is mid-turn, it queues. The pending-queue is shown in the UI ("1 wake-up pending").
- `process_group(0)` (Linux/macOS) ensures killing IRE doesn't kill running experiments. On Windows we use `CREATE_NEW_PROCESS_GROUP`.
- Logs are streamed to disk; the UI tails them via `experiment-log-line` events.

### Cancellation

- **User cancels agent turn**: kill the running subprocess; emit `chat-cancelled`. Session id is retained.
- **User resets session**: `chat_reset_session(tab_id)` clears the stored `session_id` for that tab. The frontend simultaneously clears the tab's message list. The next send starts a fresh CC session with no `--resume` flag.
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

**Session isolation.** Each tab carries its own `tab_id`. The backend `SessionManager` maintains a `HashMap<tab_id, PerTabSession>` where `PerTabSession` holds `{ session_id: Option<String>, session_provider: Option<String>, running_pid: Option<u32> }`.

**Event routing.** `chat-stream` events are wrapped as `{ tab_id, stream_id, event_id, event }` before being emitted. The frontend maintains a single global listener, routes each event to the correct tab's message list using `tab_id`, and ignores any already-seen `{tab_id, stream_id, event_id}` delivery.

**History persistence.** Chat tabs carry optional `historySessionUuid`, `historyStartedAt`, and `agentOptions` fields. The frontend creates the history UUID/start time on first send, persists those fields in `.ire/workspace.json`, and passes the UUID plus tab agent metadata to `chat_history_save` so every completed turn upserts the same `chat_sessions` row. The History menu filters out any row whose UUID is currently open as a chat tab.

**IPC changes from single-session baseline**

| Command / Event | Old signature | New signature |
|---|---|---|
| `chat_send` | `{ mode, message }` | `{ tab_id, message }` |
| `chat_cancel` | `{}` | `{ tab_id }` |
| `chat_reset_session` | `{}` | `{ tab_id }` |
| `chat-stream` event | `StreamEvent` | `{ tab_id, stream_id, event_id, event: StreamEvent }` |
| `tab-created` event | — | `{ tab_id, label, kind, resource_id?, agent_options? }` |

---

## Agent Subprocess Layer

Claude Code implements the patterns from [docs/blueprints/claude-code-wrapper.md](blueprints/claude-code-wrapper.md). Codex uses the same frontend event contract through a parallel backend module.

### Binary discovery (`binary`, `cc::discovery`, `codex::discovery`)

`find_claude_binary()` and `find_codex_binary()` share `binary::find_binary()`, which tries, in order:
1. `which <name>` on the current process PATH.
2. `$SHELL -lc "command -v <name>"` to load nvm/asdf/mise shims.
3. Provider-specific candidate paths.

Claude candidates include `.local/bin/claude`, `.claude/local/claude`, mise/asdf shims, npm locations, Homebrew locations, and all `$HOME/.nvm/versions/node/*/bin/claude`. Codex candidates include npm locations, Homebrew locations, all `$HOME/.nvm/versions/node/*/bin/codex`, and `%APPDATA%\npm\codex.cmd` on Windows.

Returns `Result<DiscoveredBinary, DiscoveryError>` with three error variants: `NotFound`, `NotExecutable`, `Io`.

### Spawn (`cc::spawn`, `codex::spawn`)

Claude Code spawn non-negotiables:
- `.env_remove("CLAUDECODE")` — prevent nested-session refusal.
- `.stdin(Stdio::null())` — don't hang waiting for stdin.
- `.current_dir(workspace_root)` — relative paths resolve correctly.

Always pair `--output-format stream-json` with `--verbose --include-partial-messages`.

Codex spawn uses `codex exec`, `--json`, `-m <model>`, `--dangerously-bypass-approvals-and-sandbox`, and `-c model_reasoning_effort=<low|medium|high|xhigh>`. Fresh turns pass `-C <workspace>`; resumed turns run with `Command::current_dir(workspace_root)` because `codex exec resume` does not accept `-C`. The prompt is passed after a `--` separator so messages beginning with `-` are not parsed as Codex CLI flags. `.ire/mcp.json` is translated into Codex config flags such as `-c mcp_servers.ire.command="node"`, `-c mcp_servers.ire.args=[...]`, and `-c mcp_servers.ire.env.IRE_WORKSPACE="..."`.

### JSONL parsers (`cc::stream`, `codex::stream`)

Reads stdout line-by-line on a `spawn_blocking` thread; deserialises each line into `serde_json::Value`; dispatches provider-specific JSONL into typed `StreamEvent`s emitted to the frontend on the `chat-stream` channel. Each emitted payload includes `stream_id = "{tab_id}:{stream_uuid}"` and a per-process monotonic `event_id`.

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

`ToolCall` is the provider-neutral tool-card contract defined in `tool_cards.rs`: `{ tool_id, provider, kind, raw_name, title, input, output, status, meta }`. `kind` is one of `command`, `file_read`, `file_write`, `file_edit`, `file_search`, `web_fetch`, `wiki_read`, `wiki_write`, `wiki_append`, `wiki_rename`, `memory_write`, `pulse_update`, `experiment_start`, `experiment_status`, `experiment_tail_logs`, or `other`.

Claude and Codex both normalize native tool records into `ToolCall` before emitting `ToolStart`. Claude maps built-ins such as `Bash`, `Read`, `Write`, `Edit`/`MultiEdit`, `Grep`/`Glob`/`LS`, and `WebFetch`; MCP names such as `ire__wiki.read` normalize through the `wiki.*`, `memory.*`, `pulse.update`, and `experiment.*` mapping.

`AskUserQuestion` is emitted when CC's built-in `AskUserQuestion` tool fires. The parser intercepts that `tool_use` block, parses its `questions[]` payload, and tracks the tool id so the matching `tool_result` is suppressed. The frontend renders an `AskQuestionCard` and, on submit, sends the formatted answers as the next chat turn via `chat_send`.

### Session management

Each chat tab has its own provider-scoped `session_id`, stored inside `SessionManager`:

```rust
struct PerTabSession {
    session_id: Option<String>,
    session_provider: Option<String>,
    model: Option<String>,
    effort: Option<String>,
    running_pid: Option<u32>,
}

pub struct SessionManager(Arc<Mutex<HashMap<String, PerTabSession>>>);
```

`session_id` is captured from the first `Init` event for a given `tab_id` and provider. Subsequent `chat_send` calls for that same tab and provider pass `--resume <session_id>` for Claude or `codex exec resume <thread_id>` for Codex. Switching providers in a tab starts a fresh provider session. `experiment.start` records the active `tab_id`, `session_id`, `session_provider`, `model`, and optional `effort` from the running turn before spawning the detached command; the monitor uses those values to fire the wake-up through the same provider and model settings.
