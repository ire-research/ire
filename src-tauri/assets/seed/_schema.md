# Wiki Schema

Conventions for the agent reading and writing this wiki.

## Paths

- Every page has a stable path. Renames go through `wiki.rename`.
- `_index.md` is the canonical catalog. It is regenerated automatically — do not edit it directly.

## Frontmatter

Optional but preferred for structured pages:

```yaml
---
title: <human title>
type: summary | entity | concept | comparison | meta
sources: [path/to/raw/source.pdf, ...]
updated: YYYY-MM-DD
summary: <one-line summary used by _index.md>
---
```

## Status pages (`status/`)

- `pulse.md` — the current research question, the blocker, and the focus.
- `long-term.md` — architectural decisions, pivots, "this is the approach we settled on".
- `failures.md` — structured "what didn't work". Always re-injected so dead-end methods are not re-proposed.
- `short-term/YYYY-MM-DD.md` — daily operational notes. Only the last two day-files are auto-injected; promote anything still relevant to `long-term.md`.

## Resources (`resources/`)

One file per accepted ingest job. A resource may summarize one source or synthesize multiple URLs/local files into one document. Each file holds a structured summary, links to existing pages, and a relevance assessment.
