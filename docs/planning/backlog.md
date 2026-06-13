# Backlog

Fine-grained, repo-tracked work items. This is the canonical to-do list. Each
non-trivial task gets a spec file in [`tasks/`](tasks/) named `TASK-XXXX-<slug>.md`.
Status mirrors [`status.md`](status.md).

## Columns

- **ID** — `TASK-XXXX`, sequential.
- **Task** — short description (links to the task file when one exists).
- **Phase** — roadmap phase.
- **Role** — primary owner role (`product-owner`, `architect`, `implementer`, `tester`).
- **Status** — `todo` / `in-progress` / `blocked` / `done`.
- **Parallel** — `yes` if it can be worked concurrently with other `yes` tasks
  (disjoint files, no ordering dependency); `no` otherwise.
- **Depends on** — task IDs that must complete first.

## Phase 0 — Foundation

| ID | Task | Role | Status | Parallel | Depends on |
|---|---|---|---|---|---|
| TASK-0001 | Bootstrap repo foundation (docs, crew, structures, configs) | architect | in-progress | no | — |
| TASK-0002 | Create private GitHub repo and push `main` | architect | todo | no | TASK-0001 |
| TASK-0003 | Configure branch protection + squash-only merge settings | architect | todo | no | TASK-0002 |
| TASK-0004 | CI skeleton workflow (md-lint, spell, links, commitlint, structure) | architect | todo | no | TASK-0001 |
| TASK-0005 | Dependabot + security scan + release skeleton (dry-run) | architect | todo | no | TASK-0001 |
| TASK-0006 | SDLC smoke-test PR (prove review + merge end-to-end) | architect | todo | no | TASK-0003, TASK-0004, TASK-0005 |
| TASK-0007 | Phase 0 wrap-up: mark foundation complete, queue Phase 1 | architect | todo | no | TASK-0006 |

## Phase 1 — Product definition

| ID | Task | Role | Status | Parallel | Depends on |
|---|---|---|---|---|---|
| TASK-0101 | Brainstorm + author PRD-0001 (dedup scope, UX, safety) | product-owner | todo | no | TASK-0007 |

> Further Phase 1+ tasks are added as PRDs and ADRs are written. Keep this table
> and the `tasks/` specs in sync with reality in every PR.
