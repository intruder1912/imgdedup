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
| Product Owner | `product-owner` | Requirements, PRDs, scope, acceptance criteria, functional acceptance, and user-facing docs (README + `docs/guide/`, ADR-0012) |
| Architect / Reviewer | `architect` | ADRs, design, and **code review** on every PR by default (narrow, criteria-bound skip exception: ADR-0011 amendment) |
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
6. **One unit of work = one branch = one focused PR.** Classify the unit up front
   as **feature**, **fix**, or **chore** (this sets the cadence — prefix, CI
   weight, review kind). A branch carries everything that unit needs — design,
   TDD code, technical/user docs, **and** the planning/status update — into one
   squash PR; planning/doc updates ride that PR (no separate doc-only PR). Choose
   commit granularity to match the work. **For feature work the unit is a PRD
   slice** — a coherent shippable increment of a PRD (product-owner-carved),
   reviewed in-progress on the branch and opened as one PR when the product-owner
   has accepted its requirements and delivered its docs; `fix`/`chore` keep the
   small one-unit-one-PR cadence. Branch naming and flow:
   [`docs/process/branching.md`](docs/process/branching.md); cadence rationale:
   [`docs/adr/0011-unit-of-work-cadence-and-planning-gate.md`](docs/adr/0011-unit-of-work-cadence-and-planning-gate.md)
   and [`docs/adr/0013-prd-slice-delivery-cadence.md`](docs/adr/0013-prd-slice-delivery-cadence.md).
7. **Tooling: `git` and `gh` CLI only. No MCP** for git/GitHub operations.

## 5. The development cycle (summary)

Full detail in [`docs/process/sdlc.md`](docs/process/sdlc.md). In short:

```
classify unit (feature/fix/chore) → branch (one unit of work)
  → brainstorm/design if needed → produce/iterate artifact (TDD for code),
    incl. tech/user docs + planning update → local validation (make validate)
  → architect review on the branch diff vs main (iterate until converged)
  → hold the PR until converged → open PR ready (conventional title; record review)
  → ci-gate + required checks pass → architect confirms → HUMAN OWNER merges (squash)
  → branch auto-deletes
```

Review comes in two kinds, scaled to the artifact: a **lightweight document
review** for PRDs/ADRs/process docs and a **heavyweight code review** for
implementation. To keep turnaround low the substantive review runs on the
**branch diff vs `main` before the PR is opened** (push freely on the branch — it
triggers no CI; hold the PR until converged); it is recorded on the PR, where a
quick confirmation pass runs before merge. See
[`docs/adr/0011-unit-of-work-cadence-and-planning-gate.md`](docs/adr/0011-unit-of-work-cadence-and-planning-gate.md).

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
- `docs/guide/` — user-facing guide (detailed, per-topic pages; product-owner-owned,
  ADR-0012). General usage lives in the top-level `README.md`.
- `claude-plugin/` — the official end-user Claude Code plugin (skill, slash
  command, operator agent) that wraps the CLI; architect-designed
  (ADR-0016), implementer-built. `docs/guide/using-with-claude.md`
  (product-owner-owned) documents it for users.
- `docs/superpowers/` — brainstormed design specs (`specs/`) and
  implementation plans (`plans/`) produced when a unit of work goes through
  a structured design/planning process before implementation.
- `.github/` — CI/release/security workflows, Dependabot, PR template, CODEOWNERS.

## 7. When in doubt

Ask the human owner. Prefer a clarifying question over an assumption. If a
decision has lasting impact, capture it in an ADR (architecture/process) or PRD
(product) so it survives beyond the session.

**Kaizen.** The orchestrator, product-owner, and architect may each propose
process improvements when they spot material potential — raised as a chore unit
and, if lasting, captured in an ADR (see
[`docs/adr/0011-unit-of-work-cadence-and-planning-gate.md`](docs/adr/0011-unit-of-work-cadence-and-planning-gate.md)).
