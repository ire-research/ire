# Frontend Redesign — Design Spec
**Date:** 2026-05-15  
**Source of truth:** `docs/design/workspace_home.html` and `docs/design/workspace_picker.html`

---

## 1. Overview

The entire frontend visual language is replaced. The old custom CSS variables and bespoke component styles are removed and replaced with a Tailwind-based design system (matching the token set defined in the HTML prototypes). All components are restyled to match the HTML mockups exactly.

Three areas also involve data-model or backend changes:

| Area | Change |
|---|---|
| Focus pane | Read from `pulse/RESEARCH-QUESTION.md` + `pulse/THIS-WEEK.md` instead of single `status/pulse.md` |
| Ideas | Read/write `ideas.json` (array with `trashed` flag) instead of `ideas.md` |
| Experiment tab | New tab kind `"experiment"` opens in center tab bar when user clicks an experiment in left rail |
| Status bar | Real system metrics (git branch, CPU, GPU, RAM, hostname, claude-code connection) from new IPC surface |

The old `status/pulse.md` and `ideas.md` files and all their associated Rust/TS code are removed entirely (no backward compatibility needed).

---

## 2. Design Tokens

The Tailwind config from the HTML prototype is the canonical token set. Key colors:

| Token | Value | Role |
|---|---|---|
| `background` / `surface` | `#0e0e11` | App background |
| `surface-container` | `#19191e` | Card/pane bg |
| `surface-container-low` | `#131317` | Rail bg |
| `surface-container-high` | `#1e1f26` | Hover states |
| `surface-container-highest` | `#24252d` | Active tab bg |
| `outline-variant` | `#474750` | Borders |
| `on-surface` | `#e6e4ef` | Primary text |
| `on-surface-variant` | `#abaab4` | Secondary text |
| `primary` | `#c6c6c9` | Accent (active tab top border) |
| `accent` | `#E4E4E7` | Send button bg |
| `accent-fg` | `#0A0A0A` | Send button text |
| `ok` | `#10b981` | Success/done status |
| `warn` | `#d97706` | Running status |
| `error` | `#ec7c8a` | Failed status |

Border radius: `2px` default, `4px` lg, `8px` xl, `12px` full.  
Fonts: Inter (body), JetBrains Mono (code/logs).

The existing `styles.css` custom properties and all bespoke component CSS classes are **removed**. Tailwind utility classes are used throughout.

---

## 3. Workspace Picker (`SetupScreen`)

Redesigned to match `workspace_picker.html` exactly:

- Centered card, max-width 520px
- Title: "Open or create a workspace."
- Subtitle explaining `.ire/` directory
- **Recent workspaces list**: each row has workspace name (bold), monospace path + relative time, left border accent on active/most-recent, chevron right
- **Actions**: two equal-width buttons — "Open folder…" and "New workspace…"
- **Divider**
- **Status row**: green dot + "claude-code · authenticated" on left; "settings" link on right
- No binary-check section visible in the new design — the binary status is surfaced only if not found (shown inline, not as a separate step)

---

## 4. Layout Shell (`workspace_home.html`)

### 4.1 Top NavBar

```
[running 1 exp badge] .............. [close workspace] [settings]
```

- Height: `h-10` (40px), `border-b border-outline-variant`
- Left: amber "running N exp" badge (animated pulse dot) — shows count of currently running experiments; hidden when zero
- Right: close-workspace button (text or icon), settings gear icon
- No theme toggle — dark mode is fixed; all theme-toggle UI and `[data-theme="light"]` CSS are removed
- Close workspace navigates back to the picker (existing `handleClose` logic)

### 4.2 Left Rail

Width: 280px default, draggable (160px–420px). Three vertically-stacked sections with row drag handles between them.

**Focus section** (top third):
- Header: target icon + "Focus" label
- Research Question subsection: label + inline edit button (pencil, hover-revealed) + paragraph text → reads/writes `pulse/RESEARCH-QUESTION.md`
- This Week subsection: same pattern → reads/writes `pulse/THIS-WEEK.md`
- Both are click-to-edit inline fields (not a separate edit mode like the old FocusBanner)

**Resources section** (middle third):
- Header: description icon + "Resources" label
- List of resource items as clickable buttons → opens preview tab (existing behavior)

**Experiments section** (bottom, flex-1):
- Header: science icon + "Experiments" label
- List of experiments from `experiment_list` IPC, each row shows monospace name + status pill (Run/Done/Fail)
- Status pill colors: warn/ok/error
- Clicking opens an experiment tab in the center tab bar

### 4.3 Center Column

Tab bar (`h-8`):
- Each tab: icon + label, active tab has `bg-surface-container-highest`, top border `border-t-primary`, merges with content
- Inactive: `text-on-surface-variant`, hover highlights
- Spinning icon for summarizing resource tabs
- `+` button at end

Chat header strip (`h-8`): just the reset button aligned right (mode switch removed from header — the design doesn't show it).

Content host (absolute-positioned views, `hidden`/shown):
- `view-chat`: message list (see §12)
- `view-resource`: resource preview pane (see §13)
- `view-experiment`: new experiment pane (see §6)

Floating composer (absolute, bottom-6):
- Auto-resizing textarea (min 52px, max 240px, `field-sizing: content`)
- Footer row: model picker pill + effort picker pill + `/` slash button on left; `⌘↵` hint + Send button on right
- Dropdowns open upward (`bottom-full`)

### 4.4 Right Rail

Width: 320px default, draggable (180px–440px). Three vertically-stacked sections.

**Notes section** (top third):
- Header: edit_note icon + "Notes" label + pencil edit button (hover-revealed)
- When not editing: render notes as a bullet list (parse `notes.md` lines)
- When editing: textarea replacing the bullet list; save on blur or explicit save button
- Wired to `save_notes` IPC (existing)

**Ideas section** (middle third):
- Header: lightbulb icon + "Ideas" label + add button
- Ideas rendered as draggable cards (drag-to-reorder)
- Each card: text + hover-revealed trash button
- Data from `ideas.json` (new format, see §7)

**Add Resource section** (bottom, flex-1):
- Header: add_link icon + "Add resource"
- URL input + Add button (existing ResourceInput behavior)

### 4.5 Bottom Status Bar

Height: `h-6`, monospace 10px, `bg-surface-container-lowest`, `border-t border-outline-variant`.

Draggable items (left to right):
1. Git info: `~/workspace/path · branch +adds -dels` (real git data)
2. CPU: chip icon + model name + usage %
3. GPU: developer_board icon + model + usage % + VRAM
4. RAM: storage icon + total
5. Hostname: `user@host`
6. (ml-auto) claude-code connection: "claude-code · connected" + green dot

All items are real data from new Tauri IPC commands (see §8).

---

## 5. CSS Strategy

The old `styles.css` (~1546 lines) is replaced. Approach:

1. Add Tailwind CSS (via PostCSS + `tailwindcss` package) to the Vite build
2. Configure `tailwind.config.ts` with the exact token set from the HTML prototype
3. Add `@tailwind base/components/utilities` directives to a minimal `styles.css`
4. Keep only non-utility global resets (scrollbars, font-face, `field-sizing`)
5. All component styling moves to Tailwind utility classes inline in TSX

Material Symbols Outlined font is added (Google Fonts CDN import in `index.html`). JetBrains Mono added similarly (or from npm).

---

## 6. Experiment Tab (New Feature)

### Tab kind

Add `"experiment"` to `TabKind`. A new `ExperimentTab` component renders when `activeTab.kind === "experiment"`.

### Content

Matches the `view-experiment` section in `workspace_home.html`:

- Header row: monospace experiment name + status pill
- Metadata grid (2-col): Status (with elapsed time), Runtime, Command (full command truncated)
- Live logs panel: "Logs" header + "live" indicator + scrollable log body (JetBrains Mono 11px, `h-48`)

### Opening

Clicking an experiment row in the left rail:
1. Check if a tab for this experiment UUID already exists → activate it
2. Otherwise create a new tab with `kind: "experiment"`, label = experiment name, store UUID
3. Load initial logs via `experiment_logs` IPC
4. Live log lines stream via existing `onExperimentLogLine` event (already wired up in ChatPane, needs routing to experiment tabs too)

### Tab state

Extend `Tab` type: `experimentUuid?: string` (already exists as part of tool state; promote to tab-level field for experiment tabs).

---

## 7. Ideas JSON Format

Replace `ideas.md` with `ideas.json`:

```json
[
  { "id": "uuid-1", "text": "Ablate positional embeddings (RoPE vs ALiBi)", "trashed": false, "order": 0 },
  { "id": "uuid-2", "text": "Train small proxy on C4", "trashed": false, "order": 1 }
]
```

- `trashed: true` items are not rendered (kept for future recovery)
- Drag-to-reorder updates `order`
- Add idea creates a new entry with generated UUID
- Trash button sets `trashed: true` (no deletion)

**Rust changes:**
- Remove `save_ideas` command (markdown)
- Add `read_ideas` → returns `Vec<IdeaItem>`
- Add `save_ideas_json` → accepts `Vec<IdeaItem>`, writes `ideas.json`
- Wiki-changed event fires on `ideas.json` path

**Frontend changes:**
- Remove `MarkdownPane` usage for ideas
- New `IdeasPane` component: renders draggable idea cards, inline add, trash

---

## 8. Focus Split Files

### File layout

```
.ire/wiki/pulse/RESEARCH-QUESTION.md   ← plain text (no frontmatter needed)
.ire/wiki/pulse/THIS-WEEK.md           ← plain text
```

**Rust changes:**
- Remove `update_pulse_focus` command
- Add `read_pulse` → returns `{ research_question: String, this_week: String }`
- Add `save_pulse_field(field: "research_question" | "this_week", content: String)` → writes appropriate file, fires wiki-changed event
- Init workspace creates these files with placeholder content

**Frontend changes:**
- Remove `FocusBanner` component
- New `FocusPane` component with two inline-editable fields
- Wiki-changed listener for `pulse/RESEARCH-QUESTION.md` and `pulse/THIS-WEEK.md`

---

## 9. Status Bar System Metrics (New IPC)

New Tauri command: `get_system_status` → returns:

```rust
struct SystemStatus {
    workspace_path: String,       // display path
    git_branch: String,           // current branch or "HEAD"
    git_diff_stat: (u32, u32),    // (insertions, deletions)
    cpu_model: String,
    cpu_usage_pct: f32,
    gpu_model: Option<String>,    // None if no GPU detected
    gpu_usage_pct: Option<f32>,
    gpu_vram_gb: Option<u32>,
    ram_total_gb: u32,
    hostname: String,
    username: String,
    cc_connected: bool,           // true if a CC subprocess session is active
}
```

Polled every 5 seconds via a `setInterval` in a new `useSystemStatus` hook. GPU detection uses `nvidia-smi` (Unix) or `wmic` (Windows) via `std::process::Command` — returns `None` if unavailable.

---

## 10. Chat View (Updated)

The chat view matches the `view-chat` section of `workspace_home.html`:

- **User bubble**: right-aligned, `bg-surface-container border border-outline-variant`, max-width 560px, rounded, 14px text
- **Assistant block**: left-aligned, max-width 720px, `space-y-4`
  - Thinking block: left vertical bar (`w-px bg-outline-variant`) + italic 13px muted text, collapsed by default (accordion), content is plain text
  - Text content: 14px `text-on-surface`, rendered via `MessageMarkdown`
  - Tool call — done: `bg-surface-container-low border border-outline-variant rounded`, check_circle icon in `text-ok`, monospace tool name, duration on right
  - Tool call — running: `border-warn/40` header with spinning icon in `text-warn`, monospace tool call in `text-warn`, expandable log body in `bg-surface-container-lowest font-mono text-[11px]`
  - Experiment card: existing `ExperimentCard` component, restyled to match design
- No demo/placeholder messages in initial state — message list is empty until user sends

---

## 11. Resource Preview Tab (Updated)

The resource preview tab matches the `view-resource` section in `workspace_home.html`:

- Breadcrumb label: `text-[11px] uppercase tracking-widest text-on-surface-variant` — e.g. "Resource · Paper"
- Title: `text-base font-semibold text-on-surface`
- Subtitle: author · arxiv ID in `text-on-surface-variant`
- Body: 14px text, leading-relaxed
- Confirm/discard bar (when status = "ready"): matches existing resource bar logic but restyled

---

## 12. Removed / Deprecated

| Item | Removed |
|---|---|
| `status/pulse.md` | Replaced by `pulse/RESEARCH-QUESTION.md` + `pulse/THIS-WEEK.md` |
| `ideas.md` | Replaced by `ideas.json` |
| `FocusBanner` component | Removed |
| `update_pulse_focus` IPC command | Removed |
| `save_ideas` IPC command | Removed |
| `parseFocus()` helper in Layout | Removed |
| Old `styles.css` component classes | Replaced by Tailwind utilities |
| Mode switcher in chat header | Removed from UI (mode selection logic stays in state) |
| Theme toggle button | Removed — dark mode is fixed, no toggle UI |
| `toggleTheme` / `theme` state | Removed from workspace store |
| `[data-theme="light"]` CSS block | Removed |
| All demo/hardcoded content in HTML prototypes | Not carried into implementation |

---

## 13. Out of Scope

- Drag-to-reorder for status bar items (JS interaction is cosmetic; omit for now, add later)
- Settings panel content (gear button present, panel deferred)
- Workspace init creating the new `pulse/` files with boilerplate (placeholder empty strings are fine)
