# Roadmap

## Implementation Phases

Each phase ends with a demoable milestone.

**Phase 0 — Skeleton.** Replace the default `greet` Tauri example with the five-pane layout (static content). Add zustand, `react-resizable-panels`, types. No backend logic. Dark/light theme toggle in the topbar; dark is the default. *Milestone:* layout renders; panes resize/collapse; theme toggles between dark and light. ✅

**Phase 1 — Workspace lifecycle.** Implement setup screen, binary discovery, `init_workspace`, `open_workspace`, `.lock`, `close_workspace`. Scaffold `.ire/` with seed wiki. *Milestone:* user can pick or init a workspace; `.ire/` materialises; lock works across restarts. ✅

**Phase 2 — Wiki store + memory tools (no CC yet).** `WikiStore` with atomic writes, `_index.md` regeneration, `log.md` append. SQLite migrations. Frontend reads `pulse.json`, `notes.md`, and `ideas.json`. *Milestone:* user can manually edit notes and see them persisted; `workspace-event` variants propagate from `WikiStore::write` to the panels. ✅

**Phase 3 — CC subprocess layer.** Binary discovery + spawn + NDJSON parser + session management. A debug "Send" button next to the chat pane that sends a raw message and renders streaming text only (no tool cards yet). No MCP yet. *Milestone:* user can chat with CC inside the central pane, multi-turn via `--resume`. ✅

**Phase 4 — MCP server.** Node MCP server with the tool catalog, RPC bridge to Rust. Agent MCP config is wired through Claude Code's `--mcp-config` or Codex's `-c mcp_servers.*` flags. Implements `wiki.*`, `memory.*`, `pulse.update`. Unix-domain socket at `.ire/mcp.sock`; server path embedded at build time via `IRE_MCP_DIR` env var. `WikiStore` handles atomic writes, index regeneration, typed `workspace-event` dispatch, and renames without creating git commits. System prompt composed from wiki context files on every agent turn. *Milestone:* in chat, user can ask "save this insight to long-term memory" and the selected agent actually does it. ✅

**Phase 5 — Pipelines.** Notes/ideas/resource ingestion, including the Rust PDF/HTML/local-file extractors. `submit_resources` accepts an ordered list of URL and local-file sources plus the composer-selected provider/model/effort, extracts all text all-or-nothing, writes one cache file for a single source or `.ire/cache/<batch_sha>/source-NNN.txt` for multiple sources, inserts one DB row, refreshes an existing unindexed row with the same resource ID from the current request, emits `tab-created`, and kicks one selected-agent summarisation turn that writes `.ire/cache/<resource_id>_draft.md`. Confirm writes the draft to `resources/<slug>.md` through `WikiStore`. Discard deletes cache and marks `rejected`. *Milestone:* ingest one or more supported sources → one resource summary appears in the right pane. ✅

**Phase 6 — Experiments.** `experiment.start`, detached subprocess, monitor, wake-up turn composition. Experiment cards in chat with live log tail. *Milestone:* an agent can run a Python script ablation, tell the user "I'll be back", and resume with results when the script exits. ✅

**Phase 7 — Polish.** `workspace.json` persistence (per-group panel layouts, open tabs, and chat options via `read_workspace_state` / `save_workspace_state`, debounced 1 s, hydrated before the Layout mounts). Error toast stack. Cancel button on `ExperimentCard`. Inline focus editor saves `pulse.json` fields through `save_pulse_field`. *Milestone:* layout, tabs, model/effort, and focus survive restart; user-visible failures surface as toasts; experiments can be cancelled from the chat.

---

## Open Items & Risks

- **Node runtime detection.** The MCP server requires Node. We assume CC's installation already brought Node along, but on some systems CC is a standalone binary. Phase 4 must add a node-discovery probe similar to the binary discovery layer and surface a setup error if absent.
- **Windows process groups.** Detached experiments work differently on Windows (`CREATE_NEW_PROCESS_GROUP`); needs explicit testing in Phase 6.
- **Wake-up storms.** If multiple experiments finish near-simultaneously, several wake-up turns queue. The queue is FIFO and we surface a count in the UI; CC sees them sequentially. Acceptable for MVP.
- **Index regeneration cost.** Walking the whole `wiki/` tree on every write is fine at MVP scale (tens to low-hundreds of files). At scale, switch to incremental index updates.
- **Frontmatter parsing.** No formal frontmatter spec — using the YAML convention. We accept files without frontmatter; required fields are derived heuristically.
- **CC `--tools` flag stability.** The tool allowlist syntax may evolve; `cc::spawn` is the only place that needs to update if breaking changes land.
- **Uncommitted `.ire/` changes.** IRE writes wiki, resource index, and workspace files but never commits them. Users must commit `.ire/` changes explicitly when they want those updates captured in git history.
