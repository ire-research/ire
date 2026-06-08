# Integrated Research Environment (IRE)

> A local-first desktop environment for ML researchers that keeps your code, literature, experiments, and AI collaboration seamlessly connected—so you never lose context, never repeat dead ends, and always know what to focus on next.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Built_with-Tauri-24C8DB)](https://tauri.app)
[![TypeScript](https://img.shields.io/badge/TypeScript-3178C6?logo=typescript&logoColor=white)](https://www.typescriptlang.org)
[![Rust](https://img.shields.io/badge/Rust-CE422B?logo=rust&logoColor=white)](https://www.rust-lang.org)

![IRE interface screenshot](public/ui.png)

## The Problem

ML research workflows are **fragmented**. You're constantly juggling:

- **Code in an IDE**, papers in a browser, notes scattered across documents
- **Context loss** between sessions—every time you return to a project, you're re-establishing state from scratch
- **Knowledge fragmentation**—no indexed memory of what you've tried, what failed, and why
- **Experiment limbo**—you ask the AI to run a long experiment, then switch tasks and forget about it
- **Goal drift**—the core research question gets buried under technical details and literature exploration

**The cost?** Redundant AI suggestions. Repeated rejection of dead-end approaches. Lost insights. Wasted compute.

IRE solves this by treating your research workspace as a **unified, persistent entity**—not a collection of scattered files.

---

## What IRE Does

### 🎯 **One Workspace, One Project**
Each IRE workspace maps 1:1 to a Git repository. Your code, research wiki, experiments, and AI state all live together in `.ire/`. Version control is built in.

### 📚 **Persistent LLM Wiki**
A Git-tracked markdown wiki that IRE maintains automatically:
- **Indexed resources** — papers, articles, API docs, all one search away
- **Structured memory** — architectural decisions, failed approaches, current blockers—injected into Claude-Code's context automatically
- **Daily notes** — what you (and the AI) discovered today, promoted to long-term memory when it matters
- **Experiment logs** — every run tracked, searchable, versioned

### 🚀 **Never Hang on Experiments**
Fire off a long-running experiment and keep working. IRE monitors it. When it finishes, Claude-Code wakes up with the results, full context, and asks what to do next.

### 🧠 **Claude-Code, Deeply Integrated**
IRE wraps Claude-Code in a native desktop app with:
- **5-pane research interface** — focus banner, resources, notes, ideas, and a live chat with Claude-Code
- **MCP server bridge** — Claude-Code can read your wiki, write findings, record research memory, update your research pulse
- **Two modes** — Brainstorm (explore) and Experiment (make changes, run code)
- **Streaming chat** — watch Claude-Code think, debug, and plan in real-time

### 🔒 **Local-First, No Backend**
Everything runs on your machine. Your research stays private. No vendor lock-in. Git is your backup and collaboration tool.

---

## Quick Start

### Prerequisites
- **Claude-Code** installed and authenticated
- **Node.js** 18+ and **npm**
- **Rust** 1.70+ (for building from source)
- **Git**

### Installation

Clone the repository and install dependencies:

```bash
git clone https://github.com/yourusername/ire.git
cd ire
npm install
```

### Running the Development Environment

**Option 1: Web dev server only** (no desktop app)
```bash
npm run dev
```
Opens http://localhost:5173 in your browser.

**Option 2: Full desktop app** (recommended)
```bash
npm run tauri dev
```
Builds the Tauri binary and launches IRE as a native desktop application.

### First Launch

1. IRE checks for Claude-Code and guides setup if needed
2. Create a new workspace (empty directory or existing Git repo)
3. IRE initializes `.ire/` and the research wiki
4. Start brainstorming or planning experiments

---

## How It Works

### The Five-Pane Interface

```
┌─────────────────┬──────────────────────────────────┬──────────────────┐
│  Focus          │                                  │  Notes           │
│  (Pulse)        │      Central Chat Pane            │  (Editable)      │
│                 │  - Streaming responses            │                  │
├─────────────────┤  - Tool calls & results          ├──────────────────┤
│  Resources      │  - Experiment status              │  Ideas           │
│  (Indexed)      │                                  │  (Editable)      │
│                 │  [User message input]            │                  │
│  Experiments    │                                  │  Resource URL    │
│  (Live tail)    │                                  │  Submit          │
└─────────────────┴──────────────────────────────────┴──────────────────┘
```

### Chat Modes

| | **Brainstorm** | **Experiment** |
|---|---|---|
| Use case | Explore ideas, read papers, plan | Run code, make file changes, run experiments |
| Claude-Code tools | Wiki read, memory write, resource fetch | + File edit, bash, experiment management |
| Permissions | Prompt on changes (safer) | Auto-accept changes |

---

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for dev setup,
build verification, and PR expectations. Please also read our
[Code of Conduct](CODE_OF_CONDUCT.md).

## Security

Found a vulnerability? Please report it privately — see [SECURITY.md](SECURITY.md).

## License

IRE is licensed under the [MIT License](LICENSE).
