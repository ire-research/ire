# Contributing to IRE

Thanks for your interest in contributing! This guide covers how to get a dev
environment running, how we expect changes to be tested, and what we look for
in a pull request.

## Development Setup

### Prerequisites

- **Node.js** 18+ and **npm**
- **Rust** 1.70+ (for building the Tauri shell)
- **Git**
- **Claude-Code** installed and authenticated (IRE wraps it natively)

### Getting started

```bash
git clone https://github.com/giacomo-ciro/ire.git
cd ire
npm install
```

Run the web frontend only (no desktop shell):

```bash
npm run dev
```

Run the full desktop app:

```bash
npm run tauri -- dev
```

Avoid running `npm run tauri dev` repeatedly as part of routine iteration —
it starts an interactive dev server that can conflict with other running
instances. Prefer the build/check commands below to verify your changes.

## Before Opening a PR

Run these and make sure they succeed with **zero errors and zero warnings**
(a warning usually means dead code or speculative additions that shouldn't
ship):

```bash
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

Turn your task into a verifiable goal before you start: write a failing test
that captures the bug or requirement, then make it pass. "Fix the bug" means
"reproduce it with a test, then make the test green" — not a one-off manual
check.

## Documentation Sync

If your change touches architecture, CLI flags, event names, field names, the
wiki layout, or agent-facing behavior, update `docs/SDD.md` and
`src-tauri/assets/seed/_SYSTEM.md` **in the same PR**. These files are the
source of truth for how IRE behaves — letting them drift out of sync makes the
codebase harder to reason about for the next contributor (human or agent).

Do not document MCP tools in `_SYSTEM.md`; they're advertised automatically via
the MCP handshake (see [docs/mcp-tool-discovery.md](docs/mcp-tool-discovery.md)).

## Pull Request Expectations

- Keep changes focused and atomic — one logical change per PR. Don't bundle
  unrelated refactors or formatting changes with a feature or fix.
- Match the existing code style; don't restyle code you didn't otherwise touch.
- Make sure the build verification commands above pass before requesting review.
- Update `docs/SDD.md` / `_SYSTEM.md` in the same PR if your change diverges
  from what they describe (see above).
- Write commit messages in the imperative mood with a concise subject line
  (e.g. "Add experiment status polling", not "Added" or "Adds").
- Reference related issues in the PR description (e.g. `Fixes #123`).

## Reporting Bugs and Requesting Features

Use the issue templates under `.github/ISSUE_TEMPLATE/`. For security
vulnerabilities, see [SECURITY.md](SECURITY.md) instead of opening a public
issue.

## Code of Conduct

This project follows the [Contributor Covenant Code of Conduct](CODE_OF_CONDUCT.md).
By participating, you agree to abide by its terms.
