# Software Design Description: Integrated Research Environment (IRE)

The Integrated Research Environment (IRE) is a local desktop application that streamlines machine-learning research workflows. It wraps Claude-Code (CC) inside a Tauri desktop app and gives it well-organised research context — literature, experiment logs, project state — by maintaining a persistent **LLM Wiki** on disk. IRE runs entirely locally and is project-centric: each workspace maps one-to-one to a directory containing both the user's source code and an `.ire/` data directory.

This document describes the MVP architecture in enough detail to implement. The MVP feature non-goals live in [SCOPE.md](./SCOPE.md).

---

## Table of Contents

1. [Problem & Users](#1-problem--users)
2. [System Overview](#2-system-overview)
3. [Tech Stack](#3-tech-stack)
4. [Directory Layout](#4-directory-layout)
5. [Workspace Lifecycle](#5-workspace-lifecycle)
6. [Wiki Layer](#6-wiki-layer)
7. [Memory Layer](#7-memory-layer)
8. [Pipelines](#8-pipelines)
9. [Chat](#9-chat)
10. [Claude-Code Subprocess Layer](#10-claude-code-subprocess-layer)
11. [MCP Server](#11-mcp-server)
12. [SQLite Schema](#12-sqlite-schema)
13. [Frontend](#13-frontend)
14. [Tauri IPC Surface](#14-tauri-ipc-surface)
15. [Concurrency & Data Safety](#15-concurrency--data-safety)
16. [Source Tree Layout](#16-source-tree-layout)
17. [Implementation Phases](#17-implementation-phases)
18. [Testing Strategy](#18-testing-strategy)
19. [Open Items & Risks](#19-open-items--risks)

---

## 1. Problem & Users

ML research workflows are fragmented across IDEs, reference managers, and AI interfaces. The cost is concrete:

1. **Context loss** — switching to CC requires re-establishing project state, literature, and past decisions every session.
2. **Knowledge fragmentation** — no persistent indexed memory of insights, paper summaries, or rejected methodologies, so AI suggestions are redundant or repeat dead ends.
3. **Goal drift** — the primary objective gets buried under technical work or literature exploration.
4. **Siloed knowledge** — meeting notes, papers, experiment logs, and code state are not unified.

**Target user.** Academic / industrial ML researcher. Python-heavy, comfortable with Git and the terminal, uses LaTeX. Authenticates Claude-Code externally.

**Two pain points from the user research that drive design** (from [VITTO.md](./VITTO.md)):
- Models keep proposing methods that were already tried and rejected → IRE must record rejections as **structured, prominently re-injected** state, not buried prose.
- Models forget about long-running experiments while the user is doing something else → IRE must **wake CC up** when an experiment completes, with the right context attached.

---

## 2. System Overview

```
┌──────────────────────────────────────────────────────────────────────┐
│ Tauri WebView (React)                                                │
│   five-pane layout · streaming chat · markdown edit/preview          │
│   central column: chat tabs + resource preview tabs                  │
└──────────────────────────────────────────────────────────────────────┘
                       ▲ invoke / events
                       │
┌──────────────────────────────────────────────────────────────────────┐
│ Rust backend (Tauri)                                                 │
│   ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐   │
│   │ Workspace    │  │ Wiki         │  │ CC subprocess manager    │   │
│   │ + .lock      │  │ (atomic I/O) │  │ (NDJSON parser, --resume)│   │
│   └──────────────┘  └──────────────┘  └──────────────────────────┘   │
│   ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐   │
│   │ Resource     │  │ SQLite       │  │ Experiment monitor       │   │
│   │ fetcher      │  │ (rusqlite)   │  │ (detached subprocesses)  │   │
│   └──────────────┘  └──────────────┘  └──────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────┘
                       ▲ stdio / NDJSON          ▲ stdio (MCP JSON-RPC)
                       │                         │
              ┌────────────────┐         ┌────────────────────────┐
              │ Claude-Code    │ ◀─────▶ │ IRE MCP server         │
              │ CLI subprocess │  tools  │ (Node, stdio transport)│
              └────────────────┘         └────────────────────────┘
                       │
                       ▼ Bash tool / MCP experiment.start
              ┌────────────────────────────┐
              │ Detached experiment proc.  │
              │ (user code, e.g. python)   │
              └────────────────────────────┘
```

**Key points.**
- CC is a headless subprocess. IRE is a thin IPC bridge: messages in, typed events out.
- The MCP server is the *only* high-level surface CC uses to interact with IRE state. Plain filesystem tools (`Read`, `Edit`, etc.) are also enabled, but wiki / memory / experiment work goes through MCP for structure.
- Experiments run as **detached** child processes spawned by the MCP server. CC's turn ends; IRE wakes CC up via `--resume` when the experiment finishes.

---

## 3. Tech Stack

| Layer | Choice | Notes |
|---|---|---|
| App framework | [Tauri 2](https://v2.tauri.app/) | Cross-platform shell, Rust backend. |
| Frontend | React 18 + TypeScript + Vite | Already scaffolded. |
| State | Zustand (or React context) | Light, no Redux ceremony. Decision deferred to first frontend PR. |
| Markdown | `react-markdown` + `remark-gfm` for preview; `<textarea>` for edit | Toggle-based, not split. |
| Layout | `react-resizable-panels` | Resizable + collapsible splits. |
| Persistence | SQLite via `rusqlite` | Single DB file at `.ire/wiki/local.db`. |
| MCP server | Node + `@modelcontextprotocol/sdk` | Stdio transport. Bundled with the app. |
| PDF extract | `pdf-extract` crate | Pure Rust, no system deps. |
| HTML extract | `reqwest` + `scraper` + `readability` (custom or `readability-rs`) | Strip nav/ads; keep article text. |
| Filesystem | `std::fs` + `tempfile` for atomic writes | No `notify` watcher in MVP — wiki changes are mediated through IRE. |
| Logging | `tracing` + `tracing-subscriber` | Console logging only; experiment stdout/stderr live beside each experiment plan. |

CC is invoked as an external CLI binary; not a dependency in `Cargo.toml` or `package.json`. The Node MCP server's runtime (`node`) is also assumed installed (CC requires it anyway).

---

## 4. Directory Layout

### Workspace (per project)

```
my_research_project/
├── .git/
├── .gitignore                       # IRE adds: .ire/wiki/local.db, .ire/wiki/experiments/*/*.log, .ire/.lock, .ire/workspace.json, .ire/cache/
├── .ire/
│   ├── .lock                        # PID of running IRE instance (gitignored)
│   ├── _SYSTEM.md                   # IRE framework context + wiki schema, injected first into every CC turn
│   ├── workspace.json               # per-workspace UI layout (panel sizes); gitignored
│   ├── cache/                       # raw extracted resource text (gitignored)
│   └── wiki/                        # ALL TRACKED IN GIT
│       ├── local.db                 # SQLite (gitignored)
│       ├── _index.md                # Master index (path → one-line summary)
│       ├── notes.md                 # User notes
│       ├── ideas.json               # User ideas (`{ id, text, trashed, order }`)
│       ├── pulse.json               # Current research question and weekly focus
│       ├── long-term.md             # Agent-written architectural decisions and durable dead ends
│       ├── short-term/
│       │   └── YYYY-MM-DD.md        # Daily agent notes
│       ├── resources/
│       │   └── <slug>.md            # One file per ingested paper/article
│       └── experiments/
│           └── <experiment_uuid>/
│               ├── plan.md
│               ├── stdout.log       # gitignored
│               └── stderr.log       # gitignored
└── ... user source code ...
```

**Gitignore additions** appended on workspace init:
```
.ire/.lock
.ire/wiki/local.db
.ire/workspace.json
.ire/wiki/experiments/*/*.log
.ire/cache/
```

`wiki/` is intentionally **not** gitignored — it is the durable knowledge artefact and benefits from version history.

### User config (global, cross-project)

```
~/.config/ire/           # $XDG_CONFIG_HOME/ire/ if set, else $HOME/.config/ire/
└── config.json          # user preferences + recent workspaces
```

`config.json` schema:
```json
{
  "theme": "dark",
  "recent_workspaces": [
    "/home/user/projects/my_project",
    "/home/user/projects/other_project"
  ]
}
```

This file is managed exclusively by IRE. It is read once at app startup (before any workspace is opened) and written on theme change and whenever a workspace is opened. `recent_workspaces` is kept ordered newest-first, capped at 10 entries, and pruned on read so missing directories are not shown in the setup screen.

### App source tree

See [§16](#16-source-tree-layout).

---

## 5. Workspace Lifecycle

### 5.1 Onboarding (first launch / no recent workspace)

```
┌─ Setup screen ───────────────────────────────────────┐
│  "Open or create a workspace."                       │
│                                                      │
│  Recent workspaces (up to 5)                         │
│    • each entry shows project name + full path       │
│    • click any entry to open without a file dialog   │
│    • hover an entry to reveal a remove button        │
│    • most-recently-opened is highlighted             │
│                                                      │
│  [Open folder…]       [New workspace…]               │
│                                                      │
│  ● claude-code · authenticated  (or: not found)      │
│    retry button if binary missing                    │
└──────────────────────────────────────────────────────┘
```

On startup, `App.tsx` calls `setup_status` and `read_user_config` in parallel.  `read_user_config` removes recent workspace paths that no longer exist, persists the cleaned config, and hydrates `recentWorkspaces` in the Zustand store before the setup screen mounts so the list is immediately populated.  If the binary is missing, a `retry` link re-invokes `refreshSetup`; there is no step-by-step wizard — the binary status is a status-bar indicator, not a blocking step.

### 5.2 Open existing

1. User picks directory via Tauri's file dialog.
2. Backend validates: directory exists, is a git repo, and contains `.ire/_SYSTEM.md` plus `.ire/wiki/pulse.json` (the marker files).
3. Acquire `.ire/.lock`:
   - If absent: write current PID, continue.
   - If present and PID alive: refuse, show "already open in another window".
   - If present and PID dead: reclaim (overwrite with current PID).
4. Initialise SQLite (run pending migrations).
5. Load `workspace.json` if present (restores pane layout + last CC session id).
6. Spawn the MCP server subprocess (long-lived, lives as long as the workspace is open).
7. Emit `workspace-ready` event to the frontend.

### 5.3 Initialize new

1. User picks an empty directory (or one without `.ire/`).
2. Backend:
   - `git init` if no `.git/`.
   - Scaffold `.ire/` per [§4](#4-directory-layout).
   - Write seed files: `.ire/_SYSTEM.md` (canned framework context + schema), empty `notes.md`, empty `ideas.json`, `pulse.json`, `long-term.md`, `short-term/`, `resources/`, `experiments/`, and `_index.md` (auto-built from the seed).
   - Append IRE entries to `.gitignore` (create if missing).
   - Do not stage or commit; the user decides when to commit the initialized workspace.
3. Continue from step 3 of [§5.2](#52-open-existing).

### 5.4 Close

- Stop the CC subprocess (kill on SIGTERM, escalate to SIGKILL after grace).
- Stop the MCP server subprocess.
- Persist `workspace.json` (current layout + session id).
- Release `.ire/.lock`.

---

## 6. Wiki Layer

### 6.1 Conventions (encoded in `.ire/_SYSTEM.md`)

- **Path = identity.** Every wiki page has a stable path; renames go through `wiki.rename`.
- **Frontmatter is optional but preferred** for structured pages. Minimum:
  ```yaml
  ---
  title: <human title>
  type: summary | entity | concept | comparison | meta
  sources: [path/to/raw/source.pdf, ...]   # optional
  updated: 2026-05-03
  ---
  ```
- **`_index.md` is canonical.** It is auto-regenerated on every wiki write. CC must consult it to navigate; it must not edit it directly.
- **`pulse.json` is the focus file.** It contains exactly `research_question` and `this_week`.

### 6.2 Atomic write contract

All wiki mutations go through `WikiStore` (Rust) which holds the in-process `tokio::Mutex<()>` for the wiki. Per write:

1. Acquire mutex.
2. Write content to `<path>.<uuid>.tmp` in the same directory (`O_CREAT|O_EXCL`).
3. `sync_all()` the temp file.
4. `fs::rename(tmp, final)` — atomic on local FS.
5. Re-derive `_index.md` from a directory walk (cheap; <1k files in MVP) and atomic-rename it.
6. Emit `wiki-changed { path }` event.
7. Release mutex.

No CAS, no advisory file lock, no WAL — single-instance is enforced by `.lock` (see [§15](#15-concurrency--data-safety)).

### 6.3 Index regeneration

`_index.md` is a flat markdown list:
```
- [notes.md](./notes.md) — running user notes
- [ideas](./ideas.json) — user ideas list
- [pulse](./pulse.json) — current research question and weekly focus
- [long-term](./long-term.md) — architectural decisions and durable insights
- [resources/attention-is-all-you-need.md](./resources/attention-is-all-you-need.md) — Vaswani et al. (2017): self-attention transformer …
```

The one-line summary is sourced from frontmatter `summary:` if present, else the first non-heading paragraph truncated to 160 chars.

### 6.4 Git commit policy

IRE never creates git commits automatically. The application may initialize a git repository for a new workspace, write `.gitignore`, and write files under `.ire/`, but deciding when to stage and commit remains entirely with the user.

`WikiStore::write` and `WikiStore::rename` atomically update the target wiki path, regenerate `_index.md`, and emit `wiki-changed`. They do not run `git add` or `git commit`, regardless of path. This applies equally to `pulse.json`, `long-term.md`, `short-term/**`, `_index.md`, `notes.md`, `ideas.json`, `resources/**`, and `experiments/**`.

Resource approval follows the same rule: **Confirm** asks CC to write `resources/<slug>.md` via `wiki.write`; `index_resource` then records the DB row (`status=summarized`, `wiki_path`, `title`) and emits `wiki-changed`. The resulting wiki and index changes remain uncommitted until the user commits them.

---

## 7. Memory Layer

Memory lives at the root of `wiki/`. These files are agent-written only; the user does not edit them through the UI.

### 7.1 `long-term.md`

CC writes architectural decisions, pivots, "this approach is the one we settled on" claims here, via the MCP `memory.write_long_term` tool. Always injected into CC's system prompt context (whole file).

### 7.2 `short-term/YYYY-MM-DD.md`

CC writes daily operational notes here via `memory.write_short_term`. Only the **last two** day-files (today + yesterday) are auto-injected. Older files remain on disk for git history but are not in CC's prompt unless explicitly read.

CC is told in the system prompt:
> Use `memory.write_short_term` for detailed information about the current experiment, specific debugging steps, and daily operations. After two days these notes are no longer auto-injected — promote anything still relevant to long-term memory.
>
> Use `memory.write_long_term` for overarching architectural decisions, pivots, abandonment of approaches, and durable insights.

### 7.4 Context injection rules

When IRE spawns a CC turn, the system prompt is composed of:

1. `.ire/_SYSTEM.md` — static IRE framework context, wiki layout, behavioral rules, and schema. MCP tool descriptions are received automatically via `tools/list` and are not duplicated here. Seeded from `assets/seed/_SYSTEM.md` on workspace init; always injected first.
2. All CC-facing prompt text — the resource summarizer role, the resource-confirm follow-up, and the experiment wake-up template — lives in `src-tauri/assets/prompts/`. Edit those files to change CC's behaviour; never hardcode prompts at call sites. The experiment workflow instructions are part of `.ire/_SYSTEM.md` (point 1 above).
3. `wiki/_index.md` (catalog).
4. `wiki/pulse.json`.
5. `wiki/long-term.md` (full).
6. The two most recent `short-term/YYYY-MM-DD.md` files.

This is added via `--append-system-prompt`. Wiki/notes/resources are read on-demand by CC through MCP tools; they are not pre-injected.

---

## 8. Pipelines

### 8.1 Notes ingestion

```
User edits the notes pane (raw text)
  → blur or Ctrl+Enter after content changed
  → save_notes(content) Tauri command
    → Rust writes content to .ire/wiki/notes.md (atomic)
  → wiki-changed event causes the notes pane to re-render.
```

No CC turn is triggered. CC reads `notes.md` only if the user explicitly requests it, or if the context warrants it; it is never injected into the system prompt by default.

### 8.2 Ideas ingestion

Ideas are stored directly in `wiki/ideas.json` through `read_ideas` / `save_ideas_json`. Clicking Add in `IdeasPane` opens an inline draft card; pressing Enter writes a new `{ id, text, trashed: false, order }` entry and reorders active ideas. Clicking the trash icon soft-deletes by setting `trashed: true`; trashed ideas remain in JSON but are hidden from the pane. Drag-to-reorder rewrites active `order` values. No CC turn is triggered.

### 8.3 Resource ingestion

```
User queues one or more URLs/files → Ingest
  → submit_resources(sources) Tauri command
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
         'local_file', or 'batch')
      8. open a new resource chat tab (see §9.4), labelled by URL hostname, filename,
         or "<N> sources"
      9. kick a CC turn in that tab with prompt:
           "Read <cache file(s)> (source: <source ref(s)>). Provide one comprehensive
            executive summary — what the material is, what is relevant to this project,
            why it matters, and how it could be used. Use bullet points.
            Do NOT write to the wiki yet."
```

CC streams the summary into the resource tab. When the turn ends, **Confirm** and **Discard** buttons appear.

**Confirm**: triggers a second CC turn in the same tab with the instruction to write the summary to `resources/<slug>.md` via the `wiki.write` MCP tool. Frontmatter follows the schema in `.ire/_SYSTEM.md`: `title`, `type: summary`, `sources: [<all original sources in order>]`, `updated: YYYY-MM-DD`, and `summary`. URL sources use the original URL; local file sources use `file:<sha256>:<filename>`. Body starts with a `#` heading matching the title, then the summary. The tab auto-closes when this second CC turn ends. The written file is indexed in SQLite by matching every stored source ref in `sources`, but no git commit is created by IRE.

**Discard**: deletes `.ire/cache/<sha256>.txt` or `.ire/cache/<batch_sha>/`, marks the DB row `status=rejected`, closes the tab immediately. No wiki file is written.

---

## 9. Chat

IRE uses a **single unified agent** in the central pane. The agent always has access to all MCP tools (`wiki.*`, `memory.*`, `pulse.*`, `experiment.*`) and all CC built-ins (`Bash`, `Edit`, `Write`, `Read`, `Grep`, `Glob`, `WebFetch`). The experiment workflow instructions are part of `.ire/_SYSTEM.md` (injected on every turn per §7.4).

### 9.1 Send message flow

```
User types in central pane → Send
  → chat_send({ message }) Tauri command
  → Rust spawns:
      claude -p "<message>"
        --output-format stream-json --verbose --include-partial-messages
        --mcp-config .ire/mcp.json --strict-mcp-config
        --append-system-prompt "<composed per §7.4>"
        --resume <session_id_if_any>
  → Rust parses NDJSON line-by-line, emits chat-stream events
  → on `system.init`: capture session_id, persist to workspace.json
  → on `result`: turn complete, frontend re-enables input
```

### 9.2 Experiment lifecycle (the wake-up pattern)

This is the core of [Q1's answer](./SCOPE.md#mvp): CC must not hang on the experiment.

```
T0  User asks: "Run an ablation over learning rates [1e-3, 1e-4, 1e-5]."
T1  CC plans, gets agreement, then calls MCP tool experiment.start({
       name: "lr-ablation",
       plan_md: "<full plan as markdown>",
       command: "python scripts/ablate_lr.py --output runs/lr_ablation",
       working_dir: "<project root>",
       wake_prompt: "Experiment lr-ablation finished. Read its result.md and
                     logs, then update the wiki and pulse."
    })
T2  MCP server forwards to IRE Rust backend over its private channel:
       - inserts experiment row (status=running, uuid, start_time, command, …)
       - writes plan_md to .ire/wiki/experiments/<uuid>/plan.md
       - spawns the command as a DETACHED process group:
           Command::new("sh").args(["-c", &command])
                 .current_dir(working_dir)
                 .stdin(Stdio::null())
                 .stdout(file .ire/wiki/experiments/<uuid>/stdout.log)
                 .stderr(file .ire/wiki/experiments/<uuid>/stderr.log)
                 .process_group(0)             // setsid
                 .env_remove("CLAUDECODE")
                 .spawn()
       - returns { uuid, status: "started" } to CC
T3  CC's response to the user: "Started experiment <uuid>; I'll come back
    when it's done." Then this CC turn ENDS naturally.
T4  Backend monitor task waits on the child PID (off-thread). Frontend
    receives experiment-status events (started, log lines, …) and renders
    a live tail.
T5  Process exits. Backend:
       - updates DB row (status, exit_code, end_time)
       - reads tail of stdout/stderr (last N kB)
       - composes wake-up message:
           "<wake_prompt>\n\nExperiment uuid: <uuid>\nExit code: <n>\n
            Plan: .ire/wiki/experiments/<uuid>/plan.md\n
            stdout tail: <…>\nstderr tail: <…>"
       - spawns a CC turn with --resume <session_id> and that message
T6  CC reads result files, calls wiki.write for any new findings,
    memory.write_short_term for daily notes and transient dead ends,
    memory.write_long_term for durable conclusions, and pulse.update if
    the research question or weekly focus changed.
```

**Subtleties.**
- The user can keep using the chat pane during T3–T5. The next user message and the wake-up share the same session id; whichever arrives first runs first. We serialise CC turns: only one CC subprocess per session at a time. If a user message arrives while a wake-up is running, it queues; if a wake-up fires while the user is mid-turn, it queues. The pending-queue is shown in the UI ("1 wake-up pending").
- `process_group(0)` (Linux/macOS) ensures killing IRE doesn't kill running experiments. On Windows we use `CREATE_NEW_PROCESS_GROUP`.
- Logs are streamed to disk; the UI tails them via `experiment-log-line` events.

### 9.3 Cancellation

- **User cancels CC turn**: kill the CC subprocess; emit `chat-cancelled`. Session id is retained, so the next message can `--resume`.
- **User resets session**: `chat_reset_session(tab_id)` clears the stored `session_id` for that tab. The frontend simultaneously clears the tab's message list. The next send starts a fresh CC session with no `--resume` flag.
- **User cancels experiment**: SIGTERM the process group; on next monitor tick, mark `status=cancelled` and fire the wake-up with that fact.

### 9.4 Multi-tab chat

IRE supports multiple independent chat tabs in the central pane.

**Tab types**

| Type | Created by | Closeable | Description |
|---|---|---|---|
| Main | On workspace open (id `"main"`) | No (pinned) | The primary research conversation. |
| Chat | User clicks + button | Yes | Fresh CC session, independent conversation history. |
| Resource | Backend resource ingestion (§8.3) | Auto-closes on Confirm/Discard | Dedicated to reviewing a single resource summary; shows Confirm / Discard instead of a free-form Composer when CC finishes. |
| Preview | User clicks a resource in the Resources list | Yes | Renders a `ResourcePreviewPane` (edit/preview toggle + Submit) for the resource's wiki file. Clicking the same resource again focuses the existing tab rather than opening a duplicate. Submit persists edits to disk via `save_wiki_file`; the user commits when ready. |
| Experiment | User clicks an experiment in the left-rail ExperimentsSection | Yes | Dedicated full view of a single experiment: metadata grid (status, runtime, command), live log tail (stdout only, scrolls to bottom automatically). Clicking the same experiment again focuses the existing tab rather than opening a duplicate. |

**Session isolation.** Each tab carries its own `tab_id` (UUID for dynamically created tabs; `"main"` for the pinned tab). The backend `SessionManager` maintains a `HashMap<tab_id, PerTabSession>` where `PerTabSession` holds `{ session_id: Option<String>, running_pid: Option<u32> }`. This replaces the old single `ChatSession` global.

**Event routing.** `chat-stream` events are wrapped as `{ tab_id, event }` before being emitted to the frontend. The frontend maintains a single global listener that routes each event to the correct tab's message list using the `tab_id` field. A `tab-created` event (payload: `{ tab_id, label, kind, resource_id? }`) is emitted by the backend whenever a new tab is opened programmatically (e.g. during resource ingestion). Preview tabs are created client-side only (no `tab-created` event) — the store's `openPreviewTab` action handles deduplication and activation.

**User-initiated turn (any tab)**

```
User types → handleSend(tabId)
  → beginAssistantMessage(tabId)
  → ipc.chatSend(tabId, text)
  → Rust: resume CC for tabId with --resume <session_id>
  → events emitted as { tab_id, event }
  → frontend routes to tabId's messages
  → Done → finishMessage(tabId)
```

**Backend-initiated turn (resource tab)**

```
submit_resource() kicks CC with a new tab_id
  → emits tab-created { tab_id, label, kind: "resource" }
  → frontend adds resource tab, switches to it
  → CC emits { tab_id, Init } → frontend begins assistant message
  → CC streams summary → Done → resourceStatus = "ready"
  → Confirm / Discard buttons appear in the tab
```

**IPC changes from single-session baseline**

| Command / Event | Old signature | New signature |
|---|---|---|
| `chat_send` | `{ mode, message }` | `{ tab_id, message }` |
| `chat_cancel` | `{}` | `{ tab_id }` |
| `chat_reset_session` | `{}` | `{ tab_id }` |
| `chat-stream` event | `StreamEvent` | `{ tab_id, event: StreamEvent }` |
| `tab-created` event | — | `{ tab_id, label, kind, resource_id? }` |

---

## 10. Claude-Code Subprocess Layer

Implements the patterns from [docs/blueprints/claude-code-wrapper.md](./blueprints/claude-code-wrapper.md). Concretely:

### 10.1 Binary discovery (`cc::discovery`)

`find_claude_binary()` tries, in order:
1. `which claude` on the current process PATH.
2. `$SHELL -lc "command -v claude"` to load nvm/asdf/mise shims.
3. A canned candidate-paths list (see blueprint §1).

Returns `Result<PathBuf, DiscoveryError>` with three error variants: `NotFound`, `NotExecutable`, `IoError`. The setup screen consumes this.

### 10.2 Spawn (`cc::spawn`)

Non-negotiables:
- `.env_remove("CLAUDECODE")` — prevent nested-session refusal.
- `.stdin(Stdio::null())` — don't hang waiting for stdin.
- `.current_dir(workspace_root)` — relative paths resolve correctly.
- `.env("PATH", augmented_path(...))` — ensure node/git/python are visible.

Always pair `--output-format stream-json` with `--verbose --include-partial-messages`.

### 10.3 NDJSON parser (`cc::stream`)

Reads stdout line-by-line on a `spawn_blocking` thread; deserialises each line into `serde_json::Value`; dispatches to typed `StreamEvent`s emitted to the frontend on the `chat-stream` channel:

```rust
#[serde(tag = "kind")]
enum StreamEvent {
    Init { session_id: String },
    TextDelta { text: String },
    ThinkingDelta { text: String },
    ToolStart { tool_id: String, tool_name: String, input_preview: Option<String>, input_full: Option<String> },
    ToolDone { tool_id: String, output_preview: Option<String>, output_full: Option<String> },
    AskUserQuestion { tool_id: String, questions: Vec<AskQuestion> },
    Result { text: Option<String>, session_id: String },
    Error { message: String },
    Done,
}
```

Deduplicate `Result.text` against streamed `TextDelta`s using an `emitted_text: bool` flag (blueprint §3).

`AskUserQuestion` is emitted when CC's built-in `AskUserQuestion` tool fires. The parser
intercepts that `tool_use` block, parses its `questions[]` payload
(`{ header, question, multi_select, options: [{ label, description? }] }`), and tracks the
tool id so the matching `tool_result` is suppressed (it would otherwise render as a generic
ToolCard). The frontend renders an `AskQuestionCard` in the assistant bubble (see
§13.2) and, on submit, sends the formatted answers as the next chat turn via `chat_send` —
CC resumes the session and continues from there.

### 10.4 Session management

Each chat tab (see §9.4) has its own `session_id`, stored inside `SessionManager`:

```rust
// cc/session.rs
struct PerTabSession { session_id: Option<String>, running_pid: Option<u32> }
pub struct SessionManager(Arc<Mutex<HashMap<String, PerTabSession>>>);
```

`session_id` is captured from the first `Init` event for a given `tab_id` and stored in the map. Subsequent `chat_send` calls for that tab pass `--resume <session_id>`. Reset clears the `session_id` entry for the tab; the next send starts a fresh session with no `--resume` flag.

---

## 11. MCP Server

A Node.js stdio MCP server bundled at `mcp/server.js`. Spawned by Tauri at workspace open and torn down on close. CC connects to it via `--mcp-config .ire/mcp.json` (generated at workspace open):

```json
{
  "mcpServers": {
    "ire": {
      "command": "node",
      "args": ["<bundled>/mcp/server.js"],
      "env": {
        "IRE_WORKSPACE": "<absolute-path>",
        "IRE_BACKEND_SOCKET": "<unix-socket-path-or-tcp>"
      }
    }
  }
}
```

The MCP server is a **thin RPC bridge** to the Rust backend over a Unix domain socket (Windows: TCP on 127.0.0.1 with auth token). All real work — atomic writes, DB inserts, subprocess spawning — happens in Rust. The MCP server validates inputs against JSON schemas and forwards.

### 11.1 Tool catalog (MVP)

| Tool | Description |
|---|---|
| `wiki.read({ path })` | Read any wiki markdown or JSON file. Returns content + frontmatter for markdown. |
| `wiki.write({ path, content, summary? })` | Atomic write; updates `_index.md`; emits `wiki-changed`. Does not commit. |
| `wiki.append({ path, content })` | Append content to a wiki file. Same persistence semantics as `wiki.write`. |
| `wiki.list({ glob? })` | List wiki paths; defaults to all. |
| `wiki.rename({ from, to })` | Atomic rename + index update; emits `wiki-changed`. Does not commit. |
| `memory.write_long_term({ section, content })` | Append to `long-term.md` under section. Does not commit. |
| `memory.write_short_term({ content })` | Append to today's `short-term/YYYY-MM-DD.md`. Does not commit. |
| `pulse.update({ research_question?, this_week? })` | Patch `pulse.json`. Does not commit. |
| `resource.fetch({ url })` | Fetch URL, extract text, return it (does not save to wiki). |
| `experiment.start({ name, plan_md, command, working_dir?, wake_prompt })` | Spawn detached subprocess, return `{ uuid }`. |
| `experiment.status({ uuid })` | Return `{ status, exit_code?, started_at, ended_at? }`. |
| `experiment.list({ limit? })` | Recent experiments. |
| `experiment.tail_logs({ uuid, kb? })` | Tail of stdout/stderr. |

All tools return JSON. Errors are surfaced to CC as MCP error responses, which CC interprets as tool failures and reports in chat.

### 11.2 Backend RPC channel

The Node MCP server speaks line-delimited JSON over the socket:
```
→ { "id": 1, "method": "wiki.write", "params": { "path": "...", "content": "..." } }
← { "id": 1, "ok": true, "result": {} }
```
This is a private internal protocol; not part of any spec. It exists only because the MCP SDK is Node-only and we want all I/O to happen in Rust for atomicity.

---

## 12. SQLite Schema

Single file at `.ire/wiki/local.db`. Migrations applied at workspace open (idempotent).

```sql
CREATE TABLE schema_migrations (
  version INTEGER PRIMARY KEY,
  applied_at TEXT NOT NULL
);

CREATE TABLE experiments (
  uuid TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  command TEXT NOT NULL,
  working_dir TEXT NOT NULL,
  status TEXT NOT NULL,             -- running | completed | failed | cancelled
  exit_code INTEGER,
  started_at TEXT NOT NULL,
  ended_at TEXT,
  pid INTEGER,
  wake_prompt TEXT,
  session_id TEXT NOT NULL          -- CC session to wake up on completion
);

CREATE INDEX idx_experiments_status ON experiments(status);
CREATE INDEX idx_experiments_started ON experiments(started_at DESC);

CREATE TABLE resources (
  url_sha256 TEXT PRIMARY KEY,
  url TEXT NOT NULL,                 -- URL, file:<sha256>:<filename>, or JSON array of source refs for batches
  source_type TEXT NOT NULL DEFAULT 'url',
  source_label TEXT,                 -- display label, e.g. URL, filename, or "<N> sources"
  title TEXT,
  status TEXT NOT NULL,             -- pending_fetch | pending_summary | summarized | failed
  content_type TEXT,
  wiki_path TEXT,                   -- e.g. resources/<slug>.md once summarized
  fetched_at TEXT,
  summarized_at TEXT,
  error TEXT
);

CREATE INDEX idx_resources_status ON resources(status);
```

No table for chat messages — the CC session is the source of truth, and `--resume` rehydrates it.

---

## 13. Frontend

### 13.1 Layout

The Tauri window opens in windowed mode at 1280 × 820 so the primary rails, center tab area, navbar, and status bar are visible on launch.

```
┌──────────────────────────────────────────────────────────────────────┐
│ Navbar (h-10):  full workspace path [running N exp]     [close] [⚙] │
├──────────────────┬──────────────────────────────┬────────────────────┤
│ Left rail 280px  │   ChatPane (flex-1)           │ Right rail 320px   │
│  FocusPane       │   - tab bar                  │  NotesPane         │
│  - Research Q.   │   - message list              │  - inline edit     │
│  - This Week     │   - composer                 │  IdeasPane         │
│  ResourcesSection│   - experiment tab view      │  - draggable cards │
│  ExperimentsSection   (kind="experiment")        │  AddResourceSection│
│  - experiment list                               │  - URL + file input│
└──────────────────┴──────────────────────────────┴────────────────────┘
│ StatusBar: full workspace path + git diff · CPU · GPU · RAM · host   │
└──────────────────────────────────────────────────────────────────────┘
```

- The body uses `react-resizable-panels` group `body` with panels `left`, `center`, and `right`. Left/right default to 280px/320px, have no maximum width, and keep minimum widths of 160px / 180px. The center panel takes the remaining space and has a 320px minimum.
- The left rail is a vertical `react-resizable-panels` group `left` with panels `pulse`, `resources`, and `experiments`; row handles sit between Focus / Resources and Resources / Experiments.
- The right rail is a vertical `react-resizable-panels` group `right` with panels `notes`, `ideas`, and `resource-input`; row handles sit between Notes / Ideas and Ideas / Add resources.
- `ResourcesSection` and `ExperimentsSection` use the same outer pane padding as `NotesPane` and `IdeasPane`, and render compact inline SVG title icons. Empty lists show `no resources yet` and `no experiments yet`.
- `IdeasPane` renders active ideas sorted by `order`, opens an inline draft card on Add, saves the draft to `ideas.json` on Enter, and hides trashed ideas by persisting `trashed: true`.
- `FocusPane` and `NotesPane` use **inline editing**: clicking a field activates a textarea in place; blur/Enter saves. No separate Edit/Preview toggle. `NotesPane` renders saved `notes.md` content as markdown in display mode rather than forcing each line into a bullet item.
- Clicking an experiment row in `ExperimentsSection` opens (or re-focuses) an `experiment` tab in the centre column; the tab renders `ExperimentTabView` with a metadata grid and live log tail. Hovering an experiment row reveals an `edit_document` rename button between the experiment name and status pill; pressing Enter commits the inline rename through `experiment_rename`, while Escape or blur cancels. `ExperimentTabView` uses the same hover-revealed `edit_document` rename button beside the header name.
- The top navbar shows the full workspace path on the far left as the project title. The bottom `StatusBar` polls `get_system_status` every 5 s and displays (left-to-right): workspace path + git branch + insertions/deletions, CPU model + usage %, GPU model + usage % + VRAM (or `n/a` when unavailable), RAM total GB, `username@hostname`, and a `claude-code · connected/disconnected` indicator pushed to the far right. The CC indicator is `connected` when `find_claude_binary()` succeeds, meaning the CLI is available for interaction; it does not require an active subprocess.

### 13.2 Chat rendering

**Tab bar.** The chat pane opens with a standard Tabbed Document Interface (TDI) bar at the top — the same pattern as Chrome or the VS Code editor. Each tab is a horizontal button in a single row:

- The active tab uses `bg-surface-container-highest` and a 1 px top border in the `primary` colour token (light silver), visually merging with the content area below. The tab bar background is `bg-surface-container-low`.
- Inactive tabs use `text-on-surface-variant` with no background fill. Hovering applies `bg-surface-container-highest` and `text-on-surface`.
- The close button (×) is hidden until the tab row is **hovered** (`group-hover`). It does not appear simply because the tab is active. Pinned tabs (Main) have no close button.
- A `+` button (Material Symbol `add`) at the right end of the bar opens a new chat tab.
- If there are more tabs than fit the bar width the row scrolls horizontally (scrollbar hidden via `.no-scrollbar`).
- Each tab shows a Material Symbol icon left of the label: `chat` for chat tabs, `description` for resource/preview tabs, `science` for experiment tabs. A resource tab that is actively being summarised shows a `progress_activity` icon instead, with `animate-spin`.

**Messages.** Assistant output is stored and rendered as ordered content blocks inside the latest assistant bubble. Text deltas, thinking deltas, tool cards, and experiment cards appear in the same chronological order as their `chat-stream` events; consecutive deltas of the same block type merge into the current block.
- Both user text and assistant text blocks are rendered through `MessageMarkdown` (`react-markdown` + `remark-gfm` + `remark-math` + `rehype-katex`). GitHub-flavoured markdown — tables, fenced code, task lists — and LaTeX (`$…$`, `$$…$$`) display inline. KaTeX CSS is imported once in `main.tsx`. Inline HTML is intentionally **not** enabled (no `rehype-raw`); raw HTML in the model output is shown as text or inside a fenced code block, never injected into the DOM.
- Thinking blocks render in chronological position as collapsed-by-default accordions whose only collapsed label is `thinking...`. Clicking the label expands or collapses the full thinking content. Content is plain text (not markdown-parsed) since thinking traces are rarely well-formed markdown.
- Tool blocks render in chronological position as compact cards (`ToolCard`, defined inline in `MessageList.tsx`). Clicking expands a Claude-Code-style I/O panel with labeled `IN` and `OUT` monospace fields when the tool input/output is available. `experiment.start` tool calls render as `ExperimentCard` instead (see below).
- Experiment cards are special: collapsed by default; clicking the header toggles a log body. The header contains: a status dot (blinking amber while `starting`/`running`, solid green for `completed`, solid red for `failed`/`cancelled`); a `⚗ <tool_name>` label; a text status badge; optionally a `PID <n>` label while running; optionally an `exit <n>` label when failed; a chevron (▸/▾); and a **Cancel** button (visible only while `starting` or `running` and only when the UUID is known). Expanded body shows the tool input (IN) and the last 10 live log lines (OUT) or "No output yet." if none have arrived. The Cancel button calls `e.stopPropagation()` so it does not toggle the card.
- **AskUserQuestion cards** (`AskQuestionCard`) render in chronological position when CC calls the built-in `AskUserQuestion` tool. The card is a wizard: one question per step, a fixed 380px stage so the surrounding chrome (header counter, prev/next arrows, progress dots) does not shift while the user navigates. Single-select picks auto-advance after 220 ms; multi-select and `Other` (which expands an inline text input) wait for an explicit Next. The last question's right-side button switches from `Next` (▸) to `Review`; the Review step lists every question with its current answer and an edit-pencil affordance — clicking a row opens an `EditModal` with that question pre-populated. Submit lives only on the Review step. On submit, the card formats answers as a `- **<header>**: <value>` markdown bullet list prefixed with `Answers to your questions:` and sends it through the normal `chat_send` path; CC resumes the session and continues from there. After submit the card locks into an "Answered" summary view (green check, `→ <answer>` per question). Step transitions use a 220 ms `cubic-bezier(.4,0,.2,1)` slide (outgoing panel slides left/right, incoming panel slides in from the opposite side).

**Composer footer.** Below the textarea, a footer bar holds two dropdown selectors and the Send button. Both dropdowns share the same visual style (a small pill button that opens a menu above it):
- The textarea starts at 52px high, grows with content, caps at 240px, then scrolls internally.
- Each composer instance samples one placeholder sentence from the built-in research/discovery prompt list when it mounts, and keeps that placeholder stable until the composer is remounted.
- **Model** — selects the Claude model; options come from `MODELS` in `state/chatOptions.ts` (Haiku 4.5, Sonnet 4.6, Opus 4.7). Default: Haiku 4.5.
- **Effort** — selects the thinking-budget level; options come from `EFFORT_LEVELS` (Low → Med → High → XHigh → Max). Default: **Low**. Persisted to `.ire/workspace.json` (debounced 1 s) and rehydrated on workspace open.
Both values are passed as `options: { model, effort }` on every `chat_send` invocation.

**ExperimentTabView.** When the active tab has `kind === "experiment"`, the chat pane renders `ExperimentTabView` instead of the message list + composer. It shows: a name header with a status badge; a metadata grid (status + elapsed timer, runtime, command); and a scrollable log pane (stdout only, `h-48`, auto-scrolls to bottom). Elapsed time is updated every second via `setInterval` while the experiment is running, and frozen to the final elapsed on completion. Live log lines arrive via the `experiment-log-line` event. The pane polls `experiment_list` once on mount to load initial state and loads existing stdout via `experiment_logs`.

### 13.3 Edit/preview toggle behaviour

- Resource preview tabs open in **Preview** by default, rendering the wiki markdown via `ResourcePreviewPane`. Switching to Edit loads the raw file contents into a textarea; switching back to Preview without Submit discards local edits (with a confirm if dirty). Submit calls `save_wiki_file`.
- `NotesPane` renders `notes.md` as markdown in display mode, edits it inline as raw markdown, and saves through `save_notes` on blur / Ctrl+Enter.
- `IdeasPane` does not use markdown edit/preview. It writes the structured `ideas.json` list directly via `save_ideas_json`.

### 13.4 Resource list

The Resources list shows only confirmed (indexed) resources — those where the user clicked Confirm, the wiki file was written, and `index_resource` recorded a non-null `wiki_path`. Each entry shows the extracted title (frontmatter `title:` → first `#` heading → filename stem). No status label is shown. Resources in progress (being fetched or summarised) do not appear in the list; they are visible only in the open resource chat tab.

Clicking a resource entry (only enabled when `wiki_path` is non-null) opens a **Preview tab** in the central column. The tab fetches the wiki file content via `read_wiki_file` and renders it in a `ResourcePreviewPane` with edit/preview toggle and a Submit button. Submit calls `save_wiki_file` to persist edits; it does not commit. Clicking the same resource while its Preview tab is already open re-focuses that tab instead of opening a duplicate.

### 13.5 Theming

The UI uses a fixed dark theme. All colours are defined as Tailwind token extensions in `tailwind.config.ts` (e.g. `surface-container-low`, `on-surface`, `primary`, `error`, `warn`, `ok`, `accent`). There are no light-mode overrides and no theme-toggle button in the current implementation.

Typography uses bundled `geist` package font files (`Geist`, `Geist Mono`) referenced from `styles.css`; icons are inline SVGs from `src/components/Icon.tsx`. The app does not load Google Fonts at runtime.

`~/.config/ire/config.json` still has a `theme` field in its schema (reserved for future use), and `read_user_config` returns it, but the frontend does not apply it: `hydrateFromUserConfig` in the workspace Zustand store only restores `recentWorkspaces`.

### 13.6 Workspace state (`workspace.json`)

```json
{
  "version": 1,
  "panel_layout": {
    "groups": {
      "body":  { "left": 22, "center": 56, "right": 22 },
      "left":  { "pulse": 33.33, "resources": 33.33, "experiments": 33.34 },
      "right": { "notes": 27.5, "ideas": 27.5, "resource-input": 45 }
    }
  },
  "last_opened": "2026-05-06T10:14:00Z",
  "effort": "low"
}
```

Each entry under `panel_layout.groups.<group-id>` is the `Layout` map (`{ panel-id: percentage }`) that `react-resizable-panels` accepts as `defaultLayout` on `<Group>`. Unknown / missing groups fall back to per-`<Panel>` `defaultSize` props. Persisted via `save_workspace_state` (debounced 1 s on layout change). Hydrated by `read_workspace_state` from `SetupScreen.handlePick` immediately after `open_workspace`/`init_workspace`, before the workspace transitions to `phase = "ready"` so the panels mount with the correct sizes.

`effort` stores the last-used thinking-budget level (`"low"` | `"medium"` | `"high"` | `"xhigh"` | `"max"`). Defaults to `"low"` on first open. Persisted by `Layout` (debounced 1 s on change) and applied to `useChatOptions` during workspace hydration in `SetupScreen`.

Theme is **not** stored here — it is a user-level preference and lives in `~/.config/ire/config.json` (see [§13.7](#137-user-config-configireconfig.json)).

Per-tab CC `session_id`s are intentionally **not** persisted in MVP: sessions live in the in-memory `SessionManager` and are reset on app close.

### 13.7 User config (`~/.config/ire/config.json`)

Stores preferences that apply across all workspaces. Path: `$XDG_CONFIG_HOME/ire/config.json`, falling back to `$HOME/.config/ire/config.json`.

```json
{
  "theme": "dark",
  "recent_workspaces": [
    "/home/user/projects/my_project",
    "/home/user/projects/other_project"
  ]
}
```

**Read.** Called once during app startup (`refreshSetup` in `App.tsx`) in parallel with `setup_status`. Result passed to `hydrateFromUserConfig` in the workspace Zustand store, which sets `theme` and `recentWorkspaces`. Missing config returns defaults (`theme: null`, `recent_workspaces: []`); missing recent workspace directories are removed and the cleaned config is written back.

**Written by two paths:**
- `push_recent` (Rust, `user_config.rs`) — called at the end of every successful `open_workspace` / `init_workspace`. Reads the existing config, prepends the path, deduplicates, truncates to 10, writes back.
- `save_user_config` (Tauri command) — used by the setup-screen remove action and future callers (e.g. a theme toggle if re-introduced). Must always send the full config object including `recent_workspaces` from the store to avoid clobbering entries written by `push_recent`.

**Frontend store fields.** `recentWorkspaces: string[]` in the workspace Zustand store mirrors the persisted list. `pushRecentWorkspace(path)` prepends and caps to 10 in-memory; the actual disk write is done by the Rust backend.

---

## 14. Tauri IPC Surface

### 14.1 Commands (frontend → backend)

Directory picking is **not** a Tauri command. The frontend calls Tauri's dialog plugin directly (`@tauri-apps/plugin-dialog`) via the `pickDirectory` helper in `ipc.ts`; the path is then passed to `open_workspace` or `init_workspace`.

| Command | Args | Returns |
|---|---|---|
| `setup_status` | — | `{ binary: BinaryStatus }` where `BinaryStatus` is `{ kind: "found"; path: string; version: string \| null } \| { kind: "missing" }` |
| `open_workspace` | `{ path }` | `WorkspaceState` (`{ path, name }`) |
| `init_workspace` | `{ path }` | `WorkspaceState` |
| `close_workspace` | — | `{}` |
| `read_wiki_file` | `{ path }` | `{ content, frontmatter }` |
| `save_wiki_file` | `{ path, content }` | `{}` (atomic write) |
| `save_notes` | `{ content }` | `{}` (atomic write) |
| `read_ideas` | — | `IdeaItem[]` from `ideas.json` |
| `save_ideas_json` | `{ ideas }` | `{}` (writes `ideas.json`) |
| `read_pulse` | — | `{ research_question, this_week }` |
| `save_pulse_field` | `{ field: "research_question" \| "this_week", content }` | `{}` (updates `pulse.json`) |
| `submit_resource` | `{ url }` | `resource_id: string` |
| `submit_local_resource` | `{ path }` | `resource_id: string` |
| `submit_resources` | `{ sources: ({ kind: "url", url } \| { kind: "local_file", path })[] }` | `resource_id: string` |
| `index_resource` | `{ resource_id }` | `{}` (records `wiki_path` + title and emits `wiki-changed`) |
| `discard_resource` | `{ resource_id }` | `{}` (deletes cache file, marks DB row `rejected`) |
| `list_resources` | — | `ResourceItem[]` (only `summarized` entries) |
| `get_resource_confirm_prompt` | — | `string` (the second-turn confirm prompt loaded from `assets/prompts/`) |
| `chat_send` | `{ tab_id, message, options: { model: string, effort: EffortLevel } }` | `{}` (events follow) |
| `chat_cancel` | `{ tab_id }` | `{}` |
| `chat_reset_session` | `{ tab_id }` | `{}` (forgets session id for that tab) |
| `experiment_list` | `{ limit? }` | `[ExperimentRow]` |
| `experiment_logs` | `{ uuid, kb? }` | `{ stdout, stderr }` |
| `experiment_cancel` | `{ uuid }` | `{}` |
| `experiment_delete` | `{ uuid }` | `{}` (refuses running experiments; removes `.ire/wiki/experiments/<uuid>/` and the DB row) |
| `experiment_rename` | `{ uuid, name }` | `{}` (updates `experiments.name`) |
| `get_system_status` | — | `SystemStatus` (workspace path, git branch/diff, CPU/GPU/RAM metrics, CC connected flag) |
| `read_workspace_state` | — | `PersistedWorkspace` (panel layout from `.ire/workspace.json`) |
| `save_workspace_state` | `{ state: PersistedWorkspace }` | `{}` (debounced from frontend; atomic write) |
| `read_user_config` | — | `UserConfig` (`{ theme?, recent_workspaces? }` from `~/.config/ire/config.json`) |
| `save_user_config` | `{ config: UserConfig }` | `{}` (writes full config) |

### 14.2 Events (backend → frontend)

| Event | Payload |
|---|---|
| `chat-stream` | `{ tab_id: string, event: StreamEvent }` (see [§10.3](#103-ndjson-parser-ccstream) and [§9.4](#94-multi-tab-chat)) |
| `tab-created` | `{ tab_id: string, label: string, kind: "chat"\|"resource", resource_id?: string }` (preview tabs are created client-side only) |
| `chat-cancelled` | `{ tab_id: string }` |
| `experiment-starting` | `{ tab_id: string, uuid: string, pid?: number }` (fired when the detached process has been spawned; links the pending experiment card in `tab_id` to its assigned UUID and PID) |
| `experiment-status` | `{ uuid, status, exit_code? }` |
| `experiment-log-line` | `{ uuid, stream: "stdout"\|"stderr", line }` |
| `wiki-changed` | `{ path }` |
| `pulse-changed` | `{ question, blocker, focus }` |
| `setup-needed` | `{ reason }` |
| `error` | `{ scope, message }` |

---

## 15. Concurrency & Data Safety

Following the user's decision to **not** adopt the heavy thread-safety blueprint, the model is:

1. **Single-instance per workspace** via `.ire/.lock` PID file.
   - Created with `OpenOptions::write().create_new(true)` (atomic).
   - Stale detection: parse PID; if not alive (`kill -0` / `OpenProcess`), reclaim.
   - Released on graceful shutdown; orphan-safe via stale reclaim.
2. **In-process serialisation** of wiki writes via `tokio::Mutex<()>` held by `WikiStore`.
3. **Atomic file replacement** for every wiki mutation: temp file in same dir → `fs::rename`. `sync_all` on the temp file before rename. (No directory fsync; the strong durability guarantees from the vault blueprint are deferred.)
4. **CC turn serialisation per session**: one outstanding CC subprocess per session id; new sends queue.
5. **Experiment subprocesses** are detached with their own process group; they outlive a CC subprocess crash.

What we explicitly **do not** do (vs. the vault blueprint): file-level advisory lock for the cache, fingerprint CAS, rename WAL with crash recovery, filesystem watcher with noise filtering. If we ever need them (e.g. to support multi-window per workspace), the blueprint is a ready reference.

---

## 16. Source Tree Layout

```
ire/
├── docs/
│   ├── SDD.md                          # this file
│   ├── SCOPE.md
│   ├── JACK.md
│   ├── VITTO.md
│   └── blueprints/...
├── package.json
├── vite.config.ts
├── tailwind.config.ts                  # design-token colour palette
├── tsconfig.json
├── index.html
├── src/                                # React frontend
│   ├── main.tsx
│   ├── App.tsx
│   ├── types.ts                        # shared types (StreamEvent, Tab, ExperimentRow, …)
│   ├── ipc.ts                          # invoke/listen wrappers + pickDirectory helper
│   ├── state/                          # zustand stores
│   │   ├── workspace.ts                # phase, mode, panelLayout, recentWorkspaces
│   │   ├── chat.ts                     # tabs, messages, tool calls, experiment state
│   │   ├── chatOptions.ts              # model + effort selection (MODELS, EFFORT_LEVELS)
│   │   └── toasts.ts                   # error toast queue
│   ├── hooks/
│   │   └── useSystemStatus.ts          # polls get_system_status every 5 s
│   ├── components/
│   │   ├── Layout.tsx                  # five-pane shell + data loading + debounced saves
│   │   ├── StatusBar.tsx               # bottom status bar
│   │   ├── ToastStack.tsx              # top-right error toasts
│   │   ├── left/
│   │   │   ├── LeftRail.tsx            # vertical resizable group (pulse/resources/experiments)
│   │   │   ├── FocusPane.tsx           # research-question + this-week inline editor
│   │   │   ├── ResourcesSection.tsx    # confirmed resource list → opens preview tab
│   │   │   └── ExperimentsSection.tsx  # experiment list → opens experiment tab
│   │   ├── right/
│   │   │   ├── RightRail.tsx           # vertical resizable group (notes/ideas/resource-input)
│   │   │   ├── NotesPane.tsx           # notes.md inline editor
│   │   │   ├── IdeasPane.tsx           # ideas.json card list
│   │   │   └── AddResourceSection.tsx  # ordered URL/file buffer for resource ingestion
│   │   ├── chat/
│   │   │   ├── ChatPane.tsx            # tab router: chat / resource / preview / experiment
│   │   │   ├── TabBar.tsx              # TDI tab bar with icons and + button
│   │   │   ├── MessageList.tsx         # message bubbles, ToolCard (inline), ExperimentCard
│   │   │   ├── MessageMarkdown.tsx     # react-markdown + remark-gfm + KaTeX renderer
│   │   │   ├── ExperimentCard.tsx      # experiment.start tool-call card with live log tail
│   │   │   ├── ExperimentTabView.tsx   # full experiment detail view (metadata + logs)
│   │   │   ├── ResourcePreviewPane.tsx # edit/preview toggle for resource wiki files
│   │   │   └── Composer.tsx            # floating textarea + model/effort pickers + Send
│   │   └── setup/
│   │       └── SetupScreen.tsx         # workspace picker + recent list + CC status
│   └── styles.css
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   └── src/
│       ├── main.rs
│       ├── lib.rs                      # tauri::Builder, .manage, command registration
│       ├── user_config.rs              # UserConfig struct, read/write, push_recent
│       ├── commands/
│       │   ├── mod.rs
│       │   ├── workspace.rs            # setup_status, open/init/close_workspace, workspace state, user config
│       │   ├── wiki.rs                 # read/save wiki, notes, pulse, ideas
│       │   ├── chat.rs                 # chat_send, chat_cancel, chat_reset_session
│       │   ├── resources.rs            # submit/index/discard/list_resources, get_resource_confirm_prompt
│       │   └── system.rs               # get_system_status
│       ├── workspace/
│       │   ├── mod.rs
│       │   ├── lock.rs                 # .lock PID file
│       │   ├── init.rs                 # scaffold + git init
│       │   ├── state.rs                # WorkspaceState + ActiveWorkspace managed state
│       │   └── persisted.rs            # PersistedWorkspace (workspace.json schema)
│       ├── wiki/
│       │   ├── mod.rs
│       │   ├── store.rs                # atomic write, index regeneration, wiki-changed events
│       │   ├── index.rs                # _index.md regenerator
│       │   └── frontmatter.rs
│       ├── resources/
│       │   ├── mod.rs
│       │   ├── fetch.rs                # reqwest fetcher
│       │   ├── arxiv.rs                # arXiv shortcut: abs/pdf URL → LaTeX tarball extract
│       │   ├── local.rs                # local .txt/.md/.pdf/.docx extraction
│       │   ├── pdf.rs                  # pdf-extract crate
│       │   └── html.rs                 # readability extraction
│       ├── cc/
│       │   ├── mod.rs
│       │   ├── discovery.rs            # find_claude_binary
│       │   ├── spawn.rs                # Command setup
│       │   ├── stream.rs               # NDJSON parser → StreamEvent
│       │   └── session.rs              # SessionManager (per-tab session_id + PID)
│       ├── prompts/
│       │   └── mod.rs                  # load prompts from assets/prompts/ at runtime
│       ├── mcp/
│       │   ├── mod.rs
│       │   ├── config.rs               # write .ire/mcp.json
│       │   └── rpc.rs                  # Unix socket / TCP RPC handler
│       └── db/
│           ├── mod.rs
│           ├── migrations.rs
│           └── models.rs               # Experiment, Resource
└── mcp/                                # bundled Node MCP server
    ├── package.json
    ├── server.js                       # stdio server, JSON-RPC bridge to Rust
    └── tools.js                        # tool schema definitions
```

---

## 17. Implementation Phases

Each phase ends with a demoable milestone.

**Phase 0 — Skeleton.** Replace the default `greet` Tauri example with the five-pane layout (static content). Add zustand, `react-resizable-panels`, types. No backend logic. Dark/light theme toggle in the topbar; dark is the default. *Milestone:* layout renders; panes resize/collapse; theme toggles between dark and light. ✅

**Phase 1 — Workspace lifecycle.** Implement setup screen, binary discovery, `init_workspace`, `open_workspace`, `.lock`, `close_workspace`. Scaffold `.ire/` with seed wiki. *Milestone:* user can pick or init a workspace; `.ire/` materialises; lock works across restarts. ✅

**Phase 2 — Wiki store + memory tools (no CC yet).** `WikiStore` with atomic writes, `_index.md` regeneration, `log.md` append. SQLite migrations. Frontend reads `pulse.json`, `notes.md`, and `ideas.json`. *Milestone:* user can manually edit notes and see them persisted; `wiki-changed` events propagate. ✅

**Phase 3 — CC subprocess layer.** Binary discovery + spawn + NDJSON parser + session management. A debug "Send" button next to the chat pane that sends a raw message and renders streaming text only (no tool cards yet). No MCP yet. *Milestone:* user can chat with CC inside the central pane, multi-turn via `--resume`. ✅

**Phase 4 — MCP server.** Node MCP server with the [§11.1](#111-tool-catalog-mvp) tool catalog, RPC bridge to Rust. CC config wired up via `--mcp-config`. Implements `wiki.*`, `memory.*`, `pulse.update`. Unix-domain socket at `.ire/mcp.sock`; server path embedded at build time via `IRE_MCP_DIR` env var. `WikiStore` handles atomic writes, index regeneration, `wiki-changed` events, and renames without creating git commits. System prompt composed from wiki context files on every CC turn. *Milestone:* in chat, user can ask "save this insight to long-term memory" and CC actually does it. ✅

**Phase 5 — Pipelines.** Notes/ideas/resource ingestion, including the Rust PDF/HTML/local-file extractors. `submit_resources` accepts an ordered list of URL and local-file sources, extracts all text all-or-nothing, writes one cache file for a single source or `.ire/cache/<batch_sha>/source-NNN.txt` for multiple sources, inserts one DB row, emits `tab-created`, and kicks one CC summarisation turn. The legacy `submit_resource` and `submit_local_resource` commands remain available for single-source callers. Confirm sends a second CC turn that writes `resources/<slug>.md` via `wiki.write`; when Done fires, `index_resource` scans `wiki/resources/` for the file whose frontmatter `sources:` array contains every stored source ref (schema-aligned URL or `file:<sha256>:<filename>`), extracts the title (frontmatter `title:` → first `#` heading → filename stem), updates the DB (`status=summarized`, `wiki_path`, `title`), and emits `wiki-changed` to refresh the pane. Discard calls `discard_resource` (deletes cache, marks `rejected`). Notes and ideas write directly to disk without committing. The resources list shows only `summarized` resources with their title and non-null `wiki_path`; no status label is shown. *Milestone:* ingest one or more supported sources → one resource summary appears in the right pane. ✅

**Phase 6 — Experiments.** `experiment.start`, detached subprocess, monitor, wake-up turn composition. Experiment cards in chat with live log tail. *Milestone:* CC can run a Python script ablation, tell the user "I'll be back", and resume with results when the script exits. ✅

**Phase 7 — Polish.** `workspace.json` persistence (theme + per-group panel layouts via `read_workspace_state` / `save_workspace_state`, debounced 1 s, hydrated before the Layout mounts). Error toast stack (top-right) wired to a frontend `useToasts` zustand store; subscribes to the backend `error` event and replaces silent `console.error` calls in user-facing flows. Cancel button on `ExperimentCard` (visible while status is `starting` or `running`) calls `experiment_cancel`. Inline focus editor saves `pulse.json` fields through `save_pulse_field`. *Milestone:* layout, theme, and focus survive restart; user-visible failures surface as toasts; experiments can be cancelled from the chat.

---

## 18. Testing Strategy

- **Rust unit tests** for: atomic writes (parallel write loops), lock acquisition (with subprocess mocks), NDJSON parser (replay recorded streams), index regenerator, frontmatter parser, PDF extractor (golden files), HTML extractor (snapshot of cleaned text).
- **Rust integration tests** for: workspace init/open/close round-trip, end-to-end pipeline that drives a stubbed CC binary (a script that emits canned NDJSON).
- **Frontend tests** with Vitest for state stores and the markdown pane edit/preview reducer. No e2e for MVP.
- **Manual QA checklist** at each phase milestone, captured in `docs/QA.md` (created during Phase 0).

A stubbed CC binary lives at `src-tauri/tests/fixtures/fake_claude.sh`; it reads the prompt and echoes a deterministic NDJSON sequence. This enables CI without a real Claude binary.

---

## 19. Open Items & Risks

- **Node runtime detection.** The MCP server requires Node. We assume CC's installation already brought Node along, but on some systems CC is a standalone binary. Phase 4 must add a node-discovery probe similar to [§10.1](#101-binary-discovery-ccdiscovery) and surface a setup error if absent.
- **Windows process groups.** Detached experiments work differently on Windows (`CREATE_NEW_PROCESS_GROUP`); needs explicit testing in Phase 6.
- **Wake-up storms.** If multiple experiments finish near-simultaneously, several wake-up turns queue. The queue is FIFO and we surface a count in the UI; CC sees them sequentially. Acceptable for MVP.
- **Index regeneration cost.** Walking the whole `wiki/` tree on every write is fine at MVP scale (tens to low-hundreds of files). At scale, switch to incremental index updates.
- **Frontmatter parsing.** No formal frontmatter spec — using the YAML convention. We accept files without frontmatter; required fields are derived heuristically.
- **CC `--tools` flag stability.** The tool allowlist syntax may evolve; the [wrapper blueprint](./blueprints/claude-code-wrapper.md) reflects the current behaviour. If breaking changes land, `cc::spawn` is the only place that needs to update.
- **Uncommitted `.ire/` changes.** IRE writes wiki, resource index, and workspace files but never commits them. Users must commit `.ire/` changes explicitly when they want those updates captured in git history.
