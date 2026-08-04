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

### Consequence note — what the NFR-4 measurement actually showed (2026-08-02, TASK-0312)

This ADR deferred the concurrency width to "Phase 3 benchmarking". That
measurement has now been taken (numbers and method in
[`../planning/reviews/e2e-scale.md`](../planning/reviews/e2e-scale.md)). It
settles the memory claim and — importantly — **does not** settle the width
question, which stays open.

**Memory: confirmed, and now quantified.** Peak RSS tracks file *count*, not data
*volume*: roughly **7.5 MB baseline plus ~350 B per file** (11.1 MB at 10k files,
42.8 MB at 100k — linear above the baseline), which projects to about 0.36 GB at
one million files. 300 MB spread over 3 files cost 7.5 MB, i.e. the baseline. The
streaming-read design holds; nothing is buffered whole.

**Width: not decided by this measurement, because the experiment could not see
the knob.** The 1-vs-8-worker comparison was taken on a warm 10k-file fixture and
moved wall time 1.97 s → 1.72 s. That looks like "concurrency does not help", but
the architect review showed why it proves nothing: in the small-file regime the
rayon-parallel stage is a rounding error. A control fixture built so that **no
file is hashed at all** costs the same wall time as one that hashes ~1,800 —
because `enumerate` (walkdir) and `sniff` are **serial**, one open-and-read per
file, and they dominate. Rayon's width governs only the hashing stage.

So the honest conclusions are:

- **Keep rayon's default (one worker per core) for now, and add no `--jobs`
  flag** — but as a status-quo choice, not as a measured optimum. Re-measure with
  TASK-0314 on ordinary hardware, over a workload where the parallel stage is a
  meaningful share.
- **`RAYON_NUM_THREADS` is the de facto escape hatch.** No custom pool is built
  anywhere, so the environment variable works; the scale suite relies on it.
- The width axis this ADR originally worried about — needing to *lower*
  concurrency to avoid seek thrashing on spinning disks — is untouched by these
  measurements, which ran on flash.

**Two throughput levers this measurement did surface, neither exploited:**

1. **The scan stage is serial.** `sniff::filter_candidates` opens and reads a
   header for every candidate in a plain loop, and it is ~100% of small-file wall
   time. Parallelizing it is the lever for large libraries — and it is the same
   serial stage behind the frozen `scanning...` indicator in TASK-0317.
   Tracked as **TASK-0321**.
2. **Large files have no intra-file parallelism.** An earlier draft of this note
   claimed BLAKE3 parallelizes within a file; that is **false for this build** —
   `blake3` is compiled without its `rayon` feature and `engine.rs` calls the
   serial `update()`, so one file is hashed by one core. `update_rayon` above a
   size threshold is compatible with this ADR (which rules out mmap, not
   threaded hashing over the existing streaming buffer). Tracked as
   **TASK-0322**.

### Consequence note — deferred verification pass (2026-08-01, TASK-0315)

Progress indication (FR-19) restructured the engine into a full hashing pass
followed by a single verification pass, so that the user-facing phase toggles at
most once each way. The verify pass therefore accumulates every non-singleton
hash group across **all** size cohorts before verifying any of them, where
previously each cohort's groups were verified and freed before the next cohort
was hashed. Peak metadata goes from *max over cohorts* to *sum over cohorts* —
still `O(file count)`, so within the bound this ADR sets ("memory scales with
file count, not data volume"), and small in absolute terms. Two consequences to
keep in view: it widens the hash→verify TOCTOU window (apply-time re-validation
in TASK-0309 is the mitigation), and NFR-4 memory profiling in S4 must measure
this shape rather than the original streaming one. Recorded because a display
concern changed the engine's execution shape, which TASK-0315 otherwise warned
against.

### Consequence note — the scan stage is no longer serial (2026-08-03, TASK-0321)

The prior note above named `sniff::filter_candidates`'s serial loop as an
unexploited throughput lever. It is now parallelized with rayon
(`into_par_iter().map().collect()`, the same pattern this ADR already documents
for hashing), so **this ADR's "Parallelism is confined to hashing" framing is
now out of date**: the scan stage (magic-byte confirmation) is parallel too, and
`RAYON_NUM_THREADS` bounds its concurrency exactly as it already bounded
hashing's.

**Measured, external, on a synthetic 60k-file fixture built so every candidate
fails confirmation** (isolates scan cost from hashing, mirroring the "no file is
hashed" control fixture the prior note used) — before/after release-build
binaries, 10-core machine:

| Cache state | Before (serial) | After (parallel) | Speedup |
|---|---|---|---|
| Cold | 158.7 s | 21.5 s | ~7.4x |
| Warm | 6.1 s | 1.8 s | ~3.3x |

The larger cold-cache speedup is expected: concurrent I/O submissions overlap
their wait time, so the win exceeds what core count alone would predict.

**Determinism mechanism, and why it is a second, distinct one from the rest of
this ADR:** the canonical sort (this ADR's stated determinism source) applies to
the dedup engine's output, not to `filter_candidates`, whose contract is
**input-order preservation** — confirmed files and warnings must come back in
exactly the order they were supplied (the human report in `report.rs` prints
warnings unsorted, in this order; only the `--json` path sorts). This is
guaranteed by construction, not by luck: `Vec::into_par_iter()` is an
`IndexedParallelIterator`, and rayon's `collect` into a `Vec` from an indexed
iterator splits the destination by index and writes each worker's slice
in-place — the result is ordered by input position regardless of which worker
finishes first. Verified directly against the crate source during architect
review, and pinned by a unit test
(`filter_candidates_order_is_identical_at_single_and_multi_thread_widths`) that
asserts *exact* input order (not just "the two widths agree with each other") at
1 and 8 threads.

**The seek-thrashing risk this ADR reserved the right to bound (above) is live
here for the first time.** Until today the scan stage was width-1 by
construction; it is now `cores`-wide concurrent random opens across a directory
tree, and the measurement above ran entirely on flash. Spinning-disk behavior is
untested and unmeasured — `RAYON_NUM_THREADS` remains the escape hatch, exactly
as for hashing, if that ever needs bounding.

**One throughput lever this measurement plausibly surfaces but does not
confirm:** with sniff's cost cut ~3-7x, `enumerate` (walkdir, still fully
serial) is a plausible new dominant serial stage in the small-file regime — the
TASK-0312 finding named enumeration and sniff together as the dominant pair, and
only sniff has been addressed. Not re-measured here; worth a control-fixture
re-run before deciding whether it is worth its own task.

### Consequence note — intra-file parallel hashing adopted, threshold-gated (2026-08-04, TASK-0322)

The prior note above named the missing intra-file parallelism as an unexploited
lever: `blake3` was compiled without its `rayon` feature and `engine.rs`'s
`hash_file` called the serial `update()`, so one large file was hashed by one
core regardless of how many idle cores the run had. TASK-0322's brief was to
**measure before adopting** rather than assume the ADR's original "we use the
`blake3` crate's parallel update" framing (written before Phase 3 implementation)
was actually true.

**Method.** A standalone benchmark (not checked in) measured `update()` vs
`update_rayon()` on a 10-core machine, isolating exactly the interaction this
ADR's determinism/concurrency model cares about: **nested** rayon parallelism,
since `engine.rs` already runs one `rayon` parallel iterator over each same-size
cohort (the "outer" level) — the question is whether adding a *second*,
intra-file rayon call (the "inner" level) inside that closure helps, hurts, or
does nothing, and how that answer depends on how many files the outer level
currently has in flight.

**Findings.**

1. **Single large file, no outer contention (cohort of 1 — the case this ADR
   originally called out):** `update_rayon` with a 4 MiB read buffer sustained
   ~3.1–3.9x the throughput of serial `update()` with the existing 64 KiB
   buffer, consistently from 10 MiB to 500 MiB file sizes.
2. **Small files in isolation:** below roughly 128 KiB, `update_rayon`'s
   per-call join overhead is not recovered by the (too little) parallel work —
   measured ~2x *slower* at 32 KiB. From 256 KiB up it is a net win even in
   isolation (1.5x at 256 KiB, growing with size), so the breakeven point sits
   around 128–256 KiB.
3. **Cohort of N same-size large files (200 MiB each), outer rayon already
   spreading them across cores:** at N=1–4 (fewer files than cores),
   `update_rayon` still wins, 1.5x–3.2x. At N=8–16 (at or past core count, no
   outer spare capacity left), the two converge to parity — never a measured
   regression beyond ~6%, just a plateau where the inner call stops helping.
4. **Cohort of N same-size *medium* files (4 MiB, the realistic case — most
   duplicate groups are a handful of ordinary photos, not dozens of
   multi-hundred-MB files):** at N=2–8, `update_rayon` wins outright. At N=32
   and above (hundreds of exact-byte-size-matching files hashed at once — an
   extreme case), the regression is bounded at a few percent (measured up to
   ~6% at N=512), never a real loss.

**Decision: adopt `update_rayon`, gated by file size, not adopted
unconditionally.** `engine.rs` now branches in `hash_file`: files at or above
`RAYON_HASH_THRESHOLD` (1 MiB — comfortably past the measured ~128 KiB
breakeven, with margin) stream through a 4 MiB buffer (`RAYON_HASH_BUF_LEN`,
the measured throughput sweet spot) calling `update_rayon`; files below it keep
the original 64 KiB/`update()` path untouched, since that is where the
per-call overhead is a real (if small) loss and the vast majority of ordinary
photo files live. This keeps the common case (many small-to-medium images)
byte-for-byte on the previously-measured path while capturing the large upside
for the case this ADR flagged as unaddressed — a handful of large duplicate
files (RAW, TIFF, PSD, video) sharing one core when idle cores were available.

**Compatible with this ADR's memory bound.** `RAYON_HASH_BUF_LEN` (4 MiB) is a
larger constant than `STREAM_BUF_LEN` (64 KiB), but the ADR's framing already
covers this: "buffer count is bounded by the worker-pool width, not by data
volume." At rayon's default pool width this adds tens of MB at most, not a
volume-dependent cost.

**Correctness.** BLAKE3 guarantees the digest depends only on the input bytes,
never on how a caller chunks calls to `update`/`update_rayon` — pinned by a new
regression test (`hash_file_matches_reference_hash_across_threshold_boundary`)
asserting `hash_file` agrees with `blake3`'s own one-shot hasher at sizes just
below, exactly at, and just above `RAYON_HASH_THRESHOLD`.

**Not addressed here:** byte-for-byte verification (`files_equal`, ADR-0008's
sole removal authority) is a raw comparison, not a hash, so `update_rayon` does
not apply to it — out of scope for TASK-0322, which named hashing specifically.

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
