---
name: open-pr
description: Open a PR on GitHub with the changes introduced.
---

Open a pull request on GitHub for the current changes, following the repo's PR template.

## Steps

1. Always fetch the latest changes from the remote first (`git fetch origin`) before doing anything else.
2. Check the current branch.
   - If on `main`, create and check out a new, appropriately named branch first.
   - If already on a dedicated branch, check whether it has already been merged into `main` (e.g. `git branch --merged main`) or is substantially behind `main` (large commit/diff divergence). If either is the case, stop and surface this to the user, asking how they'd like to proceed, before doing any further work.
   - Otherwise, check whether it is up to date with `main`; if not, rebase onto `main`. Always rebase, never merge `main` into the branch.
3. Review the diff and recent commits to understand what changed and why.
4. Run the `/update-docs` skill to ensure `docs/architecture/*` and `src-tauri/assets/seed/_SYSTEM.md` are in sync with the changes before opening the PR.
5. Push the branch and open the PR (via `gh pr create`), filling in [.github/PULL_REQUEST_TEMPLATE.md](../../../.github/PULL_REQUEST_TEMPLATE.md).

## Writing the description

- Keep it minimal and human-readable: what changed and why, briefly how.
- No AI filler, no restating the diff line-by-line, no text nobody will read.
- Fill in the checklist items honestly based on what you actually verified.
