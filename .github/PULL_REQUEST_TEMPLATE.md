## What does this PR do?

<!-- One or two sentences. Link the issue it closes, e.g. "Fixes #123". -->

## Checklist

- [ ] `npm run build` passes with no errors
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml` passes
- [ ] `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` is clean
- [ ] `docs/architecture/*` and/or `src-tauri/assets/seed/_SYSTEM.md` updated
      if this PR changes architecture, CLI flags, event names, or
      agent-facing behavior (see
      [CONTRIBUTING.md](../CONTRIBUTING.md#documentation-sync))
- [ ] Changes are focused — no unrelated refactors or formatting bundled in
