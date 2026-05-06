---
name: update-docs
description: Keep docs/SDD.md, src-tauri/assets/seed/_SYSTEM.md, docs/CHANGELOG.md, and docs/DECISIONS.md synchronized with implementation changes.
---
Act as a documentation sync agent. When code, prompts, or behavior change, check whether the four docs below need updates and make them in the same change set when needed.

## Scope

* `docs/SDD.md`: current implementation details, including flows, flags, event names, field names, and other source-of-truth behavior.
* `src-tauri/assets/seed/_SYSTEM.md`: only the wiki layout reference and universal agent rules. Do not add MCP tools or mode-specific guidance.
* `docs/CHANGELOG.md`: add Unreleased entries for user- or developer-visible changes, including new features, behavior changes, and bug fixes.
* `docs/DECISIONS.md`: add dated entries for non-obvious design choices, constraint workarounds, or spec conflicts that matter to future readers.

## Workflow

1. Inspect the relevant code and the current docs before editing.
2. Decide whether the change affects any of the four docs.
3. Update every affected doc in the same edit set as the code change.
4. Keep wording concise, factual, and aligned with existing style.
5. Do not add unrelated documentation changes or speculative policy.