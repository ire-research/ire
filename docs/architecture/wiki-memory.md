# Wiki & Memory

Covers the wiki layer (atomic writes, index, git policy), the memory layer (long-term / short-term files), the context injection rules, and the SQLite schema.

---

## Wiki Layer

### Conventions (encoded in `.ire/_SYSTEM.md`)

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

### Atomic write contract

All wiki mutations go through `WikiStore` (Rust) which holds the in-process `tokio::Mutex<()>` for the wiki. Per write:

1. Acquire mutex.
2. Write content to `<path>.<uuid>.tmp` in the same directory (`O_CREAT|O_EXCL`).
3. `sync_all()` the temp file.
4. `fs::rename(tmp, final)` — atomic on local FS.
5. Re-derive `_index.md` from a directory walk (cheap; <1k files in MVP) and atomic-rename it.
6. Dispatch a typed `workspace-event` variant on the `workspace-event` channel, chosen by the path (see [frontend.md — workspace-event](frontend.md#workspace-event)). For `resources/*.md`, also link the DB row inline and emit `resource-changed` with the linked row.
7. Release mutex.

No CAS, no advisory file lock, no WAL — single-instance is enforced by `.lock` (see [workspace.md](workspace.md#concurrency--data-safety)).

### Index regeneration

`_index.md` is a flat markdown list:
```
- [notes.md](./notes.md) — running user notes
- [ideas](./ideas.json) — user ideas list
- [pulse](./pulse.json) — current research question and weekly focus
- [long-term](./long-term.md) — architectural decisions and durable insights
- [resources/attention-is-all-you-need.md](./resources/attention-is-all-you-need.md) — Vaswani et al. (2017): self-attention transformer …
```

The one-line summary is sourced from frontmatter `summary:` if present, else the first non-heading paragraph truncated to 160 chars.

### Git commit policy

IRE never creates git commits automatically. The application may initialize a git repository for a new workspace, write `.gitignore`, and write files under `.ire/`, but deciding when to stage and commit remains entirely with the user.

`WikiStore::write` and `WikiStore::rename` atomically update the target wiki path, regenerate `_index.md`, and dispatch a typed `workspace-event` variant. They do not run `git add` or `git commit`, regardless of path. This applies equally to `pulse.json`, `long-term.md`, `short-term/**`, `_index.md`, `notes.md`, `ideas.json`, `resources/**`, and `experiments/**`.

Resource approval follows the same rule: **Confirm** asks CC to write `resources/<slug>.md` via `wiki.write`. `WikiStore::write` then parses the new file's frontmatter `sources:` array, looks up each URL in the DB, updates the matching row (`status=summarized`, `wiki_path`, `title`), and emits a `workspace-event resource-changed { resource }` for each linked row — all in the same call, before `wiki.write` returns. The resulting wiki and index changes remain uncommitted until the user commits them.

---

## Memory Layer

Memory lives at the root of `wiki/`. These files are agent-written only; the user does not edit them through the UI.

### `long-term.md`

CC writes architectural decisions, pivots, "this approach is the one we settled on" claims here, via the MCP `memory.write_long_term` tool. Always injected into CC's system prompt context (whole file).

### `short-term/YYYY-MM-DD.md`

CC writes daily operational notes here via `memory.write_short_term`. Only the **last two** day-files (today + yesterday) are auto-injected. Older files remain on disk for git history but are not in CC's prompt unless explicitly read.

CC is told in the system prompt:
> Use `memory.write_short_term` for detailed information about the current experiment, specific debugging steps, and daily operations. After two days these notes are no longer auto-injected — promote anything still relevant to long-term memory.
>
> Use `memory.write_long_term` for overarching architectural decisions, pivots, abandonment of approaches, and durable insights.

### Context injection rules

When IRE spawns an agent turn, the system prompt is composed of:

1. `.ire/_SYSTEM.md` — static IRE framework context, wiki layout, behavioral rules, and schema. MCP tool descriptions are received automatically via `tools/list` and are not duplicated here. Seeded from `assets/seed/_SYSTEM.md` on workspace init; always injected first.
2. All agent-facing prompt text lives in `src-tauri/assets/prompts/`. Edit those files to change agent behaviour; never hardcode prompts at call sites.
3. `wiki/_index.md` (catalog).
4. `wiki/pulse.json`.
5. `wiki/long-term.md` (full).
6. The two most recent `short-term/YYYY-MM-DD.md` files.

This is added via Claude Code's `--append-system-prompt` or Codex's `-c developer_instructions=<TOML string>`. Wiki/notes/resources are read on-demand through MCP tools; they are not pre-injected.

---

## SQLite Schema

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
  session_id TEXT NOT NULL          -- chat session_uuid whose resume id the wake-up uses
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

CREATE TABLE resources (
  url_sha256 TEXT PRIMARY KEY,
  url TEXT NOT NULL,                 -- URL, local file path, or JSON array of source refs for batches
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

`chat_sessions` is the durable store for chat transcripts and per-provider resume ids. On reopen, tab messages are hydrated from it and the next turn resumes the underlying agent session via the stored resume id.
