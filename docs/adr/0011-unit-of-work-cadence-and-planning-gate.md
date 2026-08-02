# ADR-0011: Unit-of-work cadence & planning gate

- **Status:** accepted
- **Date:** 2026-06-14
- **Deciders:** architect, human owner
- **Related:** [ADR-0002](0002-in-repo-markdown-as-canonical-planning.md)
  (reaffirmed — planning stays in-repo), [ADR-0003](0003-main-branch-protection-and-merge-model.md)
  (merge model unchanged), [ADR-0005](0005-mandatory-tdd-and-security-gate.md)
  (TDD & security gate), [ADR-0006](0006-post-pr-review-iteration-loop.md)
  (review kinds unchanged; cadence refined), [`../process/sdlc.md`](../process/sdlc.md)

## Context and problem statement

The current SDLC treats **every artifact change as its own branch + PR + CI +
review cycle**. That cadence is uniform regardless of what changed: a trivial
planning status flip (`todo → done`) is forced through the same gate as
production code, including the required **3-OS Rust compile matrix**. A recent
status-only flip cost a full PR with that matrix attached — immense overhead for
a one-line edit.

The root cause is **not** that planning lives in-repo. ADR-0002 stands and is
reaffirmed here: in-repo Markdown remains canonical and we are **not** relocating
artifacts. The defect is that the **planning/design cycle is forced through the
same per-artifact merge cadence and CI gate as code**. The owner has decided to
fix the **cadence and the gate**, not the artifact location.

Assumptions confirmed with the owner: keep artifacts in-repo (no relocation, no
move to Issues); keep squash-only merges and the human owner as sole merge gate
(ADR-0003); the fix is a unit-of-work cadence plus a single restructured CI gate;
record it as an ADR with follow-up doc and CI tasks.

## Decision drivers

- **Kill per-artifact PR overhead** — a one-line planning flip must not drag a
  full code-CI cycle behind it.
- **Resumability from the repo alone** (ADR-0002) — planning/status must stay
  in-repo and travel with the work.
- **Docs stay current** — technical and user docs must land *with* the behavior
  they describe, not in a deferred follow-up PR.
- **Preserve the merge model** — squash-only, human owner as sole gate
  (ADR-0003), and the existing review kinds (ADR-0006).
- **CI proportional to risk** — heavy compilation runs only when code changes;
  docs/chore-only work merges fast.

## Considered options

1. **Fix cadence + gate; keep artifacts in-repo.** Adopt a unit-of-work model
   where one branch carries everything that unit needs (design, code, docs,
   planning) into one squash PR, and restructure CI so the heavy matrix is
   path-filtered behind a single aggregating required check.
2. **Relocate planning to a separate repo / orphan branch** so planning edits
   escape the code CI gate.
3. **Move backlog/status to GitHub Issues + Projects** so they live outside the
   PR/CI flow entirely.

## Decision outcome

**Chosen option 1** — fix the cadence and the gate while keeping all artifacts
in-repo, because it removes the overhead without breaking the repo-canonical
principle (ADR-0002) or the merge model (ADR-0003). Concretely:

### 1. Unit-of-work model

A **branch = one unit of work**, carrying *everything that unit needs* —
brainstorm/design (PRD/ADR if applicable), TDD implementation, technical docs,
user docs, **and** the planning/status update — into a **single squash PR = one
SemVer bump**. The status flip for a task rides in **that task's own PR**; there
is no separate doc-only PR for it. The merge model is **unchanged**: squash-only,
human owner is the sole merge gate (ADR-0003 intact).

### 2. Up-front classification (new SDLC Step 0)

Before branching, the orchestrator **classifies the unit**, which selects the
cadence:

- **feature** (`feature/…`) — longer-lived, many commits; carries design + TDD
  code + technical docs (implementer) + user docs (product-owner) + planning
  update; **full CI**; **heavyweight code review**.
- **fix** (`fix/…`) — short, few commits; failing test + fix + doc touch-ups +
  planning update; **full CI**; **scoped code review**.
- **chore** (`chore/`, `docs/`, `refactor/`, `test/`) — process changes,
  standalone technical/user doc edits, isolated planning/grooming sessions,
  standalone ADRs/PRDs; **docs-only units skip the Rust matrix**; **lightweight
  document review**.

Rule of thumb: docs that **accompany** a feature ride the feature branch;
**standalone** doc/planning work is a chore unit.

### 3. CI cadence — "hold the PR"

Pushing to a topic branch is already CI-free (CI fires only on `pull_request` and
push-to-`main`). The new discipline: **work and push freely on the branch; open
the PR only once the unit is converged AND architect-reviewed.** The PR opens in
**ready** state (not draft), CI runs once, then merge. No draft-gating is needed.

### 4. CI gate restructure (the one engineering change)

Introduce a single required **`ci-gate`** job that **always runs** and
**aggregates** the results of the other jobs (it depends on them). The heavy Rust
3-OS matrix becomes **path-filtered** — it runs only when code paths change.
Doc/planning/chore-only PRs **skip the matrix** and `ci-gate` passes fast.
Branch-protection's required check changes **from** the three
`rust (ubuntu/macos/windows-latest)` checks **to** the single `ci-gate`. This is
an **owner action** (GitHub settings) and is called out as a follow-up. The
`secret-scan`, `markdownlint`, `spellcheck`, `structure`, and `pr-title` checks
**remain**.

### 5. Docs folded into the dev lifecycle (Definition of Done)

The **implementer** must add/update **technical docs** (rustdoc / `docs/` design
notes) as part of "done," reviewed as code. The **product-owner** must add/update
**user-facing docs** (CLI/usage guide, README) for any user-visible behavior,
riding the feature's PR. Docs are **not** deferred to a later doc PR.

### 6. Lightweight planning updates

`status.md` / `backlog.md` are updated **inside the unit's own PR** (a free ride).
Cross-cutting grooming not tied to a feature is a **chore planning unit** — a
small, docs-only branch that **skips the Rust matrix** and gets a fast document
review.

### 7. Review adaptation

Because a branch now holds **many commits**, the prior "review the uncommitted
diff before the single commit" step becomes: the architect reviews the **branch
diff vs `main` BEFORE the PR is opened** (still pre-PR, off the CI cycle); the
author iterates; then the PR opens **ready**. A quick confirmation pass + a green
`ci-gate` are the merge gate. ADR-0003 is unchanged (human owner is sole merge
gate); the document-vs-code review kinds (ADR-0006) are unchanged.

### 8. Process self-improvement (kaizen)

The orchestrator, product-owner, and architect may **each** propose process
changes when they spot material improvement potential — raised as a **chore
unit** and, if lasting, captured in an ADR. **This ADR is the first such
instance.**

### Consequences

- Positive: eliminates per-artifact PR overhead; a planning status flip rides
  free in its task's own PR; technical and user docs stay current by Definition of
  Done; standalone doc/chore units skip the heavy matrix and merge fast; CI cost
  becomes proportional to risk; the repo-canonical principle (ADR-0002) and the
  merge model (ADR-0003) are both preserved.
- Negative / trade-offs: longer-lived branches risk **drift from `main`** —
  mitigate by rebasing; the model relies on **discipline** to "hold the PR" and to
  **classify correctly** up front; the `ci-gate` path-filter must be built
  carefully so a required check **never hangs as `pending`** on skipped matrix
  legs (a required check that is conditionally skipped must still report a
  conclusion) — this is an implementation detail for the follow-up task.
- Follow-ups / new tasks created:
  - **CI workflow change** — add the aggregating `ci-gate` job; path-filter the
    Rust 3-OS matrix; ensure skipped legs still resolve `ci-gate`.
  - **Owner branch-protection update** — switch the required check from the three
    `rust (…-latest)` checks to the single `ci-gate` (GitHub settings).
  - **Process-doc edits** — `sdlc.md` (Step 0 classification, hold-the-PR,
    pre-PR branch review), `branching.md`, `commits.md`, `crew.md`, the agent
    definitions, and `CLAUDE.md`.
  - **Tracking backlog TASK** — capture and sequence the above in
    `docs/planning/`.

## Pros and cons of the options

### Option 1 — Fix cadence + gate, keep artifacts in-repo (chosen)

- Good: removes the overhead at its root (cadence + CI), not its symptom; keeps a
  single clone as the whole source of truth; preserves ADR-0002 and ADR-0003;
  makes CI cost proportional to risk.
- Bad: needs the `ci-gate` engineering change done correctly; introduces
  longer-lived branches and an up-front classification call that must be made
  honestly.

### Option 2 — Relocate planning to a separate repo / orphan branch

- Good: planning edits would trivially escape the code CI gate.
- Bad: breaks single-clone simplicity and resumability; contradicts ADR-0002,
  which reaffirms in-repo Markdown; **owner explicitly rejected relocation.**

### Option 3 — Move backlog/status to GitHub Issues + Projects

- Good: planning would live entirely outside the PR/CI flow.
- Bad: breaks "resume from repo content alone" (ADR-0002); less agent-friendly as
  a narrative source of truth; splits the source of truth across two systems.

## Amendment 2026-08-02 — architect-review-skip exception for verified low-risk units

Since this ADR shipped, §7's "the architect reviews the branch diff before the
PR opens" has had an unwritten exception, applied consistently across **eight**
units without ever being recorded: TASK-0217 (this task) found the gap while
reconciling process docs against shipped practice — exactly the kind of drift
[ADR-0012](0012-documentation-ownership-and-placement.md)/this ADR exist to
prevent.

**The pattern, as actually practiced:** a docs-only planning sync (PRs #39, #42, #46, #51, #53, #55) or a low-risk `contents: read` CI-tooling chore (PRs #56, #57) skipped the architect dispatch entirely, with the orchestrator instead verifying the change's claims directly — running the tool locally, querying a live API, re-running CI — and recording that verification in the PR body in place of a review summary.

**Formalized here, narrowly:** skipping the architect dispatch is permitted
only when **all** of the following hold:

1. **No product code changes** (nothing under `src/`, `tests/`, or affecting
   the binary's behavior) — `refactor`/`feat`/`fix` touching product code
   always gets the normal review.
2. **No write-permission or secret-touching CI/workflow change.** A job
   holding `contents: write` or handling any secret gets the normal review
   (and, per the TASK-0202/0203 and TASK-0211 precedent, often *two*
   dispatches) regardless of how small the diff looks — CLAUDE.md's security
   gate (rule 3) applies by risk class, not by unit size.
3. **The change's correctness is independently verifiable by the orchestrator
   directly**, not merely asserted — running the tool with the exact pinned
   version, querying the live GitHub API, re-running the CI job — and that
   verification is recorded in the PR body. A change whose correctness rests
   on judgment (design trade-offs, security reasoning, anything a second
   pair of eyes would actually reason about differently) still needs the
   architect.

The architect (or the product-owner, or the orchestrator) may **always**
demand a review regardless of this exception — it lowers the floor, not a
ceiling. This does not relax [ADR-0006](0006-post-pr-review-iteration-loop.md)
or the merge gate (ADR-0003): checks still must be green and, when a review
did happen, its threads resolved, before the human owner merges.

## Links

- [ADR-0002](0002-in-repo-markdown-as-canonical-planning.md) — reaffirmed:
  in-repo Markdown stays canonical; artifacts are **not** relocated.
- [ADR-0003](0003-main-branch-protection-and-merge-model.md) — squash-only merge
  and human-owner-as-sole-gate are unchanged.
- [ADR-0005](0005-mandatory-tdd-and-security-gate.md) — TDD and the security
  checks (`secret-scan`) remain part of the gate.
- [ADR-0006](0006-post-pr-review-iteration-loop.md) — review **kinds** unchanged;
  the review **cadence** is refined to a pre-PR branch-diff review.
