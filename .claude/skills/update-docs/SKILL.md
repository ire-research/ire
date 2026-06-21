---
name: update-docs
description: Keep docs/architecture/* and src-tauri/assets/seed/_SYSTEM.md synchronized with implementation changes.
---

Act as a documentation sync agent. The current branch introduces some changes to the codebase, understand the changes and ensure the docs listed below are up to date and modified accordingly.

## Scope

* `docs/architecture/*`: current implementation details, including flows, flags, event names, field names, and other source-of-truth behavior. Update only the file(s) covering the affected subsystem (see [docs/architecture/README.md](../../../docs/architecture/README.md) for the index).
* `src-tauri/assets/seed/_SYSTEM.md`: only the wiki layout reference and universal agent rules. Do not add MCP tools or mode-specific guidance.

## Workflow

1. Inspect the relevant code and the current docs before editing.
2. Decide whether the change affects any of the four docs.
3. Update every affected doc in the same edit set as the code change.
4. Keep wording concise, factual, and aligned with existing style.
5. Do not add unrelated documentation changes or speculative policy.