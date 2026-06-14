# TASK-0214 — PRD-slice delivery cadence & in-progress review protocol

- **Phase:** 2 — Architecture & toolchain (process/governance)
- **Role:** architect (with product-owner for the slice-carving responsibility)
- **Status:** done
- **Depends on:** TASK-0212 (ADR-0011 cadence)

## Goal

For **feature delivery within a PRD**, move the unit of work from a single task to
a **coherent shippable slice**, reviewed *in-progress* on the branch and opened as
one PR when complete. Resolve the owner's request to deliver a PRD with granular
commits and a holistic review, without reintroducing long-lived whole-PRD branches.

## Decision (ADR-0013)

- **Feature unit = PRD slice** (end-to-end-validatable, independently mergeable
  increment; larger than a task, smaller than the PRD). **Product-owner carves**;
  **architect sanity-checks** boundaries. `fix`/`chore` keep the ADR-0011 small-PR
  cadence (coexist).
- **One slice = one branch = one squash PR** with granular commits; branch is
  CI-free and PR-less until done; squash-merge unchanged (ADR-0003/0004).
- **In-progress review protocol:** implementer requests architect review for
  significant/risky changes (not per-commit); product-owner and orchestrator may
  demand one; reviews are advisory and logged in `docs/planning/reviews/<slice>.md`;
  feedback worked immediately; uncertain now-vs-defer → product-owner decides;
  deferrals → tracked backlog tasks.
- **PR-open gate:** product-owner has accepted all the slice's functional
  requirements and delivered its user docs (ADR-0012).
- **Final review + merge:** full whole-slice architect review on the PR building on
  the in-progress log; ADR-0006 resolve-all-threads + green-checks gate (3-OS matrix
  runs at slice end); human owner squash-merges.

Recorded in [ADR-0013](../../adr/0013-prd-slice-delivery-cadence.md).

## Deliverables

- **ADR-0013** authored + indexed in `docs/adr/README.md`.
- **`docs/planning/reviews/`** established (index `README.md` + `TEMPLATE.md` for
  per-slice review logs).
- **Propagation** so the cadence is stated consistently: `docs/process/sdlc.md`
  (classification table + new "PRD-slice delivery" section + DoD), `branching.md`
  (slice lifetime + in-progress reviews), `crew.md` (product-owner / architect /
  implementer / orchestrator), the `.claude/agents/` defs for those roles, and
  `CLAUDE.md` §4.

## Acceptance criteria

- ADR-0013 is `accepted` and indexed; the slice cadence + in-progress review
  protocol + PR-open gate are stated consistently across the process, role, and
  project docs with no contradictions, and squash/linear (ADR-0003), release-per-
  merge (ADR-0004), and resolve-all (ADR-0006) are explicitly preserved.
- `docs/planning/reviews/` exists with an index and a usable template.
- `make validate` doc gates pass (markdownlint, cspell); internal links resolve.

## Out of scope

- Opening a draft PR early for cross-platform CI signal (explicitly rejected — the
  branch stays PR-less until the slice is done; late cross-platform signal is an
  accepted, contained residual risk).
- Applying the slice cadence to `fix`/`chore` units (they keep the ADR-0011 model).
- Any change to the squash-only merge model or the release pipeline.
