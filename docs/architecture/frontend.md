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
- `NotesPane` renders the `ire.json` `notes` field as markdown in display mode; in edit mode its textarea fills the remaining height of the resizable panel.
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

- Resource preview tabs open in **Preview** by default (`ResourcePreviewPane`): frontmatter metadata header + markdown body. Switching to Edit loads raw file contents into a textarea; switching back without Submit discards local edits (with a confirm if dirty). Submit calls `save_resource`.
- `NotesPane` renders markdown in display mode, edits inline as raw markdown, saves through `save_notes` on blur / Ctrl+Enter.
- `IdeasPane` does not use markdown; it writes the `ire.json` `ideas` array (ordered `{ text }[]`, identity by index) via `save_ideas`.

---

## Resource List

The Resources list shows only confirmed (indexed) resources — those where `status=summarized` and `wiki_path` is non-null. Each entry shows the extracted title (frontmatter `title:` → first `#` heading → filename stem). Clicking a resource opens (or re-focuses) a **Preview tab** in the central column.

---

## Theming

The UI uses a fixed dark theme. All colours are defined as Tailwind token extensions in `tailwind.config.ts` (e.g. `surface-container-low`, `on-surface`, `primary`, `error`, `warn`, `ok`, `accent`). No light-mode overrides.

Typography uses bundled `geist` package font files (`Geist`, `Geist Mono`) referenced from `styles.css`; icons are inline SVGs from `src/components/Icon.tsx`. The app does not load Google Fonts at runtime.

`~/.config/ire/config.json` still has a `theme` field reserved for future use, but the frontend does not apply it.

---

## Workspace State (`tauri-plugin-store`)

Per-workspace UI/session state is persisted via `@tauri-apps/plugin-store` directly from the frontend — there is no Rust persistence layer and no `workspace.json`. A single store file (`workspace-state.json`, in the plugin's app-data dir) holds one entry per workspace, keyed by workspace path. The wrapper lives in `src/state/persistedStore.ts` (`loadPersisted(path)` / `savePersisted(path, state)`); `useWorkspace.persist()` saves the current workspace's state using the path from `phase`.

The stored shape (`PersistedWorkspace`):

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
  "model": "claude-sonnet-5",
  "provider": "claude",
  "effort": "low",
  "tabs": [
    {
      "id": "main",
      "label": "Chat",
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

Persisted via `useWorkspace.persist()` (debounced 1 s on layout, collapsed-state, model, provider, effort, tab, or active-tab change; also saved immediately before/after chat sends and before workspace close). Loaded via `loadPersisted(path)` immediately after `open_workspace`/`init_workspace`, before the workspace transitions to `phase = "ready"`.

Tab `messages` are **not** stored in the workspace state — they live in the `chat_sessions` table (the durable store) and are hydrated on open via `chat_history_get(historySessionUuid)`. Per-tab agent resume ids are likewise persisted in `chat_sessions` (`claude_session_id` / `codex_thread_id`), so reopening a workspace resumes the underlying agent session; `SessionManager` keeps only transient per-turn state.

---

## Tauri IPC Surface

### Commands (frontend → backend)

Directory picking is **not** a Tauri command — the frontend calls Tauri's dialog plugin directly (`@tauri-apps/plugin-dialog`) via the `pickDirectory` helper in `ipc.ts`.

Auto-update is likewise not a Tauri command: `useAutoUpdater` (`src/hooks/useAutoUpdater.ts`), called once from `App.tsx`, uses `@tauri-apps/plugin-updater`'s `check()` on launch. If an update is found it is downloaded and installed immediately in the background (not applied until the user restarts the app, so an in-progress experiment is never interrupted), surfaced via a toast. The update manifest is served from `latest.json` on the published GitHub release, built and signed by `tauri-plugin-updater`/`tauri-action` in `.github/workflows/release.yml` using the `TAURI_SIGNING_PRIVATE_KEY`(`_PASSWORD`) secrets; the corresponding public key lives in `src-tauri/tauri.conf.json` under `plugins.updater.pubkey`.

| Command | Args | Returns |
|---|---|---|
| `setup_status` | — | `{ binary: BinaryStatus, codex_binary: BinaryStatus }` |
| `open_workspace` | `{ path }` | `WorkspaceState` (`{ path, name }`) |
| `init_workspace` | `{ path }` | `WorkspaceState` |
| `close_workspace` | — | `{}` |
| `read_resource` | `{ path }` | `{ content, frontmatter }` (reads `.ire/resources/*.md`) |
| `save_resource` | `{ path, content }` | `{}` (resource file: atomic write + index + `resource-changed`) |
| `save_notes` | `{ content }` | `{}` (patches `ire.json` notes) |
| `save_ideas` | `{ ideas: { text }[] }` | `{}` (patches `ire.json` ideas) |
| `save_focus_field` | `{ field: "research_question" \| "this_week", content }` | `{}` (patches `ire.json` focus) |
| `submit_resource` | `{ url, options }` | `resource_id: string` (transient ingest id) |
| `submit_local_resource` | `{ path, options }` | `resource_id: string` |
| `submit_resources` | `{ sources: ({ kind: "url", url } \| { kind: "local_file", path })[], options }` | `resource_id: string` |
| `read_resource_draft` | `{ resource_id }` | `string` (draft markdown with sources normalized) |
| `save_resource_draft` | `{ resource_id, content }` | `{}` |
| `confirm_resource` | `{ resource_id }` | `{}` (writes `resources/<slug>.md`) |
| `discard_resource` | `{ resource_id }` | `{}` (in-flight id → drop draft; else file path → delete file) |
| `chat_send` | `{ tab_id, message, options: { model, provider, effort }, session_uuid, tab_label, started_at }` | `{}` (events follow) |
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
| `tab-created` | `{ tab_id, label, kind: "chat"\|"resource", resource_id?, resource_status?, agent_options? }` |
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
| `focus-changed` | `{ research_question, this_week }` | `ire.json` writes (UI setters / `ire.edit`); initial-state burst |
| `notes-changed` | `{ content }` | `ire.json` writes; initial-state burst |
| `ideas-changed` | `{ ideas: { text }[] }` | `ire.json` writes; initial-state burst |
| `resource-changed` | `{ resource: { path, title, sources } }` | `IreStore::write_resource` on `resources/*.md`; initial-state burst |
| `resource-deleted` | `{ path }` | `discard_resource` / `IreStore::delete_resource` |
| `experiment-changed` | `{ experiment: ExperimentRow }` | `experiments/runner.rs` on state transitions; initial-state burst |
| `experiment-deleted` | `{ uuid }` | `experiment_delete` command |

The frontend has one subscriber (in `App.tsx`) that applies every variant to the `workspaceData` Zustand slice. There is no per-panel listener, no path-string filtering, and no polling.

### Initial-state burst on workspace open

At the end of `attach()` in `commands/workspace.rs`, `emit_initial_state(app, workspace_root)` fires:

1. Read `ire.json` → one `notes-changed`, one `focus-changed`, and one `ideas-changed` event.
2. `IreStore::list_resources()` (scan `.ire/resources/*.md`) → one `resource-changed` per file.
3. `ire.json` `experiments` → one `experiment-changed` per entry (tab_id empty on hydrate; live linkage via events).

Every event in the burst carries `source: "hydrate"`. Live mutations carry `source: "mutation"`. Animation listeners can filter to `source === "mutation"` to avoid flashing every panel on workspace open.
