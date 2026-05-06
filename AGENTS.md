### 1. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:
- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them - don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

### 2. Simplicity First

**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

### 3. Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing code:
- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it - don't delete it.

When your changes create orphans:
- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the user's request.

### 4. Goal-Driven Execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals:
- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:
```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

### 5. Build Verification

**After every change, verify the build passes.**

Run before reporting work as done:
```
npm run tauri dev
npm run dev:tauri # if running from snap-installed VSCode
```

This compiles both Rust and TypeScript. It must succeed with zero errors and zero warnings. Warnings are bugs — dead code and unused fields mean speculative code crept in (see §2).

### 6. Documentation Synchronization

**Keep SDD, `_SYSTEM.md`, CHANGELOG, and DECISIONS in lockstep. Every implementation divergence requires a doc update.**

- When code diverges from `docs/SDD.md` (flags removed, flow changed, commands altered, new features added), update the SDD **immediately** in the same commit.
- The SDD is the architectural source of truth — it must reflect the current implementation to the detail level (exact CLI flags, event names, field names, etc.).
- If a SDD section describes the old behavior, update it to match the new behavior.
- When unsure whether the SDD needs updating, err on the side of caution: stale architecture docs are worse than missing ones.

After every major change, read `docs/CHANGELOG.md` and `docs/DECISIONS.md` and update them if needed:
- **CHANGELOG.md** — add an entry under `## Unreleased` for any user- or developer-visible change: new features, behavior changes, and bug fixes. Internal-only refactors don't warrant an entry. Use the existing Added / Changed / Fixed subsections.
- **DECISIONS.md** — add a dated entry for any non-obvious design choice: when you picked one approach over alternatives, worked around a constraint, or resolved a spec conflict. If the reasoning would help a future reader understand why the code looks the way it does, write it down. Skip entries for straightforward implementation details.

`src-tauri/assets/seed/_SYSTEM.md` is the general-purpose system prompt injected into every CC turn regardless of mode. Keep it accurate:
- **Do not document MCP tools here.** Tools are advertised automatically via MCP server handshaking — duplicating them in `_SYSTEM.md` wastes context and gets stale.
- **Do not add mode-specific guidance here.** Experiment workflow instructions belong in the experiment-mode preamble (`commands/chat.rs`), not in `_SYSTEM.md`.
- When the wiki layout changes (new files, renamed paths, restructured dirs), update the layout block.
- When behavioral rules change (new agent expectations, updated conventions), update the rules list.
- `_SYSTEM.md` contains only: wiki layout reference and universal agent rules.

### 7. Parallel Intervention

**Be aware of possible multi-agent collaboration in the codebase**

Multiple agents may be working simultaneously. If you see errors in files you did NOT edit, do not try to fix them. Wait 30 seconds and retry - the other agent is likely mid-edit.