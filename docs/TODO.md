# TODOs

This document collects the next step required in the implementation of IRE. Sould be followed exactly.

- [x] ~Implement Phase 0 of `docs/SDD.md`~

- [x] ~Implement Phase 1 of `docs/SDD.md`~

- [x] ~Feat: dark theme by default and add toggle for light~

- [x] ~Feat: restart button in chat with classic arrow icon~

- [x] ~Implement Phase 2 of `docs/SDD.md`~

- [x] ~Implement Phase 3 of `docs/SDD.md`~

- [x] ~Implement Phase 4 of `docs/SDD.md`~

- [x] ~Feat: IRE system prompt in `_SYSTEM.md` with explanation of everything and how to use the `.ire/` folder~

- [x] ~Feat: IRE opens in full screen by default~

- [x] ~Feat: multi tabs chat~

- [x] ~Feat: refactor the frontend style from `docs/blueprints/frontend-style.md` to mimic Conductor and Linear minimal style.~

- [x] ~Feat: instrument the code with comprehensive logging for debug session (I should be able to see from the terminal everything that is happening)~

- [x] ~Implement Phase 5 of `docs/SDD.md`~

- [x] ~Implement Phase 6 of `docs/SDD.md`~

- [x] ~Add `docs/DECISIONS.md` and `docs/CHANGELOG.md` to record non-obvious design changes and visible changes~

- [x] ~Implement Phase 7 of `docs/SDD.md`~

- [x] ~Feat: rendering of html/latex/markdown in chat~

- [x] ~Feat: viewer for the sources (user clicks on the bottom left and it opens in the central panel as markdown editor/preview).~

- [x] ~Feat: remove `log.md` and use `short-term/` to track daily changes. Clearly explain in `_SYSTEM.md` how memory should be handled.~

- [x] ~Refactor: centralize all prompts in a single folder for visibility and maintenance (now some are hardcoded)~

- [x] ~Feat: standard config file in `~/.config/ire/config` to save user preferences (e.g., theme selected, last opened workspace etc.).~

- [x] ~Feat: claude code options menu in user UI (model, thinking, effort)~


- [ ] Feat: seed-prompt update prompt on workspace open. Detect when the bundled seed `_SYSTEM.md` (and `_schema.md`) is newer than the workspace copy and offer to update via a modal. Use a **version marker** strategy: embed `<!-- ire-system-version: N -->` in the seed and bump on every change. On `workspace-ready`, parse the marker from both bundled (`include_str!`) and `.ire/wiki/_SYSTEM.md`; if `disk_version < bundled_version`, fire a modal with the diff and Update / Keep mine buttons. Update writes through `WikiStore::write` so it picks up index regen, `wiki-changed`, and git auto-commit for free. Detect drift (user edits) via a separate hash and warn before overwriting. Tauri commands: `check_seed_updates()`, `apply_seed_update({ path })`.

- [ ] Feat: fetch latex source directly instead of parsing pdf if arXiv link

- [ ] Feat: change assistant name from CLAUDE to IRE everywhere 

- [ ] Feat: onboarding pipeline when creating new workspace (ask questions to populate pulse etc.)

- [ ] Feat: run without CC (all CC-related features return a pop up message "requires CC installed" upon interaction)

- [ ] Feat: separate review (LLM call pipeline) and submit (persist to disk and commit) button for notes / ideas.

- [ ] Feat: prompt engineering on `_SYSTEM.md` to obfuscate claude code personality and force IRE persona.