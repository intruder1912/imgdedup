# TASK-0305 — Keeper selection

- **Phase:** 3 — Implementation (TDD)
- **Role:** implementer
- **Status:** todo
- **Depends on:** TASK-0304

## Goal

A total, deterministic keeper-selection unit: exactly one keeper per group under
`--keep shortest|oldest|newest`, with the fixed tie-break chain and the unambiguous
case-insensitive rule from ADR-0008, so the choice is reproducible across runs and
platforms.

## Context & links

- **PRD-0001** FR-7 (deterministic keeper rule; default `shortest` = fewest
  segments, then fewest characters, then oldest mtime, then lexicographic), FR-8
  (`--keep` selects the primary criterion; remaining criteria break ties in fixed
  order), NFR-3 (determinism).
- **ADR-0008** — "Keeper rule + case-insensitive tie-break": the three orderings;
  canonical absolute-path normalization; **byte-wise, case-sensitive** comparison
  (`'A'` < `'a'`, no case folding); mtime compared at **whole-second** resolution;
  case-only-different paths collapse via the identity check (TASK-0304) before the
  tie-break is reached, so there is never a "two equally-valid keepers" case.
- Consumes verified groups from **TASK-0304**.

## Acceptance criteria

Test-first (ADR-0005).

- **AC-2 (FR-4, FR-7):** three byte-identical copies form exactly one group with
  exactly one keeper chosen by the rule.
- **AC-6 (FR-8):** `--keep newest` selects the file with the newest mtime as keeper;
  `--keep oldest` selects the oldest; `shortest` selects per FR-7.
- **AC-16 (NFR-3):** two separate runs over identical state produce identical keeper
  choices. The order is **total** — tests cover mtime ties (fall through to
  byte-wise path order) and segment/character ties.
- Path comparison is byte-wise/case-sensitive (no locale-dependent case folding);
  mtime is compared at whole-second resolution.

## Notes & constraints

- The order must be **total** so there is never an ambiguous keeper (ADR-0008).
- Do **not** case-fold paths — that reintroduces platform nondeterminism (ADR-0008,
  NFR-3).
- This unit defines the total order that TASK-0306's canonical sort and the engine's
  star-verify keeper (TASK-0304) both rely on — expose it as the single ordering
  source.

## Out of scope

- Hard-link identity collapsing (TASK-0304) — keeper selection assumes identity sets
  are already collapsed.
- The canonical group/report sort and parallelism (TASK-0306).
- Any reporting or removal.
