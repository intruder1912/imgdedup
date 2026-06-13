# CLAUDE.md — Master AI Context for `imgdedup`

This file is the entry point for any AI agent working in this repository. Read it
in full before doing anything. It encodes how the project is run; the linked
documents are authoritative for the details.

## 1. What we are building

`imgdedup` is a cross-platform CLI tool for finding and removing duplicate and
near-duplicate images. The product requirements are developed in
[`docs/prd/`](docs/prd/); architecture and technology choices in
[`docs/adr/`](docs/adr/). **Do not invent product scope or pick a tech stack —
those are defined through PRDs and ADRs first.**

## 2. The single source of truth is the repository

All planning, status, decisions, and process live **in this repo as Markdown**.
Never rely on chat/session memory for project state. To know where things stand,
read [`docs/planning/status.md`](docs/planning/status.md). To know what to do
next, read [`docs/planning/backlog.md`](docs/planning/backlog.md) and the task
files in [`docs/planning/tasks/`](docs/planning/tasks/).

When you make progress, **update these files in the same PR** so the next agent
or human can resume from the repo alone.

## 3. The agent crew

Work is divided across four roles. Each has a subagent definition in
[`.claude/agents/`](.claude/agents/) and a human-readable description in
[`docs/process/crew.md`](docs/process/crew.md).

| Role | Subagent | Primary responsibility |
|---|---|---|
| Product Owner | `product-owner` | Requirements, PRDs, scope, acceptance criteria |
| Architect / Reviewer | `architect` | ADRs, design, and **code review** on every PR |
| Implementer | `implementer` | Production code, under TDD |
| Tester | `tester` | Test strategy, test code, coverage, verification |

The **human owner is the only one who merges PRs** (see §5).

## 4. Non-negotiable working rules

1. **Brainstorm before building.** For any new feature, PRD, or ADR, explore
   intent and requirements first. **Grill the user on gaps and hidden
   assumptions — never assume.** This applies to product *and* process work.
2. **Test-Driven Development is mandatory** for product code: write a failing
   test, make it pass, refactor. See [`docs/process/sdlc.md`](docs/process/sdlc.md).
3. **Security gate before every merge:** no secrets in code, dependency and
   secret scanning must pass, and the architect reviews for security. See
   [`SECURITY.md`](SECURITY.md) and [`docs/adr/0005-mandatory-tdd-and-security-gate.md`](docs/adr/0005-mandatory-tdd-and-security-gate.md).
4. **Always run local validation and fix all findings BEFORE pushing / opening a
   PR.** Use `make validate` (or the scripts in [`scripts/`](scripts/)). Never
   push work that fails local checks.
5. **Conventional Commits** for every commit and PR title. **Semantic
   Versioning.** Releases are automated. See
   [`docs/process/commits.md`](docs/process/commits.md) and
   [`docs/process/release.md`](docs/process/release.md).
6. **One task = one branch = one focused PR.** Choose commit granularity to match
   the task chunk. Branch naming and flow: [`docs/process/branching.md`](docs/process/branching.md).
7. **Tooling: `git` and `gh` CLI only. No MCP** for git/GitHub operations.

## 5. The development cycle (summary)

Full detail in [`docs/process/sdlc.md`](docs/process/sdlc.md). In short:

```
pick task → branch (feature/…) → brainstorm/design if needed
  → TDD implement → local validation (make validate) → fix findings
  → commit (Conventional) → push → open PR (conventional title + description)
  → CI must pass → architect-agent review (comments) → HUMAN OWNER merges (squash)
  → branch auto-deletes → update planning docs
```

`main` is protected: PRs required, CI checks required, linear history, **0
required approvals** (the human owner is the gate and merges; this avoids the
self-approval deadlock — see
[`docs/adr/0003-main-branch-protection-and-merge-model.md`](docs/adr/0003-main-branch-protection-and-merge-model.md)).

## 6. Repository map

- `CLAUDE.md` — you are here.
- `.claude/agents/` — subagent role definitions.
- `docs/process/` — SDLC, branching, commits, release, crew.
- `docs/planning/` — roadmap, backlog, status, per-task specs.
- `docs/prd/` — product requirements documents (+ template).
- `docs/adr/` — architecture decision records (+ template).
- `.github/` — CI/release/security workflows, Dependabot, PR template, CODEOWNERS.

## 7. When in doubt

Ask the human owner. Prefer a clarifying question over an assumption. If a
decision has lasting impact, capture it in an ADR (architecture/process) or PRD
(product) so it survives beyond the session.
