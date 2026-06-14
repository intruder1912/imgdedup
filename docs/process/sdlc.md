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
- **Review before commit.** For fast turnaround, the architect reviews the
  *uncommitted* working-tree diff and the author iterates before a single commit
  lands; the review is then recorded on the PR (see
  [Review kinds and timing](#review-kinds-and-timing)).

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
                                  architect review (working-tree diff) ◄──┐
                                                              │           │ revise
                                                              ▼           │
                                              converged?  ──── no ────────┘
                                                              │ yes
                                                              ▼
                                  Conventional Commit ──► push ──► open PR
                                                              │   (review recorded as threads)
                                                              ▼
           ┌──────── stage-two loop (orchestrator-driven) ───────┐◄────────┐
           │  collect findings via gh: CI checks + review threads │         │
           │  re-dispatch implementer/tester → fix → make validate│         │
           │  → push → resolve threads → architect re-review      │         │ no
           └──────────────────────────┬──────────────────────────┘         │
                                       ▼                                    │
                        checks green & all threads resolved?  ─────────────-┘
                                       │ yes
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
6. **Review before commit.** Hand the *uncommitted* working-tree diff to the
   **architect** for review (see [Review kinds and timing](#review-kinds-and-timing)
   below), revise, and repeat until it converges. This keeps refinement off the
   commit/CI cycle.
7. **Commit** — once the review loop has converged — with a Conventional Commit
   message (see [`commits.md`](commits.md)). Choose commit granularity to match
   the task chunk.
8. **Push and open a PR.** The PR **title must be a valid Conventional Commit**
   (merges are squash-only, so the title becomes the `main` commit and drives the
   release). Fill in the PR template. **Record the architect's review on the PR**:
   a short prose summary as the overview, plus any substantive finding as its own
   **resolvable review thread** (not a single prose blob) so each can be tracked to
   resolution.
9. **Collect findings.** The **orchestrator** (the main session driving the work)
   gathers the PR's open findings: CI failures via `gh pr checks <n>` and
   `gh run view <id> --log-failed`, and architect review threads via
   `gh pr view <n> --comments` / `gh api repos/{owner}/{repo}/pulls/<n>/comments`.
10. **Iterate until converged (stage-two loop).** The orchestrator re-dispatches
    the **implementer** / **tester** with the concrete findings → fix under TDD →
    `make validate` → push → **resolve the corresponding threads** → the
    **architect re-reviews** the new diff. **Repeat until required checks are green
    and all review conversations are resolved** — the ADR-0003 merge gate. Apply
    `receiving-code-review` discipline: verify each finding and push back when it is
    wrong, rather than blind-complying. Keep this loop *lightweight* — stage one
    (step 6) front-loads the substantive review; stage two is the remainder
    (CI-only failures and confirmation-pass nits), not a second heavyweight review.
    See [ADR-0006](../adr/0006-post-pr-review-iteration-loop.md).
11. **Merge.** The **human owner** is the sole merge gate (squash-merge). See
    [`../adr/0003-main-branch-protection-and-merge-model.md`](../adr/0003-main-branch-protection-and-merge-model.md).
12. **Release.** Qualifying merges to `main` trigger automated SemVer releases
    (see [`release.md`](release.md)).
13. **Close the loop.** Ensure planning docs reflect the new reality (done/next).

## Review kinds and timing

The architect performs **two distinct kinds of review**, scaled to the artifact:

- **Document review (PRDs, ADRs, process docs)** — *lightweight, fast.* Checks
  feasibility, risk-awareness, scope (YAGNI — neither over- nor under-specified),
  testable acceptance criteria, internal consistency, and that security and
  edge-case *considerations* are acknowledged. No code is involved.
- **Code review (implementation)** — *heavyweight.* Checks correctness and logic,
  security vulnerabilities, TDD/coverage and meaningful assertions, design and
  unit boundaries, and convention adherence.

**Review before commit (low-turnaround loop).** To keep iteration fast — and
avoid a commit/push/CI cycle per revision — the review runs on the *uncommitted*
working-tree diff (`git diff main`): author drafts → `make validate` → architect
reviews the diff → author revises → repeat until it converges, then a **single
commit** lands and the PR is opened. This is the default for documents and small
changes; code changes where CI signal is itself part of the review (long diffs,
new dependencies — anywhere green checks materially inform the review) may instead
iterate on the PR. Either way:

- The architect's review is **recorded on the PR** when it opens (the review is
  posted there), so "the architect reviewed every PR" holds, and a quick
  confirmation pass runs on the PR before merge.
- The **human owner remains the sole merge gate** (ADR-0003); 0 approvals are
  required and the architect never blocks via a GitHub approval.

**Stage-two loop (post-PR, lightweight).** Once the PR is open, any remaining
findings — CI-only failures plus confirmation-pass nits — flow through a small,
**orchestrator-driven** loop (steps 9–10): the orchestrator collects findings via
`gh`, re-dispatches the implementer/tester to fix and push, the architect
re-reviews, and this repeats until **required checks are green and all review
conversations are resolved**. That conversation-resolution gate (ADR-0003) — not a
GitHub approval — is the convergence criterion; the architect still never blocks
via an approval. Substantive findings are therefore posted as **resolvable review
threads**, not a single prose comment. See
[ADR-0006](../adr/0006-post-pr-review-iteration-loop.md).

## Definition of Done (every change)

- Acceptance criteria met; behavior covered by meaningful tests (TDD).
- `make validate` and CI green; security gate satisfied.
- Conventional Commit / PR title; docs and planning updated.
- All review threads resolved and required checks green (the ADR-0003 gate);
  human owner merged.
