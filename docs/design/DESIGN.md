# IRE Design

UI design for IRE lives in a Stitch (Google Labs) project. This file is the source of truth for design IDs, principles, and tokens. Update it whenever a screen, asset, or token changes.

## Stitch project

- Project: `projects/13231898665553245452`
- Title: *IRE — Integrated Research Environment*

## Design systems

Two design systems are saved as project assets. Both share the same type, spacing, shape, and component rules; only surface tones differ.

| System | Asset ID | Color mode | Notes |
|---|---|---|---|
| IRE — Light | `assets/10196251303737747648` | LIGHT | Pinned as the project's first canvas preview. |
| IRE — Dark  | `assets/17994448157533728660` | DARK  | Used for all currently generated screens. |

## Generated screens

All desktop, 2560×2048 (rendered at 2× density). All Dark theme.

| # | Title | Screen ID | Purpose |
|---|---|---|---|
| 1 | IRE Workspace Home (Redesign) | `7d18fe45af0848f49445e749c6dcfa49` | The 5-pane workspace, redesigned: TabBar at the top of the chat column, no Brainstorm/Experiment toggle, with explicit Thinking and Tool-call states (running + done) shown inline in the assistant message. |
| 2 | IRE Workspace Picker | `bba7e48641a74a468584d70670095e85` | First-launch / open-or-create screen. Recent workspaces, `claude-code` auth state. |
| 3 | IRE Resource Detail | `1f49ed4cf8894390906b4754db51cca9` | Paper read-mode: sticky outline, body text, blockquote, inline math, code, margin notes. |
| 4 | IRE Settings | `6d20688de5574eb4bce7e62b2686e12b` | Modal dialog with left nav (General / Claude-Code / Wiki / Experiments / Appearance / About). |

To list everything currently saved, call `mcp__stitch__list_screens` and `mcp__stitch__list_design_systems` against the project ID. To extend, call `mcp__stitch__generate_screen_from_text` with `projectId=13231898665553245452` and `designSystem=assets/17994448157533728660` (Dark) or `assets/10196251303737747648` (Light).

## Prototype navigation (current state)

Originally wired via `mcp__stitch__edit_screens`. The original Workspace Home (`ec867d1863fd45428ec482aa8df77075`) was later deleted in favor of the redesign (`7d18fe45af0848f49445e749c6dcfa49`), leaving stale hrefs on the Picker, Resource Detail, and Settings screens. An attempt to strip those broken hrefs via `edit_screens` did not persist on the canonical screens (Stitch's edit agent regenerated forks that were not adopted as the canonical version). The broken anchors therefore remain in the three screens.

In Stitch's preview, clicking them no-ops (404 on a non-existent screen file). Not blocking — just untidy.

| From | Hotspot | Status |
|---|---|---|
| Workspace Picker | First recent row, both action buttons | broken (links to deleted home) |
| Workspace Picker | "settings" link | works → Settings |
| Resource Detail | Back arrow + breadcrumb | broken (links to deleted home) |
| Resource Detail | Outline list items | works → in-page anchors `#section-N` |
| Settings | × close, Cancel, Save | broken (link to deleted home) |
| Workspace Home (Redesign) | none | never wired |

If you want the broken anchors removed, the reliable path is editing the HTML by hand in Stitch's web UI rather than via `edit_screens`.

## Principles

The product is a desktop tool for ML academics who live in it for hours. Design must disappear so the work shows through.

1. **Calm over loud.** Low saturation. No accent color that pulls the eye. Chat content, paper text, and code are the visual hierarchy — not chrome.
2. **Information density.** Tight spacing, smaller type, more visible at once. No SaaS-style generous padding.
3. **Text-first, no decoration.** No illustrations, no hero images, no gradients, no shadows beyond a 1px hairline border. Icons are 16px line glyphs only when they add meaning.
4. **Monochrome by default, color by signal.** UI chrome is graphite/neutral. Color appears only to encode state: error (red), running experiment (amber), success (green), info (subtle blue). Never decorative.
5. **Borders, not fills.** Panels and cards are separated by 1px hairlines, not filled backgrounds. Backgrounds stay flat.
6. **Honor the OS.** Native scrollbars, native context menus, native window chrome on Tauri. Don't reinvent platform affordances.

## Tokens

Use these literal hex values in code. Stitch internally remaps them to a Material 3 token set for rendering, so its preview is *near* but not pixel-identical to the spec — implement against the table below, not against Stitch's output.

### Color — Light

| Token | Hex | Usage |
|---|---|---|
| `bg`            | `#FFFFFF` | Canvas |
| `bg-subtle`     | `#FAFAFA` | Sidebars, secondary panels |
| `bg-muted`      | `#F4F4F5` | Code blocks, hover states |
| `border`        | `#E4E4E7` | Hairlines |
| `border-strong` | `#D4D4D8` | Stronger separators |
| `fg`            | `#18181B` | Primary text |
| `fg-muted`      | `#52525B` | Secondary text, labels |
| `fg-subtle`     | `#A1A1AA` | Placeholders, disabled |
| `accent`        | `#27272A` | Primary buttons, focused element (graphite) |
| `accent-fg`     | `#FAFAFA` | Text on accent |
| `link`          | `#3F3F46` | Underlined links — never blue |
| `state-error`   | `#B91C1C` | Errors |
| `state-warn`    | `#A16207` | Warnings, running |
| `state-ok`      | `#15803D` | Success |
| `state-info`    | `#1E40AF` | Info, unread |

### Color — Dark

| Token | Hex | Usage |
|---|---|---|
| `bg`            | `#0A0A0A` | Canvas, near-black, OLED-friendly |
| `bg-subtle`     | `#111113` | Sidebars |
| `bg-muted`      | `#1A1A1D` | Code blocks, hover |
| `border`        | `#27272A` | Hairlines |
| `border-strong` | `#3F3F46` | Stronger separators |
| `fg`            | `#F4F4F5` | Primary text — off-white, easier on the eyes than pure white at night |
| `fg-muted`      | `#A1A1AA` | Secondary text |
| `fg-subtle`     | `#71717A` | Placeholders, disabled |
| `accent`        | `#E4E4E7` | Light graphite buttons (inverse of Light) |
| `accent-fg`     | `#0A0A0A` | Text on accent |
| `link`          | `#D4D4D8` | Links |
| `state-error`   | `#F87171` | Errors |
| `state-warn`    | `#FBBF24` | Warnings, running |
| `state-ok`      | `#4ADE80` | Success |
| `state-info`    | `#60A5FA` | Info, unread |

### Typography

One family: **Inter**. Variable-weight. Optical sizing on. Code: **JetBrains Mono**.

Rules:
- Bold is `600`, never `700+`.
- Headline tracking is slightly tightened (`-0.01em` to `-0.02em`) at large sizes; body sits at `0`.
- No italic emphasis except for paper titles, blockquotes, and inline math.

| Token | Size | Weight | Line height | Tracking | Usage |
|---|---|---|---|---|---|
| `display-lg`  | 32px | 600 | 40px  | -0.02em  | Page titles |
| `display-md`  | 24px | 600 | 32px  | -0.015em | Section heroes |
| `headline-lg` | 20px | 600 | 28px  | -0.01em  | Subsection |
| `headline-md` | 16px | 600 | 24px  | -0.005em | Card title |
| `headline-sm` | 14px | 600 | 20px  | 0        | Small heading |
| `title-md`    | 13px | 600 | 18px  | 0.01em   | Inline emphasis |
| `body-lg`     | 15px | 400 | 24px  | 0        | Reading text |
| `body-md`     | 14px | 400 | 22px  | 0        | Default body |
| `body-sm`     | 13px | 400 | 20px  | 0        | Helper text |
| `label-md`    | 12px | 500 | 16px  | 0.02em   | Inline labels |
| `label-sm`    | 11px | 500 | 14px  | 0.04em   | Pane headers, uppercase tracked |
| `code-md`     | 13px | 400 | 20px  | 0        | JetBrains Mono — code |

### Spacing

4px base. Use only the scale below. Pane padding `4` (16px). Inline gap inside a row `2` (8px). Section gap `6` (24px).

`0=0  1=4  2=8  3=12  4=16  5=20  6=24  8=32  10=40  12=48  16=64`

### Shape

- 4px radius universally — buttons, inputs, cards, panels.
- `999px` (pill) only for tags and avatars.
- No drop shadows. Elevation expressed only via 1px borders.

## Components

- **Button** — 28px height (default), 32px (primary action). 1px border, no shadow. Primary = filled `accent`. Secondary = transparent w/ border. Ghost = transparent, no border, hover `bg-muted`.
- **Input** — 28px, 1px border. Focus ring is a single `accent` border (no glow halo).
- **Pane** — bordered region with a 24px header strip. Header uses `label-sm` uppercase tracked, plus optional inline 16px action icons.
- **List item** — 1 line, 24px tall. Hover `bg-muted`. Active = 2px left bar in `accent`. No checkboxes unless multi-select active.
- **Chat bubble** — borderless. User messages right-aligned with `bg-muted`. Assistant messages full-width on canvas, no bubble. Tool calls collapsed by default in a hairline-bordered box.
- **Tag/Pill** — `label-sm`, 18px height, 6px padding, 1px border, transparent fill. Color only for state tags (running / done / failed).
- **Resource card** — title `headline-md`, authors+year `body-sm fg-muted`, source URL `label-sm fg-subtle`. No thumbnail.
- **TabBar** — sits above the chat header. 32px tall, 1px bottom border. Each tab: 28px tall, `body-sm`, `bg-subtle` resting, `bg` when active with 1px hairline separators on each side; close `×` on hover, hidden on pinned tabs. Resource tabs show a 12px spinner glyph at left while their summary is generating. A 28×28 ghost `+` button sits at the right end. Overflow scrolls horizontally; tabs never shrink below `label-md` legibility.
- **Reset button (↺)** — 24px ghost icon button in the chat-pane header's right-side action group. Title attribute "Reset conversation". Visible only when the active tab is idle. Icon glyph `↺` in `fg-muted`, `fg` on hover. Wipes the conversation and resets the underlying CC session.
- **Cancel button** — same slot as Reset, but visible only while streaming. Secondary button (transparent + 1px border), `body-sm`, label "Cancel".
- **Resource bar** — appears between MessageList and Composer when an active resource tab is in `ready` state. Two buttons inline, 8px gap: a primary `Confirm — save to wiki` (filled `accent`) and a ghost `Discard`. Disappears once confirmed or discarded.
- **Thinking block** — collapsible region preceding an assistant message when CC produces extended thinking. Visual treatment:
  - 1px left border in `fg-subtle` (4px wide gutter on the left), no other borders, no fill.
  - Header row: 16px tall, `label-sm` `fg-muted` reading "Thinking" + an inline elapsed-time string (`12s`, `2m 14s`); a 12px chevron at the right indicates expand/collapse state.
  - Body when expanded: `body-sm` `fg-muted`, italic, line-height generous (24px). Wraps cleanly. Code spans inside use `code-md` but `fg-subtle` to keep them quiet.
  - Default state: collapsed once the assistant starts producing visible output. Expanded only while actively thinking, or on user click. No animation beyond a 120ms ease-out height change.
- **Tool call block** — appears inline within an assistant message wherever CC invokes a tool. Three states:
  - **Running** — 1px hairline border in `state-warn`, 8px padding. Header: a 12px spinner glyph + tool name in `label-md` (e.g. `bash`) + a single-line truncated parameter in `code-md fg-muted` (e.g. `cd experiments/350m && ./run.sh ln-post seed=42`). Body: live tail of stdout in `code-md fg-muted`, max 4 lines visible by default, "view full log" link at bottom-right.
  - **Done (success)** — 1px hairline border in `border`. Header collapsed: `✓` in `state-ok` (12px), tool name, truncated parameter, elapsed time on the right. No body. Click to expand and inspect the captured output. Default is collapsed.
  - **Failed** — 1px hairline border in `state-error`. Header: `✕` in `state-error`, tool name, truncated parameter. Body always expanded showing the error excerpt in `code-md fg`. A "retry" ghost button sits bottom-right.
  - All three share the same 4px corner radius, the same `bg-muted` fill (matches code blocks), and never carry shadows.

## Layout

5-pane desktop grid (≥ 1280px):

- Left rail 240px: Focus + Resources + Experiments stacked.
- Center fluid: Chat.
- Right rail 320px: Notes + Ideas + URL submit.
- All rails resizable, collapsible to 0.
- < 1024px: right rail collapses.
- < 800px: left rail also collapses into a drawer.

## Motion

- Linear or ease-out, 120ms for hover/focus, 180ms for panel open/close.
- No spring animations, no bouncing, no parallax.
- Respect `prefers-reduced-motion`.

## Accessibility

- Minimum contrast 4.5:1 for body text in both modes.
- Focus ring always visible — 1px solid `accent`, no inset.
- Hit target ≥ 24px.

## Implementation note

When converting these tokens into the actual Tauri/React codebase, use the literal hex values above. Do **not** copy from Stitch's `namedColors` output — Stitch derives a Material 3 palette internally that is close to the spec but not identical (e.g. the canvas `#0A0A0A` is rendered as `#0e0e11`, accent `#E4E4E7` as `#c6c6c9`).
