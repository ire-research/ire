# Experiment Lifecycle v2 (proposal)

Not yet decided or built. This is a design proposal, not a description of the current
system — see [docs/architecture/experiments.md](../architecture/experiments.md) for
what exists today.

## 1. Start

Agent calls `experiment.start` in chat, as today — same subprocess `command`.

New: instead of (or alongside) writing state only to `ire.json`/SQLite, IRE creates a
new **wiki-tracked** folder:

```
.ire/experiments/<progressive_id>-<slugified-title>/
  EXPERIMENT.md   — the one-pager
  ...             — room for whatever else belongs with this run:
                    scripts, result files, other relevant artifacts
```

`EXPERIMENT.md` holds what used to be the standalone `wake_prompt` string — the goal,
the context, what's being tested and why — plus the mechanical facts (command,
started_at). It's the one page that makes this experiment legible on its own.

Why git-track it: today's experiment record (`.ire/cache/experiments/<uuid>/`) is
gitignored — local-only, ephemeral. Experiments start to live in the wiki too, but as
a structured entity inside it that can be git-tracked, so they become part of the
shareable project history instead of vanishing if the cache is cleared or the machine
changes.

## 2. Run & finish

Same detached-subprocess mechanics as today — spawn, monitor, tail logs. Logs stay in
`.ire/cache/experiments/<uuid>/`, exactly as today; they are not moved into the wiki
folder (see the open question below).

On completion: no automatic agent turn. Status updates as usual; the experiment
becomes **unclaimed** in the UI. A popup asks: open in a new chat, open in an existing
chat, or dismiss (meaning "I'll see it later").

## 3. After

The user can:
- **Read it directly** — open `EXPERIMENT.md` (and its folder) in any editor, no app
  needed.
- **Resolve it** — mark it handled without starting a chat.
- **Chat about it** — open a new or existing agent session; the agent reads
  `EXPERIMENT.md` cold and has everything it needs, no other context required.

## Open question: how many places does an experiment live in?

Today, state is split across:
1. SQLite `local.db` — operational fields (pid, working_dir, session id, tab id)
2. `ire.json` — git-tracked display subset
3. `.ire/cache/experiments/<uuid>/` — raw logs, gitignored, local-only

This proposal adds a wiki-tracked `.ire/experiments/<id>-<title>/` folder as a genuine
4th place: it holds the one-pager and any other artifacts worth tracking, while raw
logs stay in `.ire/cache/` as today. Worth revisiting later whether that split (curated
summary in the wiki, raw logs in local cache) is the right one, or whether logs should
eventually move alongside it too.
