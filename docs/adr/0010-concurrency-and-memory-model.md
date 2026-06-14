# ADR-0010: Concurrency & memory model

- **Status:** accepted
- **Date:** 2026-06-14
- **Deciders:** architect, human owner
- **Related:** [PRD-0001](../prd/PRD-0001-exact-image-deduplication.md)
  (NFR-3, NFR-4; AC-16), [ADR-0007](0007-technology-stack-rust-clap.md)
  (Rust; `rayon`/`blake3`), [ADR-0008](0008-core-deduplication-algorithm-and-semantics.md)
  (the pipeline this ADR executes)

## Context and problem statement

PRD-0001 must comfortably process **100k+ images and hundreds of gigabytes**
(NFR-4) while remaining **deterministic** (NFR-3): identical filesystem state and
flags yield identical groups and keeper choices across runs and platforms. NFR-4
explicitly calls for **streaming file reads (no whole-file buffering)** and
**hashing in parallel across CPU cores**. This ADR decides *how* the
[ADR-0008](0008-core-deduplication-algorithm-and-semantics.md) pipeline executes:
the parallelism strategy, the memory bound, and how determinism is preserved
**despite** parallelism. Worked through and confirmed with the human owner.

## Decision drivers

- **Scale & bounded memory (NFR-4):** memory must stay bounded regardless of file
  count or file size; never load a whole file (or all files) into RAM.
- **Parallel throughput (NFR-4):** use available cores for the hashing stage,
  which is the CPU-touching part of an otherwise I/O-bound workload.
- **Determinism despite parallelism (NFR-3, AC-16):** non-deterministic completion
  order must not leak into results.
- **Simplicity & safety (ADR-0007):** prefer a proven, data-race-free parallelism
  primitive over hand-rolled threading in a destructive tool.

## Considered options

For parallelism:

1. **`rayon` data-parallel iterators** over the work items. (chosen)
2. **Hand-rolled worker pool** (threads + a channel/queue).
3. **Single-threaded** execution.

For memory: streaming fixed-buffer reads (chosen) vs. memory-mapping vs.
whole-file buffering.

## Decision outcome

**Chosen: `rayon`** for parallelism, **fixed-buffer streaming** for I/O, and a
**canonical sort before any reporting or action** for determinism.

### Parallel hashing with `rayon`

- The expensive, parallelizable stage is **hashing within each size group**
  (ADR-0008). We express it as a **`rayon` parallel iterator** over the candidate
  files: rayon's work-stealing thread pool distributes files across cores with no
  hand-written thread or queue management.
- For a **single very large file**, BLAKE3's tree structure additionally allows
  intra-file parallel hashing; we use the `blake3` crate's parallel update so one
  huge file still uses multiple cores rather than serializing the tail of a run.
- **Why rayon over a hand-rolled pool:** rayon gives data-race-free,
  work-stealing parallelism with a tiny, declarative API surface; a hand-rolled
  pool re-implements scheduling, backpressure, and shutdown — more code and more
  ways to introduce a concurrency bug in a destructive tool, for no throughput
  gain on this workload. Rust's `Send`/`Sync` bounds make rayon's safety
  compiler-checked (ADR-0007).

### Bounded, streaming memory model (NFR-4)

- **In-memory state is per-file *metadata only*** — path, byte size, identity key
  (dev+inode / file ID), and the 32-byte hash. **File *contents* are never
  retained.** For 100k files this is on the order of a few tens of MB regardless of
  total bytes on disk.
- **Hashing** reads each file in a **fixed-size buffer** (streaming chunks fed to
  the hasher), so peak memory per hashing task is one buffer, independent of file
  size. **Byte verification** (ADR-0008) likewise reads keeper and candidate in
  lock-step fixed buffers, short-circuiting on first difference. **No stage ever
  buffers a whole file**, satisfying the NFR-4 "no whole-file buffering" mandate
  for arbitrarily large files.
- We **stream-read rather than memory-map**: mmap of huge/untrusted files invites
  SIGBUS on truncation and complicates the bounded-memory guarantee; explicit
  buffered reads keep behavior simple, portable, and predictable.
- Total resident memory scales with **file count** (bounded metadata) plus
  **buffer count** (bounded by the worker-pool width), not with total data volume —
  the property NFR-4 needs.

### Determinism despite parallelism (NFR-3, AC-16)

Parallel execution finishes work in **non-deterministic order**, which must never
leak into results:

- Parallelism is confined to **pure, commutative computation** (hashing,
  comparing) whose *output does not depend on completion order*. Grouping is by
  value (size, then hash), not by arrival.
- Before **any** reporting or destructive action, results are sorted into a
  **single canonical order**: groups are ordered by a stable group key, files
  within a group by the ADR-0008 keeper/tie-break order (which is *total*), and the
  keeper is selected by that same total order. Two runs over identical state
  therefore emit byte-identical groupings, keeper choices, and apply order (NFR-3,
  AC-16) no matter how the threads interleaved.
- Apply-time actions iterate this canonical order, so even the **sequence of
  trash/delete operations** is reproducible.

### Bounding concurrency — I/O is usually the bottleneck

The workload is **largely I/O-bound** (ADR-0007): on spinning disks and many SSDs,
saturating all CPU threads with concurrent reads can cause **seek thrashing** and
*reduce* throughput versus a more modest reader count.

- The default parallelism is rayon's pool sized to available cores, which is a
  reasonable default for hashing on fast storage.
- We **reserve the ability to bound read concurrency** (e.g. a degree-of-
  parallelism cap) and treat the optimal width as an **empirical, tunable** value
  to be confirmed against the NFR-4 targets during Phase 3 benchmarking — **not**
  hard-coded blindly here. Crucially, **the concurrency width never affects
  results** (determinism comes from the canonical sort, not from any execution
  order), so it can be tuned freely without risk to NFR-3 or NFR-1. Whether to
  expose a user-facing `--jobs`/`--threads` flag is a minor product/UX question
  deferred to Phase 3 (not decided here).

### Consequences

- **Positive:** core-scaled hashing with a data-race-free, compiler-checked
  primitive; memory bounded by file *count*, not data *volume*, for arbitrarily
  large libraries and files (NFR-4); determinism is structural (a canonical sort),
  decoupled from thread scheduling (NFR-3); concurrency width is a free tuning knob
  with no safety/determinism impact.
- **Negative / trade-offs:** the optimal read-concurrency bound is storage-
  dependent and must be tuned empirically (Phase 3 benchmarking); rayon adds a
  dependency (audited per ADR-0007).
- **Follow-ups / new tasks created:** Phase 3 implementation tasks for the parallel
  hashing executor, the streaming hash/verify readers, and the canonical-sort stage
  are seeded after this PR; an NFR-4 benchmarking task to set concurrency defaults
  and confirm memory/throughput targets is seeded alongside.

## Pros and cons of the options

### Option 1 — `rayon` (chosen)

- Good: minimal, declarative parallelism; work-stealing balances uneven file
  sizes; data-race freedom is compiler-checked; pairs with `blake3` intra-file
  parallelism; results stay deterministic via the canonical sort.
- Bad: another dependency; default pool width may over-subscribe slow disks (hence
  the tunable bound).

### Option 2 — hand-rolled worker pool

- Good: full control over scheduling and backpressure.
- Bad: re-implements what rayon provides; more concurrency-bug surface in a
  destructive tool; no throughput advantage on an I/O-bound workload.

### Option 3 — single-threaded

- Good: trivially deterministic; simplest.
- Bad: leaves cores idle and misses the NFR-4 parallel-hashing requirement on large
  libraries.

## Links

- [PRD-0001](../prd/PRD-0001-exact-image-deduplication.md) NFR-3/NFR-4 — the
  determinism and scale requirements this model satisfies.
- [ADR-0008](0008-core-deduplication-algorithm-and-semantics.md) — the pipeline
  executed here and the total keeper/tie-break order the canonical sort uses.
- [ADR-0007](0007-technology-stack-rust-clap.md) — `rayon` and `blake3`, and the
  `Send`/`Sync`-checked parallelism that makes this safe.
- The `rayon` and `blake3` crates; BLAKE3's tree-hash parallelism.
