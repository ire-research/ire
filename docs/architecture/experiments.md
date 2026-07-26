# Experiments

An experiment is a detached shell subprocess the agent (Claude Code / Codex / OpenCode) launches on its own initiative to test something, e.g. an ablation run, a build, a script. It gets **woken up** once the process exits. There is no "Run experiment" button; the trigger is conversational (see [Triggering](#triggering-mcp-tools--the-experiment-workflow-rule)), and the whole mechanism exists because CLI-agent sessions don't stay alive waiting on a background job.

---

## Design Principles

**Lean.** Minimal moving parts. Cheap at idle. Anything added in on top of the experiment feature should first meet this principle.

**Robust.** An experiment's state must never depend on the process, the app, or the machine it ran on staying alive to remain true. The system reconciles its own ground truth on its own; nobody has to remember an experiment happened or notice that it didn't finish cleanly.

**Self-contained.** Every experiment's own artifact carries everything needed to interpret it: what was run, why, and what "done" looks like. It must be readable cold by any agent (weak or strong) or person, at any point in time.

This document describes the system as built. Where a later section falls short of a
principle above, that's an open item, tracked as a GitHub issue — not a reason to
weaken the principle.

---

## Lifecycle (the wake-up pattern)

```
T0  User asks: "Run an ablation over learning rates [1e-3, 1e-4, 1e-5]."
T1  Agent plans, gets agreement, then calls MCP tool experiment.start({
       name: "lr-ablation",
       command: "python scripts/ablate_lr.py --output runs/lr_ablation",
       working_dir: "<project root>",
       wake_prompt: "Experiment lr-ablation finished. Read its result.md and
                     logs, then update the wiki and pulse."
    })                                              — see Triggering below.
T2  MCP server forwards to IRE's Rust backend, which inserts an experiment row
    and spawns the command as a DETACHED process group:
       Command::new("sh").args(["-c", &command])
             .current_dir(working_dir)
             .stdin(Stdio::null())
             .process_group(0)             // setsid
             .env_remove("CLAUDECODE")
             .spawn()
    returning { uuid, status: "started" } to the agent — see Data model and
    Spawn & monitor below.
T3  Agent's response to the user: "Started experiment <uuid>; I'll come back
    when it's done." Then this agent turn ENDS naturally.
T4  Backend monitor thread polls the child every 500ms and tails new log
    lines to the frontend — see Spawn & monitor below.
T5  Process exits. Backend updates the DB row + `ire.json`, then resumes the
    same agent session with a wake-up message built from `wake_prompt` +
    exit code + log tail — see Wake-up below.
T6  Agent reads result files, uses memory.write_short_term for daily notes,
    memory.write_long_term for durable conclusions, and ire.read + ire.edit
    to update focus/notes/ideas if the research direction changed.
```

The user can keep using the chat pane during T3–T5, and can cancel or delete an
experiment at any point — see [Cancellation, deletion, rename](#cancellation-deletion-rename).
Rendering of the running/finished experiment in the UI is covered in
[Frontend & UI](#frontend--ui).

---

## Data model

Experiments are **duplicated by design** across two stores:

- **`ire.json`** (git-tracked, the shareable display subset) — `IreExperiment` in
  `src-tauri/src/ire/store.rs`:
  ```json
  {
    "uuid": "…", "name": "…", "command": "…", "status": "running",
    "started_at": "RFC3339", "ended_at": null, "exit_code": null
  }
  ```
- **`local.db` `experiments` table** (SQLite, `~/.ire/workspaces/<id>/local.db`, the
  operational superset) — `src-tauri/src/db/schema.rs`:
  ```sql
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
    session_id TEXT NOT NULL,         -- chat session_uuid whose resume id the wake-up uses
    tab_id TEXT NOT NULL DEFAULT 'main'
  );

  CREATE INDEX idx_experiments_status ON experiments(status);
  CREATE INDEX idx_experiments_started ON experiments(started_at DESC);
  ```

`local.db` retains the operational fields (`pid`, `working_dir`, `wake_prompt`,
`session_id`, `tab_id`) that have no reason to be shared/committed; `ire.json` keeps
only what's meaningful to read on a fresh clone (logs and operational data are absent
there). `experiments::sync_to_ire` (`src-tauri/src/experiments/mod.rs`) mirrors a DB
row into `ire.json` on every state transition; `remove_from_ire` does the same for
deletion. Neither of these emits a `workspace-event` — that's the caller's job (see
[Spawn & monitor](#spawn--monitor)).

**Frontend type** (`src/types.ts`):
```ts
type ExperimentStatus = "starting" | "running" | "completed" | "failed" | "cancelled";

interface ExperimentRow {
  uuid: string; name: string; command: string; status: string;
  exit_code: number | null; started_at: string; ended_at: string | null; tab_id: string;
}
```
`"starting"` is frontend-only — it covers the gap between the agent's tool call
returning and the `experiment-starting` event linking a `uuid`/`pid` to the pending UI
card (see [Frontend & UI](#frontend--ui)); it never appears in the DB or `ire.json`,
whose `status` starts directly at `"running"`.

Logs are not part of either record: stdout/stderr stream to
`.ire/cache/experiments/<uuid>/{stdout,stderr}.log` (gitignored), read on demand by
`experiment.tail_logs` / `experiment_logs`.

---

## Triggering: MCP tools & the Experiment Workflow rule

Every agent session gets these rules injected via `.ire/_SYSTEM.md` (see
[wiki-memory.md — context injection rules](wiki-memory.md#context-injection-rules)),
verbatim:

```
## Experiment Workflow

When asked to run an experiment:
1. Plan the run and get user agreement.
2. Verify the setup first (e.g., binary exists, paths resolve) to avoid cluttering with failed experiments.
3. Call `experiment.start` with `name`, `command`, and a `wake_prompt`. The `wake_prompt` is given back to you
   when the process finishes — include all relevant context so you know exactly what you were testing and what
   to do with the results.
4. End your turn — do **not** wait. IRE resumes this same agent session when the process exits.
5. On wake-up: read the logs from the `wake_prompt` context (or `experiment.tail_logs`), then proceed accordingly
   (e.g., report to the user, update ire.json, update memories, propose next steps or whatever action you deem
   appropriate based on the results).
```

The three MCP tools that back this (schemas from `src-tauri/src/mcp/stdio_server.rs`;
general MCP catalog format in [mcp.md — Tool Catalog](mcp.md#tool-catalog)):

| Tool | Params | Notes |
|---|---|---|
| `experiment.start` | `name` (required), `command` (required, `sh -c` string), `working_dir` (optional, defaults to workspace root), `wake_prompt` (required) | Returns `{ uuid, status: "started" }` immediately. |
| `experiment.status` | `uuid` (required) | Returns `{ status, exit_code?, started_at, ended_at? }`. |
| `experiment.tail_logs` | `uuid` (required), `kb` (optional, default 64) | Tail of stdout/stderr from `.ire/cache/experiments/<uuid>/`. |

`experiment.start` requires an active agent turn to attach to — the MCP handler
(`src-tauri/src/mcp/rpc.rs`) rejects the call with "no active agent session" if none is
running. There is no `experiment.list` MCP tool; the agent reads experiment history
from `ire.json` via `ire.read` instead.

---

## Spawn & monitor

`start_experiment` (`src-tauri/src/experiments/runner.rs`):

1. Generates a `uuid`, creates `.ire/cache/experiments/<uuid>/{stdout,stderr}.log`,
   inserts a `status='running'` DB row (`db::insert_experiment`).
2. Spawns the command detached (`spawn_detached`): `sh -c <command>`, `stdin(Stdio::null())`,
   its own process group (`process_group(0)` / `setsid` on Unix, `CREATE_NEW_PROCESS_GROUP`
   on Windows), `CLAUDECODE` removed from the environment — so killing IRE, or the agent
   CLI exiting, does not kill the experiment.
3. Records the `pid` (`db::update_experiment_pid`), emits `experiment-status`
   (`{ uuid, status: "running" }`) and `experiment-starting` (`{ tab_id, uuid, pid }` —
   the bridge event that lets the frontend link this `uuid` to the pending
   `experiment_start` tool card), then syncs the row to `ire.json` and emits
   `experiment-changed`.
4. Spawns a background `monitor` task (via `spawn_blocking`) that loops every 500ms:
   tails any new bytes written to `stdout.log`/`stderr.log` since the last read,
   emitting one `experiment-log-line` event per new line, and calls `try_wait()` on
   the child.

On exit, the monitor:
- Drains any remaining log output.
- Sets `status` to `"completed"` (exit code 0) or `"failed"` (anything else, including
  a `try_wait` error, which is also logged and recorded as exit code `-1`) via
  `db::update_experiment_completed`.
- Emits `experiment-status` with the final status/exit code, syncs the row to
  `ire.json`, emits `experiment-changed`.
- Calls `experiments::wake::fire_wakeup` synchronously, from the same monitor thread —
  see [Wake-up](#wake-up).

---

## Wake-up

`fire_wakeup` (`src-tauri/src/experiments/wake.rs`) resumes the **same provider session**
that started the experiment:

1. Reads the last 8KB of `stdout.log`/`stderr.log` (`tail_file`) and composes the
   wake-up message from the seed template `src-tauri/assets/prompts/experiment_wakeup.md`
   (embeds `wake_prompt`, `uuid`, `exit_code`, and both log tails). On exit code 126/127
   (permission denied / command not found) that template tells the agent not to retry
   `experiment.start` — report to the user and stop instead.
2. Rebuilds the composed system prompt (`build_system_prompt`) exactly as a normal turn
   would.
3. Branches on `agent.transport()`:
   - **OpenCode** (`TurnTransport::OpenCodeServer`): calls `opencode::turn::send` with
     the wake-up message and `tab_label: "Wake-up"` — the same entry point every
     OpenCode turn goes through (see
     [chat-agents.md — OpenCode Server Transport](chat-agents.md#opencode-server-transport)).
   - **Claude / Codex** (`TurnTransport::CliSubprocess`): looks up the persisted resume
     id (`chat_resume_ids(session_uuid, provider)`), builds the CLI command via
     `CliTurn::build_command` with that `resume_id`, spawns it, and parses its JSONL
     stdout line-by-line into `chat-stream` events exactly like a normal turn (a fresh
     `stream_id`, per-process `event_id` counter, persisting any new resume id seen on
     `Init`).
4. If the subprocess stream never emits `Done` (e.g. it dies mid-stream), `fire_wakeup`
   emits a synthetic `Done` itself so the frontend doesn't hang waiting.

**Concurrency.** The wake-up and a live user message share the same provider-scoped
session id; only one subprocess runs per session at a time. Whichever arrives first
runs first — if a user message arrives while a wake-up is running, it queues; if a
wake-up fires while the user is mid-turn, it queues. The UI surfaces this as "1 wake-up
pending" (see [chat-agents.md — Session management](chat-agents.md#session-management)
for the general `SessionManager`/`RunningTurn` machinery this shares with ordinary chat
turns).

---

## Cancellation, deletion, rename

All three are Tauri commands in `src-tauri/src/commands/experiments.rs`, driven from the
UI (not exposed to the agent via MCP):

- **`experiment_cancel({ uuid })`** — sends `SIGTERM` to the whole process group
  (`killpg` on Unix; `taskkill /F /T /PID` on Windows), marks the DB row
  `status="cancelled"` immediately (not via the monitor loop), syncs to `ire.json`, and
  emits `experiment-changed`. Note the monitor thread spawned in `start_experiment` is
  still running and will separately observe the child exit and call `fire_wakeup` as
  usual — so the agent still gets woken up, now seeing `status: "cancelled"`.
- **`experiment_delete({ uuid })`** — rejected while `status` is `"running"` or
  `"starting"`. Otherwise removes the `.ire/cache/experiments/<uuid>/` log directory,
  the DB row, and the `ire.json` entry, and emits `experiment-deleted`.
- **`experiment_rename({ uuid, name })`** — updates `name` only, re-syncs to `ire.json`,
  emits `experiment-changed`.

Read-only commands: `experiment_list({ limit? })` and `experiment_logs({ uuid, kb? })`
(tail stdout/stderr by KB, default 64KB) back the sidebar list and log views — see
[Frontend & UI](#frontend--ui).

---

## Frontend & UI

Covered in full in [frontend.md](frontend.md); summarized here for the experiment-specific
pieces:

- **`ExperimentsSection`** (left rail) lists experiments via `experiment_list`;
  supports rename/delete but not creation — experiments are only ever started by the
  agent.
- **`ExperimentCard`** — how an `experiment_start` tool call renders inline in chat
  (instead of a generic `ToolCard`): collapsed by default, header has a status dot
  (blinking amber while running, solid green/red on completion), the canonical tool
  title, a status badge, optional PID/exit label, and a **Cancel** button visible only
  while running. Expanded body shows the last 10 live log lines.
- **`ExperimentTabView`** — clicking an experiment opens a dedicated tab
  (`kind: "experiment"`): name header + status badge, a metadata grid (status +
  elapsed timer, runtime, command), and a scrollable stdout log pane that auto-scrolls
  as `experiment-log-line` events arrive. Elapsed time ticks every second while
  running.
- **Events consumed**: `experiment-starting`, `experiment-status`, `experiment-log-line`
  (all uuid-scoped, not wrapped in `workspace-event`), plus the `workspace-event`
  variants `experiment-changed` / `experiment-deleted` used for the git-tracked/list
  view — full payload shapes in
  [frontend.md — Tauri IPC Surface](frontend.md#tauri-ipc-surface).
