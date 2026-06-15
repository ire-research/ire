# Frontend & Tauri IPC

Covers the React layout, chat rendering, edit/preview behaviour, resource list, theming, workspace state persistence, and the full Tauri IPC surface (commands, events, workspace-event channel).

---

## Layout

The Tauri window opens in windowed mode at 1280 × 820.

```
┌──────────────────────────────────────────────────────────────────────┐
│ Navbar (h-10):  full workspace path [running N exp]     [close] [⚙] │
├──────────────────┬──────────────────────────────┬────────────────────┤
│ Left rail 280px  │   ChatPane (flex-1)           │ Right rail 320px   │
│  FocusPane       │   - tab bar                  │  NotesPane         │
│  - Research Q.   │   - message list              │  - inline edit     │
│  - This Week     │   - composer                 │  IdeasPane         │
│  ResourcesSection│   - experiment tab view      │  - draggable cards │
│  ExperimentsSection   (kind="experiment")        │  AddResourceSection│
│  - experiment list                               │  - URL + file input│
└──────────────────┴──────────────────────────────┴────────────────────┘
│ StatusBar: full workspace path + git diff · CPU · GPU · RAM · host   │
└──────────────────────────────────────────────────────────────────────┘
```

- Body uses `react-resizable-panels` group `body` with panels `left`, `center`, `right`. Left/right default to 280px/320px, have no maximum width, keep minimum widths of 160px/180px, and are collapsible to `0px` through two top-navbar icon buttons.
- The left rail is a vertical group `left` with panels `pulse`, `resources`, `experiments`.
- The right rail is a vertical group `right` with panels `notes`, `ideas`, `resource-input`.
- `FocusPane` and `NotesPane` use **inline editing**: clicking a field activates a textarea in place; blur/Enter saves.
- `NotesPane` renders saved `notes.md` as markdown in display mode; in edit mode its textarea fills the remaining height of the resizable panel.
- The top navbar shows the full workspace path. The bottom `StatusBar` displays: workspace path (from workspace state) + git branch + insertions/deletions, CPU, GPU, RAM, `username@hostname`, and `Claude Code` / `codex` availability chips. Machine-level facts (CPU/RAM/GPU model, hostname, username, agent availability) come from `get_system_info`, fetched once and cached in-memory on the Rust side; volatile metrics (git branch/diff, CPU usage, GPU usage) come from `get_system_metrics`, polled every 5 s. Both run off the main thread via `spawn_blocking`.

---

## Chat Rendering

### Tab bar

Each tab is a horizontal button in a single TDI row:
- Active tab: `bg-surface-container-highest` + 1 px top border in the `primary` colour token.
- Inactive tabs: `text-on-surface-variant` with no background fill.
- Close button (×) is hidden until the tab row is **hovered** (`group-hover`). Pinned tabs (Main) have no close button.
- A `+` button opens a new chat tab. A history button is fixed at the far right.
- Each tab shows a Material Symbol icon: `chat` for chat tabs, `description` for resource/preview tabs, `science` for experiment tabs. A resource tab being summarised shows `progress_activity` + `animate-spin`.

### Messages

Assistant output is stored and rendered as ordered content blocks. Text deltas, thinking deltas, tool cards, and experiment cards appear in the same chronological order as their `chat-stream` events.

- Both user and assistant text are rendered through `MessageMarkdown` (`react-markdown` + `remark-gfm` + `remark-math` + `rehype-katex`). Inline HTML is intentionally **not** enabled; raw HTML in model output is shown as text, never injected into the DOM.
- Thinking blocks render as collapsed-by-default accordions labelled `thinking...`.
- Tool blocks render as compact canonical `ToolCard`s. React renders from `tool.kind`, `tool.title`, `tool.input`, `tool.output`, `tool.status`, and `tool.meta`; provider-specific raw names are not used for frontend branching. `experiment_start` tool calls render as `ExperimentCard` instead.
- Experiment cards: collapsed by default; header contains status dot (blinking amber while running, solid green/red on completion), canonical tool title, status badge, optional PID/exit label, chevron, and **Cancel** button (visible only while running). Expanded body shows IN and last 10 live log lines.
- **AskUserQuestion cards** render when CC calls IRE's `ask_user_question` MCP tool (the built-in `AskUserQuestion` tool is disabled via `--disallowedTools`). One question per step, fixed 380px stage, single-select auto-advances after 220 ms. The last question's button switches from `Next` to `Review`; the Review step lists every answer with an edit-pencil affordance. On submit, the card calls `ipc.submitAskAnswer(tabId, answers)`, which delivers the answers to the blocked MCP call so the same subprocess turn continues. After submit the card locks into an "Answered" summary view.

### Composer footer

- Textarea: starts at 60px, grows with content, caps at 240px, then scrolls internally. Fixed placeholder `Ask IRE to brainstorm directions, ingest resources, or run experiments...`.
- **Model selector** — grouped options from `MODELS` in `state/chatOptions.ts`, filtered by `availableProviders`. Claude Code models: Opus 4.7, Sonnet 4.6, Haiku 4.5. Codex models: GPT-5.5, GPT-5.4, GPT-5.4-Mini, GPT-5.3-Codex, GPT-5.2. Default (both providers): Claude Code / Sonnet 4.6.
- **Effort / Reasoning** — Codex: Low → Med → High → XHigh, labelled `reasoning`. Claude: labelled `effort`, filtered by model: Opus 4.7 (Low–Max), Sonnet 4.6 (Low–Max), Haiku 4.5 (no effort selector). Default: **Low**.

### ExperimentTabView

When the active tab has `kind === "experiment"`, the chat pane renders `ExperimentTabView`: name header + status badge, metadata grid (status + elapsed timer, runtime, command), and a scrollable log pane (stdout only, `h-48`, auto-scrolls). Elapsed time updates every second while running. Live log lines arrive via `experiment-log-line`.

---

## Edit/Preview Toggle

- Resource preview tabs open in **Preview** by default (`ResourcePreviewPane`): frontmatter metadata header + markdown body. Switching to Edit loads raw file contents into a textarea; switching back without Submit discards local edits (with a confirm if dirty). Submit calls `save_wiki_file`.
- `NotesPane` renders markdown in display mode, edits inline as raw markdown, saves through `save_notes` on blur / Ctrl+Enter.
- `IdeasPane` does not use markdown; it writes `ideas.json` directly via `save_ideas_json`.

---

## Resource List

The Resources list shows only confirmed (indexed) resources — those where `status=summarized` and `wiki_path` is non-null. Each entry shows the extracted title (frontmatter `title:` → first `#` heading → filename stem). Clicking a resource opens (or re-focuses) a **Preview tab** in the central column.

---

## Theming

The UI uses a fixed dark theme. All colours are defined as Tailwind token extensions in `tailwind.config.ts` (e.g. `surface-container-low`, `on-surface`, `primary`, `error`, `warn`, `ok`, `accent`). No light-mode overrides.

Typography uses bundled `geist` package font files (`Geist`, `Geist Mono`) referenced from `styles.css`; icons are inline SVGs from `src/components/Icon.tsx`. The app does not load Google Fonts at runtime.

`~/.config/ire/config.json` still has a `theme` field reserved for future use, but the frontend does not apply it.

---

## Workspace State (`workspace.json`)

```json
{
  "version": 1,
  "panel_layout": {
    "groups": {
      "body":  { "left": 22, "center": 56, "right": 22 },
      "left":  { "pulse": 33.33, "resources": 33.33, "experiments": 33.34 },
      "right": { "notes": 27.5, "ideas": 27.5, "resource-input": 45 }
    },
    "collapsed": { "left": false, "right": false }
  },
  "last_opened": "2026-05-06T10:14:00Z",
  "model": "claude-sonnet-4-6",
  "provider": "claude",
  "effort": "low",
  "tabs": [
    {
      "id": "main",
      "label": "Chat",
      "messages": [],
      "isStreaming": false,
      "isPinned": false,
      "kind": "chat",
      "historySessionUuid": "550e8400-e29b-41d4-a716-446655440000",
      "historyStartedAt": "2026-05-06T10:14:00Z"
    }
  ],
  "active_tab_id": "main"
}
```

Each entry under `panel_layout.groups.<group-id>` is the `Layout` map (`{ panel-id: percentage }`) that `react-resizable-panels` accepts as `defaultLayout`. `panel_layout.collapsed.left/right` stores the sidebar collapsed state.

Persisted via `save_workspace_state` (debounced 1 s on layout, collapsed-state, model, provider, effort, tab, message, or active-tab change; also saved immediately before/after chat sends and before workspace close). Hydrated by `read_workspace_state` immediately after `open_workspace`/`init_workspace`, before the workspace transitions to `phase = "ready"`.

Per-tab agent `session_id`s are intentionally **not** persisted in MVP — sessions live in the in-memory `SessionManager` and are reset on app close.

---

## Tauri IPC Surface

### Commands (frontend → backend)

Directory picking is **not** a Tauri command — the frontend calls Tauri's dialog plugin directly (`@tauri-apps/plugin-dialog`) via the `pickDirectory` helper in `ipc.ts`.

| Command | Args | Returns |
|---|---|---|
| `setup_status` | — | `{ binary: BinaryStatus, codex_binary: BinaryStatus }` |
| `open_workspace` | `{ path }` | `WorkspaceState` (`{ path, name }`) |
| `init_workspace` | `{ path }` | `WorkspaceState` |
| `close_workspace` | — | `{}` |
| `read_wiki_file` | `{ path }` | `{ content, frontmatter }` |
| `save_wiki_file` | `{ path, content }` | `{}` (atomic write) |
| `save_notes` | `{ content }` | `{}` (atomic write) |
| `read_ideas` | — | `IdeaItem[]` |
| `save_ideas_json` | `{ ideas }` | `{}` |
| `read_pulse` | — | `{ research_question, this_week }` |
| `save_pulse_field` | `{ field: "research_question" \| "this_week", content }` | `{}` |
| `submit_resource` | `{ url, options }` | `resource_id: string` |
| `submit_local_resource` | `{ path, options }` | `resource_id: string` |
| `submit_resources` | `{ sources: ({ kind: "url", url } \| { kind: "local_file", path })[], options }` | `resource_id: string` |
| `discard_resource` | `{ resource_id }` | `{}` |
| `list_resources` | — | `ResourceItem[]` (only `summarized` entries) |
| `get_resource_confirm_prompt` | — | `string` |
| `chat_send` | `{ tab_id, message, options: { model, provider, effort } }` | `{}` (events follow) |
| `chat_cancel` | `{ tab_id }` | `{}` |
| `chat_reset_session` | `{ tab_id }` | `{}` |
| `submit_ask_answer` | `{ tab_id, answers }` | `{}` |
| `generate_chat_title` | `{ message, model, provider }` | `string` |
| `experiment_list` | `{ limit? }` | `[ExperimentRow]` |
| `experiment_logs` | `{ uuid, kb? }` | `{ stdout, stderr }` |
| `experiment_cancel` | `{ uuid }` | `{}` |
| `experiment_delete` | `{ uuid }` | `{}` |
| `experiment_rename` | `{ uuid, name }` | `{}` |
| `get_system_info` | — | `SystemInfo` (cached after first call) |
| `get_system_metrics` | — | `SystemMetrics` |
| `read_workspace_state` | — | `PersistedWorkspace` |
| `save_workspace_state` | `{ state: PersistedWorkspace }` | `{}` |
| `chat_history_save` | `{ session_uuid?, tab_label, provider, model, started_at, messages_json }` | `{}` |
| `chat_history_list` | `{ limit? }` | `[ChatSessionRow]` ordered by `ended_at DESC` |
| `chat_history_get` | `{ session_uuid }` | `messages_json: string \| null` |
| `chat_history_delete` | `{ session_uuid }` | `{}` |
| `read_user_config` | — | `UserConfig` |
| `save_user_config` | `{ config: UserConfig }` | `{}` |

### Events (backend → frontend)

| Event | Payload |
|---|---|
| `chat-stream` | `{ tab_id, stream_id, event_id, event: StreamEvent }` (see [chat-agents.md — JSONL parsers](chat-agents.md#jsonl-parsers-ccstream-codexstream)) |
| `tab-created` | `{ tab_id, label, kind: "chat"\|"resource", resource_id?, agent_options? }` |
| `chat-cancelled` | `{ tab_id }` |
| `experiment-starting` | `{ tab_id, uuid, pid? }` |
| `experiment-status` | `{ uuid, status, exit_code? }` |
| `experiment-log-line` | `{ uuid, stream: "stdout"\|"stderr", line }` |
| `workspace-event` | Tagged union — see below |
| `setup-needed` | `{ reason }` |
| `error` | `{ scope, message }` |

### workspace-event

A single typed channel carrying workspace-level state changes for the side panels. Every payload carries a `kind` discriminator and a `source: "hydrate" | "mutation"` field. The `source` lets side-effect listeners distinguish the initial state burst on workspace open from live mutations; the slice reducer treats both identically.

| `kind` | Payload (excl. `source`) | Emitted from |
|---|---|---|
| `pulse-changed` | `{ research_question, this_week }` | `WikiStore::write` on pulse files; initial-state burst |
| `notes-changed` | `{ content }` | `WikiStore::write` on `notes.md`; initial-state burst |
| `ideas-changed` | `{ ideas: IdeaItem[] }` | `WikiStore::write` on `ideas.json`; initial-state burst |
| `resource-changed` | `{ resource: ResourceItem }` | `WikiStore::write` on `resources/*.md`; initial-state burst |
| `resource-deleted` | `{ resource_id }` | `discard_resource` command |
| `experiment-changed` | `{ experiment: ExperimentRow }` | `experiments/runner.rs` on state transitions; initial-state burst |
| `experiment-deleted` | `{ uuid }` | `experiment_delete` command |

The frontend has one subscriber (in `App.tsx`) that applies every variant to the `workspaceData` Zustand slice. There is no per-panel listener, no path-string filtering, and no polling.

### Initial-state burst on workspace open

At the end of `attach()` in `commands/workspace.rs`, `emit_initial_state(app, workspace_root)` fires:

1. Read pulse files → one `pulse-changed` event.
2. Read `notes.md` → one `notes-changed` event.
3. Parse `ideas.json` → one `ideas-changed` event (silently skipped if JSON is not parseable).
4. `models::list_resources(ire_dir)` → one `resource-changed` per row.
5. `models::list_experiments(ire_dir, 50)` → one `experiment-changed` per row.

Every event in the burst carries `source: "hydrate"`. Live mutations carry `source: "mutation"`. Animation listeners can filter to `source === "mutation"` to avoid flashing every panel on workspace open.
