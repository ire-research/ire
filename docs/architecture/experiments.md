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
    returning { uuid, status: "started", dir } to the agent — see Data model
    and Spawn & monitor below.
T3  Agent's response to the user: "Started experiment <uuid>; I'll come back
    when it's done." Then this agent turn ENDS naturally.
T4  Backend monitor thread polls the child every 500ms and tails new log
    lines to the frontend — see Spawn & monitor below.
T5  Process exits. Backend updates the DB row + `EXPERIMENT.md`, then resumes the
    same agent session with a wake-up message built from the record's
    `## Goal & context` + exit code + log tail — see Wake-up below.
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

Experiments live in **two** stores, split by ownership rather than duplicated:

- **`.ire/experiments/<NNN>-<slug>/EXPERIMENT.md`** (git-tracked) — the single source of
  truth for what ran and how it ended, created on start by `experiments::record`
  (`src-tauri/src/experiments/record.rs`). It is an
  [Open Knowledge Format](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md)
  concept: a YAML frontmatter block over a markdown body.

  ```yaml
  ---
  type: Experiment
  title: LR ablation
  uuid: 11111111-2222-3333-4444-555555555555
  started_at: 2026-08-11T10:00:00+02:00
  working_dir: /tmp/project
  run_status: running          # running | completed | failed | cancelled | unknown
  exit_code: null
  ended_at: null
  ---
  ```

  The run state is `run_status`, **not** `status`: OKF §5.4 reserves `status` for a
  document's lifecycle (`draft | stable | deprecated`), and one key cannot carry both
  meanings once other concept types share the bundle.

  **Ownership boundary.** The frontmatter block is exclusively runner-owned and
  atomically rewritten on every status transition. Everything below it — the `# {name}`
  H1, `## Goal & context` (the `wake_prompt` the agent passed to `experiment.start`,
  kept nowhere else), `## Command` in a fenced block, and anything an agent or user
  appends — is never touched by a transition rewrite. The H1 is kept even though `title` is in
  frontmatter, since GitHub's markdown renderer does not render YAML specially. The rest
  of the folder is the run's own home for scripts, result files, and notes.

  `<NNN>` is a zero-padded three-digit prefix, allocated as one past the highest already
  present — gaps from deleted folders are never reissued. Allocation and folder creation
  happen under an in-process mutex, so two experiments starting at once can't claim the
  same number. `<slug>` is the name lowercased to ASCII alphanumerics with every other
  run of characters collapsed to `-`, capped at 60 characters (`experiment` if nothing
  survives). The folder is created before the subprocess spawns and removed again if the
  DB insert or the spawn fails, so a folder implies a run that actually started.
- **`local.db` `experiments` table** (SQLite, `~/.ire/workspaces/<id>/local.db`, purely
  ephemeral operational state) — `src-tauri/src/db/schema.rs`:
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
    session_id TEXT NOT NULL,         -- chat session_uuid whose resume id the wake-up uses
    tab_id TEXT NOT NULL DEFAULT 'main',
    record_dir TEXT                   -- workspace-relative folder holding EXPERIMENT.md
  );

  CREATE INDEX idx_experiments_status ON experiments(status);
  CREATE INDEX idx_experiments_started ON experiments(started_at DESC);
  ```

`local.db` holds the fields that have no reason to be shared or committed (`pid`,
`session_id`, `tab_id`, `record_dir`). The goal/context is **not** among them: it is
read back out of the record's `## Goal & context` at wake-up time, so it survives a
clone or a cleared database, and an edit to that section is what the agent is handed. Its `status`/`exit_code`/`ended_at`
columns are written first on a transition and then superseded by the file: `experiments::row`
(`src-tauri/src/experiments/mod.rs`) composes the row the UI sees by overlaying what
`EXPERIMENT.md` says on top of the database row, so the two can never disagree about how a
run ended.

**Write path.** `experiments::transition` updates the DB row, rewrites the frontmatter,
then **re-reads the file** and emits `experiment-changed` from what was actually
persisted — never from in-process state assembled separately. That read-back is the
one-source-of-truth guarantee: the event cannot say something the file does not.

**`reconcile()` is unchanged** — experiment records are not watched. Only the
*frontmatter* is runner-owned; the body and the rest of the folder are explicitly not,
so both can and do change outside the app (an agent appending findings on a wake-up, a
user editing notes, a run dropping result files in). Nothing detects those edits live:
they surface on the next workspace open, when hydrate re-reads every record.

That is a known gap, not a guarantee. What makes it tolerable is that the fields the UI
renders — `run_status`, `exit_code`, `ended_at` — are the ones only the runner writes,
so a stale view can never show the wrong outcome. Edits made anywhere else survive a
transition: `rewrite` parses the existing block and sets only the runner-owned keys on
it, so added keys (`tags`, `description`, anything else) and their order are preserved,
per OKF §4.1. Watching the records live is tracked separately.

**Migration.** `experiments::migrate` (`src-tauri/src/experiments/migrate.rs`) runs on
workspace open. Records written before the frontmatter existed are backfilled from
`local.db` (falling back to the outgoing `ire.json` entry, then to `run_status: unknown`),
the restated `- **uuid**` / `- **started**` / `- **working dir**` header bullets the
frontmatter replaces are dropped, `record_dir` is filled in, and `ire.json`'s
`experiments` array is removed from the file for good. It is idempotent and best-effort:
a workspace it cannot upgrade still opens, and the next open tries again.

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
card (see [Frontend & UI](#frontend--ui)); it never appears in the DB or in
`EXPERIMENT.md`, whose run state starts directly at `"running"`.

Logs are not part of any of the three: stdout/stderr stream to
`.ire/cache/experiments/<uuid>/{stdout,stderr}.log` (gitignored), read on demand by
`experiment.tail_logs` / `experiment_logs`. Keeping raw logs local while the curated
record is git-tracked is a deliberate split — see
[experiment-lifecycle-v2.md](../proposals/experiment-lifecycle-v2.md).

`started_at` is generated once in `start_experiment` and passed to both
`db::insert_experiment` and `EXPERIMENT.md`, so they carry the same timestamp.

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
   (e.g., report to the user, append findings to the experiment's EXPERIMENT.md body, update memories, propose next steps or whatever action you deem
   appropriate based on the results).
```

The three MCP tools that back this (schemas from `src-tauri/src/mcp/stdio_server.rs`;
general MCP catalog format in [mcp.md — Tool Catalog](mcp.md#tool-catalog)):

| Tool | Params | Notes |
|---|---|---|
| `experiment.start` | `name` (required), `command` (required, `sh -c` string), `working_dir` (optional, defaults to workspace root), `wake_prompt` (required) | Returns `{ uuid, status: "started", dir }` immediately, where `dir` is the workspace-relative wiki folder for the run. |
| `experiment.status` | `uuid` (required) | Returns `{ status, exit_code?, started_at, ended_at? }`. |
| `experiment.tail_logs` | `uuid` (required), `kb` (optional, default 64) | Tail of stdout/stderr from `.ire/cache/experiments/<uuid>/`. |

`experiment.start` requires an active agent turn to attach to — the MCP handler
(`src-tauri/src/mcp/rpc.rs`) rejects the call with "no active agent session" if none is
running. There is no `experiment.list` MCP tool; the agent reads experiment history
by reading `.ire/experiments/<NNN>-<slug>/EXPERIMENT.md` instead.

---

## Spawn & monitor

`start_experiment` (`src-tauri/src/experiments/runner.rs`):

1. Generates a `uuid` and a canonical `started_at`, creates
   `.ire/cache/experiments/<uuid>/{stdout,stderr}.log`, creates the wiki record
   `.ire/experiments/<NNN>-<slug>/EXPERIMENT.md` (`experiments::record::create`), then
   inserts a `status='running'` DB row (`db::insert_experiment`). If the insert or the
   spawn in step 2 fails, `experiments::record::remove` deletes the folder again.
2. Spawns the command detached (`spawn_detached`): `sh -c <command>`, `stdin(Stdio::null())`,
   its own process group (`process_group(0)` / `setsid` on Unix, `CREATE_NEW_PROCESS_GROUP`
   on Windows), `CLAUDECODE` removed from the environment — so killing IRE, or the agent
   CLI exiting, does not kill the experiment.
3. Records the `pid` (`db::update_experiment_pid`), emits `experiment-status`
   (`{ uuid, status: "running" }`) and `experiment-starting` (`{ tab_id, uuid, pid }` —
   the bridge event that lets the frontend link this `uuid` to the pending
   `experiment_start` tool card), then emits
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
  `EXPERIMENT.md`, emits `experiment-changed` from the re-read file.
- Calls `experiments::wake::fire_wakeup` synchronously, from the same monitor thread —
  see [Wake-up](#wake-up).

---

## Wake-up

`fire_wakeup` (`src-tauri/src/experiments/wake.rs`) resumes the **same provider session**
that started the experiment:

1. Reads the last 8KB of `stdout.log`/`stderr.log` (`tail_file`) and composes the
   wake-up message from the seed template `src-tauri/assets/prompts/experiment_wakeup.md`
   (embeds the goal read back from the record's `## Goal & context`, `uuid`, `exit_code`,
   the wiki folder path, and both log tails),
   pointing the agent at `EXPERIMENT.md` and at that folder for result files. On exit code 126/127
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
  (`killpg` on Unix; `taskkill /F /T /PID` on Windows), marks the run cancelled
  immediately (not via the monitor loop) in both the DB row and `EXPERIMENT.md`, and
  emits `experiment-changed`. Note the monitor thread spawned in `start_experiment` is
  still running and will separately observe the child exit and call `fire_wakeup` as
  usual — so the agent still gets woken up, still seeing `run_status: cancelled`. That
  second transition reports a signalled process as `failed` with exit `-1`; it is
  discarded because `transition` refuses to move a run that already reached a terminal
  state, which is what keeps the cancellation in git history.
- **`experiment_delete({ uuid })`** — rejected while `status` is `"running"` or
  `"starting"`. Otherwise removes the `.ire/cache/experiments/<uuid>/` log directory,
  the DB row, and the whole git-tracked `.ire/experiments/<NNN>-<slug>/` folder
  (`record::remove`), then emits `experiment-deleted`. The folder has to go now that
  hydrate reads it — leaving the record behind would resurrect the deleted experiment on
  the next open. Because that also takes the run's scripts and result files, the UI
  confirms first (`ConfirmDeleteExperimentModal`, see [Frontend & UI](#frontend--ui));
  git history is what makes it recoverable. Its `<NNN>` is not reissued either.
- **`experiment_rename({ uuid, name })`** — updates `name` in the DB and `title` in the
  frontmatter (the body's H1 is left as written, since the body is not runner-owned),
  emits `experiment-changed`.

Read-only commands: `experiment_list({ limit? })` and `experiment_logs({ uuid, kb? })`
(tail stdout/stderr by KB, default 64KB) back the sidebar list and log views — see
[Frontend & UI](#frontend--ui).

---

## Frontend & UI

Covered in full in [frontend.md](frontend.md); summarized here for the experiment-specific
pieces:

- **`ExperimentsSection`** (left rail) renders the experiments the `workspaceData` event store holds (`experiment_list` seeds `ExperimentTabView`, not this);
  supports rename/delete but not creation — experiments are only ever started by the
  agent. The trash button opens **`ConfirmDeleteExperimentModal`** rather than deleting:
  the delete takes the run's whole folder, artifacts included, so it says so and waits
  for confirmation.
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
