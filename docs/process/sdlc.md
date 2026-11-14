# Software Development Life Cycle

This is the authoritative description of how work flows in `imgdedup`. It applies
to humans and AI agents alike. The goal: high quality, security, and traceability,
with nothing held in chat or session memory: **what to build and how** is in
the repository (PRDs, ADRs, process documents), **what is open** is in GitHub
Issues, and **what happened in a unit** is in its pull request
([ADR-0021](../adr/0021-issue-based-planning-for-a-public-repository.md)).

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
- **Review before the PR opens.** For fast turnaround, the architect reviews the
  **branch diff vs `main`** and the author iterates on the branch (pushes are
  CI-free) before the PR is opened; the review is then recorded on the PR (see
  [Review kinds and timing](#review-kinds-and-timing)).

## The cycle

```
        ┌─────────────────────────────────────────────────────────────┐
        │  Phase work: PRD (product) / ADR (architecture)  — as needed  │
        └───────────────────────────────┬─────────────────────────────┘
                                         │
   pick issue (GitHub Issues) ──► branch ──► brainstorm/design (if needed)
        │                                                     │
        │                                                     ▼
        │                                         TDD: failing test → pass → refactor
        │                                                     │
        ▼                                                     ▼
   label the issue `in progress`                  make validate  ──► fix findings
                                                              │
                                                              ▼
                                  architect review (branch diff vs main) ◄─┐
                                                              │           │ revise
                                                              ▼           │
                                              converged?  ──── no ────────┘
                                                              │ yes
                                                              ▼
                                  commits ──► push (CI-free) ──► open PR (ready)
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
                                  release: dispatched manually when wanted
```

## Steps in detail

0. **Classify the unit of work.** Before branching, decide the class —
   **feature**, **fix**, or **chore** — which sets the cadence (branch prefix,
   expected lifetime, what rides along, CI weight, review kind). See the
   classification table below and
   [ADR-0011](../adr/0011-unit-of-work-cadence-and-planning-gate.md). A
   **branch = one unit of work**: it carries everything that unit needs —
   design (PRD/ADR if applicable), TDD code, technical and user docs — into
   a **single squash PR** that closes the unit's issue.

   | Class | Branch prefix | Lifetime / commits | Rides along | CI weight | Review kind |
   |---|---|---|---|---|---|
   | **feature** | `feature/…` | a **PRD slice** (see below): longer-lived, many commits | design + TDD code + technical docs (implementer) + user docs (product-owner: README + `docs/guide/`) | full CI | in-progress reviews + a heavyweight whole-slice review |
   | **fix** | `fix/…` | short, few commits | failing test + fix + doc touch-ups | full CI | scoped code review |
   | **chore** | `chore/`, `docs/`, `refactor/`, `test/` | process changes, standalone tech/user doc edits, standalone ADRs/PRDs | the doc/process edit itself | docs-only units skip the Rust matrix | lightweight document review |

   Rule of thumb: docs that **accompany** a feature ride the feature branch;
   **standalone** doc work is a chore unit. For **feature** work the unit
   is a **PRD slice**, not a single task — see
   [PRD-slice delivery](#prd-slice-delivery-feature-work) below and
   [ADR-0013](../adr/0013-prd-slice-delivery-cadence.md); `fix`/`chore` keep the
   one-unit-one-PR cadence.

1. **Select work.** Take an accepted issue from
   [GitHub Issues](https://github.com/intruder1912/imgdedup/issues)
   (`gh issue list --label accepted`), or open one for work that has none.
   An issue you open for your own work gets its class label (`feature`,
   `fix` or `chore`) and `accepted`, plus `needs-prd` or `needs-adr` when
   requirements or a design must come first; the PR that lands the PRD or ADR
   takes that label off. Put the `in progress` label on the issue when work
   starts, and take `needs-triage` off if it is still there. There is no
   planning file to update:
   the unit's PR closes the issue, and the PR description is the unit's record
   ([ADR-0021](../adr/0021-issue-based-planning-for-a-public-repository.md)).
2. **Branch.** From up-to-date `main`, create `feature/<slug>`, `fix/<slug>`,
   `chore/<slug>`, or `docs/<slug>` (see
   [`branching.md`](branching.md)).
3. **Brainstorm / design** if the task introduces new behavior or decisions.
   PRDs and ADRs are themselves produced via this cycle.
4. **Implement under TDD.** Write a failing test, make it pass, refactor. Keep
   units small with clear interfaces.
5. **Validate locally:** `make validate` — Markdown lint, spell check, link
   check, shell-script lint (`shellcheck`), GitHub Actions workflow lint
   (`actionlint`), and (once code exists) format check, lint, and the test
   suite. **Fix every finding before pushing.**
6. **Review before the PR opens.** Because a unit now spans **many commits** on
   the branch, the architect reviews the **branch diff vs `main` before the PR is
   opened** (see [Review kinds and timing](#review-kinds-and-timing) below); the
   author iterates on the branch (pushes are CI-free) and repeats until it
   converges. This keeps refinement off the CI cycle. **Hold the PR:** pushing to a
   topic branch triggers no CI (CI fires only on `pull_request` and push-to-`main`)
   — work and push freely, and open the PR only once the unit is converged **and**
   architect-reviewed.
7. **Commit** — as the unit progresses — with Conventional Commit messages (see
   [`commits.md`](commits.md)). Choose commit granularity to match the work; the
   per-commit messages collapse into the squash, so only the **PR title** drives
   the release.
8. **Push and open a PR.** The PR opens in **ready** state (not draft); a green
   `ci-gate` plus a confirmation pass are the merge gate. The PR **title must be a
   valid Conventional Commit**
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
12. **Release.** Merging publishes nothing. A SemVer release is cut **on
    demand** by dispatching the release workflow against `main`
    (`gh workflow run release.yml --ref main`), which refuses any commit whose
    required checks are not green and then releases everything accumulated
    since the last tag (see [`release.md`](release.md)).
13. **Close the loop.** The merged PR closes its issue. Open follow-up work as
    new issues. If a unit is paused or dropped, take `in progress` off its
    issue.

## PRD-slice delivery (feature work)

For **feature delivery within a PRD**, the unit of work is a **coherent shippable
slice** of the PRD — an increment that is **end-to-end-validatable** and
**independently mergeable/releasable**, larger than a single task but smaller than
the whole PRD ([ADR-0013](../adr/0013-prd-slice-delivery-cadence.md)). `fix`,
`chore`, and standalone doc/ADR units keep the one-unit-one-PR cadence above; the
two models coexist.

- **Carving slices.** The **product-owner** groups a PRD's tasks into slices, one
  issue per slice (owning scope + acceptance); the **architect** sanity-checks the
  boundaries for design coherence.
- **One slice = one branch = one squash PR**, built with granular, well-chosen
  commits. The branch is **CI-free and PR-less until the slice is complete**;
  rebase on `main` to stay current. Squash-merge is unchanged (ADR-0003/0004): the
  commits are the branch/review narrative, `main` gets one squash per slice, and
  that drives one SemVer release per slice.
- **In-progress review protocol.** The **implementer requests an architect review**
  for **significant or especially risky/complex** changes — its discretion, **not
  per-commit**. The **product-owner and orchestrator may demand** a review of any
  aspect regardless. With no PR open yet, the architect reviews the relevant
  **commit range** and findings go in a per-slice **review log**
  (`docs/planning/reviews/<slice>.md`, see
  [`../planning/reviews/`](../planning/reviews/)). These reviews are
  **advisory/incremental**. Feedback is **worked immediately**; if the implementer
  is unsure whether to fix **now or defer**, it **asks the product-owner to
  decide**, and any **deferral is opened as a GitHub issue** (noted in the
  log) so nothing is lost.
- **PR-open gate.** The PR opens **ready** only when the **product-owner has (1)
  confirmed all functional requirements in the slice are accepted and (2) delivered
  all user-facing docs** (README + `docs/guide/`, ADR-0012), plus the standing DoD
  (`make validate` green, implementer technical docs, the PR description
  records the unit and closes its issue).
- **Final review + merge.** The architect runs a **full whole-slice review** on the
  opened PR (resolvable threads), **building on the in-progress log** (already-
  reviewed chunks re-confirmed, not re-reviewed wholesale). Then the usual gate
  holds (ADR-0003/0006): all threads resolved **and** required checks green — the
  **3-OS matrix runs here for the first time** — tester verifies fixes under TDD,
  the **human owner squash-merges**. Residual risk: cross-platform CI signal arrives
  at slice end; contained by bounded slice size, portable-code discipline, and the
  stage-two loop.

## Review kinds and timing

The architect performs **two distinct kinds of review**, scaled to the artifact:

- **Document review (PRDs, ADRs, process docs)** — *lightweight, fast.* Checks
  feasibility, risk-awareness, scope (YAGNI — neither over- nor under-specified),
  testable acceptance criteria, internal consistency, and that security and
  edge-case *considerations* are acknowledged. No code is involved.
- **Code review (implementation)** — *heavyweight.* Checks correctness and logic,
  security vulnerabilities, TDD/coverage and meaningful assertions, design and
  unit boundaries, and convention adherence.

**Review before the PR opens (low-turnaround loop).** To keep iteration fast —
and avoid a CI cycle per revision — the review runs on the **branch diff vs
`main`** (`git diff main`): author drafts → `make validate` → architect reviews
the diff → author revises → repeat on the branch (pushes are CI-free) until it
converges, then the PR is opened **ready** (not draft). A green `ci-gate` plus a
confirmation pass are the merge gate. This is the default for documents and small
changes; code changes where CI signal is itself part of the review (long diffs,
new dependencies — anywhere green checks materially inform the review) may instead
iterate on the PR. Either way:

- The architect's review is **recorded on the PR** when it opens (the review is
  posted there), so "the architect reviewed the PR" is visible from the PR
  alone, and a quick confirmation pass runs on the PR before merge. (The one
  exception — units where the dispatch is skipped outright — is immediately
  below; those record the orchestrator's direct verification instead.)
- The **human owner remains the sole merge gate** (ADR-0003); 0 approvals are
  required and the architect never blocks via a GitHub approval.

**Skipping the review entirely.** For a narrow class of unit — a docs-only
planning sync, or a low-risk `contents: read` CI-tooling chore whose
correctness the orchestrator verifies directly (running the tool locally,
querying a live API, re-running CI) rather than asserting — the architect
dispatch may be skipped, with that verification recorded in the PR body in
its place. This never applies to product code changes or to anything holding
write permissions or touching a secret. See the
[2026-08-02 amendment to ADR-0011](../adr/0011-unit-of-work-cadence-and-planning-gate.md#amendment-2026-08-02--architect-review-skip-exception-for-verified-low-risk-units)
for the exact criteria.

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
- Conventional Commit / PR title; docs updated; the PR closes its issue.
- **Technical docs** (implementer — rustdoc on public items / `docs/` design
  notes) and **user-facing docs** (product-owner — **general usage in `README.md`**,
  **detailed per-topic pages in `docs/guide/`**) updated with the behavior they
  describe; not deferred to a later doc PR. See
  [ADR-0012](../adr/0012-documentation-ownership-and-placement.md).
- For a **feature (PRD slice)**: the product-owner has **accepted all the slice's
  functional requirements** and delivered its user docs before the PR opened, and
  any in-progress-review deferrals were opened as GitHub issues
  ([ADR-0013](../adr/0013-prd-slice-delivery-cadence.md)).
- All review threads resolved and required checks green (the ADR-0003 gate);
  human owner merged.
