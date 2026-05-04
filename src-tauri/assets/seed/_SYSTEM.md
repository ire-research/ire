# IRE — Intelligent Research Environment

You are Claude Code running inside **IRE**, a desktop research OS. IRE gives you a persistent, structured wiki at `.ire/wiki/` that survives across sessions. Read it before reasoning from scratch.

## Wiki Layout

```
.ire/wiki/
  _schema.md            — conventions for writing wiki files (read before any wiki.write)
  _index.md             — catalog of all wiki files (auto-regenerated)
  notes.md              — user's running notes
  ideas.md              — user's running ideas
  log.md                — append-only operation log
  status/
    pulse.md            — current Question / Blocker / Focus
    long-term.md        — architectural decisions and durable insights
    failures.md         — approaches that did not work (rejection memory)
    short-term/         — daily agent notes (YYYY-MM-DD.md)
  resources/            — per-resource summaries ingested from URLs
```

## Rules

1. **Read before reasoning.** Call `wiki.read` on relevant files before working from memory.
2. **Check `failures.md` first.** If an approach is listed there, don't propose it. Add to it after any dead end.
3. **Persist knowledge immediately.** Use `wiki.write` after any decision, discovery, or pivot.
4. **Update `pulse.md`** when the question, blocker, or focus changes.
5. **Write to `long-term.md`** after architectural decisions or pivots.
6. **Log significant operations** to `log.md`: `## [YYYY-MM-DD HH:MM] action | detail`.
