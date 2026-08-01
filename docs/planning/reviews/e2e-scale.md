# Review log — Slice S4 `e2e-scale` (`feature/e2e-scale`)

- **PRD:** [PRD-0001](../../prd/PRD-0001-exact-image-deduplication.md)
- **Slice scope:** TASK-0312 — every acceptance criterion verified through the
  real CLI, plus the NFR-4 scale validation and the empirical concurrency
  decision (ADR-0010). AC-1..AC-19, AC-18a; NFR-3, NFR-4, NFR-5.
- **Status:** closed — converged; slice complete and opened as a PR (2026-08-02).

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

Development machine (macOS, filesystem behind a sandbox layer), release build.
Fixtures in `tests/scale.rs`, `#[ignore]`d, run via `make scale`.

**Method**, so TASK-0314 can reproduce this on the owner's hardware: each row is
`/usr/bin/time -l ./target/release/imgdedup --json --no-progress <fixture>`, run
after the fixture was built, single run per row unless noted. The 100k row was
repeated three times (296 s, 273 s, 296 s) and the 10k row was measured both
immediately after creation and again once cached. No explicit cache purge was
performed — "uncached" below means "first read of a fixture larger than the cache
this environment gave us", which is an observation rather than a controlled
condition.

| Fixture | Files | Data | Wall | User | Sys | Peak RSS |
|---|---|---|---|---|---|---|
| Small, cached | 10,000 | 40 MB | 1.77 s | 0.43 s | 3.24 s | **11.1 MB** |
| Small, uncached | 100,000 | 397 MB | 296 s | 2.45 s | 33.7 s | **42.8 MB** |
| Large | 3 | 300 MB | 0.08 s | 0.16 s | 0.02 s | **7.5 MB** |
| Large, volume control | 3 | 3 MB | — | — | — | **7.5 MB** |

### Memory: confirmed, and quantified

Peak RSS is **≈7.5 MB baseline + ~350 B per file** — 11.1 MB at 10k and 42.8 MB
at 100k are 3.6 MB and 35.3 MB above baseline, near-perfectly linear in file
count. That projects to roughly **0.36 GB at one million files**.

The last two rows are the control the architect asked for: holding the file count
at 3 and changing the data volume by 100× (3 MB → 300 MB) moves peak RSS by
nothing measurable, 7.49 MB against 7.45 MB. Volume-independence is therefore
*shown*, not inferred from a comparison that moved both variables at once. This
is the axis where cache state cannot confound the result: there is no `mmap`
anywhere and every read goes through a fixed buffer, so page-cache residency
cannot inflate RSS.

### Throughput: no target can be set from this machine

The 100k run took 296 s of which **2.45 s was user CPU and 33.7 s system** — about
12% utilization, the rest blocked. That utilization ratio, not any comparison
against another tool, is what supports "this machine's absolute numbers are not a
target".

An earlier draft leaned on a `tar cf /dev/null` control (524 s over the same
tree). That comparison is **withdrawn as evidence**: `tar` is single-threaded and
reads 100% of bytes, while `imgdedup` reads a 64-byte header per file and full
content only for size-cohort members, so it confounds threading with workload.
The direction reproduces, but it does not license the conclusion.

### Concurrency: reopened — this measurement cannot answer it

The original conclusion here was that widening concurrency does not help. **That
was not supported by the experiment**, and the architect review demonstrated why.

The 1-vs-8-worker comparison (1.97 s → 1.72 s) was taken on a warm 10k fixture —
the regime with no I/O latency to hide, where extra workers are expected to do
nothing. Worse, the parallel stage is barely present in that workload at all: a
control fixture built so that **no file is hashed** costs the same wall time as
one hashing ~1,800 files, because `enumerate` (walkdir) and
`sniff::filter_candidates` are **serial** — one open-and-read per file — and they
dominate. Rayon's width governs only the hashing stage.

So "8 threads only bought 13%" is evidence that the parallel stage is a rounding
error in the measured workload, not evidence about the knob. The general claim
"I/O-bound, therefore concurrency cannot help" is also wrong: latency-bound work
is the classic case where more outstanding requests *do* help.

**What stands:** keep rayon's default and add no `--jobs` flag — as the status
quo, not as a measured optimum. `RAYON_NUM_THREADS` remains the escape hatch (no
custom pool is built anywhere; the scale suite relies on it). Re-measure with
TASK-0314 on ordinary hardware, over a workload where the parallel stage is a
meaningful share. Nothing here touches the axis ADR-0010 originally worried
about — needing to *lower* width to avoid seek thrashing on spinning disks.

**Two throughput levers this surfaced, neither exploited:** the serial scan stage
(**TASK-0321**, also the stage behind TASK-0317's frozen `scanning...`), and the
absence of intra-file parallelism for large files (**TASK-0322** — `blake3` is
compiled without its `rayon` feature here, so one file is hashed by one core).

### Coverage honesty

"One test per criterion" is true of the mapping but three rows are partial, and
the map now says so:

- **AC-3** — recoverability is verified on Linux and Windows only; on macOS the
  test prints a note. CI now sets `IMGDEDUP_REQUIRE_TRASH_CHECK` on the
  non-macOS jobs so a trash that cannot be listed there is a failure rather than a
  silent downgrade to no coverage anywhere.
- **AC-17** — the criterion's own scenario (a non-keeper modified between scan
  and apply) is *not* exercised end to end; the suite substitutes a different
  failure mode (a removal that fails). Accepted: a deterministic end-to-end race
  would need a test hook in production code, which is not worth adding to a
  destructive tool. The core clause is covered at unit level.
- **AC-21** — the "zero-workload phase is skipped" clause lives in `engine.rs`
  tests, not only in `progress.rs` as first recorded.

## Architect review — whole slice (2026-08-02)

**Verdict: changes requested → all resolved in-unit.** Two blockers, both against
things *I* wrote rather than against the tester's suite, plus a set of should-fixes
concentrated on assertions that could not fail. The reviewer ran their own bounded
5,000-file benchmarks to attack the measurement rather than taking it on trust.

| # | Severity | Finding | Resolution |
|---|---|---|---|
| 1 | blocker | The ADR-0010 note claimed large files are "already parallelized inside BLAKE3". **False for this build** — `blake3` is compiled without its `rayon` feature and `engine.rs` calls the serial `update()`, so one file is hashed by one core. An ADR is the durable record; it must not carry a false premise, and this one inverted the finding: few-huge-files is exactly where an unexploited lever exists. | Fixed — claim removed, measured fact recorded, lever tracked as **TASK-0322**. |
| 2 | blocker | The concurrency conclusion was not supported: the 1-vs-8-worker comparison ran on a *warm* fixture where the rayon-parallel stage is a rounding error. The reviewer's control — a fixture where **no file is hashed** — cost the same wall time as one hashing ~1,800, because `enumerate` and `sniff` are serial and dominate. "I/O-bound therefore concurrency cannot help" is also wrong in general; latency-bound work is the classic case where more outstanding requests do help. | Fixed — the concurrency half is **reopened**, recorded as status quo rather than measured optimum, with the serial-scan decomposition captured and tracked as **TASK-0321**. The memory half stands. |
| 3 | should-fix | AC-4's two "never grouped" assertions were both vacuous: the report always emits a `Warnings:` header when any warning exists, and it prints absolute paths, so the literal `"REMOVE photo.jpg"` was unsatisfiable regardless of behavior. | Fixed — asserts on the JSON document instead: no group's keeper, removable or protected may end with `photo.jpg`. |
| 4 | should-fix | AC-13 checked that `KEEP` and `REMOVE` both appear, not which file got which — swapping the labels in the renderer would pass. | Fixed — asserts the exact `KEEP  <path>` / `REMOVE <path>` lines. |
| 5 | should-fix | AC-2 quoted a deterministic keeper rule but asserted only set membership, which a random keeper would satisfy. | Fixed — pins the keeper the rule selects. |
| 6 | should-fix | AC-14's hidden half was count-only: an inverted filter including the hidden pair and excluding the visible one still yields 1 group. | Fixed — asserts membership. |
| 7 | should-fix | The NFR-4 headline test asserted `groups > 0` — finding 1 of ~10,000 planted duplicates would pass. | Fixed — asserts the exact count, with a note on why accidental collisions are impossible in that fixture. |
| 8 | should-fix | The macOS gap's mitigation ("CI covers all platforms") was assumed, not enforced: on Linux/Windows a failure to enumerate the trash printed a note and passed, so every platform could silently degrade to no coverage. | Fixed — CI sets `IMGDEDUP_REQUIRE_TRASH_CHECK` on the non-macOS jobs, making that branch fatal there. |
| 9 | should-fix | Exact path equality against platform trash metadata will not survive Windows (8.3 short names, `\\?\` prefixes) — a red job in AC-3, and a silently vacuous assertion in AC-7. | Fixed, and the prediction was right for a different reason: the first CI run **did** fail on Windows, but on the item *name*, not the parent path. The Recycle Bin reports display names with known extensions hidden, so `b.png` came back as `b`. Both sites now match on the file stem plus the tempdir's unique final component. |
| 10 | should-fix | Scale fixtures used fixed names in the shared temp dir: collides between runs, `remove_dir_all` on a predictable path in a world-writable `/tmp`, and leaks ~400 MB on any panic since cleanup only ran on success. | Fixed — `TempDir` for RAII cleanup and uniqueness; the path is printed for debugging. |
| 11 | should-fix | The planning/status update did not ride the slice. | Fixed. |
| 12 | should-fix | The suite froze a doc/behavior divergence: `--delete` without `--apply` exits 1, while the guide documents 2 for usage errors. | Tracked as **TASK-0323** for the product-owner — catching this is what an acceptance layer is for. |
| 13 | should-fix | The `tar` control did not license its conclusion: `tar` is single-threaded and reads every byte, so it confounds threading with workload. "Uncached" was asserted without a stated method. | Fixed — the control is withdrawn as evidence, the 12% utilization ratio leads instead, and the method is now recorded so TASK-0314 can reproduce it. |
| 14 | should-fix | Three coverage rows were nominal rather than full (AC-3 macOS, AC-17's substituted scenario, AC-21's misattributed clause). | Fixed — the map now states each partial explicitly. |
| 15 | should-fix | The memory conclusion was the sound half but recorded only qualitatively, and the 3-file/100k comparison moved two variables at once. | Fixed — recorded as a linear model (~7.5 MB + ~350 B/file, ≈0.36 GB at 1M files) and a volume-only control added: 3 files at 3 MB and at 300 MB both peak at 7.5 MB. |
| 16–22 | nit | AC-9's fixture had no unique-size file to exercise its own claim; AC-1 asserted count not membership; a hard-coded absolute path is drive-relative on Windows; the widths test could compare two empty documents; the Makefile overstated fixture size; `SCALE_FILES` should be env-overridable so CI can exercise the code path cheaply. | Addressed; `SCALE_FILES` override and the CI smoke run left for TASK-0314's session, noted here rather than silently dropped. |

**Confirmed sound by the reviewer:** AC-9's independence claim is true — the expected
grouping is built by bucketing raw bytes in the test, with nothing from `src/`
participating; AC-15 and AC-16 are honest (AC-16 is stronger than its criterion);
the AC-20/22/23/24 pointers are accurate; `make scale` correctly sits outside
`validate` while `cargo test` still compiles the ignored tests so they cannot
bit-rot; and the memory claim survives attack — no `mmap` anywhere, all reads
through fixed buffers, so page-cache state cannot inflate RSS.
