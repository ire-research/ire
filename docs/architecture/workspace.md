# Workspace

Covers the workspace lifecycle (open, init, close) and the concurrency model that keeps it safe.

---

## Lifecycle

### Onboarding (first launch / no recent workspace)

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
│  ● claude-code · found  (or: not found)              │
│  ● codex · found        (or: not found)              │
│    retry button if a binary is missing               │
└──────────────────────────────────────────────────────┘
```

On startup, `App.tsx` calls `setup_status` and `read_user_config` in parallel. `read_user_config` removes recent workspace paths that no longer exist, persists the cleaned config, and hydrates `recentWorkspaces` in the Zustand store before the setup screen mounts so the list is immediately populated. If either binary is missing, a `retry` link re-invokes `refreshSetup`; there is no step-by-step wizard. Workspace open/create is enabled when at least one of Claude Code or Codex is found. The binaries detected at workspace open/init become the workspace session's `availableProviders`.

### Open existing

1. User picks directory via Tauri's file dialog.
2. Backend validates: directory exists, is a git repo, and contains `.ire/_SYSTEM.md` plus `.ire/ire.json` (the marker files).
3. Resolve and create the per-workspace home data dir `~/.ire/workspaces/<name>-<8-hex>/` (`workspace::init::home_data_dir`), then acquire `<data_dir>/.lock`:
   - If absent: write current PID, continue.
   - If present and PID alive: refuse, show "already open in another window".
   - If present and PID dead: reclaim (overwrite with current PID).
4. Initialise SQLite at `<data_dir>/local.db` (`CREATE TABLE IF NOT EXISTS`; no versioned migrations — greenfield).
5. The frontend loads UI/session state via `tauri-plugin-store` (keyed by workspace path) after the open command returns — restores pane layout, open-tab UI metadata, and chat options. Tab messages are not stored there — they are hydrated from the `chat_sessions` table by each tab's `historySessionUuid`.
6. Spawn the MCP server subprocess bound to `<data_dir>/mcp.sock` and write `<data_dir>/mcp.json` (long-lived, lives as long as the workspace is open).
7. Emit `workspace-ready` event to the frontend.

### Initialize new

1. User picks an empty directory (or one without `.ire/`).
2. Backend:
   - `git init` if no `.git/`.
   - Scaffold `.ire/` per the directory layout in [overview.md](overview.md#directory-layout).
   - Create `.ire/{resources,short-term,cache}` and write seed files: `.ire/_SYSTEM.md` (canned framework context), `.ire/ire.json` (seed notes/focus/ideas/experiments), `.ire/long-term.md`, and an empty `resources/_index.md`.
   - Append IRE entries to `.gitignore` (create if missing).
   - Do not stage or commit; the user decides when to commit the initialized workspace.
3. Continue from step 3 of Open existing above.

### Close

- Stop the MCP server (drops `McpHandle`, which aborts the task and removes the socket file).
- SIGTERM every in-flight CC subprocess tracked by `SessionManager` and clear all per-tab session state. The frontend `chat-stream` listener is global, so leaving stragglers running would leak late `TextDelta`/`Done` events into whichever workspace opens next.
- Frontend resets the `useChat` Zustand store (`tabs = [MAIN_TAB with empty messages]`, `activeTabId = "main"`) so the next workspace starts with a clean chat pane.
- Release `<data_dir>/.lock` (drops `WorkspaceHandle`, which releases the lock).

---

## Concurrency & Data Safety

Following the decision to **not** adopt the heavy thread-safety blueprint, the model is:

1. **Single-instance per workspace** via the `<data_dir>/.lock` PID file (`~/.ire/workspaces/<id>/.lock`).
   - Created with `OpenOptions::write().create_new(true)` (atomic).
   - Stale detection: parse PID; if not alive (`kill -0` / `OpenProcess`), reclaim.
   - Released on graceful shutdown; orphan-safe via stale reclaim.
2. **In-process serialisation** of wiki writes via `tokio::Mutex<()>` held by `WikiStore`.
3. **Atomic file replacement** for every wiki mutation: temp file in same dir → `fs::rename`. `sync_all` on the temp file before rename.
4. **Agent turn serialisation per session**: one outstanding agent subprocess per session id; new sends queue.
5. **Experiment subprocesses** are detached with their own process group; they outlive an agent subprocess crash.

What we explicitly **do not** do (vs. the vault blueprint): file-level advisory lock for the cache, fingerprint CAS, rename WAL with crash recovery, filesystem watcher with noise filtering. If we ever need them (e.g. to support multi-window per workspace), `docs/blueprints/vault-thread-safety.md` is a ready reference.
