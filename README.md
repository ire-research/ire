# Integrated Research Environment (IRE)
First MVP for a general-purpose Integrated Research Environment (IRE).

## Deliverable Product
Cross-platform app with one-click download and one-click startup.

## Tech Stack (tentative)
- [Tauri](https://v2.tauri.app/start/create-project/) as the cross-platform app framework (Rust backend, frontend-agnostic). It automatically integrates:
    - React frontend with TypeScript
    - Rust backend
    - development mode with file watching and an auto-updated binary (`npm run tauri dev` tested)