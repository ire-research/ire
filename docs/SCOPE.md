# IRE Scope

Tracks what is out of scope of this project, i.e., what IRE is not meant to do by design.

The SDD ([SDD.md](./SDD.md)) is the source of truth for *what* IRE does and *how*.

This file is the source of truth for *whether* a feature belongs to IRE at all.

## Out of scope (non-goals)

- Direct code execution by IRE itself; only CC executes commands (via its `Bash` tool or MCP `experiment.start`).
- Local LLM execution. CC must be authenticated externally.
- Proprietary sync / cloud backend.
- User analytics or telemetry.
