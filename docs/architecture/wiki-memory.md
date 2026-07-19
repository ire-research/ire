# State & Memory

Covers the per-workspace state record (`ire.json`), file-based resources, the memory layer (long-term / short-term files), the context injection rules, and the SQLite schema.

The store layer is rooted at `.ire/` (`src-tauri/src/ire/`). There is no longer a `wiki/` subtree.

---

## `.ire/` layout

```
.ire/
  _SYSTEM.md            git-tracked  — always-injected framework context
  ire.json              git-tracked  — notes, focus, ideas, experiments
  long-term.md          git-tracked
  short-term/           git-tracked  — YYYY-MM-DD.md
  resources/            git-tracked
    _index.md           auto-generated (resources only)
    <slug>.md
  claims/               git-tracked
    _index.md           auto-generated (claims only)
    <id>.md
  cache/                gitignored   — ingestion temp + experiments/<uuid>/{stdout,stderr}.log
```

Runtime/local artifacts live outside the workspace, under `~/.ire/workspaces/<id>/`:

```
~/.ire/workspaces/<name>-<8-hex>/
  local.db    — chat_sessions + experiments (operational) only
  mcp.json    — MCP server config consumed by the agent
  mcp.sock    — Unix socket for the MCP RPC server
  .lock       — single-instance PID guard
```

UI/session state (panel sizes, tabs, chat options) is persisted by `tauri-plugin-store` in the app-data dir, keyed by workspace path — there is no `workspace.json`.

`.gitignore` entry written at init: `.ire/cache/` (the only workspace-local thing to ignore).

There is **no migration**: new workspaces only. The "is an IRE workspace" probe checks `.ire/_SYSTEM.md` + `.ire/ire.json`.

---

## `ire.json`

The git-tracked record of shareable state. Read/written through `IreStore` (`src-tauri/src/ire/store.rs`).

```json
{
  "notes": "free-form markdown",
  "focus": { "research_question": "", "this_week": "" },
  "ideas": [ { "text": "an idea" } ],
  "experiments": [
    { "uuid": "…", "name": "…", "command": "…", "status": "running",
      "started_at": "RFC3339", "ended_at": null, "exit_code": null }
  ]
}
```

### Read/write API

- `read_ire()` → typed `IreContent`; `read_ire_raw()` → `(raw_text, version)` where `version` is the sha256 of the on-disk bytes.
- `update_ire(app, mutate)` — UI setters (`save_notes`, `save_focus_field`, `save_ideas`) take this path: read-modify-write under a process-wide `IRE_LOCK`, then emit the `notes-changed` / `focus-changed` / `ideas-changed` events.
- `edit_ire(old, new, version, app)` — backs the `ire.edit` MCP tool. Validates `version` against the current on-disk hash (rejects a stale/missing version), requires `old` to be present and **unique**, applies the replacement, re-parses against the schema, writes, and emits the section events. The pure core is `apply_edit` (unit-tested).
- `upsert_experiment` / `remove_experiment` — the experiment runner and commands mirror DB rows into `ire.json` here; these **do not** emit section events (`experiment-changed` is owned by the runner).

Experiments are duplicated by design: `ire.json` holds the git-tracked **display** subset; `local.db.experiments` retains the **operational** fields (`pid`, `working_dir`, `wake_prompt`, `session_id`, `tab_id`). On a fresh clone, `ire.json` shows experiment history while logs/operational data are absent.

---

## Resources (file-based)

There is **no `resources` DB table**. Each resource lives entirely in `.ire/resources/<slug>.md` — title and `sources` in frontmatter:

```yaml
---
title: <human title>
sources:
  - <url or local path>
updated: 2026-05-03
TL;DR: <one-liner or 'Not relevant'>
---
```

- `IreStore::write_resource(rel_path, content, app)` — atomic write, regenerate `resources/_index.md`, emit `resource-changed { path, title, sources }` derived from the file.
- `IreStore::delete_resource(rel_path, app)` — remove the file, regenerate the index, emit `resource-deleted { path }`.
- `IreStore::list_resources()` — scan `resources/*.md` (skip `_index.md`/dotfiles), parse frontmatter; used for the open-workspace hydration burst.
- `resources/_index.md` is auto-generated: one bullet `- [title](./slug.md) — TL;DR/summary` per file, sorted by filename. The old master `_index.md` is gone; `resources/_index.md` and `claims/_index.md` (below) are the only auto-generated indexes. Resources are identified by their **file path** (`resources/<slug>.md`); there is no sha256 DB key.

Resources are read by the agent with the built-in `Read` tool; only ingestion goes through a tool (`resource.add`). See [chat-agents.md](chat-agents.md) for the ingestion pipeline.

---

## Claims (file-based)

The claim ledger, one belief per file. There is **no `claims` DB table** — same file-based shape as resources, no draft/approve UI (the agent writes directly, no ingestion pipeline). Each claim lives entirely in `.ire/claims/<id>.md`:

```yaml
---
type: Claim
id: <stable-kebab-id>
status: proposed | supported | contradicted | retracted
scope: <conditions this is asserted under>
asserted_by: owned | imported
revision: <integer, starts at 1>
depends_on: [<other claim id>]
contradicts: []
supersedes: []
---

<the statement, phrased so it's falsifiable>

## Falsification criterion
## Evidence
```

`type`/`status`/`scope`/`asserted_by`/`revision` are flat frontmatter scalars; `depends_on`/`contradicts`/`supersedes` are frontmatter lists (the hand-rolled `frontmatter::parse` handles scalars and flat string lists, not nested YAML, which is why `evidence[]` is a markdown section in the body rather than structured frontmatter — it needs typed sub-fields per entry that the parser can't represent).

- `IreStore::write_claim(rel_path, content) -> Result<BTreeMap<String, Vec<String>>>` — atomic write, regenerate `claims/_index.md`, return the dangling-relation report (below). No CAS: the caller sends the full file each time, so a revision is a full overwrite, not a diff. No event emission yet — there is no claims UI panel to listen for one.
- `claims/_index.md` is auto-generated by `index::build_claims`: one bullet `- [id](./id.md) — status — <first body paragraph>` per file, sorted by filename, plus an inline `  ⚠ dangling reference: <id>` line under any claim whose `depends_on`/`contradicts`/`supersedes` names an id with no matching file. `status` defaults to `proposed` if the frontmatter omits it.
- **Dangling-reference detection**: `build_claims` cross-checks every relation id against the full set of claim filenames in the same pass that builds the index — no separate validation step, so it can't drift from what's actually on disk. It returns `(index_markdown, dangling_by_id)`; `write_claim` propagates `dangling_by_id`, and `claim_write` (`rpc.rs`) looks up the just-written claim's own entry and adds `dangling_references`/`hint` fields to the tool result if it has any. The `⚠` line in `claims/_index.md` is the same information surfaced persistently (visible across turns, not just at write time) since the index is auto-injected into every system prompt.
- Backed by the MCP tool `claim.write` (`id`, `markdown`) — `src-tauri/src/mcp/rpc.rs::claim_write`, catalogued in `src-tauri/src/mcp/stdio_server.rs::tool_catalog`. The `id` is kebab-cased server-side; writing an existing id overwrites that claim's file (a revision).
- Injected into the system prompt alongside `resources/_index.md` (see Context injection rules below) so the agent can check for an existing claim before writing a duplicate.
- Declare/evaluate binding to experiments is not a structured field yet — deliberately. The existing `experiment.start` `wake_prompt` (free text, echoed back verbatim at wake — see [chat-agents.md](chat-agents.md)) is the binding mechanism for now: the agent folds a claim `id` + predicted outcome into `wake_prompt`, and evaluates against it at wake-up by calling `claim.write` again. `claim_id`/`predicted_outcome` as real fields on `IreExperiment`/`ExperimentRow` is deferred until this free-text loop proves out with real usage.

### Atomic write contract

`IreStore::atomic_write` writes to `<path>.<uuid>.tmp` in the same dir, `sync_all()`, then `fs::rename` (atomic on local FS). `ire.json` mutations additionally serialize under the in-process `IRE_LOCK`. No CAS, no WAL — single-instance is enforced by `.lock` (see [workspace.md](workspace.md#concurrency--data-safety)).

### Git commit policy

IRE never creates git commits automatically. The app may `git init` a new workspace, write `.gitignore`, and write files under `.ire/`, but staging/committing is entirely the user's decision. This applies equally to `ire.json`, `long-term.md`, `short-term/**`, `resources/**`, and `claims/**`.

---

## Memory Layer

Memory files live at `.ire/` root. Agent-written only; the user does not edit them through the UI.

### `long-term.md`

Architectural decisions, pivots, and durable "settled on this" claims, written via the MCP `memory.write_long_term` tool. Always injected into the agent system prompt (whole file).

### `short-term/YYYY-MM-DD.md`

Daily operational notes via `memory.write_short_term`. Only the **last two** day-files (today + yesterday) are auto-injected. Older files remain on disk but are not in the prompt unless explicitly read.

### Context injection rules

When IRE spawns an agent turn, the system prompt (`build_system_prompt`) is:

1. `.ire/_SYSTEM.md` — static framework context. MCP tool descriptions arrive via `tools/list` and are not duplicated here.
2. **Focus** — rendered from `ire.json` `focus` (research question + this week).
3. `resources/_index.md` (catalog).
4. `claims/_index.md` (catalog).
5. `long-term.md` (full).
6. The two most recent `short-term/YYYY-MM-DD.md` files.

`notes`, `ideas`, `experiments`, and individual resources/claims are read on demand (via `ire.read` / built-in `Read`); they are not pre-injected. Added via Claude Code's `--append-system-prompt` or Codex's `-c developer_instructions=<TOML string>`.

---

## SQLite Schema

Single file at `~/.ire/workspaces/<id>/local.db`, created on workspace open (`src-tauri/src/db/schema.rs`). Greenfield — `CREATE TABLE IF NOT EXISTS`, no `schema_migrations`, no versioned migrations. Only two tables remain:

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

CREATE TABLE chat_sessions (
  session_uuid      TEXT PRIMARY KEY, -- frontend historySessionUuid
  tab_label         TEXT NOT NULL,
  provider          TEXT NOT NULL,
  model             TEXT NOT NULL,
  started_at        TEXT NOT NULL,
  ended_at          TEXT NOT NULL,
  message_count     INTEGER NOT NULL,
  first_user_msg    TEXT,
  messages_json     TEXT NOT NULL,    -- full transcript (durable store)
  claude_session_id TEXT,             -- Claude --resume id (per provider)
  codex_thread_id   TEXT              -- Codex resume thread id (per provider)
);

CREATE INDEX idx_chat_sessions_ended ON chat_sessions(ended_at DESC);
```

`chat_sessions` is the durable store for chat transcripts and per-provider resume ids. On reopen, tab messages are hydrated from it and the next turn resumes the underlying agent session via the stored resume id.
