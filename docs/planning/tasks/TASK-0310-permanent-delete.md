# TASK-0310 — Permanent delete

- **Phase:** 3 — Implementation (TDD)
- **Role:** implementer
- **Status:** done (slice S3, 2026-08-01; see
  [`../reviews/apply-delete.md`](../reviews/apply-delete.md))
- **Depends on:** TASK-0309

## Goal

`--apply --delete` permanently removes non-keepers via `std::fs::remove_file` (no
trash indirection), reusing the TASK-0309 executor and its apply-time re-validation.
`--delete` is only ever valid together with `--apply`.

## Context & links

- **PRD-0001** FR-11 (`--delete`, only with `--apply`, permanently deletes instead of
  trashing — opt-in), AC-7. The `--delete`-without-`--apply` rejection is enforced at
  the CLI layer (TASK-0301, AC-12).
- **ADR-0009** — permanent delete is direct `std::fs::remove_file`; `--delete`
  rejected without `--apply` at the clap layer before any filesystem call.
- **ADR-0008** — the same apply-time re-validation gates deletion (TASK-0309).
- Shares the executor built in **TASK-0309**.

## Acceptance criteria

Test-first (ADR-0005).

- **AC-7 (FR-11):** with `--apply --delete` on a duplicate group, non-keepers are
  **permanently removed** and are **not** present in the trash; the keeper remains.
- Apply-time re-validation (FR-16) applies identically to the delete path: a file
  changed since the scan is skipped with a warning, not deleted.
- The keeper is never deleted; no group is left with zero survivors (NFR-1).
- `--delete` without `--apply` is rejected (covered by TASK-0301/AC-12; assert the
  delete path is unreachable in that case).
- The summary distinguishes **permanently deleted** from trashed/skipped (ADR-0009).

## Notes & constraints

- Reuse the TASK-0309 executor by injecting `remove_file` as the removal mechanism —
  do not duplicate the re-validation/iteration logic.
- This path is irreversible: tests must confirm re-validation and the keeper-safety
  invariant hold before any `remove_file` call.

## Out of scope

- The trash path and shared executor internals (TASK-0309).
- CLI-layer guard for `--delete` requires `--apply` (TASK-0301).
- Exit-code wiring (TASK-0311).
