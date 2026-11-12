//! Live progress indication (FR-19, TASK-0315, PRD-0001 Amendment 2026-07-19).
//!
//! While a scan runs, the pipeline notifies a [`ProgressSink`] as candidate
//! files reach their final disposition. It renders exclusively to the writer
//! it is constructed with — in production the stream `streams::route` selects
//! (FR-21: stdout on an interactive terminal in the default mode, stderr
//! otherwise) — and is a **pure output concern**: nothing here
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
//!   inside this module. Both streams are probed (`is_terminal()`) exactly
//!   once, in `main.rs`, and `streams::route` binds each target stream to its
//!   own TTY-ness. This is what makes the TTY (AC-21)
//!   and non-TTY (AC-22) rendering paths unit-testable without a real
//!   terminal.
//! - The sink is shared behind `&dyn ProgressSink` and called concurrently
//!   from rayon worker threads (hashing runs in parallel, ADR-0010).
//!   `current` (the numerator) is a lock-free atomic so the hot path
//!   (`advance`, called once per file from inside the rayon closure) never
//!   contends on a mutex in the common case. Every other piece of state —
//!   phase, denominators, and all repaint bookkeeping — lives behind a
//!   single [`Mutex`] alongside the writer, so a repaint always sees a
//!   internally-consistent snapshot and there is only ever one lock to
//!   reason about (no lock-ordering hazard).
//!
//! ## Rendering rules
//!
//! - **TTY:** a single line, redrawn in place (`\r` + erase-to-end-of-line;
//!   never a newline mid-run), showing phase, `x of y`, a percentage, and
//!   the elapsed time since the run started (AC-32; the completion line
//!   carries the run's total).
//!   Repaints triggered by `advance` are throttled to at most one per
//!   [`DEFAULT_REPAINT_INTERVAL_MS`] so a fast run does not flood the
//!   terminal; phase transitions (`set_phase`) and [`ProgressSink::finish`]
//!   always repaint immediately, bypassing the throttle, so they are never
//!   silently dropped. `finish` renders a final `Phase::Done` line and then
//!   terminates it with a newline, leaving the terminal clean for whatever
//!   prints next. Because `advance`-driven repaints are throttled, a phase's
//!   **final** `x == y` frame is rendered only if something forces it — the
//!   last `advance` of a phase lands on a due tick only by luck. Callers that
//!   need that closing frame must force it (as `main.rs` does for scanning's
//!   determinate state, whose closing `N of N (100%)` AC-21a mandates, by
//!   pairing the state with a `set_phase` call at *both* ends); phases whose
//!   end is immediately superseded by another forced transition — hashing
//!   into verifying, verifying into `finish`'s `Done` — need nothing extra.
//! - **Non-TTY:** no carriage returns, no ANSI escapes, ever. Occasional
//!   plain, newline-terminated lines at coarse milestones only — phase
//!   transitions plus [`NON_TTY_PERCENT_STEP`]-sized percentage steps — so
//!   redirected logs stay small and readable. Determinate milestone lines
//!   carry elapsed time too; only the indeterminate `scanning...` line stays
//!   bare (AC-22). `advance`-triggered checks are
//!   also rate-limited (the same lock-free pre-check as the TTY path) so a
//!   burst of concurrent hashing does not pay a mutex per file; phase
//!   transitions and `finish` still always produce a line.
//! - **Disabled:** use [`NoProgress`] — it writes nothing, anywhere, ever.
//!
//! ## Per-phase denominators (FR-19(a): show the overall problem size)
//!
//! Hashing is the slow, dominant phase on a real photo library, so the
//! engine (`engine.rs`) advances `x` incrementally, per file, as it is
//! hashed — not in one bulk jump at the end — measured against the full
//! candidate count `y` set once via [`ProgressSink::set_total`].
//!
//! Verifying is typically a much smaller workload than `y` (only files whose
//! hash matched another file's are byte-verified). Reusing `y` as the
//! denominator there would either overrun it or pin the display at a
//! misleading "100%" while the tool is still working, and redrawing in place
//! would otherwise **erase** the overall problem size from the screen right
//! when the run is ending. So verifying gets its own denominator via
//! [`ProgressSink::retotal`] — resetting the counter — while the run-wide
//! total is *also* rendered alongside it as a `(N candidates)` suffix (e.g.
//! `verifying: 1 of 2 (50%) (302 candidates)`), so the problem size stays
//! visible. When a phase's workload is zero (including Hashing, for an empty
//! candidate set) the phase is skipped entirely — no `set_phase`, and for
//! Verifying no `retotal` either — rather than showing a degenerate `0 of 0`.
//! Finally, [`ProgressSink::finish`] always renders a terminal [`Phase::Done`]
//! line against the run-wide total (`done: 302 of 302 (100%)`, or `done: 0 of
//! 0 (100%)` for an empty candidate set — the AC explicitly sanctions the
//! completion line against the full total even when it is zero), so the
//! resting state after *any* run — with or without a verify phase — shows
//! the problem size and a real completion state.
//!
//! ## Nothing else may write to the progress stream's terminal while a TTY line is live
//!
//! On a TTY the progress line is redrawn in place and left un-terminated
//! (no trailing newline) between repaints, by design (AC-21). Any other
//! write to the terminal while that line is live — e.g. `eprintln!` for a
//! diagnostic — lands in the *middle* of it, corrupting what the user sees
//! (`scanning...error: ...`). Because Rust's `Stdout` is line-buffered, the
//! `flush()` after every in-place repaint is load-bearing when progress is on
//! stdout: a `\r`-terminated frame would otherwise never appear (pinned by
//! `tty_repaints_are_flushed_so_a_line_buffered_stdout_shows_them`).
//! `main.rs` enforces the ordering half of this
//! (every other terminal writer runs strictly after [`ProgressSink::finish`]
//! returns, which always leaves the cursor on a fresh line) — but ordering
//! alone is only as durable as every future edit to `run()`; an early
//! `return` added by later work (e.g. TASK-0309/TASK-0310's apply/delete
//! diagnostics) could skip the explicit `finish()` call entirely. [`SinkGuard`]
//! makes this robust against that: it owns the sink and calls `finish()` from
//! `Drop`, so progress is *always* terminated no matter how the caller's
//! function returns. `finish()` is idempotent (double-calling it — once
//! explicitly, once from `Drop` — renders the completion line exactly once).

use std::io::Write;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Default minimum interval between repaints triggered by `advance` (AC-21),
/// so a fast run does not flood the terminal with redraws. Injectable via
/// [`Progress::new_with_interval_ms`] (test-only) so tests can pin
/// deterministic throttling behavior instead of racing the wall clock.
pub(crate) const DEFAULT_REPAINT_INTERVAL_MS: u64 = 100;

/// Percentage-point granularity for non-TTY milestone lines (AC-22).
const NON_TTY_PERCENT_STEP: u64 = 10;

/// User-facing phase label shown alongside the progress indicator.
///
/// Phases are a **display concept only** — they do not prescribe or
/// constrain the underlying ADR-0008 pipeline. A phase change is just a
/// change of the label (and, for [`Phase::Verifying`], the denominator)
/// shown next to the numerator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Enumerating and magic-byte-confirming candidate files; the total
    /// (`y`) is not yet known.
    Scanning,
    /// Hashing candidate files (BLAKE3 pre-filter), measured against the
    /// full candidate count `y`.
    Hashing,
    /// Byte-for-byte verifying candidates against their group's keeper,
    /// measured against its own (typically much smaller) workload, with `y`
    /// still shown alongside it as a `(N candidates)` suffix.
    Verifying,
    /// Terminal state rendered once by [`ProgressSink::finish`], against the
    /// full candidate count `y`. Never set via [`ProgressSink::set_phase`].
    Done,
}

impl Phase {
    /// The lower-case label rendered for this phase.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Phase::Scanning => "scanning",
            Phase::Hashing => "hashing",
            Phase::Verifying => "verifying",
            Phase::Done => "done",
        }
    }
}

/// Observer the dedup pipeline notifies as it makes progress (FR-19).
///
/// # Thread safety
///
/// Every method is data-race-free to call from any thread — internally,
/// only the numerator is a lock-free atomic; everything else (phase,
/// denominators, repaint bookkeeping) lives behind one mutex — so no
/// combination of concurrent calls can produce undefined behavior or a
/// torn/inconsistent read.
///
/// That said, the methods are not designed to be individually *meaningful*
/// under arbitrary concurrency: the intended usage is `advance` called
/// concurrently from rayon worker threads (ADR-0010) while `set_total`,
/// `set_phase`, `retotal`, and `finish` are called sequentially from the
/// single orchestrating thread, with no `advance` call started specifically
/// *because of* one of those transitions racing its effect (e.g. two
/// concurrent `set_total` calls with different values will not corrupt
/// state, but which one "wins" is unspecified last-write). Calling any
/// method must never influence pipeline results; progress is strictly an
/// output-only concern (AC-24).
pub trait ProgressSink: Sync + Send {
    /// Fix the run-wide denominator (`y`) once it is known — the number of
    /// confirmed candidate files (AC-21, FR-19(a)). Called once per
    /// production run, before the phase that uses it. **Resets the numerator
    /// to zero** (TASK-0317), symmetric with [`retotal`](ProgressSink::retotal):
    /// the indeterminate phase preceding this call (Scanning) may already
    /// have advanced the counter with its own, unrelated meaning ("files
    /// found so far", not measured against any denominator), and that count
    /// must not leak into the phase this total is for — otherwise the very
    /// next phase would render as already-complete.
    fn set_total(&self, total: u64);

    /// Switch to a new user-facing phase. Always renders immediately
    /// (bypassing any repaint throttle) — phase transitions are rare and
    /// meaningful, and must never be silently dropped.
    fn set_phase(&self, phase: Phase);

    /// Advance the numerator (`x`) by `n`, measured against the current
    /// phase's denominator (`y` normally, or the phase-local total set by
    /// the most recent [`retotal`](ProgressSink::retotal) call).
    ///
    /// **Invariant (within a phase):** each file entering that phase is
    /// counted exactly once, at the point its final disposition for that
    /// phase is reached, so the sum of `advance` calls during a phase
    /// equals that phase's denominator. This does **not** hold *across* a
    /// `set_total` or `retotal` boundary — the numerator is deliberately
    /// reset at both (see their docs) — so global monotonicity of the
    /// displayed percentage is guaranteed only *within* a phase, not across
    /// the whole run.
    ///
    /// **`n == 0` is a deliberate liveness "tick" (TASK-0332), not a
    /// no-op call callers should avoid.** A caller with real work to report
    /// but nothing yet accepted — e.g. `enumerate.rs`'s tree walk, visiting
    /// directory entries that do not become a found file — calls
    /// `advance(0)` to request a repaint-due check without changing the
    /// numerator, so a display element that depends only on wall-clock time
    /// (elapsed time, [`Progress`]'s AC-21b suffix) can still update during a
    /// stretch where the count itself does not move. An implementation must
    /// therefore run its normal repaint-due check on every call regardless
    /// of `n`, and must never let `n == 0` change what is displayed beyond
    /// whatever that due check would already repaint.
    fn advance(&self, n: u64);

    /// Reset the numerator to zero and fix a **new**, phase-local
    /// denominator, for a phase whose workload differs from the run-wide
    /// total set via `set_total` (verifying is typically a much smaller
    /// workload than the full candidate count `y`). Without this, verifying
    /// would either keep counting past a denominator it has already
    /// exceeded, or stay pinned at "100%" while genuinely still working —
    /// either reads to the user as hung or finished when it is neither.
    /// The run-wide total from `set_total` is retained and still shown
    /// alongside the reset counter (see the module doc).
    ///
    /// Does **not** itself trigger a repaint — callers should pair it with
    /// an immediately-following `set_phase` call so the phase label and the
    /// new denominator change together, atomically, in the rendered output,
    /// rather than flashing a stale phase/denominator combination. If a
    /// phase's workload is zero, skip both calls entirely rather than
    /// showing a degenerate `0 of 0`.
    fn retotal(&self, total: u64);

    /// The run is complete. Always renders a terminal [`Phase::Done`] line
    /// against the run-wide total (`done: y of y (100%)`) — regardless of
    /// which phase was last active or whether verifying ran at all — so the
    /// resting state after any run shows the overall problem size and a
    /// real completion state, then flushes and cleans up any in-place
    /// render state. See the module doc: nothing else may write to the same
    /// stream until this returns.
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

/// Decides when a coarse, non-redrawn milestone is due: on a phase change,
/// on entering a new [`NON_TTY_PERCENT_STEP`] bucket, or when forced. Shared
/// by [`Progress`]'s non-TTY path and the run record's milestones
/// (`record::MilestoneSink`) so AC-22's granularity has exactly one
/// definition.
#[derive(Debug, Default)]
pub(crate) struct MilestoneGate {
    last_phase: Option<Phase>,
    last_bucket: Option<u64>,
}

impl MilestoneGate {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// `true` when a milestone should be written now. Updates the gate only
    /// when it returns `true`.
    pub(crate) fn should_emit(&mut self, phase: Phase, pct: Option<u64>, force: bool) -> bool {
        let phase_changed = self.last_phase != Some(phase);
        let bucket = pct.map(|p| p - (p % NON_TTY_PERCENT_STEP));
        let bucket_changed = matches!(
            (bucket, self.last_bucket),
            (Some(b), shown) if shown != Some(b)
        );
        if !(force || phase_changed || bucket_changed) {
            return false;
        }
        self.last_phase = Some(phase);
        if let Some(b) = bucket {
            self.last_bucket = Some(b);
        }
        true
    }
}

/// `current / total` as a whole percentage, `100` when `total == 0`
/// ("nothing to do" is complete, not a division by zero).
pub(crate) fn percent(current: u64, total: u64) -> u64 {
    current
        .min(total)
        .saturating_mul(100)
        .checked_div(total)
        .unwrap_or(100)
}

/// Everything about a [`Progress<W>`] except the lock-free numerator and
/// throttle-gate, held behind one [`Mutex`] so a repaint always sees an
/// internally-consistent snapshot and updates its bookkeeping atomically
/// with deciding to print (no separate lock, no lock-ordering hazard).
struct State<W: Write> {
    writer: W,
    phase: Phase,
    /// The run-wide denominator (`y`) fixed by `set_total`; unaffected by
    /// `retotal`. Used directly by Scanning/Hashing/Done, and shown as the
    /// `(N candidates)` suffix during Verifying.
    run_total: u64,
    /// The denominator the numerator is currently measured against: equal
    /// to `run_total` except during Verifying, where `retotal` overrides it
    /// with the (typically smaller) verify-phase workload.
    phase_total: u64,
    /// Whether `set_total` (or `retotal`) has been called yet. Before that,
    /// the phase renders as an indeterminate `"<phase>..."` (used for
    /// Scanning, before the candidate count is known).
    total_known: bool,
    /// Decides when a non-TTY milestone line is due (unused on a TTY, which
    /// repaints in place instead).
    gate: MilestoneGate,
    /// Set by `finish`, so a second call (e.g. from [`SinkGuard`]'s `Drop`
    /// after `main.rs` already called it explicitly) is a no-op rather than
    /// rendering a second completion line.
    finished: bool,
}

/// Live progress renderer (FR-19).
///
/// Generic over the output stream so tests can capture into a `Vec<u8>`
/// instead of needing a real terminal. TTY-ness is **injected** (`tty`),
/// never probed here.
pub struct Progress<W: Write> {
    state: Mutex<State<W>>,
    tty: bool,
    /// The numerator (`x`). Deliberately the *only* lock-free atomic
    /// (`Relaxed` — snapshotted fresh from inside the state lock on every
    /// repaint decision, which is what makes cross-thread monotonicity of
    /// *displayed* percentages hold within a phase) so that `advance`,
    /// called once per file from inside a rayon parallel closure
    /// (ADR-0010), never contends on the state mutex in the common case.
    current: AtomicU64,
    /// Construction-time reference instant for the millis-since-start clock
    /// used by the throttle. Plain `Instant`, never subtracted from — unlike
    /// an earlier version of this type, nothing here can panic on a process
    /// that has been running for less than the repaint interval.
    start: Instant,
    /// Lock-free pre-check gate for `advance` (millis-since-`start` at which
    /// the next repaint attempt is worth even considering). `Relaxed`: this
    /// is a heuristic fast path only — the authoritative, race-free
    /// decision (and the authoritative update of this same field) happens
    /// again inside the state lock, so a stale or racy read here can only
    /// cause an unnecessary lock attempt, never an incorrect repaint.
    next_check_millis: AtomicU64,
    /// Injectable repaint-throttle interval (finding 7): defaults to
    /// [`DEFAULT_REPAINT_INTERVAL_MS`]; tests can pin a deterministic value
    /// via [`Progress::new_with_interval_ms`].
    repaint_interval_ms: u64,
}

/// Everything one rendered status line depends on ([`Progress::line`]).
struct LineParams {
    phase: Phase,
    current: u64,
    phase_total: u64,
    run_total: u64,
    /// `None` while the denominator is unknown (indeterminate scanning).
    pct: Option<u64>,
    /// TTY-only: show the live found-so-far count in the indeterminate state.
    show_count: bool,
    /// Whole seconds since the run started (AC-32).
    elapsed_secs: u64,
}

impl<W: Write> Progress<W> {
    /// Construct a new renderer writing to `writer`.
    ///
    /// `tty` says whether the destination stream is an interactive terminal
    /// — probed once by the caller (`main.rs`), never inside this type.
    pub fn new(writer: W, tty: bool) -> Self {
        Self::new_with_interval_ms(writer, tty, DEFAULT_REPAINT_INTERVAL_MS)
    }

    /// Like [`Progress::new`], but with an injectable repaint-throttle
    /// interval. `pub(crate)` (rather than `#[cfg(test)]`) so it stays a
    /// normal, always-typechecked part of the crate's API surface instead
    /// of a lint-suppressed test-only shim; production code just never
    /// calls it, since [`Progress::new`] already supplies the right
    /// default.
    pub(crate) fn new_with_interval_ms(writer: W, tty: bool, repaint_interval_ms: u64) -> Self {
        Self {
            state: Mutex::new(State {
                writer,
                phase: Phase::Scanning,
                run_total: 0,
                phase_total: 0,
                total_known: false,
                gate: MilestoneGate::new(),
                finished: false,
            }),
            tty,
            current: AtomicU64::new(0),
            start: Instant::now(),
            // 0 == immediately due: the very first repaint should never
            // wait out a full throttle interval.
            next_check_millis: AtomicU64::new(0),
            repaint_interval_ms,
        }
    }

    /// Like [`Progress::new_with_interval_ms`], but the constructed
    /// instance's clock claims to have started `elapsed_ms` in the past
    /// rather than at the moment of construction. The only way to
    /// deterministically test the elapsed-time liveness cue's `now / 1000`
    /// arithmetic (TASK-0332): every other test observes a process that has
    /// been running for single-digit milliseconds and could only ever
    /// assert "0s", which would not catch a regression to e.g. raw
    /// milliseconds or a wrong divisor.
    ///
    /// Unlike its sibling `new_with_interval_ms` — which stays a normal,
    /// always-typechecked `pub(crate)` function because *production* code
    /// also calls it (via [`Progress::new`]) — this one is `#[cfg(test)]`:
    /// no production caller ever has a legitimate reason to lie about when
    /// it started, so there is no always-compiled use for it to share.
    #[cfg(test)]
    pub(crate) fn new_with_interval_ms_and_elapsed_ms(
        writer: W,
        tty: bool,
        repaint_interval_ms: u64,
        elapsed_ms: u64,
    ) -> Self {
        let mut p = Self::new_with_interval_ms(writer, tty, repaint_interval_ms);
        p.start = Instant::now()
            .checked_sub(std::time::Duration::from_millis(elapsed_ms))
            .expect("elapsed_ms too large to subtract from Instant::now()");
        p
    }

    /// Milliseconds elapsed since construction. Never panics (unlike
    /// subtracting a fixed offset from `Instant::now()`, which can underflow
    /// for a process still within its first repaint interval of uptime).
    fn now_millis(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    /// Lock-free pre-check used only by `advance`: skip even attempting to
    /// lock the state mutex unless a repaint is plausibly due. Purely a
    /// fast path for the common case (most `advance` calls, arriving faster
    /// than the throttle interval from concurrent rayon workers, will not
    /// result in a repaint) — the authoritative decision is re-made inside
    /// [`Progress::paint`], under the lock.
    fn due(&self) -> bool {
        self.now_millis() >= self.next_check_millis.load(Ordering::Relaxed)
    }

    /// Render one status line: `"<phase>: <x> of <y> (<p>%) (<n>s elapsed)"`
    /// — every determinate line carries the elapsed time since the run
    /// started, and the [`Phase::Done`] line carries it as `(<n>s total)`
    /// (AC-32) — with a
    /// `(N candidates)` suffix before it for [`Phase::Verifying`] (so the run-wide
    /// problem size stays visible even though the counter has been reset to
    /// the phase-local workload — see the module doc); `"<phase>... <x>
    /// file(s) found (<elapsed_secs>s elapsed)"` while the total is not yet
    /// known **and** `show_count` is set (indeterminate, TTY-only, TASK-0317
    /// count + TASK-0332 elapsed-time suffix) — `x` animates live even though
    /// there is no denominator yet to show a percentage against, and the
    /// elapsed suffix keeps moving even on a stretch where `x` itself does
    /// not (a low-match-density tree); or a bare `"<phase>..."` when
    /// indeterminate and `show_count` is unset.
    ///
    /// `show_count` (and, identically, the elapsed-time suffix it gates) are
    /// TTY-only by design (architect review, TASK-0317 for the count,
    /// TASK-0332 for elapsed time): a non-TTY repaint of the indeterminate
    /// phase happens at most once (at phase entry, before anything has been
    /// counted — see the module doc's non-TTY repaint rules), so either value
    /// included there would render as a permanently-frozen, false "0 files
    /// found (0s elapsed)" for the rest of the phase — worse than the old
    /// bare "scanning...", which made no claim at all. That reasoning is
    /// specific to the indeterminate state: a determinate milestone line is
    /// written at the instant it describes, so its elapsed time is a fact,
    /// not a stale redraw, and it is shown on TTY and non-TTY alike (AC-22's
    /// 2026-08-06 scope note). The only line with no elapsed time is the
    /// bare indeterminate one.
    ///
    /// Pure ASCII, deliberately: this is the only text the binary ever
    /// writes at runtime that could tempt a non-ASCII character (a
    /// `Phase::Verifying` "N candidates" aside), and a redirected log read
    /// under `LANG=C` or a non-UTF-8 code page would mojibake anything
    /// outside that range — safer to just never emit it. No thousands
    /// separators either, for the same reason and for consistency with the
    /// unadorned integers in the `x of y` form above.
    ///
    /// Takes a [`LineParams`] rather than a parameter list: the display has
    /// grown past what a positional signature keeps readable.
    fn line(p: &LineParams) -> String {
        let label = p.phase.label();
        let (current, total, elapsed) = (p.current, p.phase_total, p.elapsed_secs);
        match p.pct {
            None if p.show_count => {
                let noun = if current == 1 { "file" } else { "files" };
                // Elapsed time (TASK-0332, AC-21b): a liveness cue for the
                // stretch of a walk where almost nothing matches the
                // extension scope, so this count alone can sit static for a
                // long time even though the walk is actively working. Shown
                // from the very first render, not only past some threshold —
                // uniform behavior beats a special case that only appears on
                // slow trees. Deliberately not "entries visited": there is no
                // denominator for that, so a growing number there would look
                // like quantified progress it isn't (see the PRD amendment).
                format!("{label}... {current} {noun} found ({elapsed}s elapsed)")
            }
            // Non-TTY indeterminate: written once, at phase entry, so a clock
            // here would be a permanently stale "0s" (AC-22).
            None => format!("{label}..."),
            Some(pct) if p.phase == Phase::Verifying => format!(
                "{label}: {current} of {total} ({pct}%) ({} candidates) ({elapsed}s elapsed)",
                p.run_total
            ),
            // AC-32: the completion state reports the run's total.
            Some(pct) if p.phase == Phase::Done => {
                format!("{label}: {current} of {total} ({pct}%) ({elapsed}s total)")
            }
            Some(pct) => format!("{label}: {current} of {total} ({pct}%) ({elapsed}s elapsed)"),
        }
    }

    /// The authoritative repaint decision and action, called with the state
    /// lock already held. `force`: bypass the throttle (used by
    /// `set_phase`, never by `advance`). Snapshots `current` fresh from the
    /// atomic — combined with every repaint being serialized by holding
    /// `st` for its whole duration, this is what guarantees a printed
    /// percentage is monotonically non-decreasing *within a phase*
    /// regardless of which rayon worker's `advance` call triggered it (a
    /// staler snapshot can never be shown after a fresher one already was,
    /// because acquiring the lock second means observing everything the
    /// first repaint already committed, including the bumped throttle gate
    /// below).
    fn paint(&self, st: &mut State<W>, force: bool) {
        let now = self.now_millis();
        if !force && now < self.next_check_millis.load(Ordering::Relaxed) {
            return;
        }
        // `saturating_add` (not a bare `+`): progress accounting must not be
        // where the program panics (debug) or silently wraps (release) —
        // same principle as the `saturating_sub` in `engine.rs`.
        self.next_check_millis.store(
            now.saturating_add(self.repaint_interval_ms),
            Ordering::Relaxed,
        );

        let phase = st.phase;
        let phase_total = st.phase_total;
        let run_total = st.run_total;
        // Only clamp to the denominator once one is known (TASK-0317): before
        // that, `phase_total` is still its zero-initialized default, and
        // clamping against it would pin the indeterminate scanning count at
        // 0 forever regardless of how many `advance` calls actually landed.
        let raw_current = self.current.load(Ordering::Relaxed);
        let current = if st.total_known {
            raw_current.min(phase_total)
        } else {
            raw_current
        };
        let pct = st.total_known.then(|| percent(current, phase_total));
        // AC-32: every render carries elapsed time; only the non-TTY
        // indeterminate line omits it (AC-22). `show_count` is TTY-only for
        // the same reason (see `line`).
        let params = LineParams {
            phase,
            current,
            phase_total,
            run_total,
            pct,
            show_count: self.tty,
            elapsed_secs: now / 1000,
        };

        if self.tty {
            let line = Self::line(&params);
            let mut buf = String::with_capacity(line.len() + 4);
            buf.push('\r');
            buf.push_str(&line);
            buf.push_str("\x1b[K");
            let _ = st.writer.write_all(buf.as_bytes());
            let _ = st.writer.flush();
        } else if st.gate.should_emit(phase, pct, force) {
            let mut buf = Self::line(&params);
            buf.push('\n');
            let _ = st.writer.write_all(buf.as_bytes());
        }
    }
}

impl<W: Write + Send> ProgressSink for Progress<W> {
    fn set_total(&self, total: u64) {
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        st.run_total = total;
        st.phase_total = total;
        st.total_known = true;
        // TASK-0317 regression: the indeterminate Scanning phase may have
        // already advanced the counter with its own "files found" meaning
        // (unrelated to this new denominator) — without this reset, that
        // count leaked straight into Hashing, pinning it at 100% for the
        // entire phase (an architect-review-caught defect, not hypothetical).
        self.current.store(0, Ordering::Relaxed);
        // Silent by design: paired with the `set_phase` call that follows
        // in production, which is what actually (and always) repaints.
    }

    fn set_phase(&self, phase: Phase) {
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        st.phase = phase;
        self.paint(&mut st, true);
    }

    fn advance(&self, n: u64) {
        self.current.fetch_add(n, Ordering::Relaxed);
        if !self.due() {
            return;
        }
        // `try_lock`, not `lock` (a rayon worker must never block on I/O
        // here): if another thread already holds the lock it is painting
        // data that is at least as fresh as ours (`current` only grows), so
        // skipping is harmless — the next `advance` from any thread retries
        // once `due()` allows it again.
        match self.state.try_lock() {
            Ok(mut st) => self.paint(&mut st, false),
            Err(std::sync::TryLockError::Poisoned(poisoned)) => {
                self.paint(&mut poisoned.into_inner(), false);
            }
            Err(std::sync::TryLockError::WouldBlock) => {}
        }
    }

    fn retotal(&self, total: u64) {
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        st.phase_total = total;
        st.total_known = true;
        self.current.store(0, Ordering::Relaxed);
        // Silent by design — see the trait doc.
    }

    fn finish(&self) {
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        // Idempotent (finding N4): `main.rs` calls this explicitly (so
        // progress is fully flushed before the stdout report — AC-20), and
        // `SinkGuard`'s `Drop` also calls it (so progress is *always*
        // terminated, even on an early `return` future code might add). The
        // second call must not render a second completion line.
        if st.finished {
            return;
        }
        st.finished = true;

        let run_total = st.run_total;
        st.phase = Phase::Done;
        st.phase_total = run_total;
        st.total_known = true;
        self.current.store(run_total, Ordering::Relaxed);

        let pct = Some(percent(run_total, run_total));
        let line = Self::line(&LineParams {
            phase: Phase::Done,
            current: run_total,
            phase_total: run_total,
            run_total,
            pct,
            show_count: self.tty,
            // AC-32: the completion state reports the run's total elapsed time.
            elapsed_secs: self.now_millis() / 1000,
        });

        if self.tty {
            let mut buf = String::with_capacity(line.len() + 6);
            buf.push('\r');
            buf.push_str(&line);
            buf.push_str("\x1b[K\n");
            let _ = st.writer.write_all(buf.as_bytes());
            let _ = st.writer.flush();
        } else {
            let mut buf = line;
            buf.push('\n');
            let _ = st.writer.write_all(buf.as_bytes());
        }
    }
}

/// Owns a `Box<dyn ProgressSink>` and calls [`ProgressSink::finish`] from
/// `Drop`, so progress is *always* terminated — cleanly ending a live TTY
/// line — no matter how the owning function returns, including an early
/// `return` (finding N4). `finish` is idempotent, so this never renders a
/// second completion line when the caller (as `main.rs` does, for AC-20's
/// stdout-ordering guarantee, which `Drop` alone cannot provide) also calls
/// `finish` explicitly before printing anything else.
///
/// Derefs to `&dyn ProgressSink` so it can be used everywhere a `&dyn
/// ProgressSink` is expected (e.g. passed straight into
/// [`crate::engine::find_duplicates_with_progress`]) without an extra
/// `.as_ref()` at every call site.
pub struct SinkGuard(Box<dyn ProgressSink>);

impl SinkGuard {
    /// Wrap `sink` so it is guaranteed to be finished, exactly once, by the
    /// time the guard is dropped.
    pub fn new(sink: Box<dyn ProgressSink>) -> Self {
        Self(sink)
    }
}

impl std::ops::Deref for SinkGuard {
    type Target = dyn ProgressSink;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl Drop for SinkGuard {
    fn drop(&mut self) {
        self.0.finish();
    }
}

/// Forwards every call to two sinks, in order. `main.rs` uses it to feed the
/// terminal renderer and the run record's milestones from the one observer
/// the pipeline already calls (ADR-0018 §2), so the record keeps its
/// milestones under `--no-progress` and `--timings`.
pub struct Tee {
    first: Box<dyn ProgressSink>,
    second: Box<dyn ProgressSink>,
}

impl Tee {
    pub fn new(first: Box<dyn ProgressSink>, second: Box<dyn ProgressSink>) -> Self {
        Self { first, second }
    }
}

impl ProgressSink for Tee {
    fn set_total(&self, total: u64) {
        self.first.set_total(total);
        self.second.set_total(total);
    }
    fn set_phase(&self, phase: Phase) {
        self.first.set_phase(phase);
        self.second.set_phase(phase);
    }
    fn advance(&self, n: u64) {
        self.first.advance(n);
        self.second.advance(n);
    }
    fn retotal(&self, total: u64) {
        self.first.retotal(total);
        self.second.retotal(total);
    }
    fn finish(&self) {
        self.first.finish();
        self.second.finish();
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn captured(p: &Progress<Vec<u8>>) -> String {
        String::from_utf8(p.state.lock().unwrap().writer.clone()).unwrap()
    }

    /// A renderer with throttling effectively disabled (interval = 0), so
    /// every `advance` call actually reaches the data-driven decision
    /// instead of being suppressed by timing — used by tests that need to
    /// exercise that decision deterministically rather than the throttle.
    fn immediate(tty: bool) -> Progress<Vec<u8>> {
        Progress::new_with_interval_ms(Vec::new(), tty, 0)
    }

    /// Like [`immediate`], but the clock claims to have started `elapsed_ms`
    /// in the past — see [`Progress::new_with_interval_ms_and_elapsed_ms`].
    fn immediate_with_elapsed(tty: bool, elapsed_ms: u64) -> Progress<Vec<u8>> {
        Progress::new_with_interval_ms_and_elapsed_ms(Vec::new(), tty, 0, elapsed_ms)
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

    // ── TASK-0317: live indeterminate count during Scanning ────────────────

    #[test]
    fn tty_indeterminate_scanning_shows_a_live_growing_count() {
        // Before enumeration completes, y is unknown, but the count of files
        // found so far can still animate live rather than sitting frozen at
        // "scanning..." for the whole enumeration+sniff sub-phase.
        let p = immediate(true);
        p.set_phase(Phase::Scanning);
        p.advance(1);
        p.advance(1);
        p.advance(1);

        let out = captured(&p);
        assert!(
            out.contains("scanning... 3 files found"),
            "expected the indeterminate scanning count to reach 3, got: {out:?}"
        );
        assert!(
            !out.contains(" of "),
            "must stay indeterminate (no percentage/denominator) until the \
             total is known, got: {out:?}"
        );
    }

    // ── TASK-0332 (AC-21b): walk liveness cue (elapsed time) ────────────────

    #[test]
    fn tty_indeterminate_scanning_shows_elapsed_time_alongside_the_count() {
        let p = immediate(true);
        p.set_phase(Phase::Scanning);
        p.advance(1);
        p.advance(1);
        p.advance(1);

        let out = captured(&p);
        // A test runs in microseconds, well under one second, so "0s" is the
        // only value this can deterministically assert without controlling
        // the clock — see tty_indeterminate_scanning_elapsed_time_reflects_a_nonzero_clock
        // below for a test that pins the actual now/1000 arithmetic instead.
        assert!(
            out.contains("scanning... 3 files found (0s elapsed)"),
            "expected the elapsed-time liveness cue alongside the count, got: {out:?}"
        );
    }

    #[test]
    fn tty_indeterminate_scanning_elapsed_time_reflects_a_nonzero_clock() {
        // Architect review, TASK-0332: every other elapsed-time test observes
        // a process running for single-digit milliseconds, so it can only
        // ever assert "0s" — which would not catch a regression to e.g. raw
        // milliseconds, or a wrong divisor. This pins the real arithmetic
        // (`now_millis() / 1000`) deterministically via an injected start
        // time, without racing the wall clock.
        let p = immediate_with_elapsed(true, 61_500); // 61.5s "ago"
        p.set_phase(Phase::Scanning);
        p.advance(1);

        let out = captured(&p);
        assert!(
            out.contains("scanning... 1 file found (61s elapsed)"),
            "expected 61s (whole seconds, truncated not rounded), got: {out:?}"
        );
    }

    #[test]
    fn line_format_indeterminate_with_count_includes_elapsed_seconds() {
        // A narrower companion to the test above: exercises `line()` in
        // isolation (no Progress/paint/clock plumbing at all), pinning
        // exactly the format string a regression to that plumbing shouldn't
        // be able to hide behind.
        let line = Progress::<Vec<u8>>::line(&LineParams {
            phase: Phase::Scanning,
            current: 3,
            phase_total: 0,
            run_total: 0,
            pct: None,
            show_count: true,
            elapsed_secs: 14,
        });
        assert_eq!(line, "scanning... 3 files found (14s elapsed)");
    }

    #[test]
    fn tty_indeterminate_scanning_shows_elapsed_time_even_before_any_file_found() {
        // AC-21b: shown from the first render, not only once some threshold
        // of files/time has passed — this is the exact case the cue exists
        // for (a tree where almost nothing is found for a long stretch).
        let p = Progress::new(Vec::new(), true);
        p.set_phase(Phase::Scanning);

        let out = captured(&p);
        assert!(
            out.contains("scanning... 0 files found (0s elapsed)"),
            "expected the elapsed-time cue on the very first render, got: {out:?}"
        );
    }

    #[test]
    fn advance_zero_repaints_without_changing_the_indeterminate_count() {
        // The walk liveness cue depends on a zero-count "tick" (`advance(0)`)
        // triggering the same throttled repaint path a real advance does,
        // without itself ever inflating the displayed "N files found" count
        // (enumerate.rs calls this once per directory entry visited, not
        // just once per accepted candidate — see AC-21b's PRD amendment).
        let p = immediate(true);
        p.set_phase(Phase::Scanning);
        p.advance(1);
        p.advance(0);
        p.advance(0);
        p.advance(0);

        let out = captured(&p);
        // The very first (forced, phase-entry) repaint legitimately shows
        // "0 files found" before the real advance below it — this asserts
        // the *last* line, not that "0 file" never appears anywhere in the
        // whole captured history.
        let last_line = out.rsplit('\r').next().unwrap();
        assert!(
            last_line.contains("scanning... 1 file found"),
            "advance(0) must never change or regress the indeterminate \
             count, got last line: {last_line:?} (full capture: {out:?})"
        );
        // Every call above hits paint() under immediate()'s zero-throttle
        // interval, so each one repaints — proving advance(0) really does
        // reach the repaint path rather than being short-circuited away.
        let repaints = out.matches('\r').count();
        assert!(
            repaints >= 4,
            "expected a repaint per call (1 phase-entry + 1 real advance + \
             3 zero-count ticks), got {repaints} in: {out:?}"
        );
    }

    #[test]
    fn non_tty_indeterminate_scanning_never_shows_elapsed_time() {
        // AC-21b is TTY-only, for the identical reason the found-so-far
        // count already is (AC-22): a non-redrawn line could only ever
        // record a stale elapsed value.
        let p = immediate(false);
        p.set_phase(Phase::Scanning);
        p.advance(1);

        let out = captured(&p);
        assert!(
            !out.contains("elapsed"),
            "non-TTY must never show the elapsed-time cue, got: {out:?}"
        );
    }

    #[test]
    fn tty_indeterminate_scanning_count_pluralizes_correctly() {
        let p = immediate(true);
        p.set_phase(Phase::Scanning);
        p.advance(1);

        let out = captured(&p);
        assert!(
            out.contains("scanning... 1 file found"),
            "singular 'file' for a count of 1, got: {out:?}"
        );
    }

    #[test]
    fn set_total_resets_the_numerator_carried_from_scanning() {
        // Architect-review-caught regression (TASK-0317): the Scanning phase
        // advances the counter with its own "files found" meaning, unrelated
        // to any denominator. Without resetting it in set_total, that count
        // leaked straight into Hashing, pinning it at "100%" from the first
        // repaint of the entire hashing phase — a real bug this test would
        // have caught before merge.
        let p = immediate(false);
        p.set_phase(Phase::Scanning);
        p.advance(1);
        p.advance(1);
        p.advance(1);

        p.set_total(10);
        p.set_phase(Phase::Hashing);

        let out = captured(&p);
        assert!(
            out.contains("hashing: 0 of 10 (0%)"),
            "hashing must start at 0, not carry Scanning's count forward; got: {out:?}"
        );
        assert!(
            !out.contains("hashing: 3 of 10"),
            "must not show the leaked Scanning count; got: {out:?}"
        );
    }

    // ── TASK-0328: scanning's determinate second state (AC-21a) ────────────

    #[test]
    fn scanning_second_state_retotal_then_same_phase_forces_immediate_zero_of_n_render() {
        // AC-21a: once the walk finishes with N candidates, the indicator
        // leaves the indeterminate "scanning... N files found" state for a
        // determinate "scanning: x of N (p%)" state — a *second* state of
        // the same phase, so the label never changes and `set_phase` is
        // called again with the *same* Phase::Scanning value. `retotal`
        // alone never repaints (by design, see its trait doc and
        // `retotal_alone_does_not_trigger_a_repaint` below); pairing it with
        // an (even same-phase) `set_phase` call is what produces the
        // AC-21a-mandated forced immediate `0 of N (0%)` render.
        let p = immediate(true);
        p.set_phase(Phase::Scanning);
        p.advance(1);
        p.advance(1);
        p.advance(1); // indeterminate: "scanning... 3 files found"

        p.retotal(3);
        p.set_phase(Phase::Scanning); // same phase; must still force a repaint

        let out = captured(&p);
        assert!(
            out.contains("scanning: 0 of 3 (0%)"),
            "expected a forced immediate 0-of-N render on entering the \
             determinate scanning state, got: {out:?}"
        );

        p.advance(1);
        p.advance(1);
        p.advance(1);
        let out = captured(&p);
        assert!(
            out.contains("scanning: 3 of 3 (100%)"),
            "expected the determinate scanning state to end at N of N (100%), \
             got: {out:?}"
        );
        // Caveat this test cannot express (architect review, TASK-0328):
        // `immediate()` disables the repaint throttle, so the closing frame
        // above lands purely because *every* advance repaints here. Under the
        // production throttle the last advance almost never falls on a due
        // tick, so `main.rs` must force the closing repaint explicitly — pinned
        // through the real binary by
        // `scanning_determinate_state_appears_as_a_milestone_line_on_non_tty`
        // in `tests/cli_integration.rs`, which is where a regression in that
        // pairing would actually be caught.
    }

    #[test]
    fn scanning_determinate_state_has_no_candidates_suffix() {
        // AC-21a: unlike Verifying, scanning's determinate second state must
        // not show a "(N candidates)" suffix — the run-wide candidate total
        // y is genuinely not known yet at this point in the run.
        let p = immediate(true);
        p.set_phase(Phase::Scanning);
        p.retotal(5);
        p.set_phase(Phase::Scanning);
        p.advance(2);

        let out = captured(&p);
        assert!(out.contains("scanning: 2 of 5 (40%)"), "got: {out:?}");
        assert!(
            !out.contains("candidates)"),
            "scanning's determinate state must never show a candidates \
             suffix, got: {out:?}"
        );
    }

    #[test]
    fn scanning_determinate_state_numerator_does_not_leak_into_hashing() {
        // TASK-0328 regression, one stage earlier than TASK-0317's
        // `set_total_resets_the_numerator_carried_from_scanning`: scanning's
        // own determinate second state (AC-21a) ends at N of N, so
        // `set_total` must reset the numerator again before Hashing starts,
        // or hashing would render as already-100%-complete from its very
        // first repaint — exactly the class of bug that test already guards
        // one transition later.
        let p = immediate(false);
        p.set_phase(Phase::Scanning);
        p.advance(1);
        p.advance(1);
        p.advance(1);
        p.retotal(3);
        p.set_phase(Phase::Scanning);
        p.advance(1);
        p.advance(1);
        p.advance(1);
        assert!(captured(&p).contains("scanning: 3 of 3 (100%)"));

        p.set_total(10);
        p.set_phase(Phase::Hashing);

        let out = captured(&p);
        assert!(
            out.contains("hashing: 0 of 10 (0%)"),
            "hashing must start at 0, not carry scanning's determinate-state \
             count forward; got: {out:?}"
        );
        assert!(
            !out.contains("hashing: 3 of 10"),
            "must not show the leaked scanning count; got: {out:?}"
        );
    }

    #[test]
    fn tty_hashing_shows_phase_count_and_percent() {
        let p = immediate(true);
        p.set_total(4);
        p.set_phase(Phase::Hashing);
        p.advance(1);
        p.advance(1);
        p.advance(2);

        let out = captured(&p);
        assert!(
            out.contains("hashing: 4 of 4 (100%)"),
            "expected phase/x-of-y/percent, got: {out:?}"
        );
    }

    #[test]
    fn tty_uses_carriage_return_for_in_place_redraw() {
        let p = immediate(true);
        p.set_total(1);
        p.set_phase(Phase::Hashing);
        p.advance(1);

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
        let p = immediate(true);
        p.set_total(5);
        p.set_phase(Phase::Scanning);
        p.set_phase(Phase::Hashing);
        p.advance(5);
        p.retotal(3);
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
    fn tty_verifying_shows_own_workload_and_run_total_suffix() {
        // Architect finding 2 (ratified display): verifying must show its
        // own workload/percentage *and* the overall problem size, or the
        // resting state at the end of a run loses "how big was this job"
        // (FR-19(a)) right when the run is ending.
        let p = immediate(true);
        p.set_total(302);
        p.set_phase(Phase::Hashing);
        p.advance(302);

        p.retotal(2);
        p.set_phase(Phase::Verifying);
        assert!(
            captured(&p).contains("verifying: 0 of 2 (0%) (302 candidates)"),
            "got: {:?}",
            captured(&p)
        );

        p.advance(1);
        assert!(
            captured(&p).contains("verifying: 1 of 2 (50%) (302 candidates)"),
            "got: {:?}",
            captured(&p)
        );
    }

    #[test]
    fn tty_finish_renders_done_against_run_total() {
        // Finding 2: finish() must always show a real completion state
        // against the *run-wide* total, so the resting state after any run
        // shows the problem size — never the verify phase's smaller,
        // already-exhausted denominator, and never a stale mid-run phase.
        let p = immediate(true);
        p.set_total(302);
        p.set_phase(Phase::Hashing);
        p.advance(302);
        p.retotal(2);
        p.set_phase(Phase::Verifying);
        p.advance(2);
        p.finish();

        let out = captured(&p);
        assert!(
            out.contains("done: 302 of 302 (100%)"),
            "expected a terminal Done line against the run total, got: {out:?}"
        );
    }

    #[test]
    fn tty_erase_to_eol_present_on_every_repaint() {
        // Without `\x1b[K`, a shorter new line does not erase leftover
        // characters from a longer previous one (the corruption the
        // architect specifically flagged: "verifying: 0 of 2 (0%)00%)").
        let p = immediate(true);
        p.set_total(2);
        p.set_phase(Phase::Hashing);
        p.advance(1);
        p.advance(1);

        let out = captured(&p);
        for segment in out.split('\r').skip(1) {
            assert!(
                segment.contains("\x1b[K"),
                "every repaint segment must erase to end of line: {segment:?} in {out:?}"
            );
        }
    }

    #[test]
    fn tty_zero_interval_produces_one_repaint_per_advance() {
        // Finding 7 (positive half): with throttling disabled, N advances
        // must produce N repaints, each carriage-return + erase-to-EOL.
        let p = immediate(true);
        p.set_total(5);
        p.set_phase(Phase::Hashing); // one forced repaint, not counted below
        let baseline = captured(&p).matches('\r').count();

        for _ in 0..5 {
            p.advance(1);
        }

        let out = captured(&p);
        let repaints = out.matches('\r').count() - baseline;
        assert_eq!(
            repaints, 5,
            "interval=0 must repaint on every advance, got {repaints} in {out:?}"
        );
    }

    #[test]
    fn tty_throttle_interval_blocks_repeated_repaints_within_the_window() {
        // Finding 7 (negative half, deterministic): with an hour-long
        // interval, no advance-triggered repaint may occur — pins the
        // throttle without racing the wall clock like the smoke test below.
        let p = Progress::new_with_interval_ms(Vec::new(), true, 3_600_000);
        p.set_total(1000);
        p.set_phase(Phase::Hashing); // forced; establishes the throttle window
        let after_phase_change = captured(&p);

        for _ in 0..50 {
            p.advance(1);
        }

        assert_eq!(
            captured(&p),
            after_phase_change,
            "no advance-triggered repaint should occur within a 1-hour window"
        );
    }

    #[test]
    fn tty_repaints_are_throttled_not_flooded() {
        // Smoke test at the real default interval: a burst of advances
        // arriving faster than the throttle must not each produce their own
        // repaint (AC-21: "does not flood the terminal").
        let p = Progress::new(Vec::new(), true);
        p.set_total(1000);
        p.set_phase(Phase::Hashing);
        for _ in 0..1000 {
            p.advance(1);
        }
        let out = captured(&p);
        let repaints = out.matches('\r').count();
        assert!(
            repaints < 50,
            "expected throttling to collapse a tight burst of advances, got {repaints} repaints"
        );
    }

    #[test]
    fn progress_new_and_immediate_advance_do_not_panic() {
        // Regression (finding 5): an earlier version computed
        // `Instant::now() - DEFAULT_REPAINT_INTERVAL_MS` at construction,
        // which panics for a process within its first repaint interval of
        // uptime. The current design has no such subtraction anywhere.
        let p = Progress::new(Vec::new(), true);
        p.set_total(1);
        p.set_phase(Phase::Hashing);
        p.advance(1);
        p.finish();
    }

    // ── Non-TTY fallback (AC-22) ─────────────────────────────────────────

    #[test]
    fn non_tty_indeterminate_scanning_stays_bare_no_count() {
        // Architect review (TASK-0317): a non-TTY repaint of the
        // indeterminate Scanning phase happens at most once — at phase
        // entry, before anything has been counted (see the module doc's
        // non-TTY repaint rules: no bucket concept applies when there is no
        // percentage). Including the count there would freeze at a false
        // "0 files found" for the rest of the phase, worse than the old
        // neutral "scanning..." which made no claim. The count is TTY-only.
        let p = immediate(false);
        p.set_phase(Phase::Scanning);
        p.advance(1);
        p.advance(1);
        p.advance(1);

        let out = captured(&p);
        assert!(
            out.contains("scanning...\n"),
            "non-TTY scanning must stay the bare, neutral line; got: {out:?}"
        );
        assert!(
            !out.contains("found"),
            "non-TTY must never show the (necessarily stale) found-count; got: {out:?}"
        );
    }

    #[test]
    fn non_tty_never_emits_carriage_return_or_ansi() {
        let p = Progress::new(Vec::new(), false);
        p.set_total(10);
        p.set_phase(Phase::Scanning);
        p.set_phase(Phase::Hashing);
        for _ in 0..10 {
            p.advance(1);
        }
        p.retotal(2);
        p.set_phase(Phase::Verifying);
        p.advance(2);
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
        p.set_total(2);
        p.set_phase(Phase::Scanning);
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
    fn non_tty_emits_milestone_on_every_phase_transition_and_finish() {
        // set_phase(Scanning) first, matching production order (main.rs
        // enters Scanning before the candidate total is known; only the
        // engine, later, calls set_total), so the indeterminate "..." form
        // actually gets exercised.
        let p = Progress::new(Vec::new(), false);
        p.set_phase(Phase::Scanning);
        p.set_total(5);
        p.set_phase(Phase::Hashing);
        p.retotal(1);
        p.set_phase(Phase::Verifying);
        p.finish();

        let out = captured(&p);
        assert!(out.contains("scanning..."), "got: {out:?}");
        assert!(out.contains("hashing:"), "got: {out:?}");
        assert!(out.contains("verifying:"), "got: {out:?}");
        assert!(out.contains("done:"), "got: {out:?}");
    }

    #[test]
    fn non_tty_verifying_shows_run_total_suffix() {
        let p = Progress::new(Vec::new(), false);
        p.set_total(6);
        p.set_phase(Phase::Hashing);
        p.retotal(2);
        p.set_phase(Phase::Verifying);
        p.advance(1);

        let out = captured(&p);
        assert!(
            out.contains("verifying: 0 of 2 (0%) (6 candidates)"),
            "got: {out:?}"
        );
    }

    #[test]
    fn non_tty_percent_milestones_are_coarse_grained() {
        // 100 advances of 1/100 must not yield 100 lines — only occasional
        // milestones at NON_TTY_PERCENT_STEP granularity (bounded, log
        // friendly). Uses the zero-interval renderer so every advance
        // actually reaches the data-driven bucket check (not suppressed by
        // the throttle pre-check), isolating this from finding 7's timing
        // concerns.
        let p = immediate(false);
        p.set_total(100);
        p.set_phase(Phase::Hashing);
        for _ in 0..100 {
            p.advance(1);
        }

        let out = captured(&p);
        let hashing_lines = out.lines().filter(|l| l.starts_with("hashing:")).count();
        // At most one line per 10%-step (11 possible: 0,10,..,100).
        assert!(
            hashing_lines <= 11,
            "expected bounded hashing milestone lines, got {hashing_lines}: {out:?}"
        );
        assert!(
            hashing_lines > 1,
            "expected more than one milestone: {out:?}"
        );
        assert!(out.contains("hashing: 100 of 100 (100%)"), "got: {out:?}");
    }

    #[test]
    fn non_tty_bounded_under_default_throttle_for_a_fast_burst() {
        // Same shape as above but at the real default interval and a larger
        // burst — a fast run must not log per-file regardless of the
        // throttle timing.
        let p = Progress::new(Vec::new(), false);
        p.set_total(1000);
        p.set_phase(Phase::Hashing);
        for _ in 0..1000 {
            p.advance(1);
        }
        p.finish();

        let lines = captured(&p).lines().count();
        assert!(
            lines <= 15,
            "expected a small, bounded number of lines, got {lines}"
        );
    }

    #[test]
    fn non_tty_percent_milestones_are_monotonic_under_concurrent_advances() {
        // Regression: `advance` is called from rayon worker threads during
        // hashing (real engine usage, FR-19). A racy implementation could
        // snapshot a *stale*, lower percentage in one thread and print it
        // after a fresher, higher one has already been shown by another
        // thread — a confusing "progress went backward" log. Milestone
        // percentages must be monotonically non-decreasing regardless of
        // thread interleaving. Uses the zero-interval renderer to maximize
        // contention/exposure of the race this guards against.
        let p = immediate(false);
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
            // The percentage is the parenthesised group ending in `%`; since
            // AC-32 it is no longer the last group on the line (the elapsed
            // time follows it).
            let Some(pct_str) = line
                .split('(')
                .filter_map(|chunk| chunk.split(')').next())
                .find_map(|group| group.strip_suffix('%'))
            else {
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
        let p = immediate(false);
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
        let p = immediate(false);
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
            out.contains("verifying: 0 of 4 (0%) (10 candidates)"),
            "phase transition must show the new, reset denominator immediately \
             (not a stale 100%), got: {out:?}"
        );
        assert!(
            out.contains("verifying: 2 of 4 (50%) (10 candidates)"),
            "got: {out:?}"
        );
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
        let p = immediate(false);
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

    // ── Finding N3: the default interval itself must be pinned ────────────

    #[test]
    fn default_repaint_interval_is_in_a_sane_band() {
        // Nothing else in this suite pins DEFAULT_REPAINT_INTERVAL_MS: the
        // tests that exercise the true default (`tty_repaints_are_throttled_
        // not_flooded`, `non_tty_bounded_under_default_throttle_for_a_fast_
        // burst`) only assert upper bounds and would pass just as well if
        // the constant were an hour, while the deterministic positive test
        // (`tty_zero_interval_produces_one_repaint_per_advance`) injects 0
        // and never touches the default at all. Pin the constant directly so
        // a regression that defeats "live" (e.g. accidentally set to
        // something absurd) fails here instead of nowhere.
        assert!(
            (20..=500).contains(&DEFAULT_REPAINT_INTERVAL_MS),
            "expected the default repaint interval to sit in a sane, \
             genuinely-live band (20-500ms), got {DEFAULT_REPAINT_INTERVAL_MS}"
        );
    }

    // ── Finding N4: finish() is idempotent, and SinkGuard enforces it runs ─

    #[test]
    fn finish_is_idempotent_renders_exactly_one_completion_line() {
        let p = immediate(false);
        p.set_total(3);
        p.set_phase(Phase::Hashing);
        p.advance(3);

        p.finish();
        p.finish();
        p.finish();

        let out = captured(&p);
        assert_eq!(
            out.matches("done:").count(),
            1,
            "a second (and third) finish() must not render another \
             completion line, got: {out:?}"
        );
    }

    #[test]
    fn finish_is_idempotent_on_tty_too_only_one_terminator_newline() {
        let p = immediate(true);
        p.set_total(3);
        p.set_phase(Phase::Hashing);
        p.advance(3);

        p.finish();
        p.finish();

        let out = captured(&p);
        assert_eq!(
            out.matches('\n').count(),
            1,
            "a second finish() must not append another line-terminating \
             newline, got: {out:?}"
        );
    }

    /// A tiny `Write` that mirrors every write into a shared buffer, so the
    /// `SinkGuard` tests below can inspect what was written even after the
    /// `Progress`/`SinkGuard` (which own the writer) have been dropped.
    #[derive(Clone)]
    struct Mirror(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
    impl Write for Mirror {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn sink_guard_calls_finish_on_drop_when_never_called_explicitly() {
        // The whole point of finding N4: even if a caller never calls
        // `finish()` explicitly (e.g. an early `return` added by future
        // work), dropping the guard must still terminate progress.
        let buf = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        {
            let p = Progress::new_with_interval_ms(Mirror(buf.clone()), false, 0);
            p.set_total(2);
            p.set_phase(Phase::Hashing);
            p.advance(2);
            let guard = SinkGuard::new(Box::new(p));
            drop(guard); // never called finish() explicitly
        }

        let out = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert_eq!(
            out.matches("done:").count(),
            1,
            "Drop must render the completion line when finish() was never \
             called explicitly, got: {out:?}"
        );
    }

    #[test]
    fn sink_guard_drop_after_explicit_finish_is_still_exactly_one_line() {
        // Mirrors real `main.rs` usage: an explicit `finish()` call (so
        // progress is flushed before the stdout report, AC-20) followed by
        // the guard's `Drop` when `run()` returns. Must still render the
        // completion line exactly once.
        let buf = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        {
            let p = Progress::new_with_interval_ms(Mirror(buf.clone()), false, 0);
            p.set_total(2);
            p.set_phase(Phase::Hashing);
            p.advance(2);
            p.finish(); // explicit, as main.rs does
            let guard = SinkGuard::new(Box::new(p));
            drop(guard); // Drop's finish() must be a no-op now
        }

        let out = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert_eq!(
            out.matches("done:").count(),
            1,
            "explicit finish() + SinkGuard Drop must render exactly one \
             completion line, got: {out:?}"
        );
    }

    // ── AC-32: elapsed time on every render (Amendment 2026-08-06) ─────────

    #[test]
    fn determinate_lines_carry_elapsed_time_in_every_phase() {
        for tty in [true, false] {
            let p = immediate_with_elapsed(tty, 12_000);
            p.set_total(10);
            p.set_phase(Phase::Hashing);
            p.retotal(2);
            p.set_phase(Phase::Verifying);
            let out = captured(&p);
            assert!(
                out.contains("hashing: 0 of 10 (0%) (12s elapsed)"),
                "tty={tty}: {out:?}"
            );
            assert!(
                out.contains("verifying: 0 of 2 (0%) (10 candidates) (12s elapsed)"),
                "tty={tty}: {out:?}"
            );
        }
    }

    #[test]
    fn completion_line_reports_total_elapsed_time() {
        for tty in [true, false] {
            let p = immediate_with_elapsed(tty, 141_300);
            p.set_total(302);
            p.finish();
            let out = captured(&p);
            assert!(
                out.contains("done: 302 of 302 (100%) (141s total)"),
                "tty={tty}: {out:?}"
            );
        }
    }

    #[test]
    fn non_tty_indeterminate_scanning_stays_bare() {
        // AC-22: a non-redrawn indeterminate line would record a stale clock.
        let p = immediate_with_elapsed(false, 12_000);
        p.set_phase(Phase::Scanning);
        assert_eq!(captured(&p), "scanning...\n");
    }

    // ── MilestoneGate: the one definition of "a milestone" (ADR-0018 §2) ────

    #[test]
    fn milestone_gate_emits_on_phase_change_and_bucket_change_only() {
        let mut g = MilestoneGate::new();
        assert!(
            g.should_emit(Phase::Scanning, None, false),
            "first sight of a phase"
        );
        assert!(
            !g.should_emit(Phase::Scanning, None, false),
            "nothing changed"
        );
        assert!(
            g.should_emit(Phase::Hashing, Some(0), false),
            "phase changed"
        );
        assert!(
            !g.should_emit(Phase::Hashing, Some(9), false),
            "same 10% bucket"
        );
        assert!(
            g.should_emit(Phase::Hashing, Some(10), false),
            "next bucket"
        );
        assert!(!g.should_emit(Phase::Hashing, Some(19), false));
        assert!(
            g.should_emit(Phase::Hashing, Some(19), true),
            "force always emits"
        );
    }

    #[test]
    fn milestone_gate_does_not_advance_its_bucket_when_it_declines() {
        let mut g = MilestoneGate::new();
        assert!(g.should_emit(Phase::Hashing, Some(0), false));
        assert!(!g.should_emit(Phase::Hashing, Some(5), false));
        assert!(g.should_emit(Phase::Hashing, Some(10), false));
    }

    // ── Tee ──────────────────────────────────────────────────────────────

    /// Shares one buffer between a test and a boxed `Progress`.
    #[derive(Clone, Default)]
    struct SharedBuf(std::sync::Arc<Mutex<Vec<u8>>>);

    impl Write for SharedBuf {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl SharedBuf {
        fn text(&self) -> String {
            String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
        }
    }

    #[test]
    fn tee_forwards_every_call_to_both_sinks() {
        let (a, b) = (SharedBuf::default(), SharedBuf::default());
        let tee = Tee::new(
            Box::new(Progress::new_with_interval_ms(a.clone(), false, 0)),
            Box::new(Progress::new_with_interval_ms(b.clone(), false, 0)),
        );
        tee.set_phase(Phase::Scanning);
        tee.set_total(4);
        tee.set_phase(Phase::Hashing);
        tee.advance(4);
        tee.retotal(2);
        tee.set_phase(Phase::Verifying);
        tee.finish();

        assert_eq!(a.text(), b.text());
        assert!(a.text().contains("hashing: 4 of 4 (100%)"));
        assert!(a.text().contains("verifying: 0 of 2 (0%) (4 candidates)"));
        assert!(a.text().contains("done: 4 of 4 (100%)"));
    }

    /// Records how many bytes had been written at each `flush` call.
    #[derive(Default)]
    struct FlushProbe {
        bytes: Vec<u8>,
        flushed_at: Vec<usize>,
    }

    impl Write for FlushProbe {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.bytes.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.flushed_at.push(self.bytes.len());
            Ok(())
        }
    }

    #[test]
    fn tty_repaints_are_flushed_so_a_line_buffered_stdout_shows_them() {
        // ADR-0018 §8: with progress on stdout (a LineWriter), this flush is
        // what makes a `\r`-terminated repaint visible at all.
        let p = Progress::new_with_interval_ms(FlushProbe::default(), true, 0);
        p.set_phase(Phase::Scanning);
        p.advance(1);
        p.finish();

        let st = p.state.lock().unwrap();
        assert_eq!(
            st.writer.flushed_at.last().copied(),
            Some(st.writer.bytes.len()),
            "every byte written must have been flushed"
        );
        assert!(st.writer.flushed_at.len() >= 3, "one flush per repaint");
    }
}
