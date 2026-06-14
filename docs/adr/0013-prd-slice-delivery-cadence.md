# ADR-0013: PRD-slice delivery cadence & in-progress review protocol

- **Status:** accepted
- **Date:** 2026-06-14
- **Deciders:** architect, human owner
- **Related:** extends [ADR-0011](0011-unit-of-work-cadence-and-planning-gate.md);
  reaffirms [ADR-0003](0003-main-branch-protection-and-merge-model.md) (squash /
  linear history), [ADR-0004](0004-conventional-commits-semver-and-releases.md)
  (release per merge), [ADR-0006](0006-post-pr-review-iteration-loop.md)
  (resolve-all gate); builds on [ADR-0012](0012-documentation-ownership-and-placement.md)
  (product-owner owns functional acceptance + user docs)

## Context and problem statement

[ADR-0011](0011-unit-of-work-cadence-and-planning-gate.md) made the unit of work a
single **task** delivered as one small squash PR. That keeps PRs tiny and review
tractable, but for a multi-task product capability it produces a lot of per-task
ceremony (branch → review → PR → CI → merge, repeated) and means the architect
never reviews a coherent capability *as a whole* — only its fragments. The human
owner asked whether a whole PRD could instead land as **one PR with granular
commits**, reviewed holistically.

A whole-PRD PR was rejected during design: it reintroduces **long-lived feature
branches** (the anti-pattern trunk-based development avoids), lets `main` drift
from reality (weakening the repo-as-source-of-truth guarantee of
[ADR-0002](0002-in-repo-markdown-as-canonical-planning.md)), accrues integration
conflict risk, and produces a giant diff that gets a *worse* final review, not a
better one. PRD-0001 alone is ~12 tasks — essentially the whole MVP.

The middle ground — a **coherent shippable slice** of a PRD — keeps the holistic
benefit (review and ship a meaningful increment as a unit) without the long-lived
branch. This ADR defines that cadence and the **in-progress review protocol** that
lets review happen *during* a slice rather than only at its end.

## Decision drivers

- **Holistic review of a coherent capability** without giant, un-reviewable diffs.
- **Avoid long-lived branches / `main` drift** (trunk-based; ADR-0002).
- **Preserve the existing gates** unchanged: squash/linear (ADR-0003), release per
  merge (ADR-0004), resolve-all-threads + green-checks (ADR-0006), TDD + security
  (ADR-0005).
- **Tie shipping to functional acceptance + docs** (ADR-0012).
- **Review cadence under the implementer's control**, with product-owner and
  orchestrator able to compel review of anything.
- **Bounded cross-platform risk** despite CI only running at slice end.

## Considered options

1. **PRD-slice unit + in-progress review protocol** (this ADR): feature delivery
   moves to a coherent shippable slice; `fix`/`chore` keep the ADR-0011 small-PR
   cadence.
2. **Whole-PRD-per-PR**: an entire PRD on one branch, opened as one PR at the end.
3. **Status quo (ADR-0011 task-per-PR unchanged)**.

## Decision outcome

**Chosen option:** Option 1. For **feature delivery within a PRD**, the unit of
work becomes a **PRD slice**; `fix`, `chore`, and standalone doc/ADR units keep the
ADR-0011 one-unit-one-PR cadence (the two models **coexist**).

### What a slice is

A **coherent, shippable slice** of a PRD: an increment that is
**end-to-end-validatable** and **independently mergeable/releasable** — larger than
a single task, smaller than the whole PRD. The **product-owner carves** the PRD
into slices in the backlog (the role owns scope and acceptance); the **architect
sanity-checks** slice boundaries for design coherence.

### Cadence

- **One slice = one branch = one squash PR**, built with granular, well-chosen
  commits. The branch is **CI-free and PR-less until the slice is complete**
  (ADR-0011's CI-free branches are unchanged).
- **Squash-merge is unchanged** (ADR-0003/0004): the commits are the branch and
  review narrative; `main` gets **one squash commit per slice**, which drives
  **one SemVer release per slice**. Keep the branch current by rebasing on `main`.

### In-progress review protocol

- The **implementer requests an architect review** for **significant or especially
  risky/complex** changes — at the implementer's discretion, **not per-commit**.
- The **product-owner and orchestrator may demand** a review of any aspect,
  whether or not the implementer requested one.
- These mid-stream reviews are **advisory / incremental**. With no PR open yet, the
  architect reviews the relevant **commit range** (`git diff`) and findings are
  recorded in a per-slice **review log** on the branch
  (`docs/planning/reviews/<slice>.md`), which is summarized into the final PR
  description.
- **Feedback is worked immediately.** If the implementer is unsure whether to
  address a finding **now or defer it**, it **calls the product-owner to decide**.
  **Deferred findings are promoted to tracked backlog tasks** and noted in the
  review log with the task they became — nothing is lost.

### PR-open gate

The PR opens (in **ready** state) only when the **product-owner has (1) confirmed
all functional requirements in the slice are accepted, and (2) delivered all
user-facing documentation** (README + `docs/guide/`, ADR-0012) — in addition to the
standing Definition of Done (`make validate` green, implementer technical docs,
planning/status updated).

### Final review + merge

On the opened PR the architect performs a **full review over the whole slice
scope**, recorded as resolvable threads, **building on the in-progress review log**
(already-reviewed chunks are re-confirmed, not fully re-reviewed — this keeps the
final pass tractable). Then the existing gate holds unchanged (ADR-0003/0006): all
threads resolved **and** required checks green — the **3-OS CI matrix runs here for
the first time** — with the tester verifying fixes under TDD. The **human owner
squash-merges**; the branch auto-deletes; release automation runs.

### Consequences

- Positive: a coherent capability is reviewed and shipped as one unit without a
  giant diff or a long-lived branch; less per-task PR ceremony; review happens
  continuously during the slice; shipping is gated on real acceptance + docs.
- Negative / trade-offs: a slice branch lives longer than a task branch (mitigated
  by bounded slice size + rebasing on `main`); **cross-platform CI signal arrives
  only at slice end** (mitigated by bounded slice size, portable-code discipline,
  and the ADR-0006 stage-two loop); the in-progress review log is an extra,
  out-of-band artifact to maintain.
- Follow-ups / new tasks: propagate the cadence into `sdlc.md`, `branching.md`,
  `crew.md`, the implementer/product-owner/architect role defs, and `CLAUDE.md`;
  add the `docs/planning/reviews/` location + template. Tracked as TASK-0214.

## Pros and cons of the options

### Option 1 — PRD-slice + in-progress review protocol

- Good: holistic-but-bounded review; no long-lived branch; preserves all existing
  gates; review cadence implementer-driven with PO/orchestrator override; ships on
  acceptance + docs.
- Bad: longer-lived than a task branch; late cross-platform signal; an extra review-
  log artifact.

### Option 2 — whole-PRD-per-PR

- Good: maximal holistic context; least PR ceremony.
- Bad: long-lived branch + `main` drift (ADR-0002); large integration/conflict
  risk; giant final diff → worse review; one release for an entire PRD.

### Option 3 — status quo (task-per-PR)

- Good: tiny PRs; frequent integration; earliest CI signal.
- Bad: high per-capability ceremony; the architect never reviews a coherent
  capability as a whole.

## Links

- [ADR-0011](0011-unit-of-work-cadence-and-planning-gate.md) — the task-level
  cadence this ADR extends for feature delivery.
- [ADR-0012](0012-documentation-ownership-and-placement.md) — product-owner owns
  functional acceptance + user docs (the PR-open gate).
- [ADR-0006](0006-post-pr-review-iteration-loop.md) — the resolve-all stage-two
  loop that still governs the final review.
- [`docs/planning/reviews/`](../planning/reviews/) — the in-progress review-log
  location this ADR establishes.
