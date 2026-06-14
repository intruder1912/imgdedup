# TASK-0307 — Human-readable report (dry-run default)

- **Phase:** 3 — Implementation (TDD)
- **Role:** implementer
- **Status:** todo
- **Depends on:** TASK-0305
- **Parallel:** yes (with TASK-0308)

## Goal

The default dry-run summary: for each duplicate group, print the keeper and the
files that would be removed, plus group/file counts, reclaimable space, and
informational hard-link notes — modifying nothing on disk.

## Context & links

- **PRD-0001** FR-9 (dry run is the default; report only, nothing changes), FR-13
  (summary: groups, keeper + removed files, counts, reclaimable space), FR-17
  (hard-link informational notes, excluded from reclaimable space).
- **ADR-0008** — report stage of the pipeline; hard-link sets excluded from
  reclaimable-space figure.
- **NFR-6** — surfaces the per-file skip warnings collected upstream (TASK-0302).
- Consumes canonical groups + keeper (TASK-0305) over the deterministic order
  (TASK-0306).

## Acceptance criteria

Test-first (ADR-0005), asserting on rendered output.

- **AC-1 (FR-9, NFR-1):** with no action flag, every duplicate group is listed and
  **no file on disk is modified, moved, or deleted** (verified by post-run fixture
  inspection).
- **AC-13 (FR-13):** the summary reports each group's keeper and removed files, the
  group and file counts, and the reclaimable space.
- **FR-17:** hard-linked files are shown as an informational note and **excluded**
  from the reclaimable-space total.
- Per-file skip/error warnings (NFR-6) are surfaced in the report.
- Output ordering is the canonical order (TASK-0306), so the report is deterministic.

## Notes & constraints

- Reclaimable space = sum of non-keeper sizes that would actually free storage —
  hard-link members excluded (ADR-0008/FR-17).
- This unit consumes already-computed groups; it must not re-scan or mutate.
- Keep rendering separate from the JSON surface (TASK-0308) — shared model, distinct
  formatters.

## Out of scope

- JSON output (TASK-0308).
- Performing any trash/delete (TASK-0309/TASK-0310).
- Exit-code wiring (TASK-0311).
