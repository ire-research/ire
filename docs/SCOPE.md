# IRE — Scope

Tracks what is in MVP, what is explicitly deferred, and what is out of scope.
The SDD ([SDD.md](./SDD.md)) is the source of truth for *how* MVP items are built.
This file is the source of truth for *whether* a feature belongs in MVP at all.

---

## MVP

### Workspace lifecycle
- Open existing IRE workspace (directory containing `.ire/` + git repo).
- Initialize new workspace (empty directory → `git init` + `.ire/` scaffold + first commit).
- Single-instance lock per workspace via `.ire/.lock` PID file.
- Stale lock reclaim (PID no longer alive → reclaim).
- Detect missing / unauthenticated Claude binary on startup → setup screen.

### Wiki layer (`.ire/wiki/`)
- Markdown files tracked in git (`.ire/wiki/**`).
- `_index.md` and `_schema.md` maintained as canonical structure.
- Atomic writes (temp file + rename) for every markdown mutation.
- In-process mutex on the wiki state.
- **Two-tier commit policy** (SDD §6.4):
  - Auto-committed on every write: `status/**`, `log.md`, `_schema.md`, `_index.md`.
  - User-committed on explicit submit/approval: `notes.md`, `ideas.md`, `resources/**`.

### Memory layer
- `status/long-term.md` — agent-written architectural decisions, pivots.
- `status/short-term/YYYY-MM-DD.md` — agent-written daily notes; only last 2 days auto-injected.
- `status/failures.md` — structured "what didn't work" log.
- `status/pulse.md` — current research question + blocker + focus banner text.

### Pipelines
- **Notes ingestion**: clean + dedupe `notes.md` on submit, persist to `wiki/notes.md`.
- **Ideas ingestion**: clean + dedupe `ideas.md` on submit, persist to `wiki/ideas.md`.
- **Resource ingestion**: Rust-side fetch + PDF/HTML→text, then CC summarises into `wiki/resources/<slug>.md` and updates `_index.md`.

### Chat
- Brainstorm and Experiment modes — both single Claude-Code subprocess in the central pane, multi-turn via `--resume`.
- Streaming text + thinking + tool calls rendered live.
- MCP server connected via `--mcp-config` exposing wiki / memory / experiment / resource tools.

### Experiments
- CC spawns experiment as a **detached** subprocess via the MCP `experiment.start` tool; CC's turn ends without blocking on it.
- Backend monitors the subprocess; on completion, **wakes CC** with a follow-up turn (`--resume`) that points at the saved context file and the logs.
- `.ire/local.db` records experiment metadata.
- Live stdout/stderr tail visible in the central pane.

### Frontend
- Five-pane layout — resizable + collapsible splits.
- Markdown panes with edit/preview toggle (no side-by-side).
- Current Focus banner (top-left).
- Real-time chat stream rendering (text, thinking, tool cards).

---

## Deferred (post-MVP)

- Codebase graph (`status/graph.json`) — Tree-sitter-based symbol/import graph.
- `/init` skill — autonomous walk-through of an existing repo to bootstrap the wiki.
- Wiki linting — periodic CC pass to detect contradictions, orphans, stale claims.
- Headless background experiment agent — autonomous experiment loop without user prompting.
- Paper draft generation (LaTeX export to Overleaf).
- Debug pipeline inspector — step-through view of each ingestion pipeline.
- Multi-window per workspace.
- Multi-project switcher / recent-projects list.
- Token usage tracking and budgeting.
- Telemetry / crash reporting.
- Search engine integration (`qmd` or equivalent) for wikis at >100 sources.
- WebSocket UI bridge from MCP server to frontend (for "open this file", "highlight", etc.).

---

## Out of scope (non-goals)

- Direct code execution by IRE itself; only CC executes commands (via its `Bash` tool or MCP `experiment.start`).
- Local LLM execution. CC must be authenticated externally.
- Proprietary sync / cloud backend.
- User analytics or telemetry.
