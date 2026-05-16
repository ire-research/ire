# Frontend Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the entire frontend with the Tailwind-based design from `docs/design/workspace_home.html` and `docs/design/workspace_picker.html`, migrate Focus to split files, Ideas to JSON, add Experiment tabs, real status bar metrics, and remove light-mode support.

**Architecture:** Tailwind replaces all bespoke CSS; each section of the layout becomes its own focused component. Rust gains new IPC commands for pulse fields, ideas JSON, and system metrics; old commands are removed and the handler list updated atomically.

**Tech Stack:** React 18, Tailwind CSS 3 (PostCSS), Zustand, Tauri 2, Rust (sysinfo crate for metrics)

---

## File Map

### New files
- `src/styles.css` — stripped to @tailwind directives + scrollbar/font-face globals only
- `tailwind.config.ts` — full token set from HTML prototype
- `postcss.config.ts` — tailwind + autoprefixer
- `src/components/left/FocusPane.tsx` — research question + this-week inline-edit
- `src/components/left/LeftRail.tsx` — left rail shell with three sections
- `src/components/left/ExperimentsSection.tsx` — experiment list in left rail
- `src/components/right/IdeasPane.tsx` — drag-to-reorder idea cards
- `src/components/right/NotesPane.tsx` — notes edit/preview
- `src/components/right/RightRail.tsx` — right rail shell
- `src/components/right/AddResourceSection.tsx` — URL input section
- `src/components/StatusBar.tsx` — bottom status bar
- `src/components/chat/ExperimentTabView.tsx` — experiment tab content
- `src/hooks/useSystemStatus.ts` — polls get_system_status every 5s
- `src-tauri/src/commands/system.rs` — get_system_status command
- `src-tauri/assets/seed/pulse/RESEARCH-QUESTION.md` — seed file
- `src-tauri/assets/seed/pulse/THIS-WEEK.md` — seed file

### Modified files
- `index.html` — add Google Fonts (Inter, JetBrains Mono, Material Symbols)
- `vite.config.ts` — no change needed; postcss auto-discovered
- `package.json` — add tailwindcss, autoprefixer, postcss, @dnd-kit packages
- `src/types.ts` — add `"experiment"` to TabKind, add IdeaItem, SystemStatus, PulseContent types; add `experimentUuid` to Tab
- `src/ipc.ts` — add read_pulse, save_pulse_field, read_ideas, save_ideas_json, get_system_status; remove save_ideas, update_pulse_focus
- `src/state/workspace.ts` — remove theme/toggleTheme
- `src/state/chat.ts` — add openExperimentTab action
- `src/App.tsx` — remove theme effect; dark class always on `<html>`
- `src/main.tsx` — no change
- `src/components/Layout.tsx` — full rewrite using new rail components + status bar
- `src/components/chat/ChatPane.tsx` — route experiment log lines to experiment tabs; remove mode switcher UI
- `src/components/chat/TabBar.tsx` — restyle to match design; add experiment icon
- `src/components/chat/MessageList.tsx` — restyle messages to match design
- `src/components/chat/Composer.tsx` — restyle floating composer
- `src/components/chat/ExperimentCard.tsx` — restyle inline experiment card
- `src/components/setup/SetupScreen.tsx` — full rewrite to match workspace_picker.html
- `src-tauri/src/lib.rs` — register new commands, remove old ones
- `src-tauri/src/commands/wiki.rs` — remove save_ideas/update_pulse_focus; add read_pulse, save_pulse_field, read_ideas, save_ideas_json
- `src-tauri/src/commands/mod.rs` — add system module
- `src-tauri/src/workspace/init.rs` — scaffold pulse/ dir + seed files; remove ideas.md/pulse.md seeding
- `src-tauri/Cargo.toml` — add sysinfo crate

### Deleted files
- `src/components/FocusBanner.tsx`
- `src/components/MarkdownPane.tsx` (replaced by NotesPane)
- `src/components/ResourceInput.tsx` (folded into AddResourceSection)
- `src/components/ResourcesList.tsx` (folded into LeftRail)

---

## Task 1: Install Tailwind and configure design tokens

**Files:**
- Create: `tailwind.config.ts`
- Create: `postcss.config.ts`
- Modify: `package.json`
- Modify: `src/styles.css`
- Modify: `index.html`

- [ ] Install packages:
```bash
cd /home/gciro/repos/ire
npm install -D tailwindcss@3 autoprefixer postcss
```

- [ ] Create `postcss.config.ts`:
```ts
export default {
  plugins: {
    tailwindcss: {},
    autoprefixer: {},
  },
};
```

- [ ] Create `tailwind.config.ts`:
```ts
import type { Config } from "tailwindcss";

export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        "surface-dim": "#0e0e11",
        "surface-container-lowest": "#000000",
        "surface": "#0e0e11",
        "background": "#0e0e11",
        "surface-container-low": "#131317",
        "surface-container": "#19191e",
        "surface-container-high": "#1e1f26",
        "surface-container-highest": "#24252d",
        "surface-bright": "#2a2b35",
        "surface-variant": "#24252d",
        "on-surface": "#e6e4ef",
        "on-surface-variant": "#abaab4",
        "on-background": "#e6e4ef",
        "primary": "#c6c6c9",
        "primary-dim": "#b8b8bb",
        "primary-fixed": "#e2e2e5",
        "primary-fixed-dim": "#d4d4d7",
        "primary-container": "#454749",
        "on-primary": "#3f4043",
        "on-primary-fixed": "#3e3f42",
        "on-primary-fixed-variant": "#5a5b5e",
        "on-primary-container": "#d0d0d3",
        "secondary": "#9d9da6",
        "secondary-dim": "#9d9da6",
        "secondary-container": "#3a3b43",
        "secondary-fixed": "#e3e1ec",
        "secondary-fixed-dim": "#d4d3dd",
        "on-secondary": "#1f2027",
        "on-secondary-fixed": "#3e3f47",
        "on-secondary-fixed-variant": "#5a5b63",
        "on-secondary-container": "#bfbec8",
        "tertiary": "#f9f9fd",
        "tertiary-dim": "#ebebef",
        "tertiary-container": "#ebebef",
        "tertiary-fixed": "#f3f3f7",
        "tertiary-fixed-dim": "#e5e5e9",
        "on-tertiary": "#5e5f62",
        "on-tertiary-container": "#55575a",
        "on-tertiary-fixed": "#484a4d",
        "on-tertiary-fixed-variant": "#65666a",
        "outline": "#75757e",
        "outline-variant": "#474750",
        "inverse-surface": "#fbf8fc",
        "inverse-on-surface": "#555458",
        "inverse-primary": "#5e5f62",
        "surface-tint": "#c6c6c9",
        "error": "#ec7c8a",
        "error-dim": "#b95463",
        "error-container": "#7f2737",
        "on-error": "#490013",
        "on-error-container": "#ff97a3",
        "warn": "#d97706",
        "ok": "#10b981",
        "accent": "#E4E4E7",
        "accent-fg": "#0A0A0A",
      },
      borderRadius: {
        DEFAULT: "0.125rem",
        lg: "0.25rem",
        xl: "0.5rem",
        full: "0.75rem",
      },
      fontFamily: {
        sans: ["Inter", "system-ui", "sans-serif"],
        mono: ["JetBrains Mono", "monospace"],
      },
    },
  },
  plugins: [],
} satisfies Config;
```

- [ ] Replace `src/styles.css` content entirely:
```css
@tailwind base;
@tailwind components;
@tailwind utilities;

/* Scrollbars */
* {
  scrollbar-width: thin;
  scrollbar-color: #3a3b43 transparent;
}
*::-webkit-scrollbar { width: 4px; height: 4px; }
*::-webkit-scrollbar-track { background: transparent; }
*::-webkit-scrollbar-thumb { background: #3a3b43; border-radius: 2px; }
*::-webkit-scrollbar-thumb:hover { background: #474750; }
*::-webkit-scrollbar-corner { background: transparent; }

.no-scrollbar::-webkit-scrollbar { display: none; }
.no-scrollbar { -ms-overflow-style: none; scrollbar-width: none; }

/* field-sizing for auto-resize textarea */
#composer-textarea {
  field-sizing: content;
}
```

- [ ] Update `index.html` to add fonts and ensure `dark` class on `<html>`:
```html
<!doctype html>
<html lang="en" class="dark">
  <head>
    <meta charset="UTF-8" />
    <link rel="icon" type="image/svg+xml" href="/vite.svg" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>IRE</title>
    <link href="https://fonts.googleapis.com/css2?family=Material+Symbols+Outlined:opsz,wght,FILL,GRAD@20..48,100..700,0..1,-50..200&display=swap" rel="stylesheet" />
    <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500&display=swap" rel="stylesheet" />
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

- [ ] Verify Tailwind is working — run dev build and confirm no CSS errors:
```bash
npm run build 2>&1 | head -30
```
Expected: no PostCSS/Tailwind errors.

- [ ] Commit:
```bash
git add tailwind.config.ts postcss.config.ts src/styles.css index.html package.json package-lock.json
git commit -m "feat: add tailwind with design token config"
```

---

## Task 2: Update types and IPC surface

**Files:**
- Modify: `src/types.ts`
- Modify: `src/ipc.ts`

- [ ] Update `src/types.ts` — add new types, extend Tab, update TabKind:

Replace the `TabKind` line:
```ts
export type TabKind = "chat" | "resource" | "preview" | "experiment";
```

Add to Tab interface (after `wikiPath?`):
```ts
  experimentUuid?: string;
```

Add new types after `ExperimentLogLinePayload`:
```ts
export interface IdeaItem {
  id: string;
  text: string;
  trashed: boolean;
  order: number;
}

export interface PulseContent {
  research_question: string;
  this_week: string;
}

export interface SystemStatus {
  workspace_path: string;
  git_branch: string;
  git_insertions: number;
  git_deletions: number;
  cpu_model: string;
  cpu_usage_pct: number;
  gpu_model: string | null;
  gpu_usage_pct: number | null;
  gpu_vram_gb: number | null;
  ram_total_gb: number;
  hostname: string;
  username: string;
  cc_connected: boolean;
}
```

- [ ] Update `src/ipc.ts` — remove old commands, add new ones.

Remove from imports and ipc object: `save_ideas`, `update_pulse_focus`.

Add to the `ipc` object:
```ts
  readPulse: (): Promise<PulseContent> => invoke("read_pulse"),
  savePulseField: (field: "research_question" | "this_week", content: string): Promise<void> =>
    invoke("save_pulse_field", { field, content }),
  readIdeas: (): Promise<IdeaItem[]> => invoke("read_ideas"),
  saveIdeasJson: (ideas: IdeaItem[]): Promise<void> => invoke("save_ideas_json", { ideas }),
  getSystemStatus: (): Promise<SystemStatus> => invoke("get_system_status"),
```

Add imports at top of ipc.ts:
```ts
import type { IdeaItem, PulseContent, SystemStatus, ... } from "./types";
```
(merge with existing import)

- [ ] Commit:
```bash
git add src/types.ts src/ipc.ts
git commit -m "feat: update types and IPC surface for redesign"
```

---

## Task 3: Rust — remove old commands, add pulse/ideas/system commands

**Files:**
- Modify: `src-tauri/src/commands/wiki.rs`
- Create: `src-tauri/src/commands/system.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`

- [ ] Add `sysinfo` to `src-tauri/Cargo.toml` dependencies:
```toml
sysinfo = "0.32"
```

- [ ] In `src-tauri/src/commands/wiki.rs`, remove `save_ideas` and `update_pulse_focus` functions entirely. Add new commands:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct PulseContent {
    pub research_question: String,
    pub this_week: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IdeaItem {
    pub id: String,
    pub text: String,
    pub trashed: bool,
    pub order: i64,
}

#[tauri::command]
pub fn read_pulse(active: State<'_, ActiveWorkspace>) -> Result<PulseContent, String> {
    let store = wiki_store(&active)?;
    let rq = store.read("pulse/RESEARCH-QUESTION.md")
        .map(|(c, _)| c)
        .unwrap_or_default();
    let tw = store.read("pulse/THIS-WEEK.md")
        .map(|(c, _)| c)
        .unwrap_or_default();
    Ok(PulseContent { research_question: rq, this_week: tw })
}

#[tauri::command]
pub fn save_pulse_field(
    field: String,
    content: String,
    active: State<'_, ActiveWorkspace>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let path = match field.as_str() {
        "research_question" => "pulse/RESEARCH-QUESTION.md",
        "this_week" => "pulse/THIS-WEEK.md",
        _ => return Err(format!("unknown field: {field}")),
    };
    let store = wiki_store(&active)?;
    store.write(path, &content, &app).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn read_ideas(active: State<'_, ActiveWorkspace>) -> Result<Vec<IdeaItem>, String> {
    let store = wiki_store(&active)?;
    let path = store.wiki_root.join("ideas.json");
    if !path.exists() {
        return Ok(vec![]);
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_ideas_json(
    ideas: Vec<IdeaItem>,
    active: State<'_, ActiveWorkspace>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let store = wiki_store(&active)?;
    let json = serde_json::to_string_pretty(&ideas).map_err(|e| e.to_string())?;
    store.write("ideas.json", &json, &app).map_err(|e| e.to_string())
}
```

- [ ] Create `src-tauri/src/commands/system.rs`:

```rust
use serde::Serialize;
use sysinfo::{CpuRefreshKind, RefreshKind, System};
use tauri::State;
use std::process::Command;

use crate::workspace::state::ActiveWorkspace;

#[derive(Debug, Serialize)]
pub struct SystemStatus {
    pub workspace_path: String,
    pub git_branch: String,
    pub git_insertions: u32,
    pub git_deletions: u32,
    pub cpu_model: String,
    pub cpu_usage_pct: f32,
    pub gpu_model: Option<String>,
    pub gpu_usage_pct: Option<f32>,
    pub gpu_vram_gb: Option<u32>,
    pub ram_total_gb: u32,
    pub hostname: String,
    pub username: String,
    pub cc_connected: bool,
}

#[tauri::command]
pub fn get_system_status(active: State<'_, ActiveWorkspace>) -> Result<SystemStatus, String> {
    let workspace_path = {
        let guard = active.0.lock().map_err(|e| e.to_string())?;
        guard.as_ref().ok_or("no workspace open")?.state.path.clone()
    };

    // Git branch
    let git_branch = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&workspace_path)
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "HEAD".to_string());

    // Git diff stat (staged + unstaged insertions/deletions vs HEAD)
    let (git_insertions, git_deletions) = git_diff_stat(&workspace_path);

    // CPU via sysinfo
    let mut sys = System::new_with_specifics(
        RefreshKind::new().with_cpu(CpuRefreshKind::everything()),
    );
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    sys.refresh_cpu_all();
    let cpu_model = sys.cpus().first()
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "Unknown CPU".to_string());
    let cpu_usage_pct = sys.global_cpu_usage();

    // RAM via sysinfo
    sys.refresh_memory();
    let ram_total_gb = (sys.total_memory() / 1_073_741_824) as u32;

    // GPU via nvidia-smi (best-effort)
    let (gpu_model, gpu_usage_pct, gpu_vram_gb) = query_nvidia_smi();

    // Hostname + username
    let hostname = System::host_name().unwrap_or_else(|| "unknown".to_string());
    let username = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "user".to_string());

    let display_path = workspace_path.to_string_lossy()
        .replace(&std::env::var("HOME").unwrap_or_default(), "~");

    Ok(SystemStatus {
        workspace_path: display_path,
        git_branch,
        git_insertions,
        git_deletions,
        cpu_model,
        cpu_usage_pct,
        gpu_model,
        gpu_usage_pct,
        gpu_vram_gb,
        ram_total_gb,
        hostname,
        username,
        cc_connected: true, // IRE is running, so CC session infra is present
    })
}

fn git_diff_stat(path: &std::path::Path) -> (u32, u32) {
    let out = Command::new("git")
        .args(["diff", "--shortstat", "HEAD"])
        .current_dir(path)
        .output();
    let Ok(out) = out else { return (0, 0) };
    let s = String::from_utf8_lossy(&out.stdout);
    let ins = parse_stat(&s, "insertion");
    let del = parse_stat(&s, "deletion");
    (ins, del)
}

fn parse_stat(s: &str, keyword: &str) -> u32 {
    s.split(',')
        .find(|part| part.contains(keyword))
        .and_then(|part| part.trim().split_whitespace().next())
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}

fn query_nvidia_smi() -> (Option<String>, Option<f32>, Option<u32>) {
    let out = Command::new("nvidia-smi")
        .args(["--query-gpu=name,utilization.gpu,memory.total", "--format=csv,noheader,nounits"])
        .output();
    let Ok(out) = out else { return (None, None, None) };
    if !out.status.success() { return (None, None, None); }
    let s = String::from_utf8_lossy(&out.stdout);
    let line = s.lines().next().unwrap_or("");
    let parts: Vec<&str> = line.splitn(3, ',').map(str::trim).collect();
    if parts.len() < 3 { return (None, None, None); }
    let model = Some(parts[0].to_string());
    let usage = parts[1].parse::<f32>().ok();
    let vram_mb = parts[2].parse::<u32>().ok();
    let vram_gb = vram_mb.map(|m| m / 1024);
    (model, usage, vram_gb)
}
```

- [ ] Add `pub mod system;` to `src-tauri/src/commands/mod.rs`:
```rust
pub mod chat;
pub mod experiments;
pub mod resources;
pub mod system;
pub mod wiki;
```

- [ ] Update `src-tauri/src/lib.rs` — update imports and handler list:

Remove from imports: `save_ideas, update_pulse_focus`
Add to imports:
```rust
use commands::wiki::{read_wiki_file, save_notes, save_wiki_file, read_pulse, save_pulse_field, read_ideas, save_ideas_json};
use commands::system::get_system_status;
```

Update `generate_handler![]` — remove `save_ideas, update_pulse_focus`, add:
```
read_pulse,
save_pulse_field,
read_ideas,
save_ideas_json,
get_system_status,
```

- [ ] Build to confirm compilation:
```bash
cd src-tauri && cargo build 2>&1 | tail -20
```
Expected: no errors (warnings about unused imports from removed commands are fine — fix them).

- [ ] Commit:
```bash
git add src-tauri/
git commit -m "feat: add pulse/ideas/system Rust commands, remove old pulse/ideas commands"
```

---

## Task 4: Workspace init — scaffold new seed files

**Files:**
- Modify: `src-tauri/src/workspace/init.rs`
- Create: `src-tauri/assets/seed/pulse/RESEARCH-QUESTION.md`
- Create: `src-tauri/assets/seed/pulse/THIS-WEEK.md`

- [ ] Create `src-tauri/assets/seed/pulse/RESEARCH-QUESTION.md`:
```
_What are you trying to figure out?_
```

- [ ] Create `src-tauri/assets/seed/pulse/THIS-WEEK.md`:
```
_What is the concrete focus this week?_
```

- [ ] Update `src-tauri/src/workspace/init.rs`:

At top, add constants:
```rust
const RESEARCH_QUESTION_MD: &str = include_str!("../../assets/seed/pulse/RESEARCH-QUESTION.md");
const THIS_WEEK_MD: &str = include_str!("../../assets/seed/pulse/THIS-WEEK.md");
```

Remove constants:
```rust
const PULSE_MD: ...  // delete this line
```

In `initialize()`, replace the pulse/ideas lines:
```rust
// OLD (remove):
write_if_absent(&wiki.join("ideas.md"), "")?;
write_if_absent(&status.join("pulse.md"), PULSE_MD)?;

// NEW (add):
let pulse_dir = wiki.join("pulse");
fs::create_dir_all(&pulse_dir).with_context(|| format!("create {}", pulse_dir.display()))?;
write_if_absent(&pulse_dir.join("RESEARCH-QUESTION.md"), RESEARCH_QUESTION_MD)?;
write_if_absent(&pulse_dir.join("THIS-WEEK.md"), THIS_WEEK_MD)?;
write_if_absent(&wiki.join("ideas.json"), "[]")?;
```

Remove `&status` from the loop that creates dirs (only if it no longer needs creating — keep `short_term` dir creation since `short-term/` is still part of the layout).

Update `index_seed()` to reflect new paths:
```rust
fn index_seed() -> String {
    "- [_SYSTEM.md](./_SYSTEM.md) — IRE framework context for the agent\n\
     - [_schema.md](./_schema.md) — wiki conventions for the agent\n\
     - [notes.md](./notes.md) — running user notes\n\
     - [ideas.json](./ideas.json) — user ideas list\n\
     - [pulse/RESEARCH-QUESTION.md](./pulse/RESEARCH-QUESTION.md) — current research question\n\
     - [pulse/THIS-WEEK.md](./pulse/THIS-WEEK.md) — this week's focus\n\
     - [status/long-term.md](./status/long-term.md) — architectural decisions and pivots\n\
     - [status/failures.md](./status/failures.md) — methods that did not work\n"
        .to_string()
}
```

Update the test assertions to check for new paths instead of old ones:
- Replace `".ire/wiki/ideas.md"` with `".ire/wiki/ideas.json"`
- Replace `".ire/wiki/status/pulse.md"` with `".ire/wiki/pulse/RESEARCH-QUESTION.md"`
- Remove `PULSE_MD` reference from test custom-content check

- [ ] Build:
```bash
cd src-tauri && cargo build 2>&1 | tail -20
```
Expected: clean build.

- [ ] Commit:
```bash
git add src-tauri/
git commit -m "feat: scaffold pulse/ split files and ideas.json on workspace init"
```

---

## Task 5: Remove theme from frontend state; clean App.tsx

**Files:**
- Modify: `src/state/workspace.ts`
- Modify: `src/App.tsx`

- [ ] In `src/state/workspace.ts`, remove all theme-related code:
  - Remove `type Theme` type
  - Remove `theme: Theme` from store interface
  - Remove `toggleTheme`, `setTheme` from interface and implementation
  - Remove `theme: "dark"` default state
  - Remove theme from `hydrateFromUserConfig`

- [ ] In `src/App.tsx`:
  - Remove `theme` and `hydrateFromUserConfig` from workspace selectors
  - Remove the `useEffect` that applies `data-theme` to DOM
  - Remove `hydrateFromUserConfig` call (user config no longer stores theme)
  - Keep `ipc.readUserConfig()` call only for `recent_workspaces`; update the handler:
```tsx
const [status, config] = await Promise.all([
  ipc.setupStatus(),
  ipc.readUserConfig().catch(() => null),
]);
if (config?.recent_workspaces) {
  useWorkspace.getState().setRecentWorkspaces(config.recent_workspaces);
}
setPhase({ kind: "setup", status });
```

- [ ] Build check:
```bash
npm run build 2>&1 | grep -E "error|Error" | head -20
```
Expected: no TypeScript errors.

- [ ] Commit:
```bash
git add src/state/workspace.ts src/App.tsx
git commit -m "feat: remove theme toggle — dark mode fixed"
```

---

## Task 6: SetupScreen redesign (workspace picker)

**Files:**
- Modify: `src/components/setup/SetupScreen.tsx`

- [ ] Rewrite `SetupScreen.tsx` to match `workspace_picker.html` exactly:

```tsx
import { useState } from "react";
import { ipc, pickDirectory, type SetupStatus } from "../../ipc";
import { useWorkspace } from "../../state/workspace";
import { useChatOptions, EFFORT_LEVELS } from "../../state/chatOptions";
import type { EffortLevel } from "../../types";

interface Props {
  status: SetupStatus;
  onRefresh: () => Promise<void>;
}

function relativeTime(path: string): string {
  // Placeholder — actual timestamps not yet stored; show empty
  return "";
}

export function SetupScreen({ status, onRefresh }: Props) {
  const setPhase = useWorkspace((s) => s.setPhase);
  const hydrateFromPersisted = useWorkspace((s) => s.hydrateFromPersisted);
  const pushRecentWorkspace = useWorkspace((s) => s.pushRecentWorkspace);
  const recentWorkspaces = useWorkspace((s) => s.recentWorkspaces);
  const setEffort = useChatOptions((s) => s.setEffort);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const binaryFound = status.binary.kind === "found";

  const applyPersisted = (persisted: Parameters<typeof hydrateFromPersisted>[0]) => {
    hydrateFromPersisted(persisted);
    if (persisted.effort && EFFORT_LEVELS.some((e) => e.value === persisted.effort)) {
      setEffort(persisted.effort as EffortLevel);
    }
  };

  const openWorkspace = async (path: string) => {
    setError(null);
    setBusy(true);
    try {
      const workspace = await ipc.openWorkspace(path);
      pushRecentWorkspace(path);
      const persisted = await ipc.readWorkspaceState().catch(() => null);
      if (persisted) applyPersisted(persisted);
      setPhase({ kind: "ready", workspace });
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const handlePick = async (kind: "open" | "init") => {
    setError(null);
    const path = await pickDirectory(
      kind === "open" ? "Open existing IRE workspace" : "Pick a directory to initialize",
    );
    if (!path) return;
    setBusy(true);
    try {
      const workspace =
        kind === "open" ? await ipc.openWorkspace(path) : await ipc.initWorkspace(path);
      pushRecentWorkspace(path);
      const persisted = await ipc.readWorkspaceState().catch(() => null);
      if (persisted) applyPersisted(persisted);
      setPhase({ kind: "ready", workspace });
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="bg-background min-h-screen flex flex-col items-center justify-center overflow-y-auto px-6 py-10 text-on-surface font-sans">
      <div className="w-full max-w-[520px] flex flex-col gap-6">

        {/* Title */}
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-semibold text-on-surface tracking-tight leading-snug">
            Open or create a workspace.
          </h1>
          <p className="text-[14px] text-on-surface-variant leading-relaxed">
            Each workspace maps 1:1 to a Git repository. Your code, wiki, experiments, and Claude Code state live together in{" "}
            <code className="font-mono text-[12px] px-1 py-0.5 bg-surface-container border border-outline-variant rounded">
              .ire/
            </code>.
          </p>
        </div>

        {/* Binary warning (only if missing) */}
        {!binaryFound && (
          <div className="border border-error/30 bg-error/5 rounded px-4 py-3 text-[13px] text-error">
            <p className="mb-1 font-medium">Claude Code CLI not found.</p>
            <p className="text-on-surface-variant">
              Install with <code className="font-mono">npm install -g @anthropic-ai/claude-code</code>
            </p>
            <button
              onClick={onRefresh}
              className="mt-2 text-[12px] border border-outline-variant rounded px-3 py-1 text-on-surface-variant hover:text-on-surface hover:bg-surface-container-low transition-colors"
            >
              Retry
            </button>
          </div>
        )}

        {/* Recent workspaces */}
        {recentWorkspaces.length > 0 && (
          <div className="flex flex-col gap-2">
            <div className="flex items-center justify-between">
              <span className="text-[12px] font-medium text-on-surface-variant">Recent</span>
              <button
                onClick={onRefresh}
                className="text-on-surface-variant hover:text-on-surface transition-colors p-0.5 border-none bg-transparent"
              >
                <span className="material-symbols-outlined text-[16px]">refresh</span>
              </button>
            </div>
            <div className="flex flex-col border border-outline-variant rounded overflow-hidden">
              {recentWorkspaces.map((path, i) => {
                const name = path.split("/").filter(Boolean).pop() ?? path;
                const display = path.replace(
                  (window as any).__HOME__ ?? "",
                  "~"
                );
                return (
                  <button
                    key={path}
                    disabled={!binaryFound || busy}
                    onClick={() => openWorkspace(path)}
                    className={[
                      "flex items-center justify-between w-full px-4 py-3 text-left transition-colors group",
                      i === 0
                        ? "bg-surface-container-low border-l-2 border-l-primary hover:bg-surface-container-highest"
                        : "border-l-2 border-l-transparent hover:bg-surface-container-low",
                      i < recentWorkspaces.length - 1 ? "border-b border-b-outline-variant" : "",
                      "disabled:opacity-50 disabled:cursor-not-allowed",
                    ].join(" ")}
                  >
                    <div className="flex flex-col gap-0.5 min-w-0">
                      <span className={`text-[14px] truncate ${i === 0 ? "font-semibold text-on-surface" : "font-medium text-on-surface"}`}>
                        {name}
                      </span>
                      <span className="font-mono text-[11px] text-on-surface-variant group-hover:text-on-surface transition-colors truncate">
                        {display}
                      </span>
                    </div>
                    <span className="material-symbols-outlined text-outline-variant text-[16px] group-hover:text-on-surface transition-colors shrink-0 ml-3">
                      chevron_right
                    </span>
                  </button>
                );
              })}
            </div>
          </div>
        )}

        {/* Actions */}
        <div className="flex gap-3">
          <button
            disabled={!binaryFound || busy}
            onClick={() => handlePick("open")}
            className="flex-1 h-9 border border-outline-variant rounded text-[14px] font-medium text-on-surface hover:bg-surface-container-low hover:border-outline transition-colors flex items-center justify-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <span className="material-symbols-outlined text-[16px] text-on-surface-variant">folder_open</span>
            Open folder…
          </button>
          <button
            disabled={!binaryFound || busy}
            onClick={() => handlePick("init")}
            className="flex-1 h-9 border border-outline-variant rounded text-[14px] font-medium text-on-surface hover:bg-surface-container-low hover:border-outline transition-colors flex items-center justify-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <span className="material-symbols-outlined text-[16px] text-on-surface-variant">add</span>
            New workspace…
          </button>
        </div>

        {error && (
          <p className="text-[13px] text-error border border-error/30 rounded px-3 py-2 bg-error/5">
            {error}
          </p>
        )}

        {/* Divider */}
        <div className="w-full h-px bg-outline-variant" />

        {/* Status row */}
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <span className={`w-1.5 h-1.5 rounded-full ${binaryFound ? "bg-ok" : "bg-error"}`} />
            <span className="font-mono text-[11px] text-on-surface-variant">
              claude-code · {binaryFound ? "authenticated" : "not found"}
            </span>
          </div>
          <button className="text-[12px] text-on-surface-variant hover:text-on-surface transition-colors border-none bg-transparent">
            settings
          </button>
        </div>

      </div>
    </div>
  );
}
```

- [ ] Build check:
```bash
npm run build 2>&1 | grep -E "error TS" | head -20
```

- [ ] Commit:
```bash
git add src/components/setup/SetupScreen.tsx
git commit -m "feat: redesign workspace picker to match new design"
```

---

## Task 7: Left rail — FocusPane

**Files:**
- Create: `src/components/left/FocusPane.tsx`
- Delete: `src/components/FocusBanner.tsx`

- [ ] Create `src/components/left/FocusPane.tsx`:

```tsx
import { useState } from "react";
import { ipc } from "../../ipc";
import { toastError } from "../../state/toasts";
import type { PulseContent } from "../../types";

interface Props {
  pulse: PulseContent;
  onChange: (updated: PulseContent) => void;
}

function InlineField({
  label,
  value,
  field,
  onChange,
}: {
  label: string;
  field: "research_question" | "this_week";
  value: string;
  onChange: (v: string) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(value);

  const commit = async () => {
    const trimmed = draft.trim();
    setEditing(false);
    if (trimmed === value.trim()) return;
    onChange(trimmed);
    try {
      await ipc.savePulseField(field, trimmed);
    } catch (e) {
      toastError("save pulse", e);
    }
  };

  return (
    <div className="group/field pl-1">
      <div className="flex items-center justify-between mb-1.5">
        <span className="text-[11px] text-on-surface-variant font-medium">{label}</span>
        {!editing && (
          <button
            onClick={() => { setDraft(value); setEditing(true); }}
            className="opacity-0 group-hover/field:opacity-100 transition-opacity p-0.5 text-on-surface-variant hover:text-on-surface border-none bg-transparent"
            title={`Edit ${label}`}
          >
            <span className="material-symbols-outlined text-[14px]">edit_document</span>
          </button>
        )}
      </div>
      {editing ? (
        <textarea
          autoFocus
          className="w-full bg-surface-container border border-outline-variant rounded text-[13px] text-on-surface px-2 py-1.5 resize-none focus:border-outline focus:outline-none"
          rows={3}
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === "Escape") { setDraft(value); setEditing(false); }
            if (e.key === "Enter" && e.metaKey) commit();
          }}
        />
      ) : (
        <p
          className="text-[14px] text-on-surface leading-relaxed cursor-text"
          onClick={() => { setDraft(value); setEditing(true); }}
        >
          {value || <span className="text-on-surface-variant italic">Click to edit…</span>}
        </p>
      )}
    </div>
  );
}

export function FocusPane({ pulse, onChange }: Props) {
  return (
    <div className="px-4 pt-4 pb-3 overflow-y-auto flex-1">
      <div className="flex items-center gap-2 px-0 py-1 mb-2">
        <span className="material-symbols-outlined text-[16px] shrink-0 text-on-surface-variant">target</span>
        <span className="text-[14px] text-on-surface-variant">Focus</span>
      </div>
      <div className="mb-3">
        <InlineField
          label="Research question"
          field="research_question"
          value={pulse.research_question}
          onChange={(v) => onChange({ ...pulse, research_question: v })}
        />
      </div>
      <div>
        <InlineField
          label="This week"
          field="this_week"
          value={pulse.this_week}
          onChange={(v) => onChange({ ...pulse, this_week: v })}
        />
      </div>
    </div>
  );
}
```

- [ ] Delete `src/components/FocusBanner.tsx`:
```bash
rm src/components/FocusBanner.tsx
```

- [ ] Commit:
```bash
git add src/components/left/ src/components/FocusBanner.tsx
git commit -m "feat: add FocusPane with split pulse fields, remove FocusBanner"
```

---

## Task 8: Left rail — ExperimentsSection and LeftRail shell

**Files:**
- Create: `src/components/left/ExperimentsSection.tsx`
- Create: `src/components/left/LeftRail.tsx`
- Modify: `src/state/chat.ts`

- [ ] Add `openExperimentTab` to `src/state/chat.ts`. In the `ChatStore` interface add:
```ts
openExperimentTab: (uuid: string, name: string) => void;
```
In the `create()` implementation add:
```ts
openExperimentTab: (uuid, name) =>
  set((s) => {
    const existing = s.tabs.find((t) => t.kind === "experiment" && t.experimentUuid === uuid);
    if (existing) {
      return { previousTabId: s.activeTabId, activeTabId: existing.id };
    }
    const id = crypto.randomUUID();
    return {
      tabs: [...s.tabs, {
        id, label: name, messages: [], isStreaming: false, isPinned: false,
        kind: "experiment", experimentUuid: uuid,
      }],
      previousTabId: s.activeTabId,
      activeTabId: id,
    };
  }),
```

- [ ] Create `src/components/left/ExperimentsSection.tsx`:

```tsx
import { useEffect, useState } from "react";
import { ipc } from "../../ipc";
import { useChat } from "../../state/chat";
import { toastError } from "../../state/toasts";
import type { ExperimentRow } from "../../types";

function statusPill(status: string) {
  switch (status) {
    case "running":
      return <span className="text-warn text-[10px] uppercase border border-warn/30 px-1 rounded bg-warn/10 shrink-0">Run</span>;
    case "completed":
      return <span className="text-ok text-[10px] uppercase border border-ok/30 px-1 rounded bg-ok/10 shrink-0">Done</span>;
    case "failed":
      return <span className="text-error text-[10px] uppercase border border-error/30 px-1 rounded bg-error/10 shrink-0">Fail</span>;
    case "cancelled":
      return <span className="text-on-surface-variant text-[10px] uppercase border border-outline-variant px-1 rounded shrink-0">Cancel</span>;
    default:
      return <span className="text-on-surface-variant text-[10px] uppercase border border-outline-variant px-1 rounded shrink-0">{status}</span>;
  }
}

export function ExperimentsSection() {
  const [experiments, setExperiments] = useState<ExperimentRow[]>([]);
  const openExperimentTab = useChat((s) => s.openExperimentTab);

  useEffect(() => {
    ipc.experimentList(20)
      .then(setExperiments)
      .catch((e) => toastError("load experiments", e));
  }, []);

  return (
    <div className="overflow-y-auto flex-1 py-1">
      <div className="flex items-center gap-2 px-4 py-2 text-on-surface-variant text-[14px]">
        <span className="material-symbols-outlined text-[16px] shrink-0">science</span>
        Experiments
      </div>
      <div className="px-4 pb-1 space-y-0.5">
        {experiments.length === 0 && (
          <p className="text-[12px] text-on-surface-variant px-2 py-1">No experiments yet</p>
        )}
        {experiments.map((exp) => (
          <button
            key={exp.uuid}
            onClick={() => openExperimentTab(exp.uuid, exp.name)}
            className="w-full flex items-center justify-between px-2 py-1.5 rounded hover:bg-surface-container-high transition-colors cursor-pointer"
          >
            <span className="font-mono text-[13px] text-on-surface truncate pr-2">{exp.name}</span>
            {statusPill(exp.status)}
          </button>
        ))}
      </div>
    </div>
  );
}
```

- [ ] Create `src/components/left/LeftRail.tsx`:

```tsx
import type { ResourceItem } from "../../types";
import type { PulseContent } from "../../types";
import { FocusPane } from "./FocusPane";
import { ExperimentsSection } from "./ExperimentsSection";

interface Props {
  pulse: PulseContent;
  onPulseChange: (p: PulseContent) => void;
  resources: ResourceItem[];
  onResourceClick: (r: ResourceItem) => void;
}

function displayTitle(r: ResourceItem): string {
  if (r.title) return r.title;
  try { return new URL(r.url).hostname; } catch { return r.url; }
}

export function LeftRail({ pulse, onPulseChange, resources, onResourceClick }: Props) {
  return (
    <nav
      className="flex flex-col bg-surface-container-low border-r border-outline-variant shrink-0 overflow-hidden"
      style={{ width: "280px", minWidth: "160px", maxWidth: "420px" }}
    >
      {/* Focus section */}
      <div className="flex flex-col overflow-hidden" style={{ minHeight: "80px", height: "calc((100vh - 64px) / 3)" }}>
        <FocusPane pulse={pulse} onChange={onPulseChange} />
      </div>

      {/* Row drag handle */}
      <div className="h-px bg-outline-variant shrink-0" />

      {/* Resources section */}
      <div className="flex flex-col overflow-hidden" style={{ minHeight: "60px", height: "calc((100vh - 64px) / 3)" }}>
        <div className="overflow-y-auto flex-1 py-1">
          <div className="flex items-center gap-2 px-4 py-2 text-on-surface-variant text-[14px]">
            <span className="material-symbols-outlined text-[16px] shrink-0">description</span>
            Resources
          </div>
          <div className="px-4 pb-1 space-y-0.5">
            {resources.length === 0 && (
              <p className="text-[12px] text-on-surface-variant px-2 py-1">No resources yet</p>
            )}
            {resources.map((r) => (
              <button
                key={r.resource_id}
                onClick={() => r.wiki_path && onResourceClick(r)}
                className="w-full text-left px-2 py-1.5 rounded text-[14px] text-on-surface hover:bg-surface-container-high transition-colors truncate"
                style={{ cursor: r.wiki_path ? "pointer" : "default" }}
              >
                {displayTitle(r)}
              </button>
            ))}
          </div>
        </div>
      </div>

      {/* Row drag handle */}
      <div className="h-px bg-outline-variant shrink-0" />

      {/* Experiments section */}
      <div className="flex flex-col overflow-hidden flex-1" style={{ minHeight: "60px" }}>
        <ExperimentsSection />
      </div>
    </nav>
  );
}
```

- [ ] Commit:
```bash
git add src/components/left/ src/state/chat.ts
git commit -m "feat: add LeftRail, ExperimentsSection, openExperimentTab"
```

---

## Task 9: Right rail — NotesPane, IdeasPane, AddResourceSection, RightRail

**Files:**
- Create: `src/components/right/NotesPane.tsx`
- Create: `src/components/right/IdeasPane.tsx`
- Create: `src/components/right/AddResourceSection.tsx`
- Create: `src/components/right/RightRail.tsx`

- [ ] Create `src/components/right/NotesPane.tsx`:

```tsx
import { useState } from "react";
import { ipc } from "../../ipc";
import { toastError } from "../../state/toasts";

interface Props {
  content: string;
  onChange: (c: string) => void;
}

export function NotesPane({ content, onChange }: Props) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(content);

  const lines = content.split("\n").filter((l) => l.trim().length > 0);

  const save = async () => {
    onChange(draft);
    setEditing(false);
    await ipc.saveNotes(draft).catch((e) => toastError("save notes", e));
  };

  return (
    <div className="px-4 pt-4 pb-3 overflow-y-auto flex-1">
      <div className="flex items-center gap-2 py-1 mb-2">
        <span className="material-symbols-outlined text-[16px] shrink-0 text-on-surface-variant">edit_note</span>
        <span className="text-[14px] text-on-surface-variant flex-1">Notes</span>
        {!editing && (
          <button
            onClick={() => { setDraft(content); setEditing(true); }}
            className="p-0.5 text-on-surface-variant hover:text-on-surface transition-colors border-none bg-transparent"
          >
            <span className="material-symbols-outlined text-[14px]">edit_document</span>
          </button>
        )}
      </div>
      {editing ? (
        <div className="flex flex-col gap-2">
          <textarea
            autoFocus
            className="w-full bg-surface-container border border-outline-variant rounded text-[13px] text-on-surface px-2 py-1.5 resize-none focus:border-outline focus:outline-none"
            rows={8}
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
          />
          <div className="flex gap-2">
            <button
              onClick={save}
              className="text-[12px] bg-accent text-accent-fg border-none rounded px-3 py-1 hover:opacity-90"
            >
              Save
            </button>
            <button
              onClick={() => setEditing(false)}
              className="text-[12px] border border-outline-variant rounded px-3 py-1 text-on-surface-variant hover:text-on-surface hover:bg-surface-container-low transition-colors"
            >
              Cancel
            </button>
          </div>
        </div>
      ) : (
        <ul className="text-[14px] text-on-surface space-y-2 list-disc pl-4 marker:text-outline-variant">
          {lines.length === 0
            ? <li className="text-on-surface-variant italic list-none -ml-4">No notes yet. Click edit to add.</li>
            : lines.map((l, i) => <li key={i} className="pl-1">{l.replace(/^[-*]\s*/, "")}</li>)
          }
        </ul>
      )}
    </div>
  );
}
```

- [ ] Create `src/components/right/IdeasPane.tsx`:

```tsx
import { useState, useRef } from "react";
import { ipc } from "../../ipc";
import { toastError } from "../../state/toasts";
import type { IdeaItem } from "../../types";

interface Props {
  ideas: IdeaItem[];
  onChange: (ideas: IdeaItem[]) => void;
}

export function IdeasPane({ ideas, onChange }: Props) {
  const [adding, setAdding] = useState(false);
  const [newText, setNewText] = useState("");
  const dragSrc = useRef<string | null>(null);

  const visible = ideas.filter((i) => !i.trashed).sort((a, b) => a.order - b.order);

  const persist = async (next: IdeaItem[]) => {
    onChange(next);
    await ipc.saveIdeasJson(next).catch((e) => toastError("save ideas", e));
  };

  const trash = (id: string) => {
    persist(ideas.map((i) => i.id === id ? { ...i, trashed: true } : i));
  };

  const addIdea = async () => {
    const text = newText.trim();
    if (!text) return;
    const next: IdeaItem = {
      id: crypto.randomUUID(),
      text,
      trashed: false,
      order: ideas.filter((i) => !i.trashed).length,
    };
    await persist([...ideas, next]);
    setNewText("");
    setAdding(false);
  };

  const onDragStart = (id: string) => { dragSrc.current = id; };

  const onDrop = (targetId: string) => {
    if (!dragSrc.current || dragSrc.current === targetId) return;
    const srcIdx = visible.findIndex((i) => i.id === dragSrc.current);
    const tgtIdx = visible.findIndex((i) => i.id === targetId);
    const reordered = [...visible];
    const [moved] = reordered.splice(srcIdx, 1);
    reordered.splice(tgtIdx, 0, moved);
    const updated = ideas.map((idea) => {
      const newOrder = reordered.findIndex((r) => r.id === idea.id);
      return newOrder >= 0 ? { ...idea, order: newOrder } : idea;
    });
    persist(updated);
    dragSrc.current = null;
  };

  return (
    <div className="px-4 pt-4 pb-3 overflow-y-auto flex-1">
      <div className="flex items-center gap-2 py-1 mb-2">
        <span className="material-symbols-outlined text-[16px] shrink-0 text-on-surface-variant">lightbulb</span>
        <span className="text-[14px] text-on-surface-variant flex-1">Ideas</span>
        <button
          onClick={() => setAdding(true)}
          className="p-0.5 text-on-surface-variant hover:text-on-surface transition-colors border-none bg-transparent"
        >
          <span className="material-symbols-outlined text-[14px]">add</span>
        </button>
      </div>

      <div className="space-y-2">
        {adding && (
          <div className="flex flex-col gap-1">
            <textarea
              autoFocus
              className="w-full bg-surface-container border border-outline-variant rounded text-[13px] text-on-surface px-2 py-1.5 resize-none focus:border-outline focus:outline-none"
              rows={2}
              placeholder="New idea…"
              value={newText}
              onChange={(e) => setNewText(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && e.metaKey) addIdea();
                if (e.key === "Escape") { setAdding(false); setNewText(""); }
              }}
            />
            <div className="flex gap-1">
              <button onClick={addIdea} className="text-[11px] bg-accent text-accent-fg border-none rounded px-2 py-0.5 hover:opacity-90">Add</button>
              <button onClick={() => { setAdding(false); setNewText(""); }} className="text-[11px] border border-outline-variant rounded px-2 py-0.5 text-on-surface-variant hover:text-on-surface transition-colors">Cancel</button>
            </div>
          </div>
        )}
        {visible.map((idea) => (
          <div
            key={idea.id}
            draggable
            onDragStart={() => onDragStart(idea.id)}
            onDragOver={(e) => e.preventDefault()}
            onDrop={() => onDrop(idea.id)}
            className="group/idea bg-surface-container border border-outline-variant p-2 rounded text-[14px] text-on-surface cursor-grab hover:border-outline transition-colors flex items-start justify-between gap-2"
          >
            <span className="flex-1">{idea.text}</span>
            <button
              onClick={() => trash(idea.id)}
              className="opacity-0 group-hover/idea:opacity-100 transition-opacity p-0.5 text-on-surface-variant hover:text-error border-none bg-transparent shrink-0 mt-0.5"
              title="Remove idea"
            >
              <span className="material-symbols-outlined text-[14px]">delete</span>
            </button>
          </div>
        ))}
        {visible.length === 0 && !adding && (
          <p className="text-[12px] text-on-surface-variant">No ideas yet. Click + to add.</p>
        )}
      </div>
    </div>
  );
}
```

- [ ] Create `src/components/right/AddResourceSection.tsx`:

```tsx
import { useState } from "react";
import { ipc } from "../../ipc";
import { toastError } from "../../state/toasts";

export function AddResourceSection() {
  const [url, setUrl] = useState("");
  const [loading, setLoading] = useState(false);

  const handleSubmit = async () => {
    if (!url.trim() || loading) return;
    setLoading(true);
    try {
      await ipc.submitResource(url.trim());
      setUrl("");
    } catch (e) {
      toastError("submit resource", e);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="px-4 pt-4 pb-3 overflow-y-auto flex-1">
      <div className="flex items-center gap-2 py-1 mb-3">
        <span className="material-symbols-outlined text-[16px] shrink-0 text-on-surface-variant">add_link</span>
        <span className="text-[14px] text-on-surface-variant">Add resource</span>
      </div>
      <div className="flex gap-2">
        <input
          className="flex-1 bg-surface-container border border-outline-variant rounded text-[13px] text-on-surface px-2 py-1.5 focus:border-outline focus:outline-none placeholder-on-surface-variant/50 min-w-0"
          placeholder="https://arxiv.org/abs/…"
          type="url"
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleSubmit()}
          disabled={loading}
        />
        <button
          onClick={handleSubmit}
          disabled={!url.trim() || loading}
          className="border border-outline text-on-surface px-3 py-1.5 rounded text-[12px] hover:bg-surface-container-high transition-colors shrink-0 disabled:opacity-50"
        >
          {loading ? "…" : "Add"}
        </button>
      </div>
    </div>
  );
}
```

- [ ] Create `src/components/right/RightRail.tsx`:

```tsx
import type { IdeaItem } from "../../types";
import { NotesPane } from "./NotesPane";
import { IdeasPane } from "./IdeasPane";
import { AddResourceSection } from "./AddResourceSection";

interface Props {
  notes: string;
  onNotesChange: (c: string) => void;
  ideas: IdeaItem[];
  onIdeasChange: (ideas: IdeaItem[]) => void;
}

export function RightRail({ notes, onNotesChange, ideas, onIdeasChange }: Props) {
  return (
    <aside
      className="flex flex-col bg-surface-container-low border-l border-outline-variant shrink-0 overflow-hidden"
      style={{ width: "320px", minWidth: "180px", maxWidth: "440px" }}
    >
      {/* Notes */}
      <div className="flex flex-col overflow-hidden" style={{ minHeight: "80px", height: "calc((100vh - 64px) / 3)" }}>
        <NotesPane content={notes} onChange={onNotesChange} />
      </div>

      <div className="h-px bg-outline-variant shrink-0" />

      {/* Ideas */}
      <div className="flex flex-col overflow-hidden" style={{ minHeight: "80px", height: "calc((100vh - 64px) / 3)" }}>
        <IdeasPane ideas={ideas} onChange={onIdeasChange} />
      </div>

      <div className="h-px bg-outline-variant shrink-0" />

      {/* Add Resource */}
      <div className="flex flex-col overflow-hidden flex-1" style={{ minHeight: "72px" }}>
        <AddResourceSection />
      </div>
    </aside>
  );
}
```

- [ ] Commit:
```bash
git add src/components/right/
git commit -m "feat: add right rail components (Notes, Ideas, AddResource)"
```

---

## Task 10: StatusBar component and useSystemStatus hook

**Files:**
- Create: `src/hooks/useSystemStatus.ts`
- Create: `src/components/StatusBar.tsx`

- [ ] Create `src/hooks/useSystemStatus.ts`:

```ts
import { useEffect, useState } from "react";
import { ipc } from "../ipc";
import type { SystemStatus } from "../types";

const EMPTY: SystemStatus = {
  workspace_path: "",
  git_branch: "",
  git_insertions: 0,
  git_deletions: 0,
  cpu_model: "",
  cpu_usage_pct: 0,
  gpu_model: null,
  gpu_usage_pct: null,
  gpu_vram_gb: null,
  ram_total_gb: 0,
  hostname: "",
  username: "",
  cc_connected: false,
};

export function useSystemStatus(): SystemStatus {
  const [status, setStatus] = useState<SystemStatus>(EMPTY);

  useEffect(() => {
    const fetch = () => {
      ipc.getSystemStatus().then(setStatus).catch(() => {});
    };
    fetch();
    const id = setInterval(fetch, 5000);
    return () => clearInterval(id);
  }, []);

  return status;
}
```

- [ ] Create `src/components/StatusBar.tsx`:

```tsx
import { useSystemStatus } from "../hooks/useSystemStatus";

export function StatusBar() {
  const s = useSystemStatus();

  return (
    <footer className="h-6 flex items-center px-3 bg-surface-container-lowest border-t border-outline-variant text-on-surface-variant font-mono text-[10px] shrink-0 overflow-hidden select-none">
      <div className="flex items-center gap-0 w-full overflow-x-auto no-scrollbar">

        {/* Git */}
        <div className="flex items-center gap-1.5 px-2 border-r border-outline-variant/40 shrink-0 h-6">
          <span className="text-on-surface-variant/70">{s.workspace_path}</span>
          {s.git_branch && (
            <>
              <span className="text-outline-variant">·</span>
              <span className="text-primary">{s.git_branch}</span>
            </>
          )}
          {s.git_insertions > 0 && <span className="text-ok">+{s.git_insertions}</span>}
          {s.git_deletions > 0 && <span className="text-error">-{s.git_deletions}</span>}
        </div>

        {/* CPU */}
        {s.cpu_model && (
          <div className="flex items-center gap-1.5 px-2 border-r border-outline-variant/40 shrink-0 h-6">
            <span className="material-symbols-outlined text-[11px]">memory</span>
            <span>{s.cpu_model}</span>
            <span className="text-outline-variant">·</span>
            <span className={s.cpu_usage_pct > 80 ? "text-warn" : "text-ok"}>
              {Math.round(s.cpu_usage_pct)}%
            </span>
          </div>
        )}

        {/* GPU */}
        {s.gpu_model && (
          <div className="flex items-center gap-1.5 px-2 border-r border-outline-variant/40 shrink-0 h-6">
            <span className="material-symbols-outlined text-[11px]">developer_board</span>
            <span>{s.gpu_model}</span>
            {s.gpu_usage_pct !== null && (
              <>
                <span className="text-outline-variant">·</span>
                <span className={s.gpu_usage_pct > 80 ? "text-warn" : "text-ok"}>
                  {Math.round(s.gpu_usage_pct)}%
                </span>
              </>
            )}
            {s.gpu_vram_gb !== null && (
              <>
                <span className="text-outline-variant">·</span>
                <span>{s.gpu_vram_gb} GB VRAM</span>
              </>
            )}
          </div>
        )}

        {/* RAM */}
        {s.ram_total_gb > 0 && (
          <div className="flex items-center gap-1.5 px-2 border-r border-outline-variant/40 shrink-0 h-6">
            <span className="material-symbols-outlined text-[11px]">storage</span>
            <span>{s.ram_total_gb} GB RAM</span>
          </div>
        )}

        {/* Hostname */}
        {s.hostname && (
          <div className="flex items-center gap-1.5 px-2 border-r border-outline-variant/40 shrink-0 h-6">
            <span>{s.username}@{s.hostname}</span>
          </div>
        )}

        {/* CC connection */}
        <div className="flex items-center gap-1.5 px-2 shrink-0 h-6 ml-auto">
          <span>claude-code</span>
          <span className="text-outline-variant">·</span>
          <span className={s.cc_connected ? "text-ok" : "text-on-surface-variant"}>
            {s.cc_connected ? "connected" : "disconnected"}
          </span>
          <span className={`w-1.5 h-1.5 rounded-full ml-0.5 ${s.cc_connected ? "bg-ok" : "bg-on-surface-variant"}`} />
        </div>

      </div>
    </footer>
  );
}
```

- [ ] Commit:
```bash
git add src/hooks/ src/components/StatusBar.tsx
git commit -m "feat: add StatusBar with live system metrics"
```

---

## Task 11: Chat components — TabBar, Composer, MessageList restyled

**Files:**
- Modify: `src/components/chat/TabBar.tsx`
- Modify: `src/components/chat/Composer.tsx`
- Modify: `src/components/chat/MessageList.tsx`
- Modify: `src/components/chat/ExperimentCard.tsx`

- [ ] Rewrite `src/components/chat/TabBar.tsx`:

```tsx
import type { Tab } from "../../types";

interface Props {
  tabs: Tab[];
  activeTabId: string;
  onSelect: (tabId: string) => void;
  onClose: (tabId: string) => void;
  onNew: () => void;
}

function tabIcon(tab: Tab): string {
  if (tab.kind === "experiment") return "science";
  if (tab.kind === "resource" || tab.kind === "preview") return "description";
  return "chat";
}

export function TabBar({ tabs, activeTabId, onSelect, onClose, onNew }: Props) {
  return (
    <div className="flex h-8 border-b border-outline-variant bg-surface-container-low shrink-0 px-2 overflow-x-auto no-scrollbar">
      {tabs.map((tab) => {
        const active = tab.id === activeTabId;
        return (
          <div
            key={tab.id}
            onClick={() => onSelect(tab.id)}
            className={[
              "flex items-center px-3 border-r border-outline-variant text-xs min-w-max cursor-pointer",
              active
                ? "bg-surface-container-highest text-on-surface border-t border-t-primary"
                : "text-on-surface-variant hover:bg-surface-container-highest hover:text-on-surface transition-colors",
            ].join(" ")}
          >
            {tab.kind === "resource" && tab.resourceStatus === "summarizing" ? (
              <span className="material-symbols-outlined text-[14px] mr-1.5 animate-spin">progress_activity</span>
            ) : (
              <span className="material-symbols-outlined text-[14px] mr-1.5">{tabIcon(tab)}</span>
            )}
            <span>{tab.label}</span>
            {!tab.isPinned && (
              <button
                className="ml-1.5 text-on-surface-variant hover:text-on-surface border-none bg-transparent p-0 text-[12px] leading-none"
                onClick={(e) => { e.stopPropagation(); onClose(tab.id); }}
                aria-label={`Close ${tab.label}`}
              >
                ×
              </button>
            )}
          </div>
        );
      })}
      <div
        onClick={onNew}
        className="flex items-center justify-center w-8 text-on-surface-variant hover:bg-surface-container-highest hover:text-on-surface transition-colors cursor-pointer"
      >
        <span className="material-symbols-outlined text-[16px]">add</span>
      </div>
    </div>
  );
}
```

- [ ] Rewrite `src/components/chat/Composer.tsx`:

```tsx
import { useEffect, useRef, useState } from "react";
import { useChatOptions, MODELS, EFFORT_LEVELS } from "../../state/chatOptions";

interface Props {
  onSend?: (text: string) => void;
  disabled?: boolean;
}

export function Composer({ onSend, disabled }: Props) {
  const [text, setText] = useState("");
  const [modelOpen, setModelOpen] = useState(false);
  const [effortOpen, setEffortOpen] = useState(false);
  const modelRef = useRef<HTMLDivElement>(null);
  const effortRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const { model, effort, setModel, setEffort } = useChatOptions();
  const modelLabel = MODELS.find((m) => m.id === model)?.label ?? model;
  const effortLabel = EFFORT_LEVELS.find((l) => l.value === effort)?.label ?? effort;

  useEffect(() => {
    if (!modelOpen) return;
    const h = (e: MouseEvent) => { if (modelRef.current && !modelRef.current.contains(e.target as Node)) setModelOpen(false); };
    document.addEventListener("mousedown", h);
    return () => document.removeEventListener("mousedown", h);
  }, [modelOpen]);

  useEffect(() => {
    if (!effortOpen) return;
    const h = (e: MouseEvent) => { if (effortRef.current && !effortRef.current.contains(e.target as Node)) setEffortOpen(false); };
    document.addEventListener("mousedown", h);
    return () => document.removeEventListener("mousedown", h);
  }, [effortOpen]);

  const send = () => {
    const t = text.trim();
    if (!t || disabled) return;
    onSend?.(t);
    setText("");
    if (textareaRef.current) textareaRef.current.style.height = "auto";
  };

  return (
    <div className="absolute bottom-6 left-1/2 -translate-x-1/2 w-full px-6 pointer-events-none z-20">
      <div className="pointer-events-auto bg-surface-container border border-outline-variant rounded-xl shadow-lg shadow-black/30 flex flex-col overflow-visible">
        <textarea
          ref={textareaRef}
          id="composer-textarea"
          className="w-full bg-transparent border-none text-on-surface text-[14px] focus:ring-0 px-3 py-2.5 placeholder-on-surface-variant/50 outline-none resize-none"
          style={{ minHeight: "52px", maxHeight: "240px", overflowY: "auto" }}
          placeholder="Continue the experiment…"
          value={text}
          disabled={disabled}
          onChange={(e) => {
            setText(e.target.value);
            e.target.style.height = "auto";
            e.target.style.height = Math.min(e.target.scrollHeight, 240) + "px";
          }}
          onKeyDown={(e) => { if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) { e.preventDefault(); send(); } }}
        />
        <div className="flex items-center justify-between px-2 pb-2 pt-0">
          <div className="flex items-center gap-1">
            {/* Model picker */}
            <div className="relative" ref={modelRef}>
              <button
                onClick={() => setModelOpen((o) => !o)}
                className="flex items-center gap-1 px-2 py-1 text-on-surface-variant hover:bg-surface-container-high rounded hover:text-on-surface transition-colors text-[11px] border border-outline-variant/50 bg-transparent"
              >
                <span className="text-[10px] text-on-surface-variant/60 mr-0.5">model</span>
                {modelLabel}
                <span className="material-symbols-outlined text-[12px]">expand_more</span>
              </button>
              {modelOpen && (
                <div className="absolute bottom-full left-0 mb-1 bg-surface-container-high border border-outline-variant rounded shadow-lg shadow-black/30 py-1 min-w-[140px] z-50">
                  {MODELS.map((m) => (
                    <button
                      key={m.id}
                      onClick={() => { setModel(m.id); setModelOpen(false); }}
                      className={`w-full text-left px-3 py-1.5 text-[12px] hover:bg-surface-container-highest transition-colors border-none bg-transparent ${m.id === model ? "text-on-surface font-medium" : "text-on-surface-variant"}`}
                    >
                      {m.label}
                    </button>
                  ))}
                </div>
              )}
            </div>
            {/* Effort picker */}
            <div className="relative" ref={effortRef}>
              <button
                onClick={() => setEffortOpen((o) => !o)}
                className="flex items-center gap-1 px-2 py-1 text-on-surface-variant hover:bg-surface-container-high rounded hover:text-on-surface transition-colors text-[11px] border border-outline-variant/50 bg-transparent"
              >
                <span className="text-[10px] text-on-surface-variant/60 mr-0.5">effort</span>
                {effortLabel}
                <span className="material-symbols-outlined text-[12px]">expand_more</span>
              </button>
              {effortOpen && (
                <div className="absolute bottom-full left-0 mb-1 bg-surface-container-high border border-outline-variant rounded shadow-lg shadow-black/30 py-1 min-w-[100px] z-50">
                  {EFFORT_LEVELS.map((lvl) => (
                    <button
                      key={lvl.value}
                      onClick={() => { setEffort(lvl.value); setEffortOpen(false); }}
                      className={`w-full text-left px-3 py-1.5 text-[12px] hover:bg-surface-container-highest transition-colors border-none bg-transparent ${lvl.value === effort ? "text-on-surface font-medium" : "text-on-surface-variant"}`}
                    >
                      {lvl.label}
                    </button>
                  ))}
                </div>
              )}
            </div>
            {/* Slash */}
            <button className="p-1.5 text-on-surface-variant hover:bg-surface-container-high rounded hover:text-on-surface transition-colors font-mono text-[11px] border-none bg-transparent">
              /
            </button>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-[10px] text-on-surface-variant/50">⌘↵</span>
            <button
              onClick={send}
              disabled={!text.trim() || disabled}
              className="bg-accent text-accent-fg px-4 py-1 rounded text-[12px] font-medium hover:opacity-90 transition-opacity flex items-center gap-1 border-none disabled:opacity-40"
            >
              Send <span className="material-symbols-outlined text-[14px]">arrow_upward</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
```

- [ ] Rewrite `src/components/chat/MessageList.tsx` — restyle messages, keep all logic:

Replace the outer container and message classes with Tailwind. Key changes:
- Wrapper: `className="absolute inset-0 overflow-y-auto px-4 md:px-8 lg:px-12 pt-4 pb-40 space-y-6"`
- Empty state: `className="flex items-center justify-center h-full text-[14px] text-on-surface-variant"`
- User bubble: `className="flex justify-end"` wrapping `className="bg-surface-container text-on-surface px-4 py-3 rounded border border-outline-variant max-w-[560px] text-[14px] leading-relaxed"`
- Assistant block: `className="flex flex-col items-start max-w-[720px] space-y-4"`
- Thinking block: left vertical bar `className="flex gap-3 text-on-surface-variant text-[13px] w-full"` with `className="w-px bg-outline-variant shrink-0 my-1"` bar and italic content
- Tool card done: `className="w-full bg-surface-container-low border border-outline-variant rounded px-3 py-2 flex items-center gap-3 text-xs cursor-pointer hover:bg-surface-container transition-colors"` with check_circle icon in `text-ok`
- Tool card running: border-warn header, spinning icon
- Text content: `className="text-on-surface text-[14px] leading-relaxed"`

Full rewrite:

```tsx
import { useEffect, useRef, useState } from "react";
import type { AssistantMessage, ChatMessage, ToolCallState } from "../../types";
import { ExperimentCard } from "./ExperimentCard";
import { MessageMarkdown } from "./MessageMarkdown";

function bareToolName(name: string): string {
  const parts = name.split("__");
  return parts[parts.length - 1].replace(/_/g, ".");
}

function isExperimentStart(toolName: string): boolean {
  return bareToolName(toolName) === "experiment.start";
}

export function MessageList({ messages }: { messages: ChatMessage[] }) {
  const bottomRef = useRef<HTMLDivElement>(null);
  useEffect(() => { bottomRef.current?.scrollIntoView({ behavior: "smooth" }); }, [messages.length]);

  if (messages.length === 0) {
    return (
      <div className="absolute inset-0 flex items-center justify-center text-[14px] text-on-surface-variant">
        Start a conversation. Brainstorm ideas or kick off an experiment.
      </div>
    );
  }

  return (
    <div className="absolute inset-0 overflow-y-auto px-4 md:px-8 lg:px-12 pt-4 pb-40 space-y-6">
      {messages.map((m) =>
        m.role === "user" ? (
          <div key={m.id} className="flex justify-end">
            <div className="bg-surface-container text-on-surface px-4 py-3 rounded border border-outline-variant max-w-[560px] text-[14px] leading-relaxed">
              <MessageMarkdown content={m.text} />
            </div>
          </div>
        ) : (
          <AssistantBubble key={m.id} msg={m as AssistantMessage} />
        )
      )}
      <div ref={bottomRef} />
    </div>
  );
}

function AssistantBubble({ msg }: { msg: AssistantMessage }) {
  const [thinkingOpen, setThinkingOpen] = useState(false);
  const thinkingRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (msg.isStreaming && thinkingOpen && thinkingRef.current) {
      thinkingRef.current.scrollTop = thinkingRef.current.scrollHeight;
    }
  }, [msg.thinking, msg.isStreaming, thinkingOpen]);

  return (
    <div className="flex flex-col items-start max-w-[720px] space-y-4">
      {msg.thinking && (
        <div className="flex gap-3 text-on-surface-variant text-[13px] w-full">
          <div className="w-px bg-outline-variant shrink-0 my-1" />
          <div className="py-1 opacity-80 text-xs flex-1">
            <button
              className="italic mb-1 text-on-surface-variant hover:text-on-surface border-none bg-transparent text-xs cursor-pointer p-0"
              onClick={() => setThinkingOpen((v) => !v)}
            >
              {thinkingOpen ? "▾" : "▸"} Thinking…
            </button>
            {thinkingOpen && (
              <div ref={thinkingRef} className="mt-1 max-h-40 overflow-y-auto whitespace-pre-wrap font-mono text-[11px]">
                {msg.thinking}
              </div>
            )}
          </div>
        </div>
      )}

      {msg.tools && msg.tools.length > 0 && (
        <div className="w-full space-y-2">
          {msg.tools.map((tool) =>
            isExperimentStart(tool.tool_name) ? (
              <ExperimentCard key={tool.tool_id} tool={tool} />
            ) : (
              <ToolCard key={tool.tool_id} tool={tool} />
            )
          )}
        </div>
      )}

      {msg.error ? (
        <div className="text-[14px] text-error border border-error/30 rounded px-3 py-2 bg-error/5">
          {msg.error}
        </div>
      ) : msg.text ? (
        <div className="text-on-surface text-[14px] leading-relaxed">
          <MessageMarkdown content={msg.text} />
        </div>
      ) : msg.isStreaming ? (
        <div className="text-on-surface-variant text-[14px] animate-pulse">▌</div>
      ) : null}
    </div>
  );
}

function ToolCard({ tool }: { tool: ToolCallState }) {
  const [expanded, setExpanded] = useState(false);
  const canExpand = !!(tool.input_full || tool.output_full);
  const isDone = tool.isDone;

  if (!isDone) {
    return (
      <div className="w-full bg-surface-container border border-warn/40 rounded flex flex-col overflow-hidden">
        <div className="px-3 py-2 flex items-center gap-3 text-xs">
          <span className="material-symbols-outlined text-warn text-[16px] animate-spin">progress_activity</span>
          <span className="font-mono text-warn flex-1 truncate">{tool.tool_name}{tool.input_preview ? `(${tool.input_preview})` : ""}</span>
        </div>
      </div>
    );
  }

  return (
    <div>
      <div
        onClick={() => canExpand && setExpanded((v) => !v)}
        className={`w-full bg-surface-container-low border border-outline-variant rounded px-3 py-2 flex items-center gap-3 text-xs ${canExpand ? "cursor-pointer hover:bg-surface-container transition-colors" : ""}`}
      >
        <span className="material-symbols-outlined text-ok text-[16px]">check_circle</span>
        <span className="font-mono text-on-surface-variant flex-1 truncate">
          {tool.tool_name}{tool.input_preview ? `(${tool.input_preview})` : ""}
          {tool.output_preview ? ` → ${tool.output_preview}` : ""}
        </span>
        {canExpand && <span className="text-on-surface-variant opacity-50">{expanded ? "▾" : "▸"}</span>}
      </div>
      {expanded && (
        <div className="mt-1 border border-outline-variant rounded bg-surface-container-lowest p-3 font-mono text-[11px] text-on-surface-variant max-h-48 overflow-y-auto">
          {tool.input_full && <><div className="text-on-surface-variant/60 mb-1 uppercase text-[9px] tracking-widest">Input</div><pre className="whitespace-pre-wrap mb-3">{tool.input_full}</pre></>}
          {tool.output_full && <><div className="text-on-surface-variant/60 mb-1 uppercase text-[9px] tracking-widest">Output</div><pre className="whitespace-pre-wrap">{tool.output_full}</pre></>}
        </div>
      )}
    </div>
  );
}
```

- [ ] Restyle `src/components/chat/ExperimentCard.tsx` — keep all logic, update classes:

Replace classNames throughout — the card structure stays the same but uses Tailwind:
- Card wrapper: `className="w-full border border-outline-variant rounded overflow-hidden"`
- Header: `className={`flex items-center gap-3 px-3 py-2 text-xs cursor-pointer ${isLive ? "bg-surface-container border-b border-warn/20" : "bg-surface-container-low"}`}`
- Status dot: `className={`w-2 h-2 rounded-full shrink-0 ${isLive ? "bg-warn animate-pulse" : status === "completed" ? "bg-ok" : "bg-error"}`}`
- Label: `className="font-mono text-[13px] text-on-surface flex-1 truncate"`
- Status pill: reuse the same pill pattern as ExperimentsSection
- Log body: `className="exp-log p-3 bg-surface-container-lowest text-on-surface-variant h-32 overflow-y-auto font-mono text-[11px] leading-relaxed"`
- Cancel button: `className="ml-auto text-[10px] border border-warn/30 text-warn px-2 py-0.5 rounded bg-warn/10 hover:bg-warn/20 transition-colors"`

Full rewrite of ExperimentCard.tsx:
```tsx
import { useState } from "react";
import { ipc } from "../../ipc";
import { toastError } from "../../state/toasts";
import type { ToolCallState } from "../../types";

export function ExperimentCard({ tool }: { tool: ToolCallState }) {
  const [expanded, setExpanded] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const status = tool.experimentStatus ?? "starting";
  const lines = tool.logLines ?? [];
  const isLive = status === "starting" || status === "running";

  const handleCancel = async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!tool.experimentUuid) return;
    setCancelling(true);
    try { await ipc.experimentCancel(tool.experimentUuid); }
    catch (err) { toastError("cancel experiment", err); }
    finally { setCancelling(false); }
  };

  const dotCls = isLive ? "bg-warn animate-pulse" : status === "completed" ? "bg-ok" : "bg-error";
  const pillText = status === "starting" ? "Starting…" : status === "running" ? "Running" : status === "completed" ? "Done" : status === "failed" ? "Failed" : "Cancelled";
  const pillCls = isLive ? "text-warn border-warn/30 bg-warn/10" : status === "completed" ? "text-ok border-ok/30 bg-ok/10" : "text-error border-error/30 bg-error/10";

  return (
    <div className="w-full border border-outline-variant rounded overflow-hidden">
      <div
        className={`flex items-center gap-3 px-3 py-2 text-xs cursor-pointer select-none ${isLive ? "bg-surface-container border-b border-warn/20" : "bg-surface-container-low"}`}
        onClick={() => setExpanded((v) => !v)}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => e.key === "Enter" && setExpanded((v) => !v)}
      >
        <span className={`w-2 h-2 rounded-full shrink-0 ${dotCls}`} />
        <span className="font-mono text-[13px] text-on-surface flex-1 truncate">
          {tool.tool_name}
        </span>
        <span className={`text-[10px] uppercase border px-1 rounded shrink-0 ${pillCls}`}>
          {pillText}
        </span>
        {tool.experimentPid !== undefined && (
          <span className="text-on-surface-variant/50">PID {tool.experimentPid}</span>
        )}
        <span className="text-on-surface-variant opacity-50">{expanded ? "▾" : "▸"}</span>
        {isLive && tool.experimentUuid && (
          <button
            disabled={cancelling}
            onClick={handleCancel}
            className="text-[10px] border border-warn/30 text-warn px-2 py-0.5 rounded bg-warn/10 hover:bg-warn/20 transition-colors disabled:opacity-50"
          >
            {cancelling ? "Cancelling…" : "Cancel"}
          </button>
        )}
      </div>
      {expanded && (
        <div className="p-3 bg-surface-container-lowest font-mono text-[11px] text-on-surface-variant max-h-32 overflow-y-auto leading-relaxed">
          {lines.length > 0
            ? lines.slice(-10).map((line, i) => <div key={i}>{line}</div>)
            : <span className="opacity-60">No output yet.</span>
          }
        </div>
      )}
    </div>
  );
}
```

- [ ] Build check:
```bash
npm run build 2>&1 | grep -E "error TS" | head -20
```

- [ ] Commit:
```bash
git add src/components/chat/
git commit -m "feat: restyle chat components (TabBar, Composer, MessageList, ExperimentCard)"
```

---

## Task 12: ExperimentTabView component

**Files:**
- Create: `src/components/chat/ExperimentTabView.tsx`

- [ ] Create `src/components/chat/ExperimentTabView.tsx`:

```tsx
import { useEffect, useRef, useState } from "react";
import { ipc, onExperimentLogLine, onExperimentStatus } from "../../ipc";
import { toastError } from "../../state/toasts";
import type { ExperimentRow } from "../../types";

interface Props {
  uuid: string;
}

function elapsed(startedAt: string): string {
  const ms = Date.now() - new Date(startedAt).getTime();
  const s = Math.floor(ms / 1000);
  const m = Math.floor(s / 60);
  const h = Math.floor(m / 60);
  if (h > 0) return `${h}h ${m % 60}m`;
  if (m > 0) return `${m}m ${s % 60}s`;
  return `${s}s`;
}

export function ExperimentTabView({ uuid }: Props) {
  const [exp, setExp] = useState<ExperimentRow | null>(null);
  const [logs, setLogs] = useState<string[]>([]);
  const [elapsedStr, setElapsedStr] = useState("");
  const logRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // Load experiment metadata + initial logs
    ipc.experimentList(100)
      .then((rows) => {
        const found = rows.find((r) => r.uuid === uuid);
        if (found) setExp(found);
      })
      .catch((e) => toastError("load experiment", e));

    ipc.experimentLogs(uuid, 64)
      .then(({ stdout, stderr }) => {
        const lines: string[] = [];
        if (stdout) lines.push(...stdout.split("\n").filter(Boolean));
        if (stderr) lines.push(...stderr.split("\n").filter(Boolean));
        setLogs(lines);
      })
      .catch(() => {});
  }, [uuid]);

  // Live log streaming
  useEffect(() => {
    let cancelled = false;
    const unlisteners: (() => void)[] = [];

    onExperimentLogLine(({ uuid: u, line }) => {
      if (u !== uuid) return;
      setLogs((prev) => [...prev, line].slice(-200));
      setTimeout(() => {
        if (logRef.current) logRef.current.scrollTop = logRef.current.scrollHeight;
      }, 0);
    }).then((u) => { if (cancelled) u(); else unlisteners.push(u); });

    onExperimentStatus(({ uuid: u, status, exit_code }) => {
      if (u !== uuid) return;
      setExp((prev) => prev ? { ...prev, status, exit_code: exit_code ?? null } : prev);
    }).then((u) => { if (cancelled) u(); else unlisteners.push(u); });

    return () => { cancelled = true; unlisteners.forEach((u) => u()); };
  }, [uuid]);

  // Elapsed timer
  useEffect(() => {
    if (!exp?.started_at) return;
    if (exp.status !== "running" && exp.status !== "starting") return;
    const id = setInterval(() => setElapsedStr(elapsed(exp.started_at)), 1000);
    return () => clearInterval(id);
  }, [exp?.started_at, exp?.status]);

  if (!exp) {
    return <div className="absolute inset-0 flex items-center justify-center text-[14px] text-on-surface-variant">Loading…</div>;
  }

  const isLive = exp.status === "running" || exp.status === "starting";
  const pillCls = isLive ? "text-warn border-warn/30 bg-warn/10" : exp.status === "completed" ? "text-ok border-ok/30 bg-ok/10" : "text-error border-error/30 bg-error/10";
  const pillText = isLive ? "Running" : exp.status === "completed" ? "Done" : exp.status === "failed" ? "Failed" : "Cancelled";

  return (
    <div className="absolute inset-0 overflow-y-auto px-4 py-4 pb-8">
      {/* Header */}
      <div className="flex items-center gap-3 mb-4">
        <span className="font-mono font-semibold text-[14px] text-on-surface">{exp.name}</span>
        <span className={`text-[10px] uppercase border px-1.5 py-0.5 rounded ${pillCls}`}>{pillText}</span>
      </div>

      {/* Metadata grid */}
      <div className="grid grid-cols-2 gap-x-6 gap-y-3 mb-5 border border-outline-variant rounded p-3 bg-surface-container-low text-[12px]">
        <div>
          <span className="text-on-surface-variant block mb-0.5">Status</span>
          <span className={isLive ? "text-warn font-medium" : "text-on-surface font-medium"}>
            {pillText}{isLive && elapsedStr ? ` · ${elapsedStr}` : ""}
          </span>
        </div>
        <div>
          <span className="text-on-surface-variant block mb-0.5">Started</span>
          <span className="text-on-surface font-mono">{new Date(exp.started_at).toLocaleTimeString()}</span>
        </div>
        <div className="col-span-2">
          <span className="text-on-surface-variant block mb-0.5">Command</span>
          <code className="font-mono text-[11px] text-on-surface bg-surface-container px-2 py-1 rounded block truncate">
            {exp.command}
          </code>
        </div>
      </div>

      {/* Logs */}
      <div className="border border-outline-variant rounded overflow-hidden">
        <div className="flex items-center justify-between px-3 py-1.5 bg-surface-container-low border-b border-outline-variant">
          <span className="text-[10px] uppercase tracking-widest text-on-surface-variant">Logs</span>
          <span className="text-[10px] text-on-surface-variant font-mono">{isLive ? "live" : "ended"}</span>
        </div>
        <div
          ref={logRef}
          className="p-3 bg-surface-container-lowest font-mono text-[11px] text-on-surface-variant h-48 overflow-y-auto leading-relaxed"
        >
          {logs.length === 0
            ? <span className="opacity-60">No output yet.</span>
            : logs.map((line, i) => <div key={i}>{line}</div>)
          }
          {isLive && <span className="opacity-60 animate-pulse">▌</span>}
        </div>
      </div>
    </div>
  );
}
```

- [ ] Commit:
```bash
git add src/components/chat/ExperimentTabView.tsx
git commit -m "feat: add ExperimentTabView component"
```

---

## Task 13: Update ChatPane to handle experiment tabs and remove mode switcher

**Files:**
- Modify: `src/components/chat/ChatPane.tsx`

- [ ] In `ChatPane.tsx`, add import for `ExperimentTabView`:
```tsx
import { ExperimentTabView } from "./ExperimentTabView";
```

- [ ] Route experiment log lines to experiment tabs. In the `onExperimentLogLine` handler inside `useEffect`, after `appendExperimentLog(uuid, line)`, add:
```tsx
// Also update any open experiment tab for this uuid
const expTab = useChat.getState().tabs.find((t) => t.kind === "experiment" && t.experimentUuid === uuid);
// (no action needed — ExperimentTabView listens directly via its own effect)
```

- [ ] In the JSX, replace the content-rendering block. The current structure is `activeTab.kind === "preview" ? <MarkdownPane ... /> : <> ... </>`. Change to:

```tsx
{activeTab.kind === "preview" ? (
  <MarkdownPane ... />  // keep as-is
) : activeTab.kind === "experiment" ? (
  <div className="relative flex-1 overflow-hidden">
    <ExperimentTabView uuid={activeTab.experimentUuid!} />
  </div>
) : (
  <>
    {/* existing chat header strip */}
    <header className="flex items-center justify-end px-4 h-8 shrink-0 border-b border-outline-variant/30">
      <button
        className="text-on-surface-variant hover:text-on-surface transition-colors p-1 border-none bg-transparent"
        title="Reset conversation"
        onClick={() => { clearMessages(activeTabId); ipc.chatResetSession(activeTabId); }}
        disabled={activeTab.isStreaming}
      >
        <span className="material-symbols-outlined text-[16px]">refresh</span>
      </button>
    </header>
    <div className="relative flex-1 overflow-hidden">
      <MessageList messages={activeTab.messages} />
    </div>
    {showResourceBar && (
      <div className="flex gap-2 px-4 py-2 border-t border-outline-variant shrink-0">
        <button onClick={handleConfirmResource} className="text-[12px] bg-accent text-accent-fg border-none rounded px-3 py-1.5 hover:opacity-90">
          Confirm — save to wiki
        </button>
        <button onClick={handleDiscardResource} className="text-[12px] border border-outline-variant rounded px-3 py-1.5 text-on-surface-variant hover:text-on-surface transition-colors">
          Discard
        </button>
      </div>
    )}
    {!showResourceBar && (
      <div className="relative">
        <Composer onSend={handleSend} disabled={activeTab.isStreaming} />
      </div>
    )}
  </>
)}
```

- [ ] Remove the `isMainTab` variable and the old mode switcher block from the header (the `{isMainTab && <div className="chat-pane__mode">...}` JSX block).

- [ ] Remove `MarkdownPane` usage: the preview tab still needs it — keep that branch. But remove the old CSS-class-based wrapper `<section className="chat-pane">` and replace with:
```tsx
<main className="flex-1 flex flex-col min-w-0 bg-background relative overflow-hidden">
```

- [ ] Build check:
```bash
npm run build 2>&1 | grep -E "error TS" | head -20
```

- [ ] Commit:
```bash
git add src/components/chat/ChatPane.tsx
git commit -m "feat: update ChatPane for experiment tabs, remove mode switcher UI"
```

---

## Task 14: Restyle resource preview (MarkdownPane replacement in preview tabs)

**Files:**
- Modify: `src/components/chat/ChatPane.tsx` (preview branch)

The `MarkdownPane` component still handles resource preview. Its CSS classes no longer exist. Replace the preview tab render in `ChatPane.tsx` with an inline styled version matching `view-resource` from the HTML prototype:

- [ ] In `ChatPane.tsx`, replace the `activeTab.kind === "preview"` branch:

```tsx
) : activeTab.kind === "preview" ? (
  <div className="absolute inset-0 overflow-y-auto px-4 md:px-8 lg:px-12 pt-6 pb-8">
    <p className="text-[11px] uppercase tracking-widest text-on-surface-variant mb-4">
      Resource · Preview
    </p>
    <h2 className="text-base font-semibold text-on-surface mb-2">{activeTab.label}</h2>
    <div className="text-[14px] text-on-surface leading-relaxed prose prose-invert max-w-none">
      <ReactMarkdown remarkPlugins={[remarkGfm]}>{previewContent}</ReactMarkdown>
    </div>
  </div>
```

Add imports:
```tsx
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
```

- [ ] Build check:
```bash
npm run build 2>&1 | grep -E "error TS" | head -20
```

- [ ] Commit:
```bash
git add src/components/chat/ChatPane.tsx
git commit -m "feat: restyle resource preview tab"
```

---

## Task 15: Rewrite Layout.tsx — wire everything together

**Files:**
- Modify: `src/components/Layout.tsx`
- Delete: `src/components/MarkdownPane.tsx`
- Delete: `src/components/ResourceInput.tsx`
- Delete: `src/components/ResourcesList.tsx`

- [ ] Delete the old components no longer used at the layout level:
```bash
rm src/components/MarkdownPane.tsx src/components/ResourceInput.tsx src/components/ResourcesList.tsx
```

- [ ] Rewrite `src/components/Layout.tsx`:

```tsx
import { useEffect, useRef, useState } from "react";
import { ipc, onWikiChanged } from "../ipc";
import { useChat } from "../state/chat";
import { useWorkspace } from "../state/workspace";
import { useChatOptions } from "../state/chatOptions";
import { toastError } from "../state/toasts";
import type { IdeaItem, PulseContent, ResourceItem } from "../types";
import { ChatPane } from "./chat/ChatPane";
import { LeftRail } from "./left/LeftRail";
import { RightRail } from "./right/RightRail";
import { StatusBar } from "./StatusBar";

export function Layout() {
  const openPreviewTab = useChat((s) => s.openPreviewTab);
  const phase = useWorkspace((s) => s.phase);
  const setPhase = useWorkspace((s) => s.setPhase);
  const panelLayout = useWorkspace((s) => s.panelLayout);
  const toPersisted = useWorkspace((s) => s.toPersisted);
  const recentWorkspaces = useWorkspace((s) => s.recentWorkspaces);
  const effort = useChatOptions((s) => s.effort);
  const tabs = useChat((s) => s.tabs);
  const workspace = phase.kind === "ready" ? phase.workspace : null;

  const [pulse, setPulse] = useState<PulseContent>({ research_question: "", this_week: "" });
  const [notes, setNotes] = useState("");
  const [ideas, setIdeas] = useState<IdeaItem[]>([]);
  const [resources, setResources] = useState<ResourceItem[]>([]);

  // Count experiments with running/starting status from all tool cards across all tabs
  const runningExperimentsCount = (() => {
    let count = 0;
    for (const tab of tabs) {
      for (const msg of tab.messages) {
        if (msg.role === "assistant") {
          for (const tool of (msg as any).tools ?? []) {
            if ((tool.experimentStatus === "running" || tool.experimentStatus === "starting") && tool.experimentUuid) {
              count++;
            }
          }
        }
      }
    }
    return count;
  })();

  useEffect(() => {
    if (phase.kind !== "ready") return;
    Promise.all([
      ipc.readPulse(),
      ipc.readWikiFile("notes.md"),
      ipc.readIdeas(),
      ipc.listResources(),
    ])
      .then(([p, notesFile, ideasData, resourcesData]) => {
        setPulse(p);
        setNotes(notesFile.content);
        setIdeas(ideasData);
        setResources(resourcesData);
      })
      .catch((e) => toastError("load wiki", e));
  }, [phase.kind]);

  useEffect(() => {
    const unlisten = onWikiChanged(({ path }) => {
      if (path === "pulse/RESEARCH-QUESTION.md" || path === "pulse/THIS-WEEK.md") {
        ipc.readPulse().then(setPulse);
      } else if (path === "notes.md") {
        ipc.readWikiFile("notes.md").then((f) => setNotes(f.content));
      } else if (path === "ideas.json") {
        ipc.readIdeas().then(setIdeas);
      } else if (path.startsWith("resources/")) {
        ipc.listResources().then(setResources).catch((e) => toastError("load resources", e));
      }
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  // Debounced layout persistence
  const skipInitialLayoutSave = useRef(true);
  useEffect(() => {
    if (skipInitialLayoutSave.current) { skipInitialLayoutSave.current = false; return; }
    const h = setTimeout(() => {
      ipc.saveWorkspaceState(toPersisted()).catch((e) => toastError("save layout", e));
    }, 1000);
    return () => clearTimeout(h);
  }, [panelLayout, toPersisted]);

  // Debounced effort persistence
  const skipInitialEffortSave = useRef(true);
  useEffect(() => {
    if (skipInitialEffortSave.current) { skipInitialEffortSave.current = false; return; }
    const h = setTimeout(() => {
      ipc.saveWorkspaceState({ ...toPersisted(), effort }).catch((e) => toastError("save effort", e));
    }, 1000);
    return () => clearTimeout(h);
  }, [effort, toPersisted]);

  const handleClose = async () => {
    await ipc.closeWorkspace();
    const status = await ipc.setupStatus();
    setPhase({ kind: "setup", status });
  };

  return (
    <div className="bg-background text-on-surface font-sans h-screen flex flex-col overflow-hidden text-sm">

      {/* Top NavBar */}
      <header className="flex items-center justify-between px-3 h-10 w-full bg-background border-b border-outline-variant shrink-0">
        <div className="flex items-center gap-2">
          {runningExperimentsCount > 0 && (
            <div className="flex items-center gap-2 border border-warn/30 text-warn px-2 py-0.5 rounded text-xs bg-warn/5">
              <span className="w-1.5 h-1.5 rounded-full bg-warn animate-pulse" />
              running {runningExperimentsCount} exp{runningExperimentsCount !== 1 ? "s" : ""}
            </div>
          )}
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={handleClose}
            className="text-on-surface-variant hover:text-on-surface transition-colors flex items-center justify-center p-1 cursor-pointer border-none bg-transparent"
            title={`Close workspace: ${workspace?.name}`}
          >
            <span className="material-symbols-outlined text-[18px]">close</span>
          </button>
          <button className="text-on-surface-variant hover:text-on-surface transition-colors flex items-center justify-center p-1 cursor-pointer border-none bg-transparent">
            <span className="material-symbols-outlined text-[18px]">settings</span>
          </button>
        </div>
      </header>

      {/* Main 3-column layout */}
      <div className="flex flex-1 overflow-hidden">
        <LeftRail
          pulse={pulse}
          onPulseChange={setPulse}
          resources={resources}
          onResourceClick={(r) => openPreviewTab(r.title ?? r.url, r.wiki_path!)}
        />

        {/* Center drag handle */}
        <div className="w-px bg-outline-variant shrink-0" />

        <main className="flex-1 flex flex-col min-w-0 bg-background relative overflow-hidden">
          <ChatPane />
        </main>

        {/* Right drag handle */}
        <div className="w-px bg-outline-variant shrink-0" />

        <RightRail
          notes={notes}
          onNotesChange={setNotes}
          ideas={ideas}
          onIdeasChange={setIdeas}
        />
      </div>

      <StatusBar />
    </div>
  );
}
```

- [ ] Remove the `parseFocus` function (it's deleted along with the old Layout content).

- [ ] Build check:
```bash
npm run build 2>&1 | grep -E "error TS" | head -20
```

- [ ] Commit:
```bash
git add src/components/Layout.tsx src/components/MarkdownPane.tsx src/components/ResourceInput.tsx src/components/ResourcesList.tsx
git commit -m "feat: rewrite Layout with new rail components, remove old components"
```

---

## Task 16: Full build and dev-server smoke test

- [ ] Run full TypeScript + Vite build:
```bash
npm run build 2>&1
```
Expected: zero TypeScript errors, zero Vite errors.

- [ ] Run Rust build:
```bash
cd src-tauri && cargo build 2>&1 | grep -E "^error" | head -20
```
Expected: zero errors.

- [ ] Run the app:
```bash
npm run dev:tauri
```
Verify manually:
  1. Workspace picker shows new design — title, recent list, two action buttons, status row
  2. Open a workspace — Layout renders with left rail, center pane, right rail, status bar
  3. Focus pane shows two fields; clicking a field enters edit mode; saving calls backend
  4. Resources section lists resources; clicking opens preview tab
  5. Experiments section lists experiments; clicking opens experiment tab with logs
  6. Ideas: add idea, trash idea, drag to reorder — changes persist to `ideas.json`
  7. Notes: edit mode, save calls `save_notes`
  8. Add resource: URL input + Add button works
  9. Status bar shows git branch, CPU %, RAM, hostname, cc-connected
  10. Chat composer: model/effort pickers, send button, auto-resize textarea
  11. Sending a message streams correctly with new message styles
  12. Tool cards render done/running states correctly
  13. Close workspace button (X icon) returns to picker
  14. No theme toggle button anywhere in the UI

- [ ] Commit any final fixes:
```bash
git add -A
git commit -m "fix: final integration fixes from smoke test"
```

---

## Task 17: Update documentation

**Files:**
- Modify: `docs/SDD.md` (§4 directory layout, §13 frontend)
- Modify: `docs/CHANGELOG.md`

- [ ] In `docs/SDD.md` §4, update wiki directory tree:
  - Replace `ideas.md` with `ideas.json`
  - Replace `status/pulse.md` with `pulse/RESEARCH-QUESTION.md` and `pulse/THIS-WEEK.md`

- [ ] In `docs/SDD.md` §13, update frontend description to reflect:
  - Tailwind CSS replacing custom styles
  - New component structure (LeftRail, RightRail, StatusBar, ExperimentTabView)
  - Removed light-mode toggle
  - Split pulse files
  - Ideas JSON format

- [ ] Add to `docs/CHANGELOG.md` under `## Unreleased`:
```markdown
### Added
- Frontend redesign: Tailwind CSS design system matching new visual spec
- Experiment tab view: click an experiment in left rail to open a dedicated tab with live logs
- Real system metrics in status bar (CPU, GPU, RAM, git branch, hostname)
- Ideas now stored as `ideas.json` with soft-delete (trashed flag) and drag-to-reorder
- Focus pane split into Research Question and This Week fields

### Changed
- Left rail now has Focus / Resources / Experiments sections
- Right rail now has Notes / Ideas / Add Resource sections
- Workspace picker redesigned to match new visual spec
- Chat messages, tool cards, and composer restyled

### Removed
- Light mode / theme toggle (dark mode is now fixed)
- `status/pulse.md` replaced by `pulse/RESEARCH-QUESTION.md` + `pulse/THIS-WEEK.md`
- `ideas.md` replaced by `ideas.json`
```

- [ ] Commit:
```bash
git add docs/
git commit -m "docs: update SDD and CHANGELOG for frontend redesign"
```
