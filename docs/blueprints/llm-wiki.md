# Implementation Blueprint: LLM Wiki

> Source: [karpathy/442a6bf555914893e9891c11519de94f](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f)

---

## 1. Technical Stack & Requirements

* **Core Dependencies:**
  * Any LLM agent with file-editing capability (Claude Code, OpenAI Codex, OpenCode/Pi, etc.)
  * Markdown editor — Obsidian strongly recommended (graph view, Dataview, Marp plugin)
  * Optional search engine: [`qmd`](https://github.com/tobi/qmd) — local hybrid BM25/vector search + LLM re-ranking with CLI and MCP interfaces
  * Optional browser extension: Obsidian Web Clipper (article-to-markdown conversion)
  * Git (for version history, branching, and collaboration)

* **Environmental Prerequisites:**
  * A schema file at the wiki root (`AGENTS.md` for Codex, `CLAUDE.md` for Claude Code) defining structure, conventions, and LLM workflows
  * Obsidian configured with "Attachment folder path" pointing to `raw/assets/` if image handling is needed
  * No embedding infrastructure required at small-to-medium scale — the `index.md` file is sufficient for navigation

---

## 2. Architecture Map

* **Entry Point:** `schema/AGENTS.md` (or `CLAUDE.md`) — the configuration document the LLM reads at session start to understand the wiki's structure, conventions, and what to do on ingest/query/lint

* **Business Logic:** LLM agent operations
  * **Ingest** — reads a new source, extracts key information, updates 10–15 wiki pages, appends to `log.md`
  * **Query** — reads `index.md` to find relevant pages, synthesizes an answer, optionally files the answer back as a new wiki page
  * **Lint** — health-checks the wiki for contradictions, orphan pages, stale claims, missing cross-references, and data gaps

* **Data Model:** Three-layer structure

  | Layer | Owner | Mutability | Description |
  |---|---|---|---|
  | `raw/` | Human | Immutable | Source documents (articles, papers, images). Source of truth. |
  | `wiki/` | LLM | Mutable | Markdown pages: summaries, entity pages, concept pages, comparisons, synthesis |
  | Schema | Human + LLM | Co-evolved | `AGENTS.md` / `CLAUDE.md` defining conventions and workflows |

* **Supporting files:**
  * `wiki/index.md` — content-oriented catalog; each page listed with link, one-line summary, and optional metadata (category, source count, date). LLM reads this first on every query.
  * `wiki/log.md` — append-only chronological record of all ingests, queries, and lint passes. Each entry prefixed consistently, e.g.:
    ```
    ## [2026-05-03] ingest | Article Title
    ```
    Parseable with unix tools: `grep "^## \[" log.md | tail -5`

---

## 3. Step-by-Step Reproduction

1. **Setup — Create directory structure**
   ```
   my-wiki/
   ├── AGENTS.md          ← schema / LLM instructions
   ├── raw/               ← immutable sources
   │   └── assets/        ← locally downloaded images
   ├── wiki/
   │   ├── index.md       ← content catalog
   │   ├── log.md         ← chronological operation log
   │   └── ...            ← entity/concept/summary pages (LLM-generated)
   └── .git/
   ```

2. **Define the schema (`AGENTS.md`)**

   Co-write with the LLM. Minimum sections to define:
   * Directory layout and file naming conventions
   * Page types: what is a "summary page" vs. "entity page" vs. "concept page"
   * Ingest workflow: which pages to always update (index, log, summary), how to handle contradictions
   * Query workflow: how to search (index-first, then drill), which output formats are available (markdown, table, Marp slide)
   * Lint workflow: what to check and what to produce

3. **Ingest a first source**
   * Drop source file into `raw/`
   * Prompt: `"Ingest raw/my-article.md"` — the LLM reads the source, discusses key takeaways, writes a summary page, updates `index.md`, updates any entity/concept pages, appends to `log.md`
   * Review the LLM's edits in Obsidian; guide emphasis via follow-up prompts

4. **Query the wiki**
   * Ask a natural-language question; the LLM reads `index.md`, drills into relevant pages, and synthesizes an answer
   * If the answer is non-trivial (a comparison table, a synthesis, a connection you discovered), tell the LLM to file it as a new wiki page — it compounds into the knowledge base

5. **Lint periodically**
   * Prompt: `"Lint the wiki"` — the LLM checks for contradictions, orphan pages, stale claims superseded by newer sources, missing cross-references, and suggests new questions or sources to investigate

6. **Add search tooling (optional, at scale ~100+ sources)**
   * Install `qmd` and configure it to index `wiki/`
   * Expose as an MCP tool or document its CLI in the schema so the LLM can `qmd search "query"` instead of reading the full index

---

## 4. Critical Logic Constraints

* **Raw sources are immutable.** The LLM reads from `raw/` but never writes to it. Treat it as the ground truth; all LLM synthesis lives in `wiki/`.

* **File good answers back into the wiki.** Query answers that represent genuine synthesis — comparisons, analyses, discovered connections — should not disappear into chat history. Filing them as new pages is what makes the wiki compound over time.

* **Use consistent log prefixes for unix parseability.** The `## [YYYY-MM-DD] operation | title` format lets you `grep` the log without custom tooling. Document the exact format in the schema so the LLM uses it consistently.

* **Co-evolve the schema.** `AGENTS.md` is not a one-time document. As you discover what works for your domain, update the schema and commit it. The LLM's quality as a wiki maintainer is directly proportional to how well the schema documents your conventions.

* **Lint catches semantic debt.** Without periodic linting, contradictions and orphan pages accumulate silently. The lint operation is the equivalent of `pnpm test` — run it regularly, not just when something feels wrong.

* **Scale ceiling for index-based navigation.** At ~100 sources / hundreds of pages, reading `index.md` first is sufficient. Beyond that, the index becomes slow and noisy — invest in `qmd` or equivalent search before hitting that ceiling, not after.

* **The wiki is just a git repo.** Version history, rollback, and diffing come for free. Commit after every ingest session. This also means you can branch the wiki for speculative analyses and merge them back.
