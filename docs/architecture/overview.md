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
│   │ Workspace    │  │ Wiki         │  │ Agent subprocess manager │   │
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
              │ CLI subprocess │  tools  │ (Node, stdio transport)│
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
| Persistence | SQLite via `rusqlite` | Single DB file at `.ire/wiki/local.db`. |
| MCP server | Node + `@modelcontextprotocol/sdk` | Stdio transport. Bundled with the app. |
| PDF extract | `pdf-extract` crate | Pure Rust, no system deps. |
| HTML extract | `reqwest` + `scraper` + readability | Strip nav/ads; keep article text. |
| Filesystem | `std::fs` + `tempfile` for atomic writes | No `notify` watcher in MVP. |
| Logging | `tracing` + `tracing-subscriber` | Console logging only. |

Claude Code and Codex are invoked as external CLI binaries; neither is a dependency in `Cargo.toml` or `package.json`. The Node MCP server's runtime (`node`) is also assumed installed.

---

## Directory Layout

### Workspace (per project)

```
my_research_project/
├── .git/
├── .gitignore                       # IRE adds: .ire/wiki/local.db, .ire/wiki/experiments/*/*.log, .ire/.lock, .ire/workspace.json, .ire/cache/
├── .ire/
│   ├── .lock                        # PID of running IRE instance (gitignored)
│   ├── _SYSTEM.md                   # IRE framework context + wiki schema, injected first into every agent turn
│   ├── workspace.json               # per-workspace UI state (panel sizes, tabs, chat options); gitignored
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
│   │   └── useSystemStatus.ts          # polls get_system_status every 5 s
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
│       │   ├── wiki.rs                 # read/save wiki, notes, pulse, ideas
│       │   ├── chat.rs                 # chat_send, chat_cancel, chat_reset_session, generate_chat_title
│       │   ├── history.rs              # chat_history_save/list/get/delete
│       │   ├── resources.rs            # submit/discard/list_resources
│       │   └── system.rs               # get_system_status
│       ├── workspace/
│       │   ├── lock.rs                 # .lock PID file
│       │   ├── init.rs                 # scaffold + git init
│       │   ├── state.rs                # WorkspaceState + ActiveWorkspace managed state
│       │   └── persisted.rs            # PersistedWorkspace (workspace.json schema)
│       ├── wiki/
│       │   ├── store.rs                # atomic write, index regeneration, workspace-event dispatch
│       │   ├── index.rs                # _index.md regenerator
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
│       │   └── session.rs              # SessionManager (per-tab session_id + PID)
│       ├── codex/
│       │   ├── discovery.rs
│       │   ├── spawn.rs
│       │   └── stream.rs               # Codex JSONL parser → StreamEvent
│       ├── mcp/
│       │   ├── config.rs               # write .ire/mcp.json
│       │   └── rpc.rs                  # Unix socket / TCP RPC handler
│       └── db/
│           ├── migrations.rs
│           └── models.rs               # Experiment, Resource
└── mcp/                                # bundled Node MCP server
    ├── server.js                       # stdio server, JSON-RPC bridge to Rust
    └── tools.js                        # tool schema definitions
```
