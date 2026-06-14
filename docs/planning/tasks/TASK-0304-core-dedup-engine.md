# TASK-0304 — Core dedup engine (the safety heart)

- **Phase:** 3 — Implementation (TDD)
- **Role:** implementer
- **Status:** done
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
- **Carrier type from the sniff stage (TASK-0303 review thread):** TASK-0303's
  `sniff::filter_candidates` currently returns `Vec<PathBuf>`, **dropping the `size`**
  that `enumerate` already stat'd. Since size pre-grouping is this engine's first
  stage, decide the confirmed-set carrier deliberately rather than re-stat'ing every
  file: either make the sniff stage emit `size` alongside `path` (e.g. a shared
  `ConfirmedFile { path, size }`, or have `filter_candidates` operate over
  `enumerate::Candidate`), or accept a single re-stat here and document the small
  TOCTOU widening. Avoid a silent double-stat.

## Resolved design decisions (brainstorm 2026-06-14)

These resolve the open questions above before implementation; recorded here so the
repo remains the single source of truth.

- **Confirmed-set carrier (the sniff carrier thread):** the sniff stage emits a
  dedicated `sniff::ConfirmedFile { path, size }`. `filter_candidates` changes from
  `(Vec<PathBuf>) -> (Vec<PathBuf>, …)` to `(Vec<enumerate::Candidate>) ->
  (Vec<ConfirmedFile>, Vec<SniffWarning>)`, preserving the `size` that `enumerate`
  already stat'd. No re-stat, no TOCTOU widening. `enumerate::Candidate` is unchanged.
- **Keeper interface:** the engine accepts an **injected keeper-ordering** closure
  (`Fn(&FileRec, &FileRec) -> Ordering`) and designates the minimum as keeper.
  TASK-0304 passes a deterministic lexicographic-path placeholder; TASK-0305 plugs in
  the real keeper rule without touching the engine's grouping/verify logic.
- **Identity key:** the `same-file` crate (already in-tree transitively via `walkdir`;
  promoted to an explicit dependency). `same-file::Handle` is `Eq + Hash` over POSIX
  `(st_dev, st_ino)` / Windows volume-serial + file-id — exactly the ADR-0008 key, with
  no `#[cfg]`/`unsafe` of our own. **Invariant:** `Handle`s hold an open FD for their
  lifetime, so identity is computed **only within a hash-group** (a tiny set of
  same-size/same-hash candidates) and the handles are dropped immediately — the full
  candidate set is never handle-mapped, bounding FD pressure (NFR-4).
- **Proving BLAKE3 is a pre-filter only:** the engine core takes the hasher as an
  internal injection point. A unit test supplies a **constant hasher** that forces
  same-size, different-content files into one hash group and asserts the byte-verify
  still splits them and reports the loser — directly proving removal never rests on a
  hash match. The public `find_duplicates` always uses BLAKE3.
- **New module `src/engine.rs`:** public `find_duplicates(files: Vec<ConfirmedFile>,
  keeper_order) -> DedupOutcome { groups: Vec<DuplicateGroup>, warnings:
  Vec<EngineWarning> }`, where `DuplicateGroup { keeper: FileRec, removable:
  Vec<FileRec> }` and `FileRec` carries metadata only (path, size, identity, 32-byte
  hash) — never file contents (ADR-0010). Pure analysis: no filesystem mutation. Wired
  into `main.rs` as stage 3, result held behind a `TODO(TASK-0307)` for the report.

## Out of scope

- Keeper selection rule and tie-breaks (TASK-0305).
- Parallelism / canonical-sort determinism wiring (TASK-0306).
- Reporting, JSON, apply/delete (TASK-0307..TASK-0310).
- Apply-time re-validation (TASK-0309) — distinct from the scan-time verify here.
