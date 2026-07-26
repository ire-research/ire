# Changelog

All notable changes to this project are documented in this file.

## [Unreleased]

## [0.1.8] - 2026-07-24
### Added
- OpenCode provider support (#82)

### Changed
- Unified `AgentProvider` trait for Claude Code / Codex adapters, provider-keyed resume-id storage, dynamic status IPC (#80, #81)

### Fixed
- Resume-id persist failures no longer silently drop and freeze the UI — now surfaced as a report-able toast (#83, #84)

## [0.1.7] - 2026-07-21
### Added
- In-app feedback popup (#75)
- Detect Claude/Codex login status instead of just binary presence (#66)
- Opt-in PostHog analytics for launches and session length, with session id/timestamp attached to events (#62, #69)

### Fixed
- Code blocks scroll instead of overflowing (#73)
- Resource cache id shrunk from 64 to 16 hex chars (#74)
- `app_launched` now fires on analytics opt-in, with send failures surfaced (#65)

## [0.1.6] - 2026-07-05
### Added
- Opt-in PostHog analytics for launches and session length (#62)

### Fixed
- Website icon (#63)

## [0.1.5] - 2026-07-05
### Fixed
- CI: Apple signing secrets forwarded to release builds, Rust cache hit on release builds, `contents:write` permission for reusable build workflow (#58, #59, #60)

## [0.1.4] - 2026-07-04
### Fixed
- Update toast restyled and fixed (#57)

## [0.1.3] - 2026-07-04
### Added
- Fable 5 added to available Claude models (#56)

## [0.1.2] - 2026-07-04
### Added
- In-app auto-update via Tauri updater plugin (#55)

## [0.1.1] - 2026-07-02
### Added
- IRE static marketing website (#18)
- Unified "open workspace" and "create workspace" flow (#49)

### Changed
- Replaced the Node-based MCP server with an in-process Rust implementation (#43)
- Chat memory and session id moved into the database (#44)
- Simplified `.ire/` layout and tools (#48)

### Fixed
- Mobile website layout and section-title scrolling (#51, #52)
- System status polling moved off the main thread (#42)

## [0.1.0] - 2026-06-29
Initial release — local-first desktop research environment built on Tauri.
- Persistent, Git-tracked LLM wiki injected into agent context
- Multi-tab chat with Claude Code and Codex support
- Local file resource ingestion and full-view resource editing
- Non-blocking experiment runs with async wake-up on completion
- Structured short-term and long-term memory
- Dark theme UI
