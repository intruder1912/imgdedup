# TASK-0302 — File enumeration & scope

- **Phase:** 3 — Implementation (TDD)
- **Role:** implementer
- **Status:** todo
- **Depends on:** TASK-0301
- **Parallel:** yes (with TASK-0303 — disjoint module)

## Goal

A traversal unit that walks the input paths and yields the **candidate file set**:
recursion control, extension allow-list filtering, hidden/symlink defaults with
opt-ins, and per-file error skip-and-report — feeding the dedup engine.

## Context & links

- **PRD-0001** FR-1 (recurse by default, `--no-recurse`), FR-2 (extension
  allow-list, `--ext`), FR-6 (skip hidden + no-follow-symlinks by default, with
  `--hidden`/`--follow-symlinks` opt-ins), NFR-6 (per-file error skip-and-report).
- **ADR-0007** — `walkdir`/`ignore` for traversal.
- **ADR-0008** — this is the `enumerate candidates (FR-1/FR-2/FR-6)` pipeline stage.
- Consumes the parsed CLI scope flags from **TASK-0301**.

## Acceptance criteria

Test-first (ADR-0005), using temporary directory fixtures.

- **AC-15 (FR-1):** duplicates nested in subdirectories are enumerated by default;
  with `--no-recurse` only the top level of each input path is considered.
- **FR-2:** default allow-list is `jpg`/`jpeg`, `png`, `gif`, `webp`, `bmp`,
  `tif`/`tiff`; `--ext` overrides it. Extension matching is case-insensitive
  (`.JPG` matches). Non-allow-listed extensions are excluded.
- **AC-14 (FR-6, NFR-2):** with defaults, a symlink is not followed and a hidden
  file/dir is not included; with `--follow-symlinks` / `--hidden` each is included
  accordingly.
- **AC-10 (NFR-6):** an unreadable/permission-denied entry is reported and skipped;
  enumeration of the rest completes successfully (no abort).
- Output is the candidate set (path + the metadata later stages need) with no
  content reads performed here.

## Notes & constraints

- Extension filtering is the **first** gate; magic-byte confirmation (TASK-0303) is
  a separate, later gate — keep the two units decoupled.
- Do not canonicalize/resolve for the keeper tie-break here; that ordering belongs
  to TASK-0305/TASK-0306.
- Symlink handling must be a genuine no-follow by default — relevant again as a
  TOCTOU defense at apply time (TASK-0309), but here it is purely scope.
- Per-file errors must be collected as structured warnings, not printed ad hoc, so
  the report unit (TASK-0307) can surface them.

## Out of scope

- Magic-byte content confirmation (TASK-0303).
- Size grouping / hashing / identity (TASK-0304, TASK-0306).
- Rendering the skipped-file warnings (TASK-0307).
