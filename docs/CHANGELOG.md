# Changelog

User- and developer-facing changes, in reverse-chronological order. Bug fixes that change visible behavior land here too. Internal-only refactors don't.

Each section corresponds to a notable batch of work. For the architectural reasoning behind a change, see [DECISIONS.md](./DECISIONS.md).

---

## Unreleased

### Added
- **Phase 7 — Polish.** Workspace layout (theme + panel sizes) persists to `.ire/workspace.json` and rehydrates before the UI mounts. Top-right toast stack surfaces backend errors and silent failures that previously only hit `console.error`. Experiment cards gained a **Cancel** button that calls `experiment_cancel` while the run is live. The focus banner is now click-to-edit and writes to `status/pulse.md` via a new `update_pulse_focus` command.
- **Markdown + LaTeX in chat.** Both user and assistant messages render through `react-markdown` + `remark-gfm` + `remark-math` + `rehype-katex`. KaTeX delimiters `$…$` and `$$…$$` work; tables, fenced code, task lists, blockquotes, headings — all GFM. Inline HTML stays sanitized (no `rehype-raw`); raw HTML in model output appears as text or inside a code block.

### Changed
- **`_SYSTEM.md` mandates MCP-only writes for `.ire/wiki/`.** Built-in `Write`/`Edit`/`MultiEdit` are forbidden on wiki paths; CC must use the IRE MCP wiki/memory/pulse tools. Bypassing this skipped `wiki-changed` events and the side panes wouldn't refresh until app restart. See [DECISIONS.md](./DECISIONS.md#2026-05-06--wiki-writes-must-go-through-mcp-not-built-in-writeedit).
- **All CC-facing prompts centralised in `src-tauri/assets/prompts/`.** Mode preambles, the resource-summarizer role, the resource-confirm follow-up, and the experiment wake-up template now live as `.md` files embedded via `include_str!` and accessed through a new `prompts` module. The frontend's confirm-resource prompt is fetched via a `get_resource_confirm_prompt` IPC command. Behaviour unchanged; one place to edit when tuning CC.

### Fixed
- **Resource indexing now matches `_schema.md`.** `index_resource` looks up the resource page by its `sources:` frontmatter array (the schema-canonical field) instead of `url:`. The Confirm-flow prompt was also aligned to write `sources: [<url>]`. Without this fix, every confirmed resource ended up with `wiki_path=NULL` in the DB and never appeared in the resources list. See [DECISIONS.md](./DECISIONS.md#2026-05-06--sources-array-not-url-is-the-canonical-resource-frontmatter-field).
