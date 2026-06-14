# TASK-0306 — Concurrency & determinism

- **Phase:** 3 — Implementation (TDD)
- **Role:** implementer
- **Status:** todo
- **Depends on:** TASK-0304

## Goal

Execute the dedup engine in parallel with bounded memory while keeping results
**fully deterministic**: rayon parallel hashing plus BLAKE3 intra-file parallelism,
fixed-buffer streaming reads (no whole-file buffering), and a **canonical sort**
before any report or action so output is independent of thread scheduling.

## Context & links

- **PRD-0001** NFR-3 (determinism), NFR-4 (100k+ files, hundreds of GB; streaming
  reads, parallel hashing), AC-16.
- **ADR-0010** — rayon data-parallel hashing within size groups; `blake3` parallel
  update for single large files; **per-file metadata only** in memory (path, size,
  identity key, 32-byte hash) — never contents; fixed-buffer streaming for both
  hashing and verify; **canonical sort** (groups by stable key, files by the
  TASK-0305 total order) before any reporting/action so determinism is structural,
  not scheduling-dependent; concurrency width is a free tuning knob (never affects
  results).
- Builds on the engine (TASK-0304) and the keeper order (TASK-0305).

## Acceptance criteria

Test-first (ADR-0005).

- Hashing runs across cores via a rayon parallel iterator over candidate files;
  reads stream through a fixed-size buffer — no stage buffers a whole file (asserted
  for an arbitrarily large fixture / bounded-buffer reader test).
- **AC-16 (NFR-3):** repeated runs over identical state emit byte-identical group
  ordering, keeper choices, and apply order regardless of thread interleaving — the
  canonical sort precedes all reporting/action.
- Determinism is **independent of concurrency width**: varying the degree of
  parallelism does not change results (test asserts identical output at different
  widths).
- Peak resident memory scales with file **count** + buffer count, not total data
  **volume**.

## Notes & constraints

- Use rayon (data-race-free, compiler-checked `Send`/`Sync`), not a hand-rolled
  pool (ADR-0010).
- Stream-read, do **not** memory-map (ADR-0010: mmap risks SIGBUS on truncation and
  complicates the memory bound).
- Reserve the ability to **bound read concurrency**; the optimal width is empirical
  and set in TASK-0312, not hard-coded here. Whether to expose `--jobs`/`--threads`
  is deferred (ADR-0010) — do not add it without a product decision.
- The canonical sort is the single determinism mechanism; never rely on completion
  order for correctness.

## Out of scope

- Choosing the empirical concurrency default / NFR-4 benchmark (TASK-0312).
- A user-facing jobs/threads flag (deferred, ADR-0010).
- Reporting, JSON, apply/delete surfaces (TASK-0307..TASK-0310).
