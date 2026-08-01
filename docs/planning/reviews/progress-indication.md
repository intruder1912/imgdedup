# Review log — Unit `progress-indication` (`feature/progress-indication`)

- **PRD:** [PRD-0001](../../prd/PRD-0001-exact-image-deduplication.md),
  [Amendment 2026-07-19](../../prd/PRD-0001-exact-image-deduplication.md#amendment-2026-07-19--progress-indication--stored-results)
- **Unit scope:** TASK-0315 — FR-19 live progress indication on stderr
  (AC-20..AC-24). A one-unit feature branch, not a PRD slice: FR-19 arrived by
  amendment after the S1..S4 carve and is a single coherent requirement.
- **Status:** open — in review

In-progress review log for this unit
([ADR-0013](../../adr/0013-prd-slice-delivery-cadence.md)). The product-owner
functional acceptance is recorded first, then the orchestrator's field finding
that sent the build back, then the architect's whole-unit review.

## Product-owner acceptance — FR-19 / AC-20..AC-24 (2026-08-01)

**Verdict: accept.** Checked against the shipped code rather than the spec:
`src/main.rs` (the sink is constructed once over stderr and `finish()`ed before
the single `print!` of the report/JSON, so progress cannot interleave with
stdout in any mode), `src/progress.rs` (the TTY and non-TTY render paths and
their unit tests), and `src/cli.rs` (`--no-progress` as a default-off opt-out).

- **AC-20** stdout purity — satisfied by construction; the renderer only ever
  writes to the stream it was constructed with, which is stderr in production.
- **AC-21** TTY — phase, `x of y`, and a percentage on a single throttled
  in-place line.
- **AC-22** non-TTY — no carriage returns or ANSI codes; bounded plain lines at
  phase transitions and 10% steps.
- **AC-23** opt-out — `--no-progress` selects a sink that holds no writer.
- **AC-24** output-only — the sink is a pure observer with no path back into
  grouping or results.

This answers the owner's amendment complaint directly: the run now says how big
the job is (the candidate total) and how far along it is (phase + percentage).

**Deferred product observations** (neither blocks this unit):

1. The phase labels `scanning` / `hashing` / `verifying` are accurate but
   implementation-flavored; `hashing` presumes the reader knows why hashing
   happens. Worth revisiting against real user feedback, and explicitly allowed
   by the PRD ("user-facing labels, not an implementation contract").
2. Non-TTY milestone density (phase transitions + 10% steps) reads as sensibly
   bounded, but has not been observed on a run at the NFR-4 scale target
   (100k+ files). Worth a dogfood note when TASK-0312/0314 run at scale.

## Orchestrator field finding — progress stalled through the slow phase (2026-08-01)

**Finding (defect, fixed in this unit).** The product-owner's acceptance and the
CLI acceptance tests both passed while the feature still failed its own purpose.
Running the real binary on a realistic tree — 300 unique PNGs plus one duplicate
pair, stderr piped — produced:

```text
scanning...
scanning: 0 of 301 (0%)
hashing: 0 of 301 (0%)
hashing: 299 of 301 (99%)
verifying: 299 of 301 (99%)
verifying: 301 of 301 (100%)
```

The hashing phase sat at 0% for its entire duration and then jumped to 99% in a
single advance, because hash-unique files were only counted in bulk after
`group_by_hash` had consumed the whole hashing pass. Hashing is the dominant
phase on a real photo library, so on a 100k-file run the user would watch 0%
for minutes — precisely the "no sense of how far along" complaint that motivated
FR-19. The counting model satisfied the letter of AC-21 while defeating its
intent.

**Why the tests missed it.** Every fixture used to validate the unit consisted
of duplicate pairs, where all files are group members and therefore all advance
in the verify phase. Absent a mostly-unique tree — the common real-world shape —
the bulk-advance behavior never showed.

**Resolution.** Advances moved to one-per-file inside the parallel hashing
closure, so the hashing phase animates smoothly against `y` = the confirmed
candidate count (AC-21 unchanged). The verifying phase was given its own
denominator — the number of files actually entering byte-verification — rather
than being pinned at "301 of 301 (100%)" while still working, which would read
as finished-but-hung. A regression test asserts the hashing phase emits
incremental advances rather than one jump.

**Second defect, found while re-measuring the fix.** `record()` snapshotted the
phase and counters *before* acquiring the state lock, so two rayon threads could
interleave such that the thread holding a stale, lower reading acquired the
write lock second and printed a percentage below one already shown. The snapshot
now happens inside the critical section, so reading the counters, deciding to
print, and updating the last-shown bookkeeping are one atomic step and printed
milestones are monotonically non-decreasing. Pinned by a regression test that
drives concurrent advances through `std::thread::scope`.

Observed cadence after both fixes, same tree, piped stderr — 15 lines, monotonic,
with verification reporting its own workload:

```text
scanning...
scanning: 0 of 302 (0%)
hashing: 0 of 302 (0%)
hashing: 32 of 302 (10%)
...
hashing: 302 of 302 (100%)
verifying: 0 of 2 (0%)
verifying: 2 of 2 (100%)
```

## Architect review — whole unit (2026-08-01)

*Pending.*
