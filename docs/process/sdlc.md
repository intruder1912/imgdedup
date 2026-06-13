# Software Development Life Cycle

This is the authoritative description of how work flows in `imgdedup`. It applies
to humans and AI agents alike. The goal: high quality, security, and traceability,
with the **repository as the single source of truth** so anyone can resume work
from the repo content alone.

## Principles

- **Brainstorm before building.** Clarify intent and requirements first. Surface
  gaps and hidden assumptions; never assume — grill the owner where unclear.
- **Decisions are documented.** Product decisions → PRDs; architecture/process
  decisions → ADRs. Nothing important lives only in a chat session.
- **Test-Driven Development is mandatory** for product code.
- **Security is a gate**, applied to every change.
- **Small, focused changes**, each finished by a sensible Conventional Commit.
- **Local validation before push.** CI is the enforcement boundary, not the first
  place you discover problems.

## The cycle

```
        ┌─────────────────────────────────────────────────────────────┐
        │  Phase work: PRD (product) / ADR (architecture)  — as needed  │
        └───────────────────────────────┬─────────────────────────────┘
                                         │
   pick task (backlog/tasks) ──► branch ──► brainstorm/design (if needed)
        │                                                     │
        │                                                     ▼
        │                                         TDD: failing test → pass → refactor
        │                                                     │
        ▼                                                     ▼
   update status.md (in progress)                 make validate  ──► fix findings
                                                              │
                                                              ▼
                                  Conventional Commit ──► push ──► open PR
                                                              │
                                                              ▼
                              CI required checks pass ──► architect review (comments)
                                                              │
                                                              ▼
                                  HUMAN OWNER merges (squash) ──► branch auto-deletes
                                                              │
                                                              ▼
                                  release automation (semantic-release) on main
```

## Steps in detail

1. **Select work.** Take the next task from
   [`../planning/backlog.md`](../planning/backlog.md) (each non-trivial task has a
   spec in [`../planning/tasks/`](../planning/tasks/)). Mark it *in progress* in
   [`../planning/status.md`](../planning/status.md).
2. **Branch.** From up-to-date `main`, create `feature/<slug>`, `fix/<slug>`,
   `chore/<slug>`, or `docs/<slug>` (see
   [`branching.md`](branching.md)).
3. **Brainstorm / design** if the task introduces new behavior or decisions.
   PRDs and ADRs are themselves produced via this cycle.
4. **Implement under TDD.** Write a failing test, make it pass, refactor. Keep
   units small with clear interfaces.
5. **Validate locally:** `make validate` — Markdown lint, spell check, link
   check, and (once code exists) format check, lint, and the test suite. **Fix
   every finding before pushing.**
6. **Commit** with a Conventional Commit message (see [`commits.md`](commits.md)).
   Choose commit granularity to match the task chunk.
7. **Push and open a PR.** The PR **title must be a valid Conventional Commit**
   (merges are squash-only, so the title becomes the `main` commit and drives the
   release). Fill in the PR template.
8. **CI.** Required checks must pass. Pushing work that fails local validation is
   a process violation.
9. **Review.** The **architect** agent reviews for correctness, design, security,
   tests, and conventions, leaving comments. Address them.
10. **Merge.** The **human owner** is the sole merge gate (squash-merge). See
    [`../adr/0003-main-branch-protection-and-merge-model.md`](../adr/0003-main-branch-protection-and-merge-model.md).
11. **Release.** Qualifying merges to `main` trigger automated SemVer releases
    (see [`release.md`](release.md)).
12. **Close the loop.** Ensure planning docs reflect the new reality (done/next).

## Definition of Done (every change)

- Acceptance criteria met; behavior covered by meaningful tests (TDD).
- `make validate` and CI green; security gate satisfied.
- Conventional Commit / PR title; docs and planning updated.
- Architect review addressed; human owner merged.
