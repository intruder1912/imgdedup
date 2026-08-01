# Review log — Slice S4 `e2e-scale` (`feature/e2e-scale`)

- **PRD:** [PRD-0001](../../prd/PRD-0001-exact-image-deduplication.md)
- **Slice scope:** TASK-0312 — every acceptance criterion verified through the
  real CLI, plus the NFR-4 scale validation and the empirical concurrency
  decision (ADR-0010). AC-1..AC-19, AC-18a; NFR-3, NFR-4, NFR-5.
- **Status:** open — in review

## Tester delivery — the acceptance suite (2026-08-01)

`tests/acceptance.rs`: 21 tests, one per criterion, driving the compiled binary
over temp-dir fixtures so coverage is traceable criterion by criterion rather
than inferred from scattered unit tests. AC-19's binary-level test lands here as
intended — it was deliberately deferred out of slice S1 to this task.

Two choices worth recording:

- **AC-9 derives its expected grouping independently**, by bucketing raw bytes
  in the test rather than asking the product what it thinks the answer is. A
  test that computes its expectation the same way the code does proves nothing.
- **AC-17 does not fake a race.** A genuine scan-then-mutate window cannot be
  driven from outside the process, so the suite asserts the reachable surface (a
  removal that fails is reported, the file survives, other groups still apply)
  and points at the per-branch TOCTOU coverage in `src/apply.rs`'s unit tests.

**Disclosed gap — AC-3 recoverability is unverifiable on macOS.** The suite
proves a trashed file is recoverable using `trash::os_limited::list()`, which
the crate supports only on Linux and Windows. On macOS the test prints a note
instead. This is not theoretical: the tester mutation-tested a "trash silently
becomes permanent delete" wiring bug and **it passed on macOS**, precisely
because the recoverability assertion is skipped there. On Linux and Windows the
same mutation fails. CI covers all three platforms, so the gap is covered where
it counts — but a developer running the suite only on a Mac would not catch that
class of defect.

The tester mutation-tested the three most load-bearing tests: breaking size
pre-grouping and breaking per-candidate failure handling both fail the suite as
claimed.

## NFR-4 scale measurement (2026-08-02)

Run on the development machine (macOS, sandboxed filesystem), release build.
Fixtures are in `tests/scale.rs`, `#[ignore]`d and run via `make scale` — they
build 100k files and take minutes, which is too slow to gate every PR.

| Fixture | Files | Data | Wall | User | Sys | Peak RSS |
|---|---|---|---|---|---|---|
| Small, cached | 10,000 | 40 MB | 1.77 s | 0.43 s | 3.24 s | **11.1 MB** |
| Small, uncached | 100,000 | 397 MB | 296 s | 2.45 s | 33.7 s | **42.8 MB** |
| Large | 3 | 300 MB | 0.08 s | 0.16 s | 0.02 s | **7.5 MB** |

**Memory scales with file count, not data volume — confirmed.** This is the
NFR-4 claim that actually matters, and the last two rows are the experiment that
tests it: near-identical data volume (397 MB vs 300 MB), file counts differing by
a factor of 33,000, and peak memory of 43 MB against 7.5 MB. Had reads been
buffered whole rather than streamed, the three-file run would have needed
hundreds of megabytes. It needed 7.5 MB while hashing 100 MB files.

**The absolute throughput number here is worthless, and should not be quoted as
a target.** The 100k run's 296 s is almost entirely I/O wait — 2.45 s of user CPU
across 100,000 files. The decisive control: `tar cf /dev/null` over the *same*
tree took **524 s**, nearly twice as long as `imgdedup` took to enumerate, sniff,
hash and verify it. The filesystem in this environment is the bottleneck, not the
tool. A real throughput target has to be measured on ordinary hardware; that
belongs with the field validation (TASK-0314), which needs the owner's machine
anyway.

**The small-file path is I/O-bound, not CPU-bound.** 100k files cost 2.45 s of
user time and 33.7 s of system time — the syscall cost of opening and reading
dominates the hashing. Widening concurrency cannot fix that.

### Empirical concurrency decision (ADR-0010)

ADR-0010 left the concurrency width to be set from benchmark data here. Measured
on the 10k fixture, warm:

| Threads | Wall |
|---|---|
| 1 | 1.97 s |
| 8 | 1.72 s |

A 13% gain for eight times the threads, which follows directly from the run
being I/O-bound. On large files the picture differs, but there BLAKE3 already
parallelizes *within* a file, so rayon's width is not the lever either.

**Decision: keep rayon's default width (one worker per core) and add no tuning
flag.** The data does not support hand-picking a number, and a `--jobs` flag
would be surface area in exchange for an effect the measurements do not show.
This is recorded as the empirical outcome ADR-0010 asked for, not as a deferral.

**Determinism across widths — confirmed.** Byte-identical stdout at 1, 2, 8 and
16 threads (`shasum` over `--json` output, all four identical), which is what
keeps concurrency a pure tuning knob (NFR-3, AC-16, ADR-0010). Pinned by
`results_are_identical_across_concurrency_widths`, so a future change to the
default cannot silently move results.

## Architect review — whole slice

*Pending.*
