# TASK-0304 — Core dedup engine (the safety heart)

- **Phase:** 3 — Implementation (TDD)
- **Role:** implementer
- **Status:** todo
- **Depends on:** TASK-0302, TASK-0303

## Goal

The safety heart: turn confirmed candidates into **verified duplicate groups** via
size pre-grouping → BLAKE3 hashing (pre-filter) → hash grouping → byte-for-byte
verification against the keeper → hard-link/identity exclusion. The byte comparison
is the **sole authority** for removal; a hash match alone never authorizes it.

## Context & links

- **PRD-0001** FR-5 (size pre-grouping, AC-9), FR-4 (content hash pre-filter +
  byte-for-byte verify as the removal authority), FR-17 (hard links already
  deduplicated; AC-18), NFR-1 (safety invariant), NFR-2.
- **ADR-0008** — the pipeline core: size group → hash within size groups → group by
  hash → collapse hard-link/identity sets → **star byte-verify each non-keeper
  against the keeper** (O(n), short-circuit on first differing byte). Identity key:
  POSIX `(st_dev, st_ino)`; Windows volume serial + file ID.
- **ADR-0010** — streaming fixed-buffer reads (no whole-file buffering); parallel
  execution wiring is TASK-0306.
- Consumes the candidate + confirmation units (TASK-0302/TASK-0303).

## Acceptance criteria

Test-first, in **TDD increments** (size grouping → hashing → hash grouping →
byte-verify → identity), each increment landing with its own failing-first tests.

- **AC-9 (FR-5):** groups produced with size pre-grouping are **identical** to those
  from hashing every candidate (explicit equivalence test). Singleton size groups
  are dropped without hashing.
- **FR-4 / NFR-1:** within a hash group, each non-keeper is **byte-for-byte verified
  against the keeper**; a candidate that fails verification (synthetic hash-collision
  fixture, or content that differs) is **excluded** from the group's removal set and
  reported — never removed on a hash match alone.
- **AC-18 (FR-17):** two hard links to the same content (same identity key) are
  collapsed; at most one representative per identity set is ever a removal candidate,
  and identical-identity files are never proposed for removal.
- The engine emits verified duplicate groups (keeper distinguished from removable
  non-keepers) with no filesystem mutation — it is a pure analysis stage.

## Notes & constraints

- BLAKE3 is a **pre-filter only** (ADR-0008); correctness must not depend on the
  hash being collision-proof — prove this with a forced-collision test that the byte
  compare rejects.
- Verification is a **star** comparison against the keeper (not all-pairs): O(n) per
  group by transitivity.
- All reads stream through a fixed buffer and short-circuit on first difference /
  length mismatch (ADR-0010); never buffer a whole file.
- Keeper *selection* logic is TASK-0305; this engine consumes a keeper ordering but
  the total order itself is defined there. Coordinate the interface so TASK-0305
  plugs in cleanly.
- In-memory state is per-file metadata only (path, size, identity key, 32-byte hash)
  — never file contents (ADR-0010).

## Out of scope

- Keeper selection rule and tie-breaks (TASK-0305).
- Parallelism / canonical-sort determinism wiring (TASK-0306).
- Reporting, JSON, apply/delete (TASK-0307..TASK-0310).
- Apply-time re-validation (TASK-0309) — distinct from the scan-time verify here.
