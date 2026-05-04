# Design Blueprint: IRE Frontend

This document synthesizes design principles from Linear and Conductor.build—focusing on utilitarian minimalism, information density, and border-first layouts—into a cohesive style guide for the Integrated Research Environment (IRE). The resulting palette uses warm gold accents as a deliberate departure from both tools (Linear's desaturated blue, Conductor's neutral tones), creating visual identity tailored to IRE's instrumentation-forward aesthetic.

### 1. Core Design Philosophy

Both Linear and Conductor prioritize **utilitarian minimalism** and **information density**:

- **Utilitarian Minimalism:** UI chrome is almost invisible. Content (code, logs, markdown) is the interface. Both tools strip unnecessary decoration in favor of functional, scannable designs.
- **Information Density:** Optimize for multi-pane layouts. Minimize padding in structural containers; reserve whitespace for reading zones (Markdown preview). Conductor's bento-box layout and Linear's compact issue tracking exemplify this.
- **Keyboard-First Affordances:** Both provide command palettes and focus-state indicators for efficient navigation.
- **Agent as a First-Class Citizen:** Linear's agent-centric UI and Conductor's agent workspace isolation show how background processes should be visible but non-intrusive.

### 2. Color Palette (Dark Theme with Gold)

IRE adopts a dark-mode-first approach, inspired by both Linear and Conductor but with a distinct warm accent.

**Design Notes:**
- Linear's brand uses a **subtle desaturated blue**, while Conductor employs neutral grays and blacks
- IRE intentionally shifts away from blue toward gold (#998200) as a distinctive accent, evoking scientific instrumentation aesthetics
- The base palette mirrors both tools' dark-mode foundations with high contrast for readability

**Colors:**
    *   `--bg-app`: `#0A0A0A` (Deepest black for the window shell).
    *   `--bg-pane`: `#121212` (Slightly elevated for panels).
    *   `--bg-hover`: `#1E1E1E` (List items, hovered tabs).
*   **Borders:**
    *   `--border-subtle`: `#262626` (Used for 1px pane dividers).
    *   `--border-focus`: `#333333` (Used for focused inputs).
*   **Typography:**
    *   `--text-primary`: `#F2F2F2` (High contrast, pure readability).
    *   `--text-secondary`: `#A1A1AA` (Muted, used for timestamps, paths, and inactive tabs).
*   **Accents (The Gold Scale):**
    *   `--accent-primary`: `#998200` (Active tab lines, primary buttons, syntax highlights).
    *   `--accent-glow`: `rgba(153, 130, 0, 0.15)` (Subtle background glow for active agent states or focused elements).
    *   `--status-running`: `#EAB308` (Yellow/Gold for active processes).
    *   `--status-done`: `#22C55E` (Muted green).
    *   `--status-failed`: `#EF4444` (Muted red).

### 3. Typography

IRE adopts a clean two-font system, common in modern dev tools like Linear and Conductor.

*   **Interface & Prose:** A clean, geometric sans-serif (e.g., *Inter*, *Geist*, or system UI). Used for UI labels, buttons, and Markdown prose. Size: 13px–14px for UI, 15px for prose. **Note:** While specific fonts may vary by platform, prioritize system fonts for performance when available.
*   **Data & Code:** A crisp monospace font (e.g., *JetBrains Mono*, *Geist Mono*, *SF Mono*, or system monospace). Strictly used for paths, experiment logs, tool call previews, and inline code. Size: 12px–13px.

### 4. Layout & Spacing (The "Border-First" Approach)

Both Linear and Conductor use strict borders over drop shadows to separate UI sections, creating a flush, modular appearance.

*   **1px Solid Dividers:** Do not use drop shadows to separate the 5 panes. Use strict `1px solid var(--border-subtle)` lines. This creates the flush, bento-box feel seen in Conductor's actual interface design.
*   **Tabbed Document Interface (TDI):** 
    *   Active tabs blend seamlessly into the pane background (`--bg-pane`).
    *   Inactive tabs are darker (`--bg-app`) with `--text-secondary`.
    *   Apply a `2px` top-border of `--accent-primary` (`#998200`) to the active tab.
*   **Scrollbars:** Hidden by default. Fade in on hover. Keep them thin and dark.

### 5. UI Elements & Components

Linear and Conductor both favor **flat, border-driven design** over gradients and shadows. Apply these patterns to IRE components:

*   **Buttons & Inputs:** Flat design. No gradients on standard UI elements. Use a `1px` border with a subtle background change on hover. Follows Conductor's minimal button style.
*   **Tool Cards & Experiment Cards (Chat Pane):** 
    *   Render Claude's tool calls (e.g., `[Read] path/to/file`) as collapsed, single-line cards (inspired by Linear's issue card compactness).
    *   Use monospace for the tool name and target.
    *   Add a subtle, pulsing left-border (`--accent-primary`) when an experiment or tool is actively running.
*   **Thinking/Loading States:** Avoid large, disruptive spinners. Use minimal, inline animations (e.g., an animated ellipsis or a small glowing dot next to the agent's name). Linear's agent status indicators model this well.
*   **Markdown Toggle:** A simple pill-shaped toggle at the top right of the markdown panes. Active state gets a slight background lift, not a heavy color fill.

### 6. Application to the IRE SDD
*   **Focus Banner:** Keep it pinned at the top left. Give it a subtle background tint (`rgba(153, 130, 0, 0.05)`) and a left-accent border of `#998200` to draw attention to the active blocker.
*   **Chat Pane:** 
    *   User messages: Right-aligned, subtle grey background (`#1E1E1E`), no border.
    *   Agent messages: Left-aligned, transparent background, pure text.
    *   Input Box: Flush to the bottom. Expandable textarea. 1px border that illuminates slightly when focused.
*   **Logs / Terminal Output:** Strictly monospace. Strip away all UI chrome. Color-code `stderr` to muted red and `stdout` to `--text-secondary`.

---

## Design Verification Notes

This document was verified against:
- **Linear's Brand Guidelines** (linear.app/brand): Linear uses a subtle desaturated blue as primary brand color, not gold. The gold (#998200) is a deliberate IRE-specific choice for scientific instrumentation aesthetic.
- **Conductor's Live Design** (conductor.build): Confirmed border-first, bento-box layout with dark mode and minimal UI chrome.

**Key Divergences:**
- IRE's gold accent is a custom addition, not sourced from either tool's official palette.
- Typography recommendations are best-practice suggestions; actual implementation should prioritize available system fonts for performance.
- The color hex values provided are target specifications for IRE and may differ from either source tool's actual implementation.