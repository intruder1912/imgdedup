# TASK-0303 — Magic-byte image confirmation

- **Phase:** 3 — Implementation (TDD)
- **Role:** implementer
- **Status:** todo
- **Depends on:** TASK-0301
- **Parallel:** yes (with TASK-0302 — disjoint module)

## Goal

A content-sniffing unit that confirms a candidate is genuinely an image by its
**magic-byte signature** before it can be grouped or acted on, rejecting files
whose extension lies. Files that fail confirmation are dropped and never enter a
group.

## Context & links

- **PRD-0001** FR-3 (magic-byte confirmation precedes grouping/action; failures are
  ignored and never removed), NFR-2 (no collateral damage to non-images).
- **ADR-0008** — "Magic-byte coverage" decision: use the `infer` crate; it reads
  only a small header prefix (fits the streaming model). The `enumerate → magic-byte
  confirm` pipeline stage.
- **ADR-0007** — `infer` crate.
- Consumes candidates from **TASK-0302**.

## Acceptance criteria

Test-first (ADR-0005), with byte fixtures for each format.

- Every default allow-list format is detected from its real header: JPEG
  (`FF D8 FF`), PNG (`89 50 4E 47 0D 0A 1A 0A`), GIF (`GIF87a`/`GIF89a`), WebP
  (`RIFF....WEBP`), BMP (`42 4D`), TIFF (`49 49 2A 00` and `4D 4D 00 2A` — both
  endiannesses). A focused test asserts detection for each (FR-3, NFR-2).
- **AC-4 (FR-3, NFR-2):** a file named `photo.jpg` whose contents are plain text is
  **rejected** — it is never grouped with images and (by exclusion) never trashed or
  deleted.
- Confirmation reads only a bounded header prefix; the full file is not buffered.
- A file passing the extension allow-list but failing content confirmation is
  dropped before grouping.

## Notes & constraints

- Prefer `infer` over a hand-rolled signature table (ADR-0008 rationale: container
  formats like WebP/RIFF and TIFF's two endiannesses are fiddly and security-
  sensitive).
- This unit is a pure predicate over a file/header; it must be independently
  testable without traversal.
- If a custom `--ext` includes an extension `infer` cannot confirm, document the
  resulting behavior (the file cannot pass confirmation and is skipped) — surface it
  consistently with the NFR-6 reporting path.

## Out of scope

- The `--sniff-all` future flag for wrong/missing extensions (PRD §4/§10) — not in
  this PRD.
- Enumeration (TASK-0302); grouping/hashing (TASK-0304).
