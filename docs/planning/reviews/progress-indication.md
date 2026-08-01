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
candidate count. The verifying phase was given its own
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

**Verdict: request-changes** → resolved in-unit (see the resolution column).
Thirteen findings over the ~1500-line diff, two of them user-visible and
empirically reproduced under a pty rather than reasoned about.

| # | Severity | Finding | Resolution |
|---|---|---|---|
| 1 | should-fix | On a TTY the live progress line is corrupted by the run's own `error:` output — the user sees `scanning...error: /x: No such file…`. Invisible to the suite because every CLI test is non-TTY. Regression risk grows with `--apply`/`--delete` (S3), which add more stderr writers inside the run window. | Fixed — root errors print after the sink is finished, and the module doc states the rule that nothing else may write to stderr while a TTY line is live. |
| 2 | should-fix + product ratification | The per-phase denominator is a genuine AC-21 deviation, not a defensible reading — and worse, on a TTY the in-place redraw *erases the candidate count*, so a 302-file run rests at `verifying: 2 of 2 (100%)`, losing FR-19(a) "overall problem size". A duplicate-free tree ends at the degenerate `verifying: 0 of 0 (100%)`. | Fixed + ratified — the verify line keeps the run total visible, the phase is skipped when its workload is zero, and a `done:` completion state renders the candidate total. Product-owner amended AC-21/AC-22. |
| 3 | should-fix | `record()` takes two mutexes *before* the throttle check, and the repaint's `write` + `flush` happen while holding the lock against unbuffered stderr. A slow terminal or a pipe applying backpressure blocks every rayon worker. Steady-state cost is negligible (<0.1% at 100k files) — the blocking repaint is the real NFR-4 hazard. | Fixed — lock-free time pre-check before any lock, single `write_all` per repaint. **Behavior change worth knowing:** the non-TTY path previously had no time throttle at all, so a very fast run now emits fewer intermediate milestone lines (a sub-interval hashing pass can go straight from its 0% line to `done:`). Still AC-22-conformant — a problem-size line and a completion line always appear — and a ~12 s run still shows genuine intermediate milestones. |
| 4 | should-fix | `total`/`total_known` are independent relaxed atomics written outside the lock (tearing window → spurious `0 of 0 (100%)`), and `phase` is a second mutex taken in the opposite order from `record()` — an AB/BA deadlock prevented only by a temporary guard's drop point. The `Relaxed` ordering on `current` was confirmed *correct*, as was the monotonicity fix. | Fixed — `total`, `total_known` and `phase` moved into the lock-protected state; `current` stays the sole atomic for the fast path. |
| 5 | should-fix | `Instant::now() - TTY_REPAINT_INTERVAL` in `Progress::new` panics if the process starts within 100 ms of boot — the only line able to violate "progress must never fail a run that would otherwise succeed". | Fixed structurally rather than guarded — no subtraction from `Instant::now()` remains anywhere in the module; a construction-time `start` plus a zero-valued "due now" sentinel replaced it. |
| 6 | should-fix | The retained `find_duplicates` wrapper is production-dead, and the test claiming to prove AC-24 compares a function against itself-with-two-extra-frames — incapable of failing for the reason it claims to guard. | Fixed — `#[cfg(test)]` so the compiler proves production never calls it; the tautological test now runs a real `Progress` sink against `NoProgress` and compares outcomes. |
| 7 | should-fix | The throttle test pins only its negative half — an implementation that never repaints, or one with a one-hour interval, passes. The animation FR-19 exists for was not pinned by any test, and `\x1b[K` was never positively asserted despite the line legitimately shrinking. | Fixed — the interval is injectable, so both halves are deterministic; added the positive erase-to-EOL assertion. |
| 8 | should-fix or defer | The `scanning` phase has the same defect just fixed in `hashing`: it is frozen while enumeration *and* magic-byte sniffing run (~200k system calls at 100k files, 5–15 s static). Spec-conformant, but the intent argument that sent the build back applies. | Deferred by decision — tracked as **TASK-0317**. |
| 9 | nit | Unchecked `usize` subtraction in progress accounting can panic in debug / wrap in release if cohort construction ever changes. | Fixed — `saturating_sub`. |
| 10 | nit | The verify pass no longer streams per cohort: peak metadata goes from max-over-cohorts to sum-over-cohorts, and the hash→verify TOCTOU window widens. Within ADR-0010's bound, but it is the engine's execution shape changed by a *display* concern. | Accepted with a consequence note added to ADR-0010. |
| 11 | nit | The denominator is derived independently in `main.rs` and `engine.rs`; a future caller filtering between them silently puts the two out of sync. | Fixed — the engine sets its own total. |
| 12 | nit | Doc claims wider than the code: monotonicity is deliberately false across the `retotal` boundary, and the trait's "safe to call concurrently" overstates `set_total`/`retotal`. | Fixed — both qualified. |
| 13 | nit | Task file still `Status: todo`; this review section was pending; the guide described `y` in AC-21's words, which the verify phase does not exhibit. | Fixed — all three. |

### Confirmation pass (2026-08-01) — verdict: approve-with-nits

A second architect dispatch verified the fix round against the code rather than
the summary (full suite, clippy, fmt, and eight live binary runs — four under a
pty — plus a 1502-file/194 MB tree to exercise a hashing phase long enough to
animate). This is a deliberate deviation from the batch plan's "one architect
dispatch per feature unit": the first review returned request-changes and the
fixes substantially rewrote the concurrency-critical module, so confirming that
the rewrite had not traded one class of defect for another was worth the second
dispatch.

Findings 1–7, 9, 11, 12, 13 confirmed genuinely resolved; the rewritten module
was judged materially simpler and safer than what was first reviewed (one mutex
instead of two, no lock-ordering hazard, no panic path, a lock-free hot path,
and forced repaints that cannot be throttled away), with no new race, deadlock
or panic constructible. Eight new items, none blocking:

- **N1 (should-fix, resolved)** — a real conformance miss against the AC written
  in this same round: the hashing phase switch was unconditional, so an empty
  tree printed `hashing: 0 of 0 (100%)`. The degenerate line removed from
  verifying still existed one phase earlier. Now guarded.
- **N2 (resolved, orchestrator)** — the amended AC-21 and the guide both claimed
  scanning reports `x of y`, which it never does now that `set_total` is silent
  and lives in the engine. Corrected in the PRD and the guide: scanning is
  indeterminate, hashing carries the problem-size line.
- **N3–N6, N8 (resolved)** — the default repaint interval was pinned by no test
  (a one-hour constant would have passed the suite); `finish()` made idempotent
  and `Drop`-guarded so the finding-1 fix survives future early returns in
  `run()` — the architect's top pick to land before S3; `try_lock` in `advance`
  to remove the residual convoy; the em dash replaced (it was the only
  non-ASCII byte the binary ever writes, and no test asserted it); unchecked
  `now + interval` add made saturating.
- **N7 (tracked)** — the TASK-0317 deferral was priced off an estimate that
  measurement does not support; the sizing note is now on the backlog row.

Confirmed answers to the two open questions: the "nothing else writes to stderr
while a TTY line is live" rule was enforced *by inspection only*, which is why
N4's `Drop` guard matters before S3 inserts apply/delete diagnostics into
exactly that window; and the non-TTY throttle change is AC-22-conformant —
milestone density is now bounded by time rather than file count, so a 100k-file
log looks like a 1k-file one, and only milestones on runs too fast to read are
lost.

**Security gate: clean.** No new dependency (`std::io::IsTerminal`; `Cargo.toml`
and `Cargo.lock` unchanged in the diff). Nothing path-derived reaches the
terminal through the progress path — lines are built from static phase labels
and integers only — so a hostile filename cannot inject terminal escapes via
progress. Buffering is bounded per repaint and non-TTY line count is capped by
the bucket logic. **Informational, pre-existing:** `main.rs` prints
`err.path.display()` raw, which *is* an escape-injection surface for a hostile
path; FR-19 makes stderr a stream we actively draw on, which raises the value of
sanitizing it — tracked as **TASK-0318**, not a change in this unit.

**ADR adherence.** ADR-0008 semantics untouched. ADR-0010 determinism traced and
preserved: reordering `process_hash_group` only permutes insertion order, and
`sort_canonical` re-establishes canonical order with no reachable stable-sort
tie. ADR-0005 TDD visibly followed — both in-unit defects landed with a
regression test. No new ADR needed; finding 2 required a PRD amendment.
