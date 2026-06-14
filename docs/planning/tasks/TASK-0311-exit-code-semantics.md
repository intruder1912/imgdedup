# TASK-0311 — Exit-code semantics & `--exit-code`

- **Phase:** 3 — Implementation (TDD)
- **Role:** implementer
- **Status:** todo
- **Depends on:** TASK-0307

## Goal

Wire the final process exit status per the ADR-0008 scheme — `0` success, `1`
run-level error, `2` clap parse error — plus the opt-in `--exit-code` that returns
`3` when a successful run found at least one duplicate group (else `0`).

## Context & links

- **PRD-0001** FR-15 (exit `0` on success, distinct non-zero on error), FR-18 (opt-in
  `--exit-code` → `3` when duplicates found, `0` when none; flag changes only the
  exit status), AC-12, AC-19.
- **ADR-0008** — exit-code scheme: `0` success (regardless of findings), `1`
  run-level error (e.g. `--delete` without `--apply`, unreadable scan root), `2` clap
  parse error, `3` opt-in `--exit-code` duplicates-found. Per-file errors are skipped
  and do **not** by themselves change the code from `0` (NFR-6).
- Builds on the exit-code constants and CLI guard from **TASK-0301**; needs the
  duplicate-found result from the report stage (TASK-0307).

## Acceptance criteria

Test-first (ADR-0005), asserting on process exit status.

- **AC-12 (FR-15):** a successful run exits `0`; a run-level failure exits a distinct
  non-zero (`1`); `--delete` without `--apply` exits non-zero (`1`) and removes
  nothing. clap parse errors exit `2`.
- **AC-19 (FR-18):** with `--exit-code`, a run that finds duplicate groups exits `3`
  and a run that finds none exits `0`; **without** `--exit-code`, a successful run
  exits `0` even when duplicates are present.
- A per-file skip/error alone does not change a successful run's exit code from `0`
  (NFR-6).
- `--exit-code` is meaningful in any mode (dry-run, `--apply`, `--delete`) and affects
  **only** the exit status — never what is scanned, grouped, or removed.

## Notes & constraints

- Reuse the named exit-code constants from TASK-0301 — no scattered literals.
- The `3` code is emitted only on a **successful** run with `--exit-code`; a
  run-level error still takes precedence as `1`.
- Keep exit-status derivation a small, pure function of (run outcome, flags,
  duplicates-found) for direct testing.
- A non-empty `CandidateSet.root_errors` (from TASK-0302) signals a run-level
  error; map it to exit code `1` per ADR-0008.

## Out of scope

- The CLI surface/guard itself (TASK-0301).
- What counts as a duplicate group (TASK-0304/TASK-0305) — this task only maps the
  outcome to a status code.
