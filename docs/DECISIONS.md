# Design Decisions

A log of non-obvious design choices and the reasoning behind them. Use this when something in the code "looks wrong" — the answer is often here.

Entries are in reverse-chronological order. Each entry: a one-line decision, the **Context** that forced it, and the **Reasoning** that picked the chosen path over alternatives.

---

## 2026-05-06 — `sources:` (array), not `url:`, is the canonical resource frontmatter field

Resource pages written to `wiki/resources/<slug>.md` use the `sources: [<url>]` YAML frontmatter array. The Confirm-flow CC prompt and the `index_resource` matcher both speak `sources:`.

**Context.** The original Confirm-flow prompt told CC to write `url:` and `date:` in the frontmatter. The matcher (`commands/resources.rs::find_resource_wiki_path`) looked for `url:`. But `_schema.md` — the canonical wiki convention loaded into every CC turn — declared `sources:` (an array) as the standard field for source materials. CC followed the schema, the matcher couldn't find the file, the row landed in the DB with `wiki_path=NULL`, and the resource never showed up in the UI.

**Reasoning.** When two specs disagree, the more general one wins. `_schema.md` covers all wiki page types (`summary`, `entity`, `concept`, `comparison`); `sources: [...]` generalizes to "comparison of three papers" while `url:` only handles the degenerate one-URL case. Picking the narrower field would force a second key later. Bonus: CC already converged on `sources:` without prompting, which is evidence the schema is doing its job.

**Implementation.** `find_resource_wiki_path` now parses the inline-array string and looks for the URL inside. The Confirm prompt was rewritten to follow `_schema.md`. See commit `6897a8e`.

---

## 2026-05-06 — Wiki writes must go through MCP, not built-in `Write`/`Edit`

The seed `_SYSTEM.md` (rule 7) explicitly forbids CC from using built-in `Write`, `Edit`, or `MultiEdit` on any path under `.ire/wiki/`. All wiki mutations must use the IRE MCP tools.

**Context.** During end-to-end testing, populating `notes.md` / `ideas.md` / `pulse.md` from a fresh chat turn worked — but the side panes did not refresh until the app was restarted. Investigation showed CC was writing the files via the built-in `Write` tool, bypassing `WikiStore::write` entirely. That meant: no atomic-rename, no `_index.md` regeneration, no `log.md` append, no git auto-commit, and crucially **no `wiki-changed` event** — which is what `Layout.tsx` listens to in order to re-read the panes live.

**Reasoning.** Three options were considered:

1. **Enforce per-mode `--allowedTools`** (the SDD §9 contract). Most rigorous, but bigger change with cross-mode implications, and CC's tool-allowlist syntax is still moving.
2. **Add a `notify` filesystem watcher.** SDD §3 explicitly opts out: *"No notify watcher in MVP — wiki changes are mediated through IRE."* Reversing that is a real architectural change.
3. **Strengthen the system prompt.** Cheapest. Relies on CC compliance.

We picked (3) for now. The seed prompt is the right place to set behavioral rules CC follows on every turn. Caveat: existing workspaces don't pick up updates to the seed automatically. Caveat: the model can still misbehave; if that becomes a recurring problem, escalate to (1) or (2).

**Implementation.** `src-tauri/assets/seed/_SYSTEM.md` rule 7. See commit `c22fd6c`.
