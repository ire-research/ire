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
9. [Chat: Brainstorm & Experiment Modes](#9-chat-brainstorm--experiment-modes)
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
| Persistence | SQLite via `rusqlite` | Single DB file at `.ire/local.db`. |
| MCP server | Node + `@modelcontextprotocol/sdk` | Stdio transport. Bundled with the app. |
| PDF extract | `pdf-extract` crate | Pure Rust, no system deps. |
| HTML extract | `reqwest` + `scraper` + `readability` (custom or `readability-rs`) | Strip nav/ads; keep article text. |
| Filesystem | `std::fs` + `tempfile` for atomic writes | No `notify` watcher in MVP — wiki changes are mediated through IRE. |
| Logging | `tracing` + `tracing-subscriber` | File sink at `.ire/logs/ire.log`. |

CC is invoked as an external CLI binary; not a dependency in `Cargo.toml` or `package.json`. The Node MCP server's runtime (`node`) is also assumed installed (CC requires it anyway).

---

## 4. Directory Layout

### Workspace (per project)

```
my_research_project/
├── .git/
├── .gitignore                       # IRE adds: .ire/local.db, .ire/logs/, .ire/.lock, .ire/workspace.json, .ire/experiments/
├── .ire/
│   ├── .lock                        # PID of running IRE instance (gitignored)
│   ├── local.db                     # SQLite (gitignored)
│   ├── workspace.json               # UI layout, last-used session ids (gitignored)
│   ├── logs/                        # ire.log + experiment logs (gitignored)
│   │   ├── ire.log
│   │   └── <experiment_uuid>/
│   │       ├── stdout.log
│   │       └── stderr.log
│   ├── experiments/                 # CC-authored context files per experiment (gitignored)
│   │   └── <experiment_uuid>/
│   │       ├── plan.md
│   │       └── result.md
│   └── wiki/                        # ALL TRACKED IN GIT
│       ├── _SYSTEM.md               # IRE framework context — injected first into every CC turn
│       ├── _index.md                # Master index (path → one-line summary)
│       ├── _schema.md               # Conventions for CC
│       ├── notes.md                 # User notes (cleaned by ingestion)
│       ├── ideas.md                 # User ideas (cleaned by ingestion)
│       ├── status/
│       │   ├── pulse.md             # Research question + blocker + focus banner
│       │   ├── long-term.md         # Agent-written architectural decisions
│       │   ├── failures.md          # Structured "what didn't work"
│       │   └── short-term/
│       │       └── YYYY-MM-DD.md    # Daily agent notes
│       └── resources/
│           └── <slug>.md            # One file per ingested paper/article
└── ... user source code ...
```

**Gitignore additions** appended on workspace init:
```
.ire/.lock
.ire/local.db
.ire/logs/
.ire/workspace.json
.ire/experiments/
```

`wiki/` is intentionally **not** gitignored — it is the durable knowledge artefact and benefits from version history.

### App source tree

See [§16](#16-source-tree-layout).

---

## 5. Workspace Lifecycle

### 5.1 Onboarding (first launch / no recent workspace)

```
┌─ Setup screen ───────────────────────────────────────┐
│ Step 1: Claude-Code binary                           │
│   • find_claude_binary() result, version, status     │
│   • if missing: instructions + retry button          │
│   • if unauthenticated: surface stderr               │
│ Step 2: choose workspace                             │
│   • [Open existing]   → file dialog                  │
│   • [Initialize new]  → file dialog (empty dir)      │
└──────────────────────────────────────────────────────┘
```

### 5.2 Open existing

1. User picks directory via Tauri's file dialog.
2. Backend validates: directory exists, is a git repo, contains `.ire/wiki/_schema.md` (the marker file).
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
   - Write seed files: empty `notes.md`, empty `ideas.md`, `pulse.md` with placeholders, `_schema.md` (canned), `_SYSTEM.md` (canned), `_index.md` (auto-built from the seed).
   - Append IRE entries to `.gitignore` (create if missing).
   - `git add .ire/wiki .gitignore && git commit -m "Initialize IRE workspace"`.
3. Continue from step 3 of [§5.2](#52-open-existing).

### 5.4 Close

- Stop the CC subprocess (kill on SIGTERM, escalate to SIGKILL after grace).
- Stop the MCP server subprocess.
- Persist `workspace.json` (current layout + session id).
- Release `.ire/.lock`.

---

## 6. Wiki Layer

### 6.1 Conventions (encoded in `_schema.md`)

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
- [status/pulse.md](./status/pulse.md) — current research question + blocker
- [resources/attention-is-all-you-need.md](./resources/attention-is-all-you-need.md) — Vaswani et al. (2017): self-attention transformer …
```

The one-line summary is sourced from frontmatter `summary:` if present, else the first non-heading paragraph truncated to 160 chars.

### 6.4 Git commit policy

Wiki paths split into two classes based on **who decides when the change becomes a commit**:

| Class | Paths | Commit trigger |
|---|---|---|
| **Auto-tracked** | `status/**`, `_schema.md`, `_index.md` | Every `WikiStore` write commits immediately. |
| **User-tracked** | `notes.md`, `ideas.md`, `resources/**` | Written to disk on every change, but **only committed** when the user explicitly submits (notes/ideas Submit button) or approves (resource summary review). |

Rationale: memory and operational state should be durably versioned without human-in-the-loop friction; user-facing artefacts deserve an explicit commit gesture so the user can review and edit before the change becomes part of git history.

**Index handling.** `_index.md` is auto-tracked, but it can be touched by either class of write. Rule:

- If the triggering write is **auto-tracked**: commit `<that path> + _index.md` in one commit. Auto-message, e.g. `auto: memory long-term.md`, `auto: pulse update`.
- If the triggering write is **user-tracked**: write `_index.md` to disk but **do not commit it yet**. The index update lands in the eventual user commit alongside the user-tracked path. Until then, `_index.md` may legitimately reference uncommitted files in the working tree — that is fine.

The `WikiStore` exposes `write(path, content, ...)` which classifies internally; the MCP server does not need to know which class a path is in.

**Auto-commit implementation.** `WikiStore` calls `git add <paths>` then `git commit -m <auto-msg>` after the rename + index regen. Failures are logged but do not crash the write — the file is still on disk and a future write will re-stage it. Hooks are not skipped (per project policy on destructive flags).

**User commit implementation.** Notes/ideas Submit and resource approval each flow through a `wiki::commit_user_changes(scope)` helper that stages the scoped paths + index and commits with a user-supplied or templated message (e.g. `notes: <first 50 chars of pane>`, `resources: add <slug>`).

**Resource approval.** The resource ingestion pipeline ([§8.3](#83-resource-ingestion)) presents an executive summary to the user via a dedicated chat tab before writing anything to the wiki. Only on **Confirm** does CC write `resources/<slug>.md` via `wiki.write` (user-tracked). The Confirm flow also commits `resources/<slug>.md + _index.md` to git. On **Discard**, the cache file is deleted and the DB row is marked `rejected`; no wiki file is ever written.

---

## 7. Memory Layer

Three files under `wiki/status/`. **All three are agent-written only**; the user does not edit them through the UI.

### 7.1 `long-term.md`

CC writes architectural decisions, pivots, "this approach is the one we settled on" claims here, via the MCP `memory.write_long_term` tool. Always injected into CC's system prompt context (whole file).

### 7.2 `short-term/YYYY-MM-DD.md`

CC writes daily operational notes here via `memory.write_short_term`. Only the **last two** day-files (today + yesterday) are auto-injected. Older files remain on disk for git history but are not in CC's prompt unless explicitly read.

CC is told in the system prompt:
> Use `memory.write_short_term` for detailed information about the current experiment, specific debugging steps, and daily operations. After two days these notes are no longer auto-injected — promote anything still relevant to long-term memory.
>
> Use `memory.write_long_term` for overarching architectural decisions, pivots, abandonment of approaches, and durable insights.

### 7.3 `failures.md`

Structured "what didn't work" log. Written via the MCP `memory.record_failure(method, reason, context_ref?)` tool, which appends a section like:

```
## <method-name>
- **why-it-failed**: <reason>
- **context**: <ref>
- **recorded**: 2026-05-03
```

Always injected into CC's context as a "do-not-propose" anchor. This directly addresses Vittorio's #1 grievance.

### 7.4 Context injection rules

When IRE spawns a CC turn, the system prompt is composed of:

1. `wiki/_SYSTEM.md` — static IRE framework context (what IRE is, wiki layout, behavioral rules). MCP tool descriptions are received automatically via `tools/list` and are not duplicated here. Seeded from `assets/seed/_SYSTEM.md` on workspace init; always injected first.
2. The mode-specific preamble (brainstorm vs. experiment), loaded from `src-tauri/assets/prompts/mode_{brainstorm,experiment}.md` via the `prompts` module. All CC-facing prompt text — mode preambles, the resource summarizer role, the resource-confirm follow-up, and the experiment wake-up template — lives in `src-tauri/assets/prompts/`. Edit those files to change CC's behaviour; never hardcode prompts at call sites.
3. `wiki/_schema.md` (conventions).
4. `wiki/_index.md` (catalog).
5. `wiki/status/pulse.md` (focus + blocker).
6. `wiki/status/long-term.md` (full).
7. `wiki/status/failures.md` (full).
8. The two most recent `short-term/YYYY-MM-DD.md` files.

This is added via `--append-system-prompt`. Wiki/notes/resources are read on-demand by CC through MCP tools; they are not pre-injected.

---

## 8. Pipelines

### 8.1 Notes ingestion

```
User edits the notes pane (raw text)
  → clicks Submit
  → save_notes(content) Tauri command
    → Rust writes content to .ire/wiki/notes.md (atomic)
    → commits notes.md + _index.md to git
  → wiki-changed event causes the notes pane to re-render.
```

No CC turn is triggered. CC reads `notes.md` only if the user explicitly requests it, or if the context warrants it; it is never injected into the system prompt by default.

### 8.2 Ideas ingestion

Identical to notes ingestion ([§8.1](#81-notes-ingestion)), target `wiki/ideas.md`. No CC turn is triggered.

### 8.3 Resource ingestion

```
User pastes URL → Submit
  → submit_resource(url) Tauri command
  → Rust:
      1. fetch URL with reqwest (10 s timeout, follow redirects)
      2. detect content-type:
           pdf  → pdf-extract crate → plain text
           html → readability extraction → plain text
      3. write extracted text to .ire/cache/<url-sha256>.txt
      4. insert resource row in SQLite (status=pending_summary)
      5. open a new resource chat tab (see §9.4), labelled by URL hostname
      6. kick a CC turn in that tab with prompt:
           "Read .ire/cache/<sha256>.txt (source: <url>). Provide an executive
            summary — what this resource is, what is relevant to this project,
            why it matters, and how it could be used. Use bullet points.
            Do NOT write to the wiki yet."
```

CC streams the summary into the resource tab. When the turn ends, **Confirm** and **Discard** buttons appear.

**Confirm**: triggers a second CC turn in the same tab with the instruction to write the summary to `resources/<slug>.md` via the `wiki.write` MCP tool. Frontmatter follows `_schema.md`: `title`, `type: summary`, `sources: [<url>]`, `updated: YYYY-MM-DD`. Body starts with a `#` heading matching the title, then the summary. The tab auto-closes when this second CC turn ends. The written file is user-tracked (not auto-committed — see §6.4); it is committed to git as part of the Confirm flow.

**Discard**: deletes `.ire/cache/<sha256>.txt`, marks the DB row `status=rejected`, closes the tab immediately. No wiki file is written.

---

## 9. Chat: Brainstorm & Experiment Modes

Both modes use a **single CC subprocess** in the central pane. The mode determines:

- The **system prompt suffix** (`--append-system-prompt`).
- The **MCP tool allowlist** advertised to CC.

| | Brainstorm | Experiment |
|---|---|---|
| Allowed MCP tools | `wiki.*`, `memory.*`, `pulse.update`, `resource.*` | All brainstorm tools + `experiment.*` |
| Allowed CC built-ins | `Read`, `Grep`, `Glob`, `WebFetch` | + `Bash`, `Edit`, `Write`, `MultiEdit` |
| `--permission-mode` | `default` (prompts on edits) | `acceptEdits` |
| Session id | one shared session per workspace, persisted in `workspace.json` | same session shared across modes |

The session is shared across modes so that brainstorming context is available when the user asks for an experiment and vice versa. Mode is just a "lens" on the same conversation.

### 9.1 Send message flow

```
User types in central pane → Send
  → chat_send({ mode, message }) Tauri command
  → Rust spawns:
      claude -p "<message>"
        --output-format stream-json --verbose --include-partial-messages
        --mcp-config .ire/mcp.json --strict-mcp-config
        --tools "<allowlist for mode>"
        --append-system-prompt "<composed per §7.4 + mode suffix>"
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
       - writes plan_md to .ire/experiments/<uuid>/plan.md
       - spawns the command as a DETACHED process group:
           Command::new("sh").args(["-c", &command])
                 .current_dir(working_dir)
                 .stdin(Stdio::null())
                 .stdout(file .ire/logs/<uuid>/stdout.log)
                 .stderr(file .ire/logs/<uuid>/stderr.log)
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
            Plan: .ire/experiments/<uuid>/plan.md\n
            stdout tail: <…>\nstderr tail: <…>"
       - spawns a CC turn with --resume <session_id> and that message
T6  CC reads result files, calls wiki.write for any new findings,
    memory.write_short_term for daily notes, memory.record_failure if
    something failed, pulse.update if blocker resolved/changed.
```

**Subtleties.**
- The user can keep using the brainstorm pane during T3–T5. The next user message and the wake-up share the same session id; whichever arrives first runs first. We serialise CC turns: only one CC subprocess per session at a time. If a user message arrives while a wake-up is running, it queues; if a wake-up fires while the user is mid-turn, it queues. The pending-queue is shown in the UI ("1 wake-up pending").
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
| Main | On workspace open (id `"main"`) | No (pinned) | Hosts the Brainstorm / Experiment mode selector. The primary research conversation. |
| Chat | User clicks + button | Yes | Fresh CC session, independent conversation history. |
| Resource | Backend resource ingestion (§8.3) | Auto-closes on Confirm/Discard | Dedicated to reviewing a single resource summary; shows Confirm / Discard instead of a free-form Composer when CC finishes. |
| Preview | User clicks a resource in the Resources list | Yes | Renders a `MarkdownPane` (edit/preview toggle + Submit) for the resource's wiki file. Clicking the same resource again focuses the existing tab rather than opening a duplicate. Submit persists edits to disk via `save_wiki_file` and commits. |

**Session isolation.** Each tab carries its own `tab_id` (UUID for dynamically created tabs; `"main"` for the pinned tab). The backend `SessionManager` maintains a `HashMap<tab_id, PerTabSession>` where `PerTabSession` holds `{ session_id: Option<String>, running_pid: Option<u32> }`. This replaces the old single `ChatSession` global.

**Event routing.** `chat-stream` events are wrapped as `{ tab_id, event }` before being emitted to the frontend. The frontend maintains a single global listener that routes each event to the correct tab's message list using the `tab_id` field. A `tab-created` event (payload: `{ tab_id, label, kind, resource_id? }`) is emitted by the backend whenever a new tab is opened programmatically (e.g. during resource ingestion). Preview tabs are created client-side only (no `tab-created` event) — the store's `openPreviewTab` action handles deduplication and activation.

**User-initiated turn (any tab)**

```
User types → handleSend(tabId)
  → beginAssistantMessage(tabId)
  → ipc.chatSend(tabId, text, mode)
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
| `chat_send` | `{ mode, message }` | `{ tab_id, mode, message }` |
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
    ToolStart { tool_id: String, tool_name: String, input_preview: Option<String> },
    ToolInputDelta { tool_id: String, partial_json: String },
    ToolDone { tool_id: String, output_preview: Option<String> },
    Result { text: Option<String>, session_id: String },
    Error { message: String },
    Done,
}
```

Deduplicate `Result.text` against streamed `TextDelta`s using an `emitted_text: bool` flag (blueprint §3).

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
| `wiki.read({ path })` | Read any wiki markdown file. Returns content + frontmatter. |
| `wiki.write({ path, content, summary? })` | Atomic write; updates `_index.md`. **Auto-committed** if `path` is in the auto-tracked class ([§6.4](#64-git-commit-policy)); otherwise uncommitted until the user submits/approves. |
| `wiki.append({ path, content })` | Append content to a wiki file. Same commit semantics as `wiki.write`. |
| `wiki.list({ glob? })` | List wiki paths; defaults to all. |
| `wiki.rename({ from, to })` | Atomic rename + index update. Auto-committed iff both `from` and `to` are auto-tracked. |
| `memory.write_long_term({ section, content })` | Append to `status/long-term.md` under section. **Auto-committed.** |
| `memory.write_short_term({ content })` | Append to today's `status/short-term/YYYY-MM-DD.md`. **Auto-committed.** |
| `memory.record_failure({ method, reason, context_ref? })` | Append structured entry to `status/failures.md`. **Auto-committed.** |
| `pulse.update({ question?, blocker?, focus? })` | Patch fields in `status/pulse.md`. **Auto-committed.** |
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

Single file at `.ire/local.db`. Migrations applied at workspace open (idempotent).

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
  url TEXT NOT NULL,
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

```
┌─────────────────────────────────────────────────────────────────────┐
│ Top bar:    [Workspace name]  [Mode: Brainstorm | Experiment]   ⚙   │
├─────────────┬───────────────────────────────────────┬───────────────┤
│ ▸ FOCUS     │                                       │ ▸ Notes       │
│  pulse.md   │                                       │  notes.md     │
│  [edit/pre] │                                       │  [edit/pre]   │
│             │     Central pane: Chat / Preview      │  [Submit]     │
├─────────────┤     - streaming text                  ├───────────────┤
│ ▸ Resources │     - thinking blocks (collapsible)   │ ▸ Ideas       │
│  list view  │     - tool cards (Read, Bash, …)      │  ideas.md     │
│  click→pane │     - experiment status cards         │  [edit/pre]   │
│             │     [input box]                       │  [Submit]     │
│             │                                       │               │
│             │                                       │ ▸ Resource    │
│             │                                       │  url + Submit │
└─────────────┴───────────────────────────────────────┴───────────────┘
```

- All splits use `react-resizable-panels`. Each pane has a collapse button.
- The **Focus banner** is the top of the left column (`pulse.md`'s `focus` field), permanently visible unless the whole left column is collapsed.
- Markdown panes have a single toggle: **Edit** (textarea) ↔ **Preview** (rendered). No side-by-side.

### 13.2 Chat rendering

**Tab bar.** The chat pane opens with a standard Tabbed Document Interface (TDI) bar at the top — the same pattern as Chrome or the VS Code editor. Each tab is a horizontal button in a single row:

- The active tab has its background set to match the content area (`--surface`) and its bottom border removed so it visually merges with the content below. A 2 px gold accent line (`--focus-accent`) runs along the top edge of the active tab.
- Inactive tabs have a dimmer foreground (`--text-dim`) and a darker background (`--bg`). Hovering brightens them.
- The close button (×) is hidden until the tab is hovered or active. Pinned tabs (Main) have no close button.
- A + button at the right end of the bar opens a new chat tab.
- If there are more tabs than fit the bar width the row scrolls horizontally (scrollbar hidden).
- A spinning indicator inside the tab label signals a resource tab that is still being summarised.

**Messages.** Text is streamed character-by-character into the latest assistant bubble.
- Both user and assistant text are rendered through `MessageMarkdown` (`react-markdown` + `remark-gfm` + `remark-math` + `rehype-katex`). GitHub-flavoured markdown — tables, fenced code, task lists — and LaTeX (`$…$`, `$$…$$`) display inline. KaTeX CSS is imported once in `main.tsx`. Inline HTML is intentionally **not** enabled (no `rehype-raw`); raw HTML in the model output is shown as text or inside a fenced code block, never injected into the DOM.
- Thinking blocks render as a collapsed-by-default accordion ("Thinking…"). Content is plain text (not markdown-parsed) since thinking traces are rarely well-formed markdown.
- Tool calls render as cards: `[Read] path/to/file ▸ output preview`. Clicking expands the full output.
- Experiment cards are special: status pill (Running / Completed / Failed), live log tail (last 10 lines), and a "view full logs" button. A **Cancel** button appears in the header while status is `starting` or `running`.

### 13.3 Edit/preview toggle behaviour

- Default state per pane: **Preview**.
- Switching to Edit loads the raw file contents.
- Switching back to Preview without Submit **discards** local edits (with a confirm if dirty).
- Submit runs the corresponding ingestion pipeline ([§8](#8-pipelines)) **and commits** the cleaned `notes.md` / `ideas.md` (+ index) to git with a templated message.

### 13.4 Resource list

The Resources list shows only confirmed (indexed) resources — those where the user clicked Confirm and the wiki file was written and committed. Each entry shows the extracted title (frontmatter `title:` → first `#` heading → filename stem). No status label is shown. Resources in progress (being fetched or summarised) do not appear in the list; they are visible only in the open resource chat tab.

Clicking a resource entry (only enabled when `wiki_path` is non-null) opens a **Preview tab** in the central column. The tab fetches the wiki file content via `read_wiki_file` and renders it in a `MarkdownPane` with edit/preview toggle and a Submit button. Submit calls `save_wiki_file` to persist edits and commit. Clicking the same resource while its Preview tab is already open re-focuses that tab instead of opening a duplicate.

### 13.5 Theming

The UI supports dark and light themes. Dark is the default. A toggle button in the topbar switches between them.

- All colors are defined as CSS custom properties in `:root` (dark values). `[data-theme="light"]` overrides them with light values.
- Theme state lives in the Zustand workspace store (`theme: "dark" | "light"`). A `toggleTheme` action flips it.
- `Layout` syncs `document.documentElement.dataset.theme` to the store value via `useEffect` — setting the attribute to `"light"` or removing it to revert to the dark `:root` defaults.
- Theme preference is persisted to `.ire/workspace.json` as part of the workspace state (see [§13.6](#136-workspace-state-workspacejson)) and rehydrated before `Layout` mounts.

### 13.6 Workspace state (`workspace.json`)

```json
{
  "version": 1,
  "theme": "dark",
  "panel_layout": {
    "groups": {
      "body":  { "left": 22, "center": 56, "right": 22 },
      "left":  { "pulse": 55, "resources": 45 },
      "right": { "notes": 40, "ideas": 40, "resource-input": 20 }
    }
  },
  "last_opened": "2026-05-06T10:14:00Z"
}
```

Each entry under `panel_layout.groups.<group-id>` is the `Layout` map (`{ panel-id: percentage }`) that `react-resizable-panels` accepts as `defaultLayout` on `<Group>`. Unknown / missing groups fall back to per-`<Panel>` `defaultSize` props. Persisted via `save_workspace_state` (debounced 1 s on theme or layout change). Hydrated by `read_workspace_state` from `SetupScreen.handlePick` immediately after `open_workspace`/`init_workspace`, before the workspace transitions to `phase = "ready"` so the panels mount with the correct sizes.

Per-tab CC `session_id`s and the central pane mode are intentionally **not** persisted in MVP: sessions live in the in-memory `SessionManager` and are reset on app close.

---

## 14. Tauri IPC Surface

### 14.1 Commands (frontend → backend)

| Command | Args | Returns |
|---|---|---|
| `setup_status` | — | `{ binary: "found"\|"missing"\|"unauth", version?, recent_workspaces: [] }` |
| `pick_workspace` | `{ kind: "open"\|"init" }` | `{ path }` (uses native dialog) |
| `open_workspace` | `{ path }` | `{ workspace: WorkspaceState }` |
| `init_workspace` | `{ path }` | `{ workspace: WorkspaceState }` |
| `close_workspace` | — | `{}` |
| `read_wiki_file` | `{ path }` | `{ content, frontmatter }` |
| `save_wiki_file` | `{ path, content }` | `{}` (atomic write + user commit for the given wiki-relative path) |
| `save_notes` | `{ content }` | `{}` (kicks ingestion + user commit on success) |
| `save_ideas` | `{ content }` | `{}` (kicks ingestion + user commit on success) |
| `submit_resource` | `{ url }` | `{ resource_id }` |
| `index_resource` | `{ resource_id }` | `{}` (commits `resources/<slug>.md` + `_index.md`) |
| `discard_resource` | `{ resource_id }` | `{}` (deletes file, marks rejected) |
| `chat_send` | `{ tab_id, mode, message }` | `{}` (events follow) |
| `chat_cancel` | `{ tab_id }` | `{}` |
| `chat_reset_session` | `{ tab_id }` | `{}` (forgets session id for that tab) |
| `experiment_list` | `{ limit? }` | `[ExperimentRow]` |
| `experiment_logs` | `{ uuid, kb? }` | `{ stdout, stderr }` |
| `experiment_cancel` | `{ uuid }` | `{}` |
| `read_workspace_state` | — | `PersistedWorkspace` (theme + panel layout from `.ire/workspace.json`) |
| `save_workspace_state` | `{ state: PersistedWorkspace }` | `{}` (debounced from frontend; atomic write) |
| `update_pulse_focus` | `{ focus }` | `{}` (replaces `**Focus:** …` line in `status/pulse.md`; auto-committed) |

### 14.2 Events (backend → frontend)

| Event | Payload |
|---|---|
| `chat-stream` | `{ tab_id: string, event: StreamEvent }` (see [§10.3](#103-ndjson-parser-ccstream) and [§9.4](#94-multi-tab-chat)) |
| `tab-created` | `{ tab_id: string, label: string, kind: "chat"\|"resource", resource_id?: string }` (preview tabs are created client-side only) |
| `chat-cancelled` | `{ tab_id: string }` |
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
├── tsconfig.json
├── index.html
├── src/                                # React frontend
│   ├── main.tsx
│   ├── App.tsx
│   ├── types.ts                        # shared types (StreamEvent, WorkspaceState, …)
│   ├── ipc.ts                          # invoke/listen wrappers, typed
│   ├── state/                          # zustand stores
│   │   ├── workspace.ts
│   │   ├── chat.ts
│   │   └── experiments.ts
│   ├── components/
│   │   ├── Layout.tsx                  # five-pane shell
│   │   ├── FocusBanner.tsx
│   │   ├── MarkdownPane.tsx            # edit/preview toggle
│   │   ├── ResourceInput.tsx
│   │   ├── ResourcesList.tsx
│   │   ├── chat/
│   │   │   ├── ChatPane.tsx
│   │   │   ├── MessageList.tsx
│   │   │   ├── ToolCard.tsx
│   │   │   ├── ExperimentCard.tsx
│   │   │   └── Composer.tsx
│   │   └── setup/
│   │       └── SetupScreen.tsx
│   └── styles.css
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   └── src/
│       ├── main.rs
│       ├── lib.rs                      # tauri::Builder, .manage, command registration
│       ├── commands/
│       │   ├── mod.rs
│       │   ├── workspace.rs
│       │   ├── wiki.rs
│       │   ├── chat.rs
│       │   ├── experiments.rs
│       │   └── resources.rs
│       ├── workspace/
│       │   ├── mod.rs
│       │   ├── lock.rs                 # .lock PID file
│       │   ├── init.rs                 # scaffold + git init
│       │   └── state.rs                # WorkspaceState struct
│       ├── wiki/
│       │   ├── mod.rs
│       │   ├── store.rs                # atomic write, mutex, log append
│       │   ├── index.rs                # _index.md regenerator
│       │   └── frontmatter.rs
│       ├── memory/
│       │   ├── mod.rs
│       │   ├── long_term.rs
│       │   ├── short_term.rs
│       │   └── failures.rs
│       ├── resources/
│       │   ├── mod.rs
│       │   ├── fetch.rs                # reqwest
│       │   ├── pdf.rs                  # pdf-extract
│       │   └── html.rs                 # readability
│       ├── cc/
│       │   ├── mod.rs
│       │   ├── discovery.rs            # find_claude_binary
│       │   ├── spawn.rs                # Command setup
│       │   ├── stream.rs               # NDJSON parser → StreamEvent
│       │   └── session.rs              # session_id persistence + queue
│       ├── experiments/
│       │   ├── mod.rs
│       │   ├── runner.rs               # detached spawn + monitor
│       │   └── wake.rs                 # wake-up turn composition
│       ├── mcp/
│       │   ├── mod.rs
│       │   ├── config.rs               # write .ire/mcp.json
│       │   ├── server_proc.rs          # spawn/teardown of node mcp/server.js
│       │   └── rpc.rs                  # Unix socket / TCP RPC handler
│       ├── db/
│       │   ├── mod.rs
│       │   ├── migrations.rs
│       │   └── models.rs               # Experiment, Resource
│       └── util/
│           └── atomic_write.rs
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

**Phase 2 — Wiki store + memory tools (no CC yet).** `WikiStore` with atomic writes, `_index.md` regeneration, `log.md` append. SQLite migrations. Frontend reads `pulse.md`, `notes.md`, `ideas.md` and renders edit/preview. *Milestone:* user can manually edit notes and see them persisted; `wiki-changed` events propagate. ✅

**Phase 3 — CC subprocess layer.** Binary discovery + spawn + NDJSON parser + session management. A debug "Send" button next to the chat pane that sends a raw message and renders streaming text only (no tool cards yet). No MCP yet. *Milestone:* user can chat with CC inside the central pane, multi-turn via `--resume`. ✅

**Phase 4 — MCP server.** Node MCP server with the [§11.1](#111-tool-catalog-mvp) tool catalog, RPC bridge to Rust. CC config wired up via `--mcp-config`. Implements `wiki.*`, `memory.*`, `pulse.update`. Unix-domain socket at `.ire/mcp.sock`; server path embedded at build time via `IRE_MCP_DIR` env var. `WikiStore` extended with `workspace_root`, git auto-commit for auto-tracked paths, and a `rename` method. System prompt composed from wiki context files on every CC turn. *Milestone:* in chat, user can ask "save this insight to long-term memory" and CC actually does it. ✅

**Phase 5 — Pipelines.** Notes/ideas/resource ingestion, including the Rust PDF/HTML extractors. `submit_resource` fetches a URL, extracts text via `scraper` (HTML) or `pdf-extract` (PDF), writes to `.ire/cache/<sha256>.txt`, inserts a DB row, emits `tab-created`, and kicks a CC summarisation turn. Confirm sends a second CC turn that writes `resources/<slug>.md` via `wiki.write`; when Done fires, `index_resource` scans `wiki/resources/` for the file whose frontmatter `sources:` array contains the resource URL (`_schema.md`-aligned), extracts the title (frontmatter `title:` → first `#` heading → filename stem), updates the DB (`status=summarized`, `wiki_path`, `title`), commits, and emits `wiki-changed` to refresh the pane. Discard calls `discard_resource` (deletes cache, marks `rejected`). Notes/ideas Submit commits via `user_commit`. The resources list shows only `summarized` resources with their title; no status label is shown. *Milestone:* paste an arXiv URL → resource summary appears in the right pane. ✅

**Phase 6 — Experiments.** `experiment.start`, detached subprocess, monitor, wake-up turn composition. Experiment cards in chat with live log tail. *Milestone:* CC can run a Python script ablation, tell the user "I'll be back", and resume with results when the script exits. ✅

**Phase 7 — Polish.** `workspace.json` persistence (theme + per-group panel layouts via `read_workspace_state` / `save_workspace_state`, debounced 1 s, hydrated before the Layout mounts). Error toast stack (top-right) wired to a frontend `useToasts` zustand store; subscribes to the backend `error` event and replaces silent `console.error` calls in user-facing flows. Cancel button on `ExperimentCard` (visible while status is `starting` or `running`) calls `experiment_cancel`. Inline focus-banner editor: click the banner to edit, Enter / blur saves through `update_pulse_focus`, Escape cancels. *Milestone:* layout, theme, and focus survive restart; user-visible failures surface as toasts; experiments can be cancelled from the chat.

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
- **Git noise from auto-commits.** Memory and operational paths auto-commit on every write ([§6.4](#64-git-commit-policy)), which can produce many small commits during a busy session (e.g. CC writing short-term notes mid-experiment). Acceptable for MVP — git history is cheap and reviewable. If it becomes painful, batch auto-commits with a debounce window or squash on session close.
- **Repos with pre-commit hooks.** Auto-commits run in the user's repo and respect hooks. A slow or failing hook on `wiki/**` paths will surface as a logged error and the file remains uncommitted until the next write. Document this in onboarding so users scope their hooks to non-`wiki/**` paths if they're sensitive.
