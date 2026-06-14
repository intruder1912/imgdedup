# TASK-0213 — Documentation ownership & guide structure

- **Phase:** 2 — Architecture & toolchain (process/governance)
- **Role:** product-owner (with architect for the ADR)
- **Status:** done
- **Depends on:** TASK-0212 (ADR-0011 cadence)

## Goal

Formalize **who owns user-facing documentation and where it lives**, and
establish the structure. Resolves the gap left by ADR-0011 (which said user docs
ride the feature PR but not how they are structured or where detailed docs live),
and which let the `README.md` drift badly out of date.

## Decision (ADR-0012)

The **product-owner** owns user-facing documentation, justified by owning
functional acceptance. Split by depth:

- **General usage → top-level `README.md`** (orientation + quick start + link).
- **Detailed → `docs/guide/`** as a linked collection of **self-contained,
  per-topic guide pages** with an index (`docs/guide/README.md`).

Recorded in [ADR-0012](../../adr/0012-documentation-ownership-and-placement.md),
extending [ADR-0011](../../adr/0011-unit-of-work-cadence-and-planning-gate.md).

## Deliverables

- **ADR-0012** authored + indexed in `docs/adr/README.md`.
- **`docs/guide/`** established: index + seed pages (getting started, scanning &
  scope, choosing what to keep, removing duplicates safely, output & exit codes),
  migrating the detailed reference out of the README.
- **README** slimmed to general usage + a "Detailed guide" link into `docs/guide/`.
- **Propagation** so the rule is consistent everywhere it is stated:
  `.claude/agents/product-owner.md` (rule 6), `docs/process/crew.md` (product-owner
  paragraph), `docs/process/sdlc.md` (DoD + unit-of-work table), `CLAUDE.md` (roles
  table + repository map), `docs/prd/TEMPLATE.md` (documentation obligation on the
  user-visible surface).

## Acceptance criteria

- ADR-0012 is `accepted` and indexed; the ownership/placement rule is stated
  consistently across the role, process, project, and PRD-template docs (no
  contradictions).
- `docs/guide/` exists with an index and self-contained, cross-linked pages; the
  README links into it and no longer duplicates the detailed reference.
- `make validate` doc gates pass (markdownlint, cspell); all internal links
  resolve.

## Out of scope

- Rendering the guide as a static HTML site (noted as a possible future direction;
  intentionally **not** added to the roadmap/backlog).
- Per-feature guide content for behavior not yet implemented (those pages fill out
  with real output as TASK-0307/0308/0309/0310/0311 land — docs ride each feature).
