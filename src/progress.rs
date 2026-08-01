//! Live progress indication (FR-19, TASK-0315, PRD-0001 Amendment 2026-07-19).
//!
//! While a scan runs, the pipeline notifies a [`ProgressSink`] as candidate
//! files reach their final disposition. This module never touches stdout —
//! it renders exclusively to whatever writer it is constructed with (in
//! production, stderr), and is a **pure output concern**: nothing here
//! influences what is scanned, grouped, or removed (AC-24).
//!
//! ## Design
//!
//! - [`ProgressSink`] is the observer trait the dedup pipeline calls into.
//!   [`NoProgress`] is the no-op implementation used when progress is
//!   disabled (`--no-progress`) and by callers/tests that don't care.
//! - [`Progress<W>`] is the real renderer, generic over its output stream so
//!   tests can capture into a `Vec<u8>` instead of needing a real terminal.
//!   TTY-ness is **injected** as a `bool` at construction — never probed
//!   inside this module. The probe (`std::io::stderr().is_terminal()`)
//!   happens exactly once, in `main.rs`. This is what makes the TTY (AC-21)
//!   and non-TTY (AC-22) rendering paths unit-testable without a real
//!   terminal.
//! - The sink is shared behind `&dyn ProgressSink` and called concurrently
//!   from rayon worker threads (hashing runs in parallel, ADR-0010), so
//!   [`Progress<W>`] uses atomics for the counters and a [`Mutex`] around the
//!   writer and its bookkeeping.
//!
//! ## Rendering rules
//!
//! - **TTY:** a single line, redrawn in place (`\r` + erase-to-end-of-line;
//!   never a newline mid-run), showing phase, `x of y`, and a percentage.
//!   Repaints are throttled to at most once per [`TTY_REPAINT_INTERVAL`] so a
//!   fast run does not flood the terminal. [`ProgressSink::finish`] forces one
//!   final repaint (bypassing the throttle) and then terminates the line with
//!   a newline, leaving the terminal clean for whatever prints next.
//! - **Non-TTY:** no carriage returns, no ANSI escapes, ever. Occasional
//!   plain, newline-terminated lines at coarse milestones only — phase
//!   transitions plus [`NON_TTY_PERCENT_STEP`]-sized percentage steps — so
//!   redirected logs stay small and readable.
//! - **Disabled:** use [`NoProgress`] — it writes nothing, anywhere, ever.
//!
//! ## Per-phase denominators
//!
//! The engine (`engine.rs`) advances `x` incrementally, per file, as it is
//! hashed — not in one bulk jump at the end of hashing — so the dominant,
//! slow phase on a real photo library actually animates from 0% to 100%
//! instead of sitting at 0% and then jumping. Verifying is typically a much
//! smaller workload than the full candidate count `y`, so the engine calls
//! [`ProgressSink::retotal`] when it enters that phase: reusing `y` would
//! either overrun it or pin the display at a misleading "100%" while the
//! tool is still working. `retotal` resets the counter and gives the new
//! phase its own, honest denominator.

use std::io::Write;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Minimum interval between in-place TTY repaints (AC-21), so a fast run
/// does not flood the terminal with redraws.
const TTY_REPAINT_INTERVAL: Duration = Duration::from_millis(100);

/// Percentage-point granularity for non-TTY milestone lines (AC-22).
const NON_TTY_PERCENT_STEP: u64 = 10;

/// User-facing phase label shown alongside the progress indicator.
///
/// Phases are a **display concept only** — they do not prescribe or
/// constrain the underlying ADR-0008 pipeline. A phase change is just a
/// change of the label shown next to the same monotonically-increasing
/// `x of y` counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Enumerating and magic-byte-confirming candidate files; the total
    /// (`y`) is not yet known.
    Scanning,
    /// Hashing candidate files (BLAKE3 pre-filter).
    Hashing,
    /// Byte-for-byte verifying candidates against their group's keeper.
    Verifying,
}

impl Phase {
    /// The lower-case label rendered for this phase.
    fn label(self) -> &'static str {
        match self {
            Phase::Scanning => "scanning",
            Phase::Hashing => "hashing",
            Phase::Verifying => "verifying",
        }
    }
}

/// Observer the dedup pipeline notifies as it makes progress (FR-19).
///
/// Implementations must be safe to call concurrently — `advance` in
/// particular is called from rayon worker threads during parallel hashing
/// (ADR-0010) as well as from the sequential orchestration code, so the
/// trait requires `Sync + Send`. Calling any method must never influence
/// pipeline results; progress is strictly an output-only concern (AC-24).
pub trait ProgressSink: Sync + Send {
    /// Fix the denominator (`y`) once it is known — the number of confirmed
    /// candidate files (AC-21). Called at most once per production run.
    fn set_total(&self, total: u64);

    /// Switch to a new user-facing phase.
    fn set_phase(&self, phase: Phase);

    /// Advance the numerator (`x`) by `n`.
    ///
    /// **Invariant:** each candidate file is counted exactly once, at the
    /// point its final disposition is reached, so the sum of every `advance`
    /// call over a run equals the total passed to `set_total`.
    fn advance(&self, n: u64);

    /// Reset the numerator to zero and fix a **new** denominator, for a
    /// phase whose workload differs from the run-wide total set via
    /// `set_total` (e.g. verifying is typically a much smaller workload than
    /// the full candidate count `y`). Without this, a later phase would
    /// either keep counting past a denominator it has already exceeded, or
    /// stay pinned at "100%" while genuinely still working — either reads to
    /// the user as hung or finished when it is neither.
    ///
    /// Does **not** itself trigger a repaint — callers should pair it with
    /// an immediately-following `set_phase` (or `advance`) call so the phase
    /// label and the new denominator change together, atomically, in the
    /// rendered output, rather than flashing a stale phase/total
    /// combination.
    fn retotal(&self, total: u64);

    /// The run is complete: flush and clean up any in-place render state.
    fn finish(&self);
}

/// No-op [`ProgressSink`] used when progress is disabled (`--no-progress`,
/// AC-23) and by callers/tests that don't care about progress. Writes
/// nothing, anywhere, ever.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoProgress;

impl ProgressSink for NoProgress {
    fn set_total(&self, _total: u64) {}
    fn set_phase(&self, _phase: Phase) {}
    fn advance(&self, _n: u64) {}
    fn retotal(&self, _total: u64) {}
    fn finish(&self) {}
}

/// Mutable render bookkeeping, held behind one [`Mutex`] alongside the
/// writer so a repaint reads/writes a consistent snapshot with a single
/// lock acquisition.
struct RenderState<W: Write> {
    writer: W,
    last_paint: Instant,
    last_phase_shown: Option<Phase>,
    /// The last non-TTY percentage *bucket* shown (a multiple of
    /// [`NON_TTY_PERCENT_STEP`]), so repeated advances within the same
    /// bucket don't each print a line.
    last_percent_shown: Option<u64>,
}

/// Live progress renderer (FR-19).
///
/// Generic over the output stream so tests can capture into a `Vec<u8>`
/// instead of needing a real terminal. TTY-ness is **injected** (`tty`),
/// never probed here.
pub struct Progress<W: Write> {
    state: Mutex<RenderState<W>>,
    tty: bool,
    total: AtomicU64,
    total_known: AtomicBool,
    current: AtomicU64,
    phase: Mutex<Phase>,
}

impl<W: Write> Progress<W> {
    /// Construct a new renderer writing to `writer`.
    ///
    /// `tty` says whether the destination stream is an interactive terminal
    /// — probed once by the caller (`main.rs`), never inside this type.
    pub fn new(writer: W, tty: bool) -> Self {
        Self {
            state: Mutex::new(RenderState {
                writer,
                // Backdated so the very first record() is always "due" —
                // the initial phase/state should paint immediately rather
                // than waiting out a full throttle interval.
                last_paint: Instant::now() - TTY_REPAINT_INTERVAL,
                last_phase_shown: None,
                last_percent_shown: None,
            }),
            tty,
            total: AtomicU64::new(0),
            total_known: AtomicBool::new(false),
            current: AtomicU64::new(0),
            phase: Mutex::new(Phase::Scanning),
        }
    }

    /// Snapshot the current phase/current/total/percent and (subject to the
    /// TTY throttle / non-TTY milestone rules, unless `force`) repaint.
    fn record(&self, force: bool) {
        // Acquire the state lock *first* and take the phase/current/total
        // snapshot while holding it, rather than snapshotting beforehand.
        // `advance` is called concurrently from rayon worker threads during
        // hashing (FR-19); if the snapshot were taken outside this lock, two
        // threads could race so that the one with a *staler* (lower)
        // `current` reading acquires the lock second and prints an
        // out-of-order, lower percentage after a fresher one was already
        // shown. Snapshotting inside the lock serializes "read counters,
        // decide whether to print, update last-shown bookkeeping" into one
        // atomic step, so printed milestones are always monotonically
        // non-decreasing (`current` only ever grows via `fetch_add`, or is
        // deliberately reset by `retotal`, which is always paired with a
        // following `set_phase`/`advance` call — never racing a hashing
        // burst).
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());

        let phase = *self.phase.lock().unwrap_or_else(|e| e.into_inner());
        let total_known = self.total_known.load(Ordering::Relaxed);
        let total = self.total.load(Ordering::Relaxed);
        let current = self.current.load(Ordering::Relaxed).min(total);
        // `total == 0` means "nothing to do" — treat that as complete (100%)
        // rather than dividing by zero.
        let pct = if total_known {
            Some(
                current
                    .saturating_mul(100)
                    .checked_div(total)
                    .unwrap_or(100),
            )
        } else {
            None
        };

        if self.tty {
            if !force && st.last_paint.elapsed() < TTY_REPAINT_INTERVAL {
                return;
            }
            let line = Self::line(phase, current, total, pct);
            let _ = write!(st.writer, "\r{line}\x1b[K");
            let _ = st.writer.flush();
            st.last_paint = Instant::now();
            st.last_phase_shown = Some(phase);
        } else {
            let phase_changed = st.last_phase_shown != Some(phase);
            let bucket = pct.map(|p| p - (p % NON_TTY_PERCENT_STEP));
            let bucket_changed = matches!(
                (bucket, st.last_percent_shown),
                (Some(b), shown) if shown != Some(b)
            );
            if phase_changed || bucket_changed {
                let line = Self::line(phase, current, total, pct);
                let _ = writeln!(st.writer, "{line}");
                st.last_phase_shown = Some(phase);
                if let Some(b) = bucket {
                    st.last_percent_shown = Some(b);
                }
            }
        }
    }

    /// Render one status line: `"<phase>: <x> of <y> (<p>%)"`, or
    /// `"<phase>..."` while the total is not yet known (indeterminate).
    fn line(phase: Phase, current: u64, total: u64, pct: Option<u64>) -> String {
        match pct {
            Some(p) => format!("{}: {} of {} ({}%)", phase.label(), current, total, p),
            None => format!("{}...", phase.label()),
        }
    }
}

impl<W: Write + Send> ProgressSink for Progress<W> {
    fn set_total(&self, total: u64) {
        self.total.store(total, Ordering::Relaxed);
        self.total_known.store(true, Ordering::Relaxed);
        self.record(false);
    }

    fn set_phase(&self, phase: Phase) {
        *self.phase.lock().unwrap_or_else(|e| e.into_inner()) = phase;
        self.record(false);
    }

    fn advance(&self, n: u64) {
        self.current.fetch_add(n, Ordering::Relaxed);
        self.record(false);
    }

    fn retotal(&self, total: u64) {
        // No `record()` call here by design (see the trait doc): the caller
        // pairs this with an immediately-following `set_phase`/`advance`,
        // which is what actually triggers the (now consistent) repaint.
        self.current.store(0, Ordering::Relaxed);
        self.total.store(total, Ordering::Relaxed);
        self.total_known.store(true, Ordering::Relaxed);
    }

    fn finish(&self) {
        if self.tty {
            // Force one final full repaint (bypassing the throttle) so the
            // completed state is always visible, then terminate the line so
            // whatever prints next (the stdout report) starts on a fresh
            // line rather than overwriting the progress line.
            self.record(true);
            let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
            let _ = writeln!(st.writer);
            let _ = st.writer.flush();
        }
        // Non-TTY: milestone lines already reflect the final state (the
        // advance that reached the total already crossed the 100% bucket);
        // nothing further to emit.
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn captured(p: &Progress<Vec<u8>>) -> String {
        String::from_utf8(p.state.lock().unwrap().writer.clone()).unwrap()
    }

    // ── NoProgress (AC-23) ───────────────────────────────────────────────

    #[test]
    fn no_progress_methods_do_not_panic() {
        let sink = NoProgress;
        sink.set_phase(Phase::Scanning);
        sink.set_total(10);
        sink.advance(3);
        sink.advance(7);
        sink.retotal(4);
        sink.set_phase(Phase::Verifying);
        sink.advance(4);
        sink.finish();
        // NoProgress holds no writer at all: there is nothing to assert on
        // beyond "this never panics" — by construction it cannot write
        // anywhere.
    }

    // ── TTY rendering (AC-21) ────────────────────────────────────────────

    #[test]
    fn tty_final_state_shows_phase_count_and_percent() {
        let p = Progress::new(Vec::new(), true);
        p.set_phase(Phase::Hashing);
        p.set_total(4);
        p.advance(1);
        p.advance(1);
        p.advance(2);
        p.finish();

        let out = captured(&p);
        // The final (forced) repaint must show the completed x-of-y and %.
        assert!(
            out.contains("hashing: 4 of 4 (100%)"),
            "expected final line with phase/x-of-y/percent, got: {out:?}"
        );
    }

    #[test]
    fn tty_uses_carriage_return_for_in_place_redraw() {
        let p = Progress::new(Vec::new(), true);
        p.set_phase(Phase::Hashing);
        p.set_total(1);
        p.advance(1);
        p.finish();

        let out = captured(&p);
        assert!(
            out.contains('\r'),
            "TTY mode must redraw in place using carriage return"
        );
    }

    #[test]
    fn tty_never_scrolls_mid_run_only_terminates_once_on_finish() {
        // A single-line indicator must not scroll: no newline should appear
        // until finish() terminates the line exactly once.
        let p = Progress::new(Vec::new(), true);
        p.set_phase(Phase::Scanning);
        p.set_total(5);
        p.set_phase(Phase::Hashing);
        p.advance(2);
        p.set_phase(Phase::Verifying);
        p.advance(3);
        p.finish();

        let out = captured(&p);
        assert_eq!(
            out.matches('\n').count(),
            1,
            "expected exactly one newline (from finish's line-terminate), got: {out:?}"
        );
        assert!(out.ends_with('\n'));
    }

    #[test]
    fn tty_indeterminate_state_before_total_known() {
        // Before enumeration completes, y is unknown: show a
        // counting/indeterminate state rather than "x of y".
        let p = Progress::new(Vec::new(), true);
        p.set_phase(Phase::Scanning);

        let out = captured(&p);
        assert!(
            out.contains("scanning...") && !out.contains(" of "),
            "expected an indeterminate scanning state, got: {out:?}"
        );
    }

    #[test]
    fn tty_repaints_are_throttled_not_flooded() {
        // A burst of advances arriving faster than the throttle interval
        // must not each produce their own repaint (AC-21: "does not flood
        // the terminal"). We can't control wall-clock time in a unit test,
        // but we CAN assert that a tight burst does not produce one write
        // per advance call.
        let p = Progress::new(Vec::new(), true);
        p.set_phase(Phase::Hashing);
        p.set_total(1000);
        for _ in 0..1000 {
            p.advance(1);
        }
        let out = captured(&p);
        // Each repaint writes at least one '\r'; 1000 calls in a tight loop
        // running well under 100ms must not yield anywhere near 1000
        // repaints.
        let repaints = out.matches('\r').count();
        assert!(
            repaints < 50,
            "expected throttling to collapse a tight burst of advances, got {repaints} repaints"
        );
    }

    // ── Non-TTY fallback (AC-22) ─────────────────────────────────────────

    #[test]
    fn non_tty_never_emits_carriage_return_or_ansi() {
        let p = Progress::new(Vec::new(), false);
        p.set_phase(Phase::Scanning);
        p.set_total(10);
        p.set_phase(Phase::Hashing);
        for _ in 0..10 {
            p.advance(1);
        }
        p.set_phase(Phase::Verifying);
        p.finish();

        let out = captured(&p);
        assert!(!out.contains('\r'), "non-TTY must never emit CR: {out:?}");
        assert!(
            !out.contains('\x1b'),
            "non-TTY must never emit ANSI escapes: {out:?}"
        );
    }

    #[test]
    fn non_tty_lines_are_plain_and_newline_terminated() {
        let p = Progress::new(Vec::new(), false);
        p.set_phase(Phase::Scanning);
        p.set_total(2);
        p.set_phase(Phase::Hashing);
        p.advance(2);
        p.finish();

        let out = captured(&p);
        assert!(!out.is_empty());
        for line in out.lines() {
            assert!(!line.is_empty());
        }
        assert!(
            out.ends_with('\n'),
            "every non-TTY line is newline-terminated"
        );
    }

    #[test]
    fn non_tty_emits_milestone_on_phase_transition() {
        let p = Progress::new(Vec::new(), false);
        p.set_phase(Phase::Scanning);
        p.set_total(5);
        p.set_phase(Phase::Hashing);
        p.set_phase(Phase::Verifying);

        let out = captured(&p);
        assert!(out.contains("scanning..."));
        assert!(out.contains("hashing:"));
        assert!(out.contains("verifying:"));
    }

    #[test]
    fn non_tty_bounds_percent_lines_to_coarse_steps() {
        // 100 advances of 1/100 must not yield 100 lines — only occasional
        // milestones at NON_TTY_PERCENT_STEP granularity (bounded, log
        // friendly).
        let p = Progress::new(Vec::new(), false);
        p.set_phase(Phase::Hashing);
        p.set_total(100);
        for _ in 0..100 {
            p.advance(1);
        }
        p.finish();

        let out = captured(&p);
        let lines = out.lines().count();
        // At most one line per 10%-step (11 possible: 0,10,..,100) plus the
        // phase-transition line already counted within that budget.
        assert!(
            lines <= 12,
            "expected bounded milestone lines, got {lines}: {out:?}"
        );
        assert!(out.contains("hashing: 100 of 100 (100%)"));
    }

    #[test]
    fn non_tty_percent_milestones_are_monotonic_under_concurrent_advances() {
        // Regression: `advance` is called from rayon worker threads during
        // hashing (real engine usage, FR-19). A racy implementation could
        // snapshot a *stale*, lower percentage in one thread and print it
        // after a fresher, higher one has already been shown by another
        // thread — a confusing "progress went backward" log. Milestone
        // percentages must be monotonically non-decreasing regardless of
        // thread interleaving.
        let p = Progress::new(Vec::new(), false);
        p.set_total(1000);
        p.set_phase(Phase::Hashing);

        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    for _ in 0..125 {
                        p.advance(1);
                    }
                });
            }
        });

        let out = captured(&p);
        let mut last_pct: Option<u64> = None;
        let mut milestones_seen = 0;
        for line in out.lines() {
            let Some(pct_str) = line.rsplit('(').next().and_then(|s| s.strip_suffix("%)")) else {
                continue;
            };
            let pct: u64 = pct_str.parse().expect("percent must parse as u64");
            milestones_seen += 1;
            if let Some(prev) = last_pct {
                assert!(
                    pct >= prev,
                    "percent must never regress: {prev}% then {pct}% in {out:?}"
                );
            }
            last_pct = Some(pct);
        }
        assert!(milestones_seen > 0, "expected at least one milestone line");
    }

    #[test]
    fn non_tty_does_not_repeat_same_percent_bucket() {
        let p = Progress::new(Vec::new(), false);
        p.set_total(100);
        p.set_phase(Phase::Hashing);
        let before = captured(&p).lines().count();

        // Advance in tiny steps that stay within the same 10%-bucket: no
        // additional lines should be emitted.
        for _ in 0..5 {
            p.advance(1);
        }
        let after = captured(&p).lines().count();
        assert_eq!(
            before, after,
            "advances within the same percent bucket must not add lines"
        );
    }

    // ── Phase retotal (defect fix: verify phase must not read as hung) ────

    #[test]
    fn retotal_resets_counter_and_denominator_for_the_new_phase() {
        // Regression: after a phase reaches 100% of the run-wide total,
        // switching phase without resetting would either pin the display at
        // a misleading "100%" (reads as finished-but-hung) or keep counting
        // past an exhausted denominator. `retotal` must reset the counter
        // and give the new phase its own, honest denominator.
        let p = Progress::new(Vec::new(), false);
        p.set_total(10);
        p.set_phase(Phase::Hashing);
        for _ in 0..10 {
            p.advance(1);
        }
        assert!(captured(&p).contains("hashing: 10 of 10 (100%)"));

        p.retotal(4);
        p.set_phase(Phase::Verifying);
        p.advance(2);

        let out = captured(&p);
        assert!(
            out.contains("verifying: 0 of 4 (0%)"),
            "phase transition must show the new, reset denominator immediately \
             (not a stale 100%), got: {out:?}"
        );
        assert!(out.contains("verifying: 2 of 4 (50%)"), "got: {out:?}");
        assert!(
            !out.contains("verifying: 10 of 10"),
            "must never show the old hashing-phase total under the new phase \
             label, got: {out:?}"
        );
    }

    #[test]
    fn retotal_alone_does_not_trigger_a_repaint() {
        // retotal() must not itself paint (see the trait doc) — only the
        // set_phase/advance call that follows it should.
        let p = Progress::new(Vec::new(), false);
        p.set_total(10);
        p.set_phase(Phase::Hashing);
        p.advance(10);
        let before = captured(&p);

        p.retotal(4);
        let after_retotal_only = captured(&p);
        assert_eq!(
            before, after_retotal_only,
            "retotal() alone must not emit any new output"
        );
    }

    // ── Disabled (AC-23) is exercised via NoProgress above; here we assert
    //    that a Progress<W> with tty=false and no calls at all writes
    //    nothing (baseline sanity, not the disabled path itself).
    #[test]
    fn unused_renderer_writes_nothing() {
        let p = Progress::new(Vec::new(), false);
        assert!(captured(&p).is_empty());
    }
}
