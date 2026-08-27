# Experiments

An experiment is a detached shell subprocess the agent (Claude Code / Codex / OpenCode) launches on its own initiative to test something, e.g. an ablation run, a build, a script. There is no "Run experiment" button; the trigger is conversational (see [Triggering](#triggering-mcp-tools--the-experiment-workflow-rule)), and the whole mechanism exists because CLI-agent sessions don't stay alive waiting on a background job. When the process exits it does **not** wake anything up automatically — see [Wake-up](#wake-up) for how the user hands the result to a chat, and how it gets marked reviewed, instead.

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
T5  Process exits. Backend updates the DB row + `EXPERIMENT.md`, leaving
    `run_status` at `completed`/`failed` — no agent turn fires automatically.
    The UI only signals: an amber dot on the sidebar row and a status pill.
    See Wake-up below.
T6  The user hands the result to whichever chat they want — copy the restart
    prompt, or drag the sidebar row onto a chat tab (or "+" for a new one),
    which drops the text into that tab's composer. They send it themselves.
    The prompt is built from the record's
    `## Goal & context` + exit code + where to read the rest. The agent then
    pulls output itself via `experiment.tail_logs`, reads result files,
    uses memory.write_short_term for daily notes, memory.write_long_term for
    durable conclusions, and ire.read + ire.edit to update focus/notes/ideas
    if the research direction changed.
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
  run_status: running          # running | completed | failed | cancelled | reviewed | unknown
  exit_code: null
  ended_at: null
  ---
  ```

  The run state is `run_status`, **not** `status`: OKF §5.4 reserves `status` for a
  document's lifecycle (`draft | stable | deprecated`), and one key cannot carry both
  meanings once other concept types share the bundle.

  `run_status` is the **whole** lifecycle, including whether the result has been
  reviewed — there is no separate flag for that. `running` moves to one of
  `completed`/`failed`/`cancelled` when the process exits; any of those three moves
  to `reviewed` once the agent has engaged with the result and called
  `experiment.mark_reviewed`. `exit_code`/`ended_at` are untouched by that
  last move, so they still say how and when the run ended after the status text
  itself has moved on. See [Wake-up](#wake-up) for how the two transitions differ,
  and [docs/product/experiment-lifecycle.md](../product/experiment-lifecycle.md) for
  the product-level framing.

  **Ownership boundary.** The frontmatter block is exclusively runner-owned and
  atomically rewritten on every status transition. Everything below it — the `# {name}`
  H1, `## Goal & context` (the `wake_prompt` the agent passed to `experiment.start`,
  kept nowhere else), `## Command` in a fenced block, and anything an agent or user
  appends — is never touched by a transition rewrite. The H1 is kept even though `title` is in
  frontmatter, since GitHub's markdown renderer does not render YAML specially. The file
  ends at the command; the rest of the folder is the run's own home for scripts, result
  files, and notes.

  `<NNN>` is a zero-padded three-digit prefix, allocated as one past the highest already
  present — gaps from deleted folders are never reissued. Allocation and folder creation
  happen under an in-process mutex, so two experiments starting at once can't claim the
  same number. `<slug>` is the name lowercased to ASCII alphanumerics with every other
  run of characters collapsed to `-`, capped at 60 characters (`experiment` if nothing
  survives). The folder is created before the subprocess spawns and removed again if the
  DB insert or the spawn fails, so a folder implies a run that actually started.
- **`local.db` `experiments` table** (SQLite, `~/.ire/workspaces/<id>/local.db`, purely
  ephemeral operational state — a run's outcome, including whether it has been
  reviewed, is not here; see below) — `src-tauri/src/db/schema.rs`:
  ```sql
  CREATE TABLE experiments (
    uuid TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    command TEXT NOT NULL,
    working_dir TEXT NOT NULL,
    started_at TEXT NOT NULL,
    pid INTEGER,
    session_id TEXT NOT NULL,         -- the chat session that started the run (informational only)
    tab_id TEXT NOT NULL DEFAULT 'main',
    record_dir TEXT                   -- workspace-relative folder holding EXPERIMENT.md
  );
  ```

`local.db` holds only the fields that have no reason to be shared or committed
(`pid`, `session_id`, `tab_id`, `record_dir`). A run's outcome —
`run_status`/`exit_code`/`ended_at` — lives exclusively in `EXPERIMENT.md`'s
frontmatter (`experiments::ExperimentView`, composed by `experiments::overlay`,
carries it in). Whether a run has been reviewed is part of that same
`run_status` field, not a separate flag — it is a fact about the run, not
about which machine or person is looking at it, so it belongs where the rest
of the run's outcome already lives and git-tracks the same way. The
goal/context is not in `local.db` either: it is read back out of the record's
`## Goal & context` at wake-up time, so it survives a clone or a cleared
database.

**Write path.** `experiments::transition` rewrites the record's frontmatter (never
`local.db`, which carries no outcome fields to write), then **re-reads the file** and
emits `experiment-changed` from what was actually persisted — never from in-process
state assembled separately. That read-back is the one-source-of-truth guarantee: the
event cannot say something the file does not.

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
workspace open, between two schema passes: `schema::run_pre_backfill` stops before the
migration that drops `wake_prompt`, the backfill copies each experiment's goal out of it,
and `schema::run` then takes the schema the rest of the way. Experiments that exist only
in `ire.json` — runs predating the git-tracked folder — are given a record folder of their
own, numbered in start order, so nothing disappears from the sidebar. Records written before the frontmatter existed are backfilled from
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
4. End your turn — do **not** wait. There is no automatic resume: the run finishes on its own and sits
   unreviewed until a chat is opened about it.
5. If resumed about one (a wake-up, or the user raises an old unreviewed run): read the logs from the
   `wake_prompt` context (or `experiment.tail_logs`), then proceed accordingly (e.g., report to the user, append
   findings to the experiment's EXPERIMENT.md body, update memories, propose next steps or whatever action you
   deem appropriate based on the results).
6. Once you've actually looked at and discussed the result, call `experiment.mark_reviewed`. Nothing does this
   for you — merely opening a chat about a run doesn't mark it reviewed.
```

The four MCP tools that back this (schemas from `src-tauri/src/mcp/stdio_server.rs`;
general MCP catalog format in [mcp.md — Tool Catalog](mcp.md#tool-catalog)):

| Tool | Params | Notes |
|---|---|---|
| `experiment.start` | `name` (required), `command` (required, `sh -c` string), `working_dir` (optional, defaults to workspace root), `wake_prompt` (required) | Returns `{ uuid, status: "started", dir }` immediately, where `dir` is the workspace-relative wiki folder for the run. |
| `experiment.status` | `uuid` (required) | Returns `{ status, exit_code?, started_at, ended_at? }`. |
| `experiment.tail_logs` | `uuid` (required), `kb` (optional, default 64) | Tail of stdout/stderr from `.ire/cache/experiments/<uuid>/`. |
| `experiment.mark_reviewed` | `uuid` (required) | Moves `run_status` to `reviewed` from any of `completed`/`failed`/`cancelled`; a no-op if still `running` or already `reviewed`. Returns `{ uuid, status }`. |

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
- Calls `experiments::transition` with `"completed"` (exit code 0) or `"failed"`
  (anything else, including a `try_wait` error, which is also logged and recorded
  as exit code `-1`), which rewrites `EXPERIMENT.md`'s frontmatter — `run_status`,
  `exit_code`, `ended_at` — and emits `experiment-changed` from the re-read file.
- Sends nothing to any agent. The run just sits at `completed`/`failed`,
  unreviewed, until the user hands it to a chat — see [Wake-up](#wake-up).

---

## Wake-up

There is no automatic resume on completion, and no UI action that sends
anything: a finished run sits at `run_status: completed`/`failed`/`cancelled`
— unreviewed — until the user hands it to a chat themselves. The whole
backend surface is one read-only command in
`src-tauri/src/commands/experiments.rs`:

- **`experiment_wake_prompt({ uuid }) -> String`** — composes the restart
  message (goal + exit code + where to read the rest) via
  `experiments::wake::compose_message` and returns it. It spawns nothing,
  resumes nothing, and does not touch `run_status`.

The frontend offers two ways to move that text into a chat, both ending in a
composer the user still has to press send on:

- **Copy** — a copy icon on unresolved sidebar rows and in the experiment
  tab's header writes the prompt to the clipboard.
- **Drag-and-drop** — unresolved sidebar rows are `draggable`, carrying the
  uuid under the `application/x-ire-experiment-uuid` mime
  (`EXPERIMENT_DRAG_MIME`, exported from `src/components/chat/TabBar.tsx`).
  Dropping one on a chat tab, or on the "+" new-tab button, fetches the
  prompt and puts it in that tab's composer via the chat store's
  `pendingComposerText` (appended to any existing draft, never clobbering
  it). Sending it is then an ordinary `chat_send` turn — nothing about it is
  a special wake-up path, and it is not tied to the session that started the
  run.

  This requires `"dragDropEnabled": false` on the window in
  `src-tauri/tauri.conf.json`. Tauri v2 defaults it to `true`, which lets the
  webview's native OS drag-drop handler swallow the `drop` event — `dragover`
  still fires, so the cursor shows a drop target and nothing lands. Nothing in
  the app consumes Tauri's file-drop events, so turning it off costs nothing;
  it would have to come back (and this drag be reworked) if IRE ever accepts
  files dragged in from Finder.

The sidebar's amber dot is purely a signal: clicking it opens the
experiment's tab to read logs and status, and marks nothing.

Marking `reviewed` is therefore only ever done by the agent, via the MCP tool
`experiment.mark_reviewed({ uuid })` (`src-tauri/src/mcp/rpc.rs`,
`experiment_mark_reviewed`). The restart prompt
(`src-tauri/assets/prompts/experiment_wakeup.md`) tells the agent to call it
once it has actually looked at and discussed the result — the same tool an
agent can call unprompted if it notices an old unreviewed run. There is no UI
path that marks a run reviewed on its own.

`record::mark_reviewed` (`src-tauri/src/experiments/record.rs`) is a guarded
transition: it only moves `run_status` to `reviewed` from one of
`completed`/`failed`/`cancelled` — a still-`running` experiment can't be
reviewed, and reviewing twice is a safe no-op. `exit_code`/`ended_at` are left
exactly as they were: they still say how and when the run ended, even once
the status text has moved past that. A cancelled run can be reviewed too —
cancelling isn't necessarily the end of the conversation.

`compose_message` (`src-tauri/src/experiments/wake.rs`) is pure text
assembly: it fills the seed template
`src-tauri/assets/prompts/experiment_wakeup.md` with the goal read back from
the record's `## Goal & context`, `uuid`, `exit_code`, and the wiki folder
path — pointing the agent at `EXPERIMENT.md` and at that folder for result
files.

It deliberately inlines **no** log output. The tails used to be embedded
here (8KB of each stream), which made the prompt mostly log and buried the
instruction under it — and since the text now lands in the user's composer
for them to read and send, that cost is visible rather than hidden. The
agent pulls output on demand instead, via the `experiment.tail_logs` MCP
tool the prompt names; raw logs are still at
`.ire/cache/experiments/<uuid>/`. On exit code 126/127 (permission denied / command not
found) that template tells the agent not to retry `experiment.start` — report
to the user and stop instead. A run whose record has gone still composes a
message, just without its goal.

**Concurrency.** There is nothing to serialize: the restart prompt reaches a
session as a normal user message, so it queues behind a running turn exactly
like any other one (see
[chat-agents.md — Session management](chat-agents.md#session-management)).

---


---

## Cancellation, deletion, rename

All three are Tauri commands in `src-tauri/src/commands/experiments.rs`, driven from the
UI (not exposed to the agent via MCP):

- **`experiment_cancel({ uuid })`** — sends `SIGTERM` to the whole process group
  (`killpg` on Unix; `taskkill /F /T /PID` on Windows), marks the run cancelled
  immediately (not via the monitor loop) in both the DB row and `EXPERIMENT.md`, and
  emits `experiment-changed`. Note the monitor thread spawned in `start_experiment` is
  still running and will separately observe the child exit and attempt its own
  transition, reporting a signalled process as `failed` with exit `-1`; it is
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
  for confirmation. Rows whose `status` is `completed`/`failed`/`cancelled`
  (unreviewed) get three things: a small amber dot that opens the experiment's
  tab, a copy button for the restart prompt (`ipc.experimentWakePrompt`), and
  `draggable` — dragging the row carries its uuid under `EXPERIMENT_DRAG_MIME`
  to a chat tab. None of them mark the run reviewed, per [Wake-up](#wake-up).
- **`TabBar`** accepts an experiment drop on any chat tab or on "+", and
  **`ChatPane.handleExperimentDrop`** fetches the prompt, makes the target tab
  active (creating it first for "+"), and parks the text in the chat store's
  `pendingComposerText`. **`Composer`** picks that up for its own `tabId`,
  appending it to any unsent draft, and clears it — the user sends it.
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
