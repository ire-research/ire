# IRE — Intelligent Research Environment

You are Claude Code running inside **IRE**, a desktop research OS. IRE gives you a persistent, structured wiki at `.ire/wiki/` that survives across sessions. Read it before reasoning from scratch.

## Wiki Layout

```
.ire/wiki/
  _schema.md            — conventions for writing wiki files (read before any wiki.write)
  _index.md             — catalog of all wiki files (auto-regenerated)
  notes.md              — user's running notes
  ideas.json            — user's running ideas
  pulse/
    RESEARCH-QUESTION.md — current research question
    THIS-WEEK.md         — this week's focus
  status/
    long-term.md        — architectural decisions and durable insights
    failures.md         — approaches that did not work (rejection memory)
    short-term/         — daily agent notes (YYYY-MM-DD.md)
  resources/            — per-resource summaries ingested from URLs
```

## Rules

1. **Read before reasoning.** Call `wiki.read` on relevant files before working from memory.
2. **Check `failures.md` first.** If an approach is listed there, don't propose it. Add to it after any dead end.
3. **Persist knowledge immediately.** Use `wiki.write` after any decision, discovery, or pivot.
4. **Update `pulse/RESEARCH-QUESTION.md` and `pulse/THIS-WEEK.md`** when the research question or weekly focus changes.
5. **Write to `long-term.md`** after architectural decisions, pivots, or "this is the approach we settled on" moments.
6. **Write to `short-term/YYYY-MM-DD.md`** for daily operational notes: experiment status, debugging steps, observations. Only today and yesterday are auto-injected — promote anything still relevant to `long-term.md` before it ages out.
7. **Record dead ends in `failures.md`** via `memory.record_failure`. These entries are always injected — consult them before proposing an approach. Add to them after any dead end.
8. **Wiki writes go through IRE MCP tools — always.** For any file under `.ire/wiki/`, use the IRE MCP wiki/memory/pulse tools. Never use built-in `Write`, `Edit`, or `MultiEdit` on `.ire/wiki/` paths — those bypass atomic writes, index regeneration, commit hooks, and UI live-update; the user will not see your changes until they restart the app. Built-in read tools (`Read`, `Grep`, `Glob`) on wiki paths are fine. Built-in `Write`/`Edit` remain available for the user's source code outside `.ire/wiki/`.
