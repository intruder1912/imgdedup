# TASK-0309 — Trash apply + apply-time re-validation

- **Phase:** 3 — Implementation (TDD)
- **Role:** implementer
- **Status:** done (slice S3, 2026-08-01; see
  [`../reviews/apply-delete.md`](../reviews/apply-delete.md))
- **Depends on:** TASK-0305

## Goal

`--apply` moves non-keepers to the OS trash via the `trash` crate, but only after
**apply-time re-validation** of each file. Fail safe on trash errors, be idempotent,
and match exactly what the dry run reported for an unchanged tree.

## Context & links

- **PRD-0001** FR-10 (`--apply` → OS trash, recoverable), FR-16 (apply-time
  re-validation; AC-17), FR-12 (idempotence; AC-5), NFR-1 (keeper never removed; no
  group left empty), NFR-6 (per-file failures skipped + reported). AC-3, AC-11.
- **ADR-0009** — `trash` crate for `--apply`; **fail safe** (a trash failure is a
  per-file skip-and-report; **never** silently fall back to permanent deletion; the
  keeper and others untouched; run continues). Summary distinguishes trashed vs
  skipped (trash failed).
- **ADR-0008** — apply-time re-validation: file still exists and is a **regular file
  (not a newly-swapped symlink)**, keeper still exists/unchanged, non-keeper still
  byte-for-byte matches the keeper; otherwise skip with a warning.
- Iterates the canonical apply order (TASK-0306); consumes keeper/groups (TASK-0305).

## Acceptance criteria

Test-first (ADR-0005).

- **AC-3 (FR-10, NFR-1):** after `--apply` on a duplicate group, non-keepers are in
  the OS trash and recoverable, and the keeper remains in place.
- **AC-17 (FR-16, NFR-6):** a non-keeper modified/replaced between scan and apply
  (including swapped for a symlink) is **skipped with a warning** and not removed;
  the remaining groups still apply normally.
- **AC-5 (FR-12):** re-running `--apply` on an already-deduplicated tree takes zero
  actions and changes nothing.
- **AC-11 (FR-9 → FR-10 parity):** for a tree unchanged between runs, the dry-run's
  reported intended removals match what `--apply` performs.
- A trash failure on one file is reported and skipped (run continues, exit per
  ADR-0008); the tool **never** falls back to permanent deletion (NFR-1). The summary
  distinguishes trashed vs skipped-on-trash-failure.

## Notes & constraints

- Re-validation happens **immediately before** each removal — it is distinct from the
  scan-time byte verify (TASK-0304) and is a TOCTOU defense.
- Build the **executor** as a small unit with the removal mechanism injectable, so
  TASK-0310 (permanent delete) shares it by swapping trash for `remove_file`.
- Never remove a keeper; never leave a group with zero survivors (NFR-1) — assert in
  tests.
- Per-platform trash integration tests run in CI (ADR-0009); keep the apply path
  testable with a mockable removal sink for unit-level tests.

## Out of scope

- Permanent delete (`--delete`) — TASK-0310 (reuses this executor).
- Reporting/JSON formatting (TASK-0307/TASK-0308) — this task supplies the
  performed-action data they render.
- Exit-code wiring (TASK-0311).
