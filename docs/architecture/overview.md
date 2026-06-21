# IRE — Architecture Overview

IRE is a local-first desktop app that wraps Claude Code and OpenAI Codex CLI inside a Tauri shell and gives the selected agent well-organised research context — literature, experiment logs, project state — by maintaining a persistent **LLM Wiki** on disk. Each workspace maps one-to-one to a directory containing the user's source code and an `.ire/` data directory.

For the full per-subsystem reference see the other docs in this folder. For the implementation roadmap and open items see [ROADMAP.md](../ROADMAP.md).

---

## Problem & Users

ML research workflows are fragmented across IDEs, reference managers, and AI interfaces:

1. **Context loss** — switching to CC requires re-establishing project state, literature, and past decisions every session.
2. **Knowledge fragmentation** — no persistent indexed memory of insights, paper summaries, or rejected methodologies, so AI suggestions repeat dead ends.
3. **Goal drift** — the primary objective gets buried under technical work.
4. **Siloed knowledge** — meeting notes, papers, experiment logs, and code state are not unified.

**Target user.** Academic / industrial ML researcher. Python-heavy, comfortable with Git and the terminal, uses LaTeX. Authenticates Claude Code and/or Codex externally.

Two design-driving pain points:
- Models keep proposing methods that were already tried and rejected → IRE must record rejections as **structured, prominently re-injected** state, not buried prose.
- Models forget about long-running experiments while the user is doing something else → IRE must **wake CC up** when an experiment completes, with the right context attached.

---

## System Overview

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
│   │ Workspace    │  │ IreStore     │  │ Agent subprocess manager │   │
│   │ + .lock      │  │ (atomic I/O) │  │ (JSONL parser, resume)   │   │
│   └──────────────┘  └──────────────┘  └──────────────────────────┘   │
│   ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐   │
│   │ Resource     │  │ SQLite       │  │ Experiment monitor       │   │
│   │ fetcher      │  │ (rusqlite)   │  │ (detached subprocesses)  │   │
│   └──────────────┘  └──────────────┘  └──────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────┘
                       ▲ stdio / NDJSON          ▲ stdio (MCP JSON-RPC)
                       │                         │
              ┌────────────────┐         ┌────────────────────────┐
              │ Claude/Codex   │ ◀─────▶ │ IRE MCP server         │
              │ CLI subprocess │  tools  │ (Rust, stdio transport)│
              └────────────────┘         └────────────────────────┘
                       │
                       ▼ Bash tool / MCP experiment.start
              ┌────────────────────────────┐
              │ Detached experiment proc.  │
              │ (user code, e.g. python)   │
              └────────────────────────────┘
```

**Key points:**
- Claude Code and Codex are headless subprocesses selected per chat turn. IRE is a thin IPC bridge: messages in, typed events out.
- The MCP server is the *only* high-level surface the selected agent uses to interact with IRE state. Plain filesystem tools are also enabled, but wiki / memory / experiment work goes through MCP for structure.
- Experiments run as **detached** child processes. The wake-up path resumes the same provider session that started the experiment: Claude Code via `--resume`, or Codex via `codex exec resume <thread_id>`.

---

## Tech Stack

| Layer | Choice | Notes |
|---|---|---|
| App framework | Tauri 2 | Cross-platform shell, Rust backend. |
| Frontend | React 18 + TypeScript + Vite | Already scaffolded. |
| State | Zustand | Light, no Redux ceremony. |
| Markdown | `react-markdown` + `remark-gfm` for preview; `<textarea>` for edit | Toggle-based, not split. |
| Layout | `react-resizable-panels` | Resizable + collapsible splits. |
| Persistence | SQLite via `rusqlite` | Single DB file at `~/.ire/workspaces/<id>/local.db` (chat sessions + experiment operational rows). Git-tracked state is `.ire/ire.json`. UI/session state via `tauri-plugin-store` (app-data dir). |
| MCP server | Rust (`ire --mcp-stdio`) | Stdio transport. Same binary as the app, re-invoked as a subprocess. |
| PDF extract | `pdf-extract` crate | Pure Rust, no system deps. |
| HTML extract | `reqwest` + `scraper` + readability | Strip nav/ads; keep article text. |
| Filesystem | `std::fs` + `tempfile` for atomic writes | No `notify` watcher in MVP. |
| Logging | `tracing` + `tracing-subscriber` | Console logging only. |

Claude Code and Codex are invoked as external CLI binaries; neither is a dependency in `Cargo.toml` or `package.json`.

---

## Directory Layout

### Workspace (per project)

```
my_research_project/
├── .git/
├── .gitignore                       # IRE adds: .ire/cache/
├── .ire/                            # git-tracked knowledge (plus the local-only cache/)
│   ├── _SYSTEM.md                   # IRE framework context, injected first into every agent turn (git-tracked)
│   ├── ire.json                     # notes, focus, ideas, experiments (git-tracked)
│   ├── long-term.md                 # Agent-written architectural decisions and durable dead ends (git-tracked)
│   ├── short-term/
│   │   └── YYYY-MM-DD.md            # Daily agent notes (git-tracked)
│   ├── resources/                   # git-tracked
│   │   ├── _index.md                # Auto-generated resource catalog (path → one-line summary)
│   │   └── <slug>.md                # One file per ingested paper/article (title + sources in frontmatter)
│   └── cache/                       # ingestion temp + experiments/<uuid>/{stdout,stderr}.log (gitignored)
└── ... user source code ...
```

Runtime/local artifacts do **not** live in the workspace; they are keyed per workspace under the user's home directory (standard app-data practice):

```
~/.ire/workspaces/<workspace-name>-<8-hex>/
├── .lock                            # PID of running IRE instance
├── local.db                         # SQLite: chat sessions + experiment operational rows
├── mcp.json                         # MCP server config consumed by Claude Code / Codex
└── mcp.sock                         # Unix socket for the MCP RPC server
```

`<8-hex>` is the first 8 chars of SHA-256 of the workspace path — it disambiguates same-named workspaces and keeps the socket path under the macOS 104-char limit.

**Gitignore additions** appended on workspace init:
```
.ire/cache/
```

The git-tracked parts of `.ire/` (`_SYSTEM.md`, `ire.json`, `long-term.md`, `short-term/`, `resources/`) are intentionally **not** gitignored — they are the durable, shareable knowledge artefact. Per-workspace UI/session state (panel sizes, tabs, chat options) is persisted by `tauri-plugin-store` in the app-data dir, keyed by workspace path — there is no `workspace.json`.

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

This file is managed exclusively by IRE. It is read once at app startup and written on theme change and whenever a workspace is opened. `recent_workspaces` is kept ordered newest-first, capped at 10 entries, and pruned on read so missing directories are not shown.

---

## Source Tree

```
ire/
├── docs/
│   ├── overview.md                     # this file — architecture overview
│   ├── workspace.md                    # workspace lifecycle + concurrency model
│   ├── wiki-memory.md                  # wiki layer, memory layer, SQLite schema
│   ├── chat-agents.md                  # pipelines, chat, agent subprocess layer
│   ├── mcp.md                          # MCP server, tool catalog, RPC channel
│   ├── frontend.md                     # frontend components, Tauri IPC surface
│   └── blueprints/                     # deep implementation guides per feature
├── ROADMAP.md                          # implementation phases + open items
├── CONTRIBUTING.md
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
│   │   ├── chatOptions.ts              # provider + model + effort selection
│   │   └── toasts.ts                   # error toast queue
│   ├── hooks/
│   │   └── useSystemStatus.ts          # useSystemInfo (once) + useSystemMetrics (polls get_system_metrics every 5 s)
│   ├── components/
│   │   ├── Layout.tsx                  # five-pane shell + data loading + debounced saves
│   │   ├── StatusBar.tsx               # bottom status bar
│   │   ├── ToastStack.tsx              # top-right error toasts
│   │   ├── left/
│   │   │   ├── LeftRail.tsx
│   │   │   ├── FocusPane.tsx
│   │   │   ├── ResourcesSection.tsx
│   │   │   └── ExperimentsSection.tsx
│   │   ├── right/
│   │   │   ├── RightRail.tsx
│   │   │   ├── NotesPane.tsx
│   │   │   ├── IdeasPane.tsx
│   │   │   └── AddResourceSection.tsx
│   │   ├── chat/
│   │   │   ├── ChatPane.tsx
│   │   │   ├── TabBar.tsx
│   │   │   ├── MessageList.tsx
│   │   │   ├── MessageMarkdown.tsx
│   │   │   ├── ExperimentCard.tsx
│   │   │   ├── ExperimentTabView.tsx
│   │   │   ├── ResourcePreviewPane.tsx
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
│       ├── binary.rs                   # shared CLI binary discovery types/helpers
│       ├── user_config.rs              # UserConfig struct, read/write, push_recent
│       ├── events.rs                   # workspace-event emit helpers + EventSource
│       ├── commands/
│       │   ├── workspace.rs            # setup_status, open/init/close_workspace, emit_initial_state
│       │   ├── ire.rs                  # read/save resource files, notes, focus, ideas (ire.json setters)
│       │   ├── chat.rs                 # chat_send, chat_cancel, chat_reset_session, generate_chat_title
│       │   ├── history.rs              # chat_history_save/list/get/delete
│       │   ├── resources.rs            # submit/confirm/discard + InflightResources registry
│       │   └── system.rs               # get_system_info (cached once), get_system_metrics (polled)
│       ├── workspace/
│       │   ├── lock.rs                 # .lock PID file (in ~/.ire/workspaces/<id>/)
│       │   ├── init.rs                 # scaffold + git init; home_data_dir(path) → ~/.ire/workspaces/<id>/
│       │   └── state.rs                # WorkspaceState (path + data_dir) + ActiveWorkspace managed state
│       ├── ire/                        # store rooted at .ire/
│       │   ├── store.rs                # ire.json read/edit/upsert + resource write/delete; atomic writes + events
│       │   ├── index.rs                # resources/_index.md regenerator
│       │   └── frontmatter.rs
│       ├── resources/
│       │   ├── fetch.rs
│       │   ├── arxiv.rs                # arXiv shortcut: abs/pdf URL → LaTeX tarball extract
│       │   ├── local.rs
│       │   ├── pdf.rs
│       │   └── html.rs
│       ├── cc/
│       │   ├── discovery.rs
│       │   ├── spawn.rs
│       │   ├── stream.rs               # NDJSON parser → StreamEvent
│       │   └── session.rs              # SessionManager (transient per-tab turn state + PID)
│       ├── codex/
│       │   ├── discovery.rs
│       │   ├── spawn.rs
│       │   └── stream.rs               # Codex JSONL parser → StreamEvent
│       ├── mcp/
│       │   ├── config.rs               # write ~/.ire/workspaces/<id>/mcp.json
│       │   └── rpc.rs                  # Unix socket / TCP RPC handler
│       └── db/
│           ├── schema.rs               # CREATE TABLE IF NOT EXISTS (experiments, chat_sessions)
│           └── models.rs               # Experiment + chat-session row access
```
