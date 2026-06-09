# Architecture Docs

This document is an index. The architecture has been split into focused files — one per subsystem.

| File | Contents |
|---|---|
| [overview.md](overview.md) | Problem & users, system diagram, tech stack, directory layout, source tree |
| [workspace.md](workspace.md) | Workspace lifecycle (open, init, close) + concurrency & data safety |
| [wiki-memory.md](wiki-memory.md) | Wiki layer, memory layer (long-term / short-term), context injection, SQLite schema |
| [chat-agents.md](chat-agents.md) | Ingestion pipelines, chat system, experiment lifecycle, agent subprocess layer |
| [mcp.md](mcp.md) | MCP server, tool catalog, backend RPC channel |
| [frontend.md](frontend.md) | React layout, chat rendering, theming, workspace state, Tauri IPC surface |
| [ROADMAP.md](../ROADMAP.md) | Implementation phases + open items & risks |

Deep-dive implementation guides live in [blueprints/](blueprints/).
