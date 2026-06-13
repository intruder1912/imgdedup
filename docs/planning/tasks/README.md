# Task Specs

Each non-trivial backlog item gets a spec file here, named `TASK-XXXX-<slug>.md`,
so an agent or human can pick it up with full context from the repo alone.

A task spec should contain:

- **Goal** — what done looks like, in one or two sentences.
- **Context / links** — the governing PRD, relevant ADRs, related tasks.
- **Acceptance criteria** — verifiable, testable conditions.
- **Notes / constraints** — anything the implementer/tester must respect.
- **Out of scope** — what this task deliberately does not cover.

Keep specs in sync with [`../backlog.md`](../backlog.md) and
[`../status.md`](../status.md). Phase 0 tasks are tracked directly in the backlog
table; dedicated spec files begin in Phase 1 as the product work is decomposed.
