# Integrated Research Environment (IRE)

## Functional Requirements

### Must

- As a user, I want to launch the IRE with one click.
- As a user, I want my previous environment (virtualenv, open papers, and active experiment) to be restored instantly when launching the IRE.
- As a user, I want a "Project Pulse" dashboard showing recent training runs, pending tasks, and a "Reading List" of papers.
- As a user, I want the IRE to be project-centric: each project maps one-to-one with a directory that stores all project data (reference papers, experiments, research questions).
- As a user, I want a constant overview of the current project state: the research question, current blockers, goals, and ongoing attempts.
- As a user, I want a banner clearly stating the current problem I'm tackling to maintain focus and prevent goal drift.
- As a user, I want to change the current problem, either after solving it or when pivoting research direction.
- As a user, I want to reference a public paper via a link (e.g., arXiv) and receive a contextual review: Is it useful? Does it solve related problems? Can we leverage insights? Is it a starting point? The system then summarizes the paper in a concise Markdown file and indexes it.
- As a user, I want a brainstorm feature for context-aware chat sessions where I can pose questions and reason with the AI, which references relevant codebase sections and indexed papers, suggesting limitations, ideas, and next steps.
- As a user, I want to quickly save notes and discussion points for team meetings.
- As a developer, I want the app to function fully locally with no server (only local LLM API orchestration).
- As a developer, I want user knowledge organized as an [LLM wiki](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f).

### In the future

- As a user, I want a "debug pipeline" feature to inspect each step of the pipeline and verify that inputs and outputs match expectations.
- As a user, I want to generate a paper draft in one click based on the current codebase and project state. The AI should ask clarifying questions about narrative, key results, and framing, then generate a LaTeX draft with the correct template for export to Overleaf. Templates are downloaded from a remote directory.
- As a user, I want a `/init` skill to initialize the `.ire/` from an already existing project repo (walk through code, summarize, save, create graph etc.)

