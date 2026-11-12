# Durable Run Record (`--output`) & Stream Routing (FR-20 / FR-21) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `--output <file>` (a continuously written, kill-surviving run record; NDJSON under `--json`), move routine output to stdout when stdout is an interactive terminal, and show elapsed time on every progress render.

**Architecture:** A pure `streams::route()` function decides, once, which stream carries progress and apply-time status. A new `record` module owns the single writer to the record file (`RunRecorder` trait, `NoRecord`, `FileRecord<W>`); pipeline stages call `recorder.record(event)` at the sites where they already push warnings and groups, and progress milestones reach the record through a `Tee` of two `ProgressSink`s. The result model, canonical sort, report and JSON document do not change.

**Tech Stack:** Rust 1.97.1 (edition 2024), clap 4, serde / serde_json, rayon; tests with `assert_cmd`, `predicates`, `tempfile`. **No new dependencies.**

**Spec:** [`docs/adr/0018-durable-run-record-and-output-stream-routing.md`](../../adr/0018-durable-run-record-and-output-stream-routing.md) (mechanism, normative) and [`docs/prd/PRD-0001-exact-image-deduplication.md`](../../prd/PRD-0001-exact-image-deduplication.md) — FR-19 (elapsed-time amendment), FR-20, FR-21, NFR-3, AC-20..AC-32 + AC-28a. Executors read both.

**Tasks:** TASK-0340 + TASK-0341, first slice. FR-22 (paging) is **out of scope**.

## Global Constraints

- One unit of work = one branch = one PR: branch `feat/output-record-and-stream-routing`, PR title `feat(output): durable run record and stdout stream routing (TASK-0340/0341)`. Code, tests, guide docs and the planning update all ride this PR.
- TDD is mandatory: failing test first, then the minimal code, then refactor.
- `make validate` must pass before anything is pushed for PR; `cargo deny check` must stay green (no new crates).
- `--json` stdout stays exactly one pretty-printed document with `schema_version: 1`; its bytes must not change. ADR-0016's plugin depends on it.
- With stdout redirected or piped, stdout bytes must be identical to today's report (AC-30) and identical with and without `--output` (AC-28).
- Never wrap the record file in a `BufWriter`: `main()` ends in `process::exit`, which runs no destructors. One record = one `write_all` of a complete, newline-terminated unit, under the recorder's mutex. No `fsync`.
- Every path and reason written to the human record passes through `safe_text::escape_control_chars`; NDJSON relies on `serde_json` string escaping.
- NDJSON `group`, `already_deduplicated`, `warning` and `error` lines reuse `json.rs`'s serde types — no parallel copies.
- `record_version: 1`; the event vocabulary is declared **unstable** in the user guide; the nested `result` keeps `schema_version: 1`'s existing stability.
- TTY-ness is probed exactly once, in `main.rs`, and injected as booleans. No renderer probes a stream.
- Nothing new may put a syscall or a contended lock on the `advance` hot path (called once per file from rayon workers).
- A record never carries a `Group i/N` index or a run total outside the completion document.
- Record-write failure mid-run: the run continues (including an in-flight `--apply`), exactly one `error: could not write to <path>: <reason>` on stderr, exit code `1`.
- An unwritable `--output` path fails before scanning with exit code `2` and writes no report.
- Conventional Commits; commit trailers as configured for the session.

## Decisions this plan makes that the spec left open

These need a nod from the owner / product-owner / architect during review; each is the recommended default and is cheap to change before Task 2 or Task 5 starts.

1. **Elapsed-time wording on progress lines (product-owner).** Determinate lines gain a trailing `(<n>s elapsed)`, separated by one space; the completion line gains `(<n>s total)`. Examples: `hashing: 1200 of 28450 (4%) (12s elapsed)`, `verifying: 1 of 2 (50%) (302 candidates) (131s elapsed)`, `done: 302 of 302 (100%) (141s total)`. The suffix goes last so every existing `contains("… (100%)")` assertion keeps passing. Non-TTY indeterminate `scanning...` stays bare (AC-22).
2. **`RunError` event (architect).** The `not applying: N scan root(s) could not be read` failure is recorded as `error` with `scope: "run"` and no `path` — an additive extension of ADR-0018 §4b's `error` vocabulary, needed for FR-20's "every error".
3. **`MilestoneSink` is a separate type wrapping `Arc<dyn RunRecorder>`** rather than `FileRecord` implementing `ProgressSink` itself (ADR-0018 §2). Same tee, same shared `MilestoneGate`; it keeps `FileRecord` to one job and lets milestones be unit-tested against a capturing recorder.
4. **Record order follows real occurrence**, so the `done` milestone precedes apply-time `trashed:` lines (ADR-0018 §4a's example shows the reverse; the example is illustrative).
5. **`RunRecorder` gains two defaulted methods**, `is_active()` (lets the engine skip a per-group clone when `--output` is absent) and `is_degraded()` (feeds the exit code).

## Review Focus

Input classes the spec implies but does not pin with an acceptance criterion; each has a test in the task named.

1. **A filename containing a newline or ESC** must not forge a record line or inject a terminal sequence into the human record — Task 5, `human_record_escapes_control_characters_in_paths`; Task 10, `record_cannot_be_forged_by_a_newline_in_a_filename`.
2. **`--output` naming a directory, or a path whose parent does not exist**, must exit `2` before scanning with an empty stdout — Task 10, `unwritable_output_path_fails_before_scanning`.
3. **A run with nothing to report** (empty directory) and **a run that refuses to apply** (unreadable root + `--apply`) must leave a truthful record: the first has a header and a completion document, the second has the errors and **no** completion document — Task 10, `empty_run_still_writes_header_and_result` and `refused_apply_leaves_errors_and_no_result`.
4. **Many rayon workers failing a record write at the same instant** must produce exactly one message — Task 6, `concurrent_write_failures_report_exactly_once`.
5. **Reading the record while the run is still going** (`tail -f`, or after a `SIGKILL`) must show everything recorded so far, with nothing held in a user-space buffer — Task 5, `records_are_visible_without_drop_or_flush`.

## File Structure

| File | Status | Responsibility |
|---|---|---|
| `src/streams.rs` | create | `Stream`, `Target`, `Routing`, pure `route()`, `ProgressStream` (a `Write` over stdout or stderr) |
| `src/record.rs` | create | `RecordEvent`, `Invocation`, `RunRecorder`, `NoRecord`, `FileRecord<W>`, human + NDJSON line rendering, `MilestoneSink`, `rfc3339_utc` |
| `src/progress.rs` | modify | `LineParams`, elapsed time on every render, `MilestoneGate`, free `percent()`, `Tee`, `Phase::label` → `pub(crate)` |
| `src/json.rs` | modify | `document()` + `JsonReport::to_pretty()`; `pub(crate)` payload constructors for record lines |
| `src/enumerate.rs`, `src/sniff.rs`, `src/engine.rs` | modify | one `&dyn RunRecorder` parameter each; `record()` at warning / root-error / group sites |
| `src/cli.rs` | modify | `--output <FILE>`; refreshed `--no-progress` help |
| `src/main.rs` | modify | fail-fast open, routing, tee, apply-event recording, completion document, exit code |
| `tests/output_record.rs` | create | integration tests for AC-25..AC-28a and the Review Focus items |
| `tests/cli_integration.rs` | modify | two assertions that pin superseded behavior |
| `README.md`, `docs/guide/output-and-exit-codes.md`, `docs/guide/getting-started.md` | modify | user docs (product-owner) |
| `docs/adr/0018-…md`, `docs/planning/status.md`, `docs/planning/backlog.md` | modify | ADR status, planning update |

## Batches and usage budget

Three work sessions on the one branch (pushing a branch triggers no CI; the PR opens once, at the end). Estimates are rough — the only calibration point is session 33's design pass (five agent dispatches, 2% → 83% of a 5h window) — and must be reconciled against `/usage` after Batch A before B is sized for real.

| Batch | Tasks | Agent dispatches | Est. 5h window | Est. weekly (general / Fable) |
|---|---|---|---|---|
| A — stream model + elapsed time | 0–4 | implementer ×1–2 | 40–55% | ~4–6% each |
| B — record core | 5–9 | implementer ×2 | 55–70% | ~6–8% each |
| C — wiring, acceptance, docs, review | 10–11 | implementer ×1, tester ×1, architect ×1–2, product-owner ×1 | 55–70% | ~6–8% each |

Cut lines if a batch trends over: A may stop after Task 3 (Task 4 moves to B); B may stop after Task 7 (8–9 move to C, and C then splits docs/review into a fourth session). Never start the architect review with less than ~35% of a window left — session 33 showed a review round-trip that surfaces a pivot costs about as much as the original pass.

---

### Task 0: Branch and planning hygiene

**Files:**

- Modify: `docs/planning/status.md` (the "In progress" section, top entry)
- Modify: `docs/planning/backlog.md` (rows `TASK-0340`, `TASK-0341`)

- [ ] **Step 1: Create the branch**

```bash
git switch -c feat/output-record-and-stream-routing
```

- [ ] **Step 2: Fix the stale status entry**

In `docs/planning/status.md`, the session 33 entry still says the unit is "ready to open its PR". Replace that closing clause with "merged as PR #98 (2026-08-06)" and add a new first entry under `## In progress`:

```markdown
- _Session 34 (TASK-0340/TASK-0341 implementation, slice 1: FR-20 + FR-21 +
  FR-19 elapsed time), 2026-11-12 — branch
  `feat/output-record-and-stream-routing`._ Executing
  [the implementation plan](../superpowers/plans/2026-11-12-output-record-and-stream-routing.md)
  in three usage-budgeted batches. FR-22 (paging) is not part of this unit.
```

Update `**Last updated:**` to `2026-11-12`.

- [ ] **Step 3: Mark the backlog rows in progress**

In `docs/planning/backlog.md`, set the Status cell of `TASK-0340` and `TASK-0341` to `in-progress`.

- [ ] **Step 4: Commit (plan file included)**

```bash
git add docs/planning/status.md docs/planning/backlog.md docs/superpowers/plans/2026-11-12-output-record-and-stream-routing.md
git commit -m "docs(planning): start FR-20/FR-21 implementation (TASK-0340/0341)"
```

---

### Task 1: `streams` — the routing decision

**Files:**

- Create: `src/streams.rs`
- Modify: `src/main.rs:5-15` (add `mod streams;`)

**Interfaces:**

- Consumes: nothing.
- Produces:
  - `pub enum Stream { Stdout, Stderr }`
  - `pub struct Target { pub stream: Stream, pub tty: bool }`
  - `pub struct Routing { pub progress: Option<Target>, pub status: Option<Target> }`
  - `pub fn route(json: bool, no_progress: bool, timings: bool, stdout_tty: bool, stderr_tty: bool) -> Routing`
  - `pub enum ProgressStream` with `pub fn open(stream: Stream) -> ProgressStream`, implementing `std::io::Write`

- [ ] **Step 1: Write the failing tests**

Create `src/streams.rs` with only the tests and the type declarations they need to name:

```rust
//! Output-stream routing (FR-21, ADR-0018 §8).
//!
//! One pure function decides which stream carries progress and apply-time
//! status. TTY-ness is probed exactly once, in `main.rs`, and passed in.

#[cfg(test)]
mod tests {
    use super::*;

    const OUT_TTY: Target = Target { stream: Stream::Stdout, tty: true };

    fn err(tty: bool) -> Target {
        Target { stream: Stream::Stderr, tty }
    }

    #[test]
    fn default_mode_on_an_interactive_stdout_routes_everything_to_stdout() {
        // AC-29, both stderr states (the second row is "stderr redirected").
        for stderr_tty in [true, false] {
            let r = route(false, false, false, true, stderr_tty);
            assert_eq!(r.progress, Some(OUT_TTY));
            assert_eq!(r.status, Some(OUT_TTY));
        }
    }

    #[test]
    fn default_mode_with_stdout_redirected_falls_back_to_stderrs_own_tty_ness() {
        // AC-30: `imgdedup ./photos > report.txt` keeps its live line.
        let r = route(false, false, false, false, true);
        assert_eq!(r.progress, Some(err(true)));
        assert_eq!(r.status, None);

        let r = route(false, false, false, false, false);
        assert_eq!(r.progress, Some(err(false)));
        assert_eq!(r.status, None);
    }

    #[test]
    fn json_never_routes_anything_to_stdout() {
        // AC-31, terminal or not.
        for stdout_tty in [true, false] {
            for stderr_tty in [true, false] {
                let r = route(true, false, false, stdout_tty, stderr_tty);
                assert_eq!(r.progress, Some(err(stderr_tty)));
                assert_eq!(r.status, None);
            }
        }
    }

    #[test]
    fn no_progress_and_timings_suppress_progress_but_not_status() {
        // AC-23 / ADR-0017: status still follows the stdout-TTY rule.
        for (no_progress, timings) in [(true, false), (false, true), (true, true)] {
            let r = route(false, no_progress, timings, true, true);
            assert_eq!(r.progress, None);
            assert_eq!(r.status, Some(OUT_TTY));
        }
    }

    #[test]
    fn invariants_hold_for_all_32_combinations() {
        for bits in 0u8..32 {
            let json = bits & 1 != 0;
            let no_progress = bits & 2 != 0;
            let timings = bits & 4 != 0;
            let stdout_tty = bits & 8 != 0;
            let stderr_tty = bits & 16 != 0;
            let r = route(json, no_progress, timings, stdout_tty, stderr_tty);
            let case = format!("{bits:05b}");

            // A target's `tty` always describes the stream it travels with,
            // so in-place control codes can never reach a redirected file.
            for t in [r.progress, r.status].into_iter().flatten() {
                let real = match t.stream {
                    Stream::Stdout => stdout_tty,
                    Stream::Stderr => stderr_tty,
                };
                assert_eq!(t.tty, real, "tty must match its stream ({case})");
            }
            // Nothing but the document reaches a stdout that is not a
            // terminal, or any stdout under --json.
            if json || !stdout_tty {
                assert!(r.progress.is_none_or(|t| t.stream == Stream::Stderr), "{case}");
                assert_eq!(r.status, None, "{case}");
            }
            // Routine status never falls back to stderr (ADR-0018 §9 #6).
            assert!(r.status.is_none_or(|t| t.stream == Stream::Stdout), "{case}");
            if no_progress || timings {
                assert_eq!(r.progress, None, "{case}");
            }
        }
    }
}
```

Add `mod streams;` to `src/main.rs` after `mod sniff;`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --bin imgdedup streams::`
Expected: compile error — `cannot find type Target`, `cannot find function route`.

- [ ] **Step 3: Write the implementation**

Insert above the test module in `src/streams.rs`:

```rust
use std::io::{self, Write};

/// One of the process's two output streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

/// A stream together with whether *that stream* is an interactive terminal.
/// Binding the two is what makes it impossible to pick the in-place renderer
/// from one stream's TTY-ness while writing to the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    pub stream: Stream,
    pub tty: bool,
}

/// Where routine output goes for this run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Routing {
    /// Progress target; `None` when `--no-progress` or `--timings` suppress it.
    pub progress: Option<Target>,
    /// Apply-time `trashed:`/`deleted:` lines; `Some` only in the default
    /// mode with stdout an interactive terminal. Never stderr.
    pub status: Option<Target>,
}

/// Decide the routing from the five facts it depends on (ADR-0018 §8).
pub fn route(
    json: bool,
    no_progress: bool,
    timings: bool,
    stdout_tty: bool,
    stderr_tty: bool,
) -> Routing {
    let interactive_stdout = !json && stdout_tty;
    let stdout = Target { stream: Stream::Stdout, tty: true };
    let progress = if no_progress || timings {
        None
    } else if interactive_stdout {
        Some(stdout)
    } else {
        Some(Target { stream: Stream::Stderr, tty: stderr_tty })
    };
    Routing { progress, status: interactive_stdout.then_some(stdout) }
}

/// The writer `Progress` renders to, chosen at runtime. An enum rather than
/// `Box<dyn Write>`: no allocation, no dynamic dispatch.
pub enum ProgressStream {
    Out(io::Stdout),
    Err(io::Stderr),
}

impl ProgressStream {
    pub fn open(stream: Stream) -> Self {
        match stream {
            Stream::Stdout => ProgressStream::Out(io::stdout()),
            Stream::Stderr => ProgressStream::Err(io::stderr()),
        }
    }
}

impl Write for ProgressStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            ProgressStream::Out(s) => s.write(buf),
            ProgressStream::Err(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            ProgressStream::Out(s) => s.flush(),
            ProgressStream::Err(s) => s.flush(),
        }
    }
}
```

`ProgressStream` is unused until Task 4; add `#[allow(dead_code)]` on the enum and its `impl` blocks now and remove it in Task 4.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --bin imgdedup streams:: && cargo clippy --all-targets -- -D warnings`
Expected: 5 passed; clippy clean.

- [ ] **Step 5: Commit**

```bash
git add src/streams.rs src/main.rs
git commit -m "feat(streams): pure output-stream routing decision (FR-21)"
```

---

### Task 2: Elapsed time on every progress render (AC-32)

**Files:**

- Modify: `src/progress.rs:400-478` (`line`), `:491-566` (`paint`), `:618-663` (`finish`), module doc
- Modify: `tests/cli_integration.rs:1008-1012`

**Interfaces:**

- Consumes: nothing new.
- Produces: `struct LineParams` (private to `progress.rs`); the rendered line format in decision 1 above.

- [ ] **Step 1: Write the failing tests**

Add to `src/progress.rs`'s test module, after the TASK-0332 tests:

```rust
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --bin imgdedup progress::tests::determinate_lines progress::tests::completion_line`
Expected: FAIL — output lacks `(12s elapsed)` / `(141s total)`. (`non_tty_indeterminate_scanning_stays_bare` already passes; it pins behavior that must not change.)

- [ ] **Step 3: Implement**

Replace `Progress::line`'s seven-parameter signature with a params struct. Above `impl<W: Write> Progress<W>`:

```rust
/// Everything one rendered status line depends on (`Progress::line`).
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
```

Replace the body of `line` (keep the existing doc comment, replacing its last paragraph about "seven parameters" with one sentence pointing at `LineParams`):

```rust
fn line(p: &LineParams) -> String {
    let label = p.phase.label();
    let (current, total, elapsed) = (p.current, p.phase_total, p.elapsed_secs);
    match p.pct {
        None if p.show_count => {
            let noun = if current == 1 { "file" } else { "files" };
            format!("{label}... {current} {noun} found ({elapsed}s elapsed)")
        }
        None => format!("{label}..."),
        Some(pct) if p.phase == Phase::Verifying => format!(
            "{label}: {current} of {total} ({pct}%) ({} candidates) ({elapsed}s elapsed)",
            p.run_total
        ),
        Some(pct) if p.phase == Phase::Done => {
            format!("{label}: {current} of {total} ({pct}%) ({elapsed}s total)")
        }
        Some(pct) => format!("{label}: {current} of {total} ({pct}%) ({elapsed}s elapsed)"),
    }
}
```

Keep the existing long comment about why elapsed time is shown from the first render above the `None if p.show_count` arm.

In `paint`, build the params once and use them in both branches:

```rust
let params = LineParams {
    phase,
    current,
    phase_total,
    run_total,
    pct,
    show_count: self.tty,
    elapsed_secs: now / 1000,
};
```

then `Self::line(&params)` in place of both seven-argument calls (the non-TTY branch previously passed `show_count: false`; `self.tty` is `false` there, so behavior is unchanged). Replace the "Only meaningful … on the TTY indeterminate branch" comment with: `// AC-32: every render carries elapsed time; only the non-TTY indeterminate line omits it (AC-22).`

In `finish`, replace the `Self::line(...)` call and the comment above it:

```rust
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
```

- [ ] **Step 4: Update the assertions that pin the old exact lines**

Run: `cargo test --bin imgdedup progress::`

Every failure is an `assert_eq!` (or an exact `contains("…\n")`) on a determinate line. Fix each by appending the suffix the test's clock implies — tests built with `immediate(..)` or `Progress::new(..)` run in well under a second, so append a space and `(0s elapsed)` to `scanning:`/`hashing:`/`verifying:` lines, and a space and `(0s total)` to `done:` lines. Do not loosen an exact assertion into a `contains`. Indeterminate lines are unchanged.

In `tests/cli_integration.rs`, the assertion at lines 1008-1012 uses `!stderr.contains("scanning: 6 of 6 (100%) (")` to prove there is no `(N candidates)` suffix; the elapsed suffix now makes that string appear. Replace it with:

```rust
let closing = stderr
    .lines()
    .find(|l| l.starts_with("scanning: 6 of 6 (100%)"))
    .expect("closing scanning line");
assert!(
    !closing.contains("candidates)"),
    "scanning's determinate state must never carry a (N candidates) \
     suffix (AC-21a; that suffix is Verifying-only); got:\n{stderr}"
);
```

- [ ] **Step 5: Update the module doc**

In `src/progress.rs`'s module doc, in "Rendering rules", add to the TTY bullet: "Every render carries the elapsed time since the run started, and the completion line carries the total (AC-32)." and to the Non-TTY bullet: "Determinate milestone lines carry elapsed time too; only the indeterminate `scanning...` line stays bare (AC-22)."

- [ ] **Step 6: Run the full suite**

Run: `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add src/progress.rs tests/cli_integration.rs
git commit -m "feat(progress): elapsed time on every render and total on completion (AC-32)"
```

---

### Task 3: Shared milestone cadence — `MilestoneGate`, `percent`, `Tee`

A behavior-preserving refactor plus one new combinator. No rendered byte changes.

**Files:**

- Modify: `src/progress.rs`

**Interfaces:**

- Consumes: nothing new.
- Produces (all in `progress.rs`):
  - `pub(crate) const DEFAULT_REPAINT_INTERVAL_MS: u64` (visibility change)
  - `pub(crate) fn percent(current: u64, total: u64) -> u64`
  - `impl Phase { pub(crate) fn label(self) -> &'static str }` (visibility change)
  - `pub(crate) struct MilestoneGate` with `pub(crate) fn new() -> Self` and `pub(crate) fn should_emit(&mut self, phase: Phase, pct: Option<u64>, force: bool) -> bool`
  - `pub struct Tee` with `pub fn new(first: Box<dyn ProgressSink>, second: Box<dyn ProgressSink>) -> Self`, implementing `ProgressSink`

- [ ] **Step 1: Write the failing tests**

Add to the test module:

```rust
// ── MilestoneGate: the one definition of "a milestone" (ADR-0018 §2) ────

#[test]
fn milestone_gate_emits_on_phase_change_and_bucket_change_only() {
    let mut g = MilestoneGate::new();
    assert!(g.should_emit(Phase::Scanning, None, false), "first sight of a phase");
    assert!(!g.should_emit(Phase::Scanning, None, false), "nothing changed");
    assert!(g.should_emit(Phase::Hashing, Some(0), false), "phase changed");
    assert!(!g.should_emit(Phase::Hashing, Some(9), false), "same 10% bucket");
    assert!(g.should_emit(Phase::Hashing, Some(10), false), "next bucket");
    assert!(!g.should_emit(Phase::Hashing, Some(19), false));
    assert!(g.should_emit(Phase::Hashing, Some(19), true), "force always emits");
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --bin imgdedup progress::tests::milestone_gate progress::tests::tee`
Expected: compile error — `MilestoneGate` and `Tee` not found.

- [ ] **Step 3: Implement**

1. Change `const DEFAULT_REPAINT_INTERVAL_MS` to `pub(crate) const`, and `fn label` on `Phase` to `pub(crate) fn label`.
2. Move `Progress::percent` out of the `impl` to a free function with the same body and doc, `pub(crate) fn percent(current: u64, total: u64) -> u64`; replace `Self::percent(` with `percent(` at its two call sites (`paint`, `finish`).
3. Add after `NoProgress`:

```rust
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
```

Then, in `State<W>`, replace the `last_phase_shown` and `last_percent_shown` fields with `gate: MilestoneGate` (initialized with `MilestoneGate::new()` in `new_with_interval_ms`). In `paint`, delete `st.last_phase_shown = Some(phase);` from the TTY branch and replace the non-TTY branch body with:

```rust
} else if st.gate.should_emit(phase, pct, force) {
    let mut buf = Self::line(&params);
    buf.push('\n');
    let _ = st.writer.write_all(buf.as_bytes());
}
```

In `finish`, delete the final `st.last_phase_shown = Some(Phase::Done);`.

Finally, add after `SinkGuard`:

```rust
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
```

`Tee`, `MilestoneGate::new` and the `pub(crate)` items are unused outside tests until Tasks 8 and 10; add `#[allow(dead_code)]` to `Tee` and its `impl Tee` only, and remove it in Task 10.

- [ ] **Step 4: Run the full suite**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: all pass, including every pre-existing non-TTY milestone test unchanged — that is the evidence the refactor preserved behavior.

- [ ] **Step 5: Commit**

```bash
git add src/progress.rs
git commit -m "refactor(progress): extract MilestoneGate and percent; add Tee sink"
```

---

### Task 4: Route progress and apply-time status in `main.rs` (FR-21)

**Files:**

- Modify: `src/main.rs:73-101` (sink construction), `:253-273` (apply event closure)
- Modify: `src/progress.rs` (module doc; one regression test)
- Modify: `src/streams.rs` (drop `#[allow(dead_code)]`)
- Modify: `src/cli.rs:93-98` (`--no-progress` help)
- Modify: `tests/cli_integration.rs:~325` and a new test

**Interfaces:**

- Consumes: `streams::route`, `streams::ProgressStream::open`, `streams::Routing` (Task 1).
- Produces: a `routing: streams::Routing` local in `run()` that Task 10 reads (`routing.progress.is_some_and(|t| t.tty)`).

- [ ] **Step 1: Write the failing regression test for the load-bearing flush**

Rust's `Stdout` is line-buffered; an in-place repaint ends in `\x1b[K`, not `\n`, so without an explicit flush it would never appear. Add to `src/progress.rs`'s test module:

```rust
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
```

Run: `cargo test --bin imgdedup progress::tests::tty_repaints_are_flushed`
Expected: PASS already (the flush exists). This is a pin, not a red test — confirm it goes red by temporarily deleting `let _ = st.writer.flush();` from `paint`'s TTY branch, then restore the line.

- [ ] **Step 2: Write the failing integration tests**

In `tests/cli_integration.rs`, find the test around line 325 that asserts `stderr.contains("deleted: ") && stderr.contains("b.png")`. Under FR-21 a removal line is routine status and, with stdout piped (as `assert_cmd` always runs it), is written to **neither** stream. Replace that assertion with:

```rust
assert!(
    !stderr.contains("deleted: "),
    "apply-time removal lines are routine status: with stdout not a \
     terminal they are written to neither stream (FR-21, ADR-0018 §9 #6); \
     got stderr:\n{stderr}"
);
```

and, in the same test, assert the removal is still reported by the document on stdout (use the test's existing stdout binding; if it has none, bind `let stdout = String::from_utf8_lossy(&out.stdout);`):

```rust
assert!(
    stdout.contains("b.png"),
    "the report's performed-actions section remains the authoritative \
     record of the removal; got stdout:\n{stdout}"
);
```

Add a new test next to it:

```rust
/// AC-29's other half, observable without a pty: with nothing going wrong,
/// an `--apply` run with `--no-progress` leaves stderr completely empty —
/// stderr carries genuine failures only (FR-21).
#[test]
fn clean_apply_run_writes_nothing_to_stderr_without_progress() {
    let dir = tempfile::tempdir().expect("tempdir");
    materialize_progress_fixture(dir.path());

    let out = cmd()
        .arg(dir.path())
        .args(["--apply", "--delete", "--no-progress"])
        .output()
        .expect("imgdedup must spawn");

    assert!(out.status.success());
    assert!(
        out.stderr.is_empty(),
        "expected an empty stderr; got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test --test cli_integration clean_apply_run_writes_nothing`
Expected: FAIL — stderr contains `deleted: …` lines. The edited test near line 325 fails the same way.

- [ ] **Step 4: Implement the routing in `main.rs`**

Replace the block from `let progress_enabled = …` through the `SinkGuard::new(…)` statement with:

```rust
// FR-21 (ADR-0018 §8): decided once, from facts probed once. In the
// default mode progress goes to stdout when stdout is an interactive
// terminal and to stderr otherwise; under `--json` it always goes to
// stderr. `--no-progress` and `--timings` suppress it (AC-23, ADR-0017).
let routing = streams::route(
    args.json,
    args.no_progress,
    args.timings,
    std::io::stdout().is_terminal(),
    std::io::stderr().is_terminal(),
);
let sink: progress::SinkGuard = progress::SinkGuard::new(match routing.progress {
    None => Box::new(progress::NoProgress),
    Some(target) => Box::new(progress::Progress::new(
        streams::ProgressStream::open(target.stream),
        target.tty,
    )),
});
```

In the long comment above it, replace "Otherwise progress always targets stderr, and TTY-ness is probed exactly once here" with "TTY-ness of both streams is probed exactly once here". In the comment above `sink.finish()`, replace "fully written to stderr" with "fully written to whichever stream carries it — which under FR-21 can be stdout itself, so this ordering is now correctness, not tidiness".

In the apply event closure, replace the `Removed` arm:

```rust
apply::Event::Removed(path) => {
    // Routine status (FR-21): stdout when stdout is an interactive
    // terminal in the default mode, otherwise not written at all —
    // the result document's `actions` and `--output` cover it
    // (ADR-0018 §9 #6). Never stderr.
    if routing.status.is_some() {
        println!(
            "{verb}: {}",
            safe_text::escape_control_chars(&path.display().to_string())
        );
    }
}
```

Leave the `Skipped` arm on `eprintln!` (ADR-0018 §9 #7). Update the Stage 4 comment: replace "Progress and per-file skips have already gone to stderr, so a run that dies mid-apply still told the user something; stdout stays reserved for the one report/JSON document (the S2 purity contract)" with "Per-file skips go to stderr as they happen; removal lines follow FR-21's routing".

Remove the `#[allow(dead_code)]` attributes from `ProgressStream` in `src/streams.rs`.

- [ ] **Step 5: Refresh the stale text**

`src/progress.rs` module doc: replace "This module never touches stdout — it renders exclusively to whatever writer it is constructed with (in production, stderr)" with "It renders exclusively to the writer it is constructed with — in production the stream `streams::route` selects (FR-21)". Retitle the section "Nothing else may write to stderr while a TTY progress line is live" to "Nothing else may write to the progress stream while a TTY line is live" and replace "stderr" with "the progress stream" in its body where it refers to the stream being redrawn.

`src/cli.rs` `--no-progress` help — replace with:

```rust
/// Disable the live progress indicator (FR-19). Progress is shown by
/// default on the terminal — on stdout when stdout is an interactive
/// terminal, otherwise on stderr; under `--json` always on stderr. This
/// flag turns it off completely: nothing progress-related is written to
/// any stream, and the report is unaffected either way (AC-23).
```

and in `--timings`' help replace "cannot share stderr" with "cannot share a terminal".

- [ ] **Step 6: Run the full suite**

Run: `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
Expected: all pass. If another integration test asserts `trashed:`/`deleted:` on stderr, apply the same replacement as Step 2.

- [ ] **Step 7: Manual check on a real terminal (cannot be automated without a pty)**

```bash
cargo build --release
./target/release/imgdedup test-images                # progress line redraws in place; `echo $?` → 0
./target/release/imgdedup test-images 2>/dev/null    # progress still visible (it is on stdout)
./target/release/imgdedup test-images > /tmp/r.txt   # live line on the terminal (stderr); r.txt has no progress text
./target/release/imgdedup test-images --json | head -3   # first line is `{`
```

Expected: as annotated. Record the four observations in the commit body.

- [ ] **Step 8: Commit**

```bash
git add src/main.rs src/progress.rs src/streams.rs src/cli.rs tests/cli_integration.rs
git commit -m "feat(output): route progress and apply status to stdout on a terminal (FR-21)"
```

**End of Batch A.** Ask the owner for `/usage` (5h window %, weekly general %, weekly Fable %), record estimate vs. actual in `docs/planning/status.md`, and re-size Batch B.

---

### Task 5: `record` — events, `FileRecord`, the human-readable format

**Files:**

- Create: `src/record.rs`
- Modify: `src/main.rs` (add `mod record;`)

**Interfaces:**

- Consumes: `progress::Phase` and `Phase::label` (Task 3), `engine::{DuplicateGroup, HardLinkNote}`, `apply::ActionMode`, `safe_text::escape_control_chars`.
- Produces:
  - `pub const RECORD_VERSION: u32 = 1;`
  - `pub struct Invocation { pub roots: Vec<String>, pub keep: &'static str, pub apply: bool, pub delete: bool, pub json: bool, pub no_recurse: bool, pub hidden: bool, pub follow_symlinks: bool, pub ext: Option<String>, pub exit_code: bool, pub no_progress: bool, pub timings: bool, pub output: String }` (derives `Debug, Clone, Serialize`)
  - `pub enum RecordEvent<'a>` — variants exactly as in Step 3
  - `pub trait RunRecorder: Sync + Send { fn record(&self, event: RecordEvent<'_>); fn is_active(&self) -> bool { true } fn is_degraded(&self) -> bool { false } }`
  - `pub struct NoRecord;`
  - `pub enum Format { Human, Ndjson }`
  - `pub struct FileRecord<W: Write + Send>` with `pub fn new(writer: W, format: Format, target: String, on_failure: Box<dyn Fn(&str) + Send + Sync>) -> Self`
  - `pub(crate) fn rfc3339_utc(secs_since_epoch: u64) -> String`
  - test-only: `pub(crate) struct Capture` (a `RunRecorder` that stores each event's human rendering) with `pub(crate) fn lines(&self) -> Vec<String>`

- [ ] **Step 1: Write the failing tests**

Create `src/record.rs` containing the module doc and this test module (types come in Step 3):

```rust
//! The durable run record (`--output <file>`, FR-20, ADR-0018).
//!
//! One type, [`FileRecord`], is the single writer to the record file. Stages
//! hand it [`RecordEvent`]s at the moment things happen; it renders each one
//! as a complete, newline-terminated unit and writes it with one unbuffered
//! `write_all`, so everything recorded survives the process dying.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{DuplicateGroup, FileRec, HardLinkNote};
    use std::path::PathBuf;
    use std::sync::Arc;

    fn rec(path: &str, size: u64) -> FileRec {
        FileRec::new_for_test(PathBuf::from(path), size, Some(1_750_000_000))
    }

    fn invocation() -> Invocation {
        Invocation {
            roots: vec!["/photos".to_owned()],
            keep: "shortest",
            apply: false,
            delete: false,
            json: false,
            no_recurse: false,
            hidden: false,
            follow_symlinks: false,
            ext: None,
            exit_code: false,
            no_progress: false,
            timings: false,
            output: "run.txt".to_owned(),
        }
    }

    // ── timestamps ───────────────────────────────────────────────────────

    #[test]
    fn rfc3339_utc_formats_known_instants() {
        assert_eq!(rfc3339_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339_utc(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(rfc3339_utc(1_786_015_353), "2026-08-06T11:22:33Z");
    }

    // ── human format (ADR-0018 §4a) ──────────────────────────────────────

    #[test]
    fn human_header_identifies_tool_start_and_invocation() {
        let text = render_human(&RecordEvent::Header(&invocation()), 0, 1_786_015_353);
        let expected = format!(
            "# imgdedup {} run record\n\
             # started:    2026-08-06T11:22:33Z\n\
             # invocation: roots=[/photos] keep=shortest apply=false delete=false hidden=false\n\
             #             recurse=true follow-symlinks=false ext=<default> exit-code=false\n",
            env!("CARGO_PKG_VERSION")
        );
        assert_eq!(text, expected);
    }

    #[test]
    fn human_milestones_are_stamped_and_follow_the_progress_vocabulary() {
        let m = |phase, current, total, percent, run_total| RecordEvent::Milestone {
            phase,
            current,
            total,
            percent,
            run_total,
        };
        assert_eq!(
            render_human(&m(Phase::Scanning, 0, None, None, 0), 0, 0),
            "[     0.0s] scanning...\n"
        );
        assert_eq!(
            render_human(&m(Phase::Hashing, 1200, Some(28450), Some(4), 28450), 12_040, 0),
            "[    12.0s] hashing: 1200 of 28450 (4%)\n"
        );
        assert_eq!(
            render_human(&m(Phase::Verifying, 12, Some(40), Some(30), 28450), 131_400, 0),
            "[   131.4s] verifying: 12 of 40 (30%) (28450 candidates)\n"
        );
        assert_eq!(
            render_human(&m(Phase::Done, 28450, Some(28450), Some(100), 28450), 141_300, 0),
            "[   141.3s] done: 28450 of 28450 (100%)\n"
        );
    }

    #[test]
    fn human_group_block_has_no_index_and_indents_its_rows() {
        let group = DuplicateGroup {
            keeper: rec("/photos/a.jpg", 4096),
            removable: vec![rec("/photos/b.jpg", 4096), rec("/photos/c.jpg", 4096)],
            protected: vec![rec("/photos/d.jpg", 4096)],
        };
        let notes = vec![HardLinkNote {
            representative: PathBuf::from("/photos/d.jpg"),
            siblings: vec![PathBuf::from("/photos/e.jpg")],
        }];
        let text = render_human(
            &RecordEvent::Group { group: &group, notes: &notes },
            131_500,
            0,
        );
        assert_eq!(
            text,
            "[   131.5s] group (8192 bytes reclaimable):\n\
             \x20             KEEP    /photos/a.jpg\n\
             \x20             REMOVE  /photos/b.jpg\n\
             \x20             REMOVE  /photos/c.jpg\n\
             \x20             PROTECT /photos/d.jpg\n\
             \x20             hard-linked: /photos/d.jpg, /photos/e.jpg\n"
        );
        assert!(!text.contains("Group "), "a record group never carries an index");
    }

    #[test]
    fn human_diagnostics_and_actions() {
        let p = PathBuf::from("/photos/x.jpg");
        let note = HardLinkNote {
            representative: PathBuf::from("/photos/f.jpg"),
            siblings: vec![PathBuf::from("/photos/g.jpg")],
        };
        let cases: Vec<(RecordEvent<'_>, &str)> = vec![
            (
                RecordEvent::AlreadyDeduplicated(&note),
                "[     1.0s] already deduplicated (hard-linked): /photos/f.jpg\n\
                 \x20             hard-linked: /photos/f.jpg, /photos/g.jpg\n",
            ),
            (
                RecordEvent::Warning { stage: "sniff", path: &p, reason: "file is empty" },
                "[     1.0s] warning: sniff: /photos/x.jpg: file is empty\n",
            ),
            (
                RecordEvent::RootError { path: &p, reason: "No such file or directory" },
                "[     1.0s] error: /photos/x.jpg: No such file or directory\n",
            ),
            (
                RecordEvent::RunError { reason: "not applying" },
                "[     1.0s] error: not applying\n",
            ),
            (
                RecordEvent::Action { mode: ActionMode::Trash, path: &p, skipped: None },
                "[     1.0s] trashed: /photos/x.jpg\n",
            ),
            (
                RecordEvent::Action {
                    mode: ActionMode::Delete,
                    path: &p,
                    skipped: Some("no longer exists"),
                },
                "[     1.0s] warning: /photos/x.jpg: no longer exists\n",
            ),
        ];
        for (event, expected) in cases {
            assert_eq!(render_human(&event, 1_000, 0), expected);
        }
    }

    #[test]
    fn human_result_is_a_banner_then_the_exact_report_bytes() {
        let report = "Group 1/1 (2 files, 10 bytes reclaimable):\n  KEEP  /a\n";
        let text = render_human(
            &RecordEvent::Result { rendered: report, json: None },
            141_300,
            1_786_015_353,
        );
        assert_eq!(
            text,
            format!(
                "\n===== RESULT \u{2014} run completed 2026-08-06T11:22:33Z after 141.3s =====\n\n{report}"
            )
        );
        assert!(text.ends_with(report), "AC-26: the tail is the report, verbatim");
    }

    /// Review Focus 1: a hostile filename must not start a forged record.
    #[test]
    fn human_record_escapes_control_characters_in_paths() {
        let evil = PathBuf::from("/p/a\n[     9.9s] trashed: /etc/passwd\u{1b}[2J.jpg");
        let group = DuplicateGroup {
            keeper: rec("/p/keep.jpg", 1),
            removable: vec![FileRec::new_for_test(evil.clone(), 1, None)],
            protected: vec![],
        };
        let mut inv = invocation();
        inv.roots = vec![evil.display().to_string()];
        let rendered = [
            render_human(&RecordEvent::Header(&inv), 0, 0),
            render_human(&RecordEvent::Group { group: &group, notes: &[] }, 0, 0),
            render_human(
                &RecordEvent::Warning { stage: "engine", path: &evil, reason: "x\ny" },
                0,
                0,
            ),
            render_human(&RecordEvent::RootError { path: &evil, reason: "r" }, 0, 0),
            render_human(
                &RecordEvent::Action { mode: ActionMode::Trash, path: &evil, skipped: None },
                0,
                0,
            ),
        ];
        for text in rendered {
            assert!(!text.contains('\u{1b}'), "raw ESC leaked: {text:?}");
            assert!(
                !text.lines().any(|l| l.starts_with("[     9.9s]")),
                "forged record line: {text:?}"
            );
        }
    }

    // ── FileRecord ───────────────────────────────────────────────────────

    #[test]
    fn no_record_is_inactive_and_never_degraded() {
        let r = NoRecord;
        r.record(RecordEvent::RunError { reason: "ignored" });
        assert!(!r.is_active());
        assert!(!r.is_degraded());
    }

    /// Review Focus 5: nothing sits in a user-space buffer. The file is read
    /// back while the recorder is still alive and never flushed or dropped —
    /// what a `tail -f`, or a reader after SIGKILL, would see (AC-25).
    #[test]
    fn records_are_visible_without_drop_or_flush() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.txt");
        let file = std::fs::File::create(&path).unwrap();
        let recorder = FileRecord::new(
            file,
            Format::Human,
            path.display().to_string(),
            Box::new(|_| panic!("no write should fail")),
        );

        recorder.record(RecordEvent::Header(&invocation()));
        recorder.record(RecordEvent::RunError { reason: "first" });

        let seen = std::fs::read_to_string(&path).unwrap();
        assert!(seen.starts_with("# imgdedup "));
        assert!(seen.ends_with("error: first\n"));
        assert!(!seen.contains("===== RESULT"), "no completion document yet");
        std::mem::forget(recorder); // as `process::exit` would: no destructor
        assert_eq!(std::fs::read_to_string(&path).unwrap(), seen);
    }

    #[test]
    fn each_record_is_one_complete_newline_terminated_write() {
        #[derive(Clone, Default)]
        struct Writes(Arc<Mutex<Vec<Vec<u8>>>>);
        impl Write for Writes {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().push(buf.to_vec());
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let writes = Writes::default();
        let recorder = FileRecord::new(
            writes.clone(),
            Format::Human,
            "run.txt".to_owned(),
            Box::new(|_| {}),
        );
        recorder.record(RecordEvent::Header(&invocation()));
        recorder.record(RecordEvent::RunError { reason: "x" });

        let writes = writes.0.lock().unwrap();
        assert_eq!(writes.len(), 2, "one write per record");
        assert!(writes.iter().all(|w| w.ends_with(b"\n")));
    }
}
```

Add `mod record;` to `src/main.rs`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --bin imgdedup record::`
Expected: compile error — `Invocation`, `RecordEvent`, `render_human`, `FileRecord`, `NoRecord`, `rfc3339_utc` not found.

- [ ] **Step 3: Write the implementation**

Insert between the module doc and the test module:

```rust
use crate::apply::ActionMode;
use crate::engine::{DuplicateGroup, HardLinkNote};
use crate::progress::Phase;
use crate::safe_text::escape_control_chars;
use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Version of the record's *event vocabulary*. Independent of the result
/// document's `schema_version`. The vocabulary is unstable for now: consumers
/// must ignore unknown `type` values and unknown fields (ADR-0018 §4c).
pub const RECORD_VERSION: u32 = 1;

/// The invocation, as written into the record header.
#[derive(Debug, Clone, Serialize)]
pub struct Invocation {
    pub roots: Vec<String>,
    pub keep: &'static str,
    pub apply: bool,
    pub delete: bool,
    pub json: bool,
    pub no_recurse: bool,
    pub hidden: bool,
    pub follow_symlinks: bool,
    pub ext: Option<String>,
    pub exit_code: bool,
    pub no_progress: bool,
    pub timings: bool,
    pub output: String,
}

/// One thing that happened during a run, handed to a [`RunRecorder`] at the
/// moment it happened.
pub enum RecordEvent<'a> {
    Header(&'a Invocation),
    /// A coarse progress milestone. `total`/`percent` are `None` while the
    /// denominator is unknown.
    Milestone {
        phase: Phase,
        current: u64,
        total: Option<u64>,
        percent: Option<u64>,
        run_total: u64,
    },
    /// A duplicate group whose membership is final (ADR-0018 §1). `group`'s
    /// members and `notes` are already in canonical order. Never indexed.
    Group {
        group: &'a DuplicateGroup,
        notes: &'a [HardLinkNote],
    },
    AlreadyDeduplicated(&'a HardLinkNote),
    Warning {
        stage: &'static str,
        path: &'a Path,
        reason: &'a str,
    },
    /// A scan root that could not be read (run-level, exit 1).
    RootError { path: &'a Path, reason: &'a str },
    /// A run-level failure not tied to one path.
    RunError { reason: &'a str },
    /// An apply-time removal (`skipped: None`) or skip (`Some(reason)`).
    Action {
        mode: ActionMode,
        path: &'a Path,
        skipped: Option<&'a str>,
    },
    /// The completion document. `rendered` is the exact string printed to
    /// stdout; `json` is the same document as a value, present under `--json`.
    Result {
        rendered: &'a str,
        json: Option<&'a crate::json::JsonReport>,
    },
}

/// Observer the pipeline notifies as results and diagnostics are produced.
/// Orthogonal to `ProgressSink`: `--no-progress` never silences it.
pub trait RunRecorder: Sync + Send {
    fn record(&self, event: RecordEvent<'_>);

    /// `false` for [`NoRecord`], so a caller can skip work that only exists
    /// to build an event.
    fn is_active(&self) -> bool {
        true
    }

    /// `true` once a write has failed; the run then exits 1 (ADR-0018 §7).
    fn is_degraded(&self) -> bool {
        false
    }
}

/// The recorder used when `--output` is absent. Does nothing.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoRecord;

impl RunRecorder for NoRecord {
    fn record(&self, _event: RecordEvent<'_>) {}

    fn is_active(&self) -> bool {
        false
    }
}

/// The record file's format: human-readable by default, NDJSON under `--json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Human,
    Ndjson,
}

/// The single writer to the record file.
///
/// Generic over its writer so the failure path is testable without a full
/// disk. In production `W` is a bare `std::fs::File` — **never** a
/// `BufWriter`: `main()` ends in `process::exit`, which runs no destructors,
/// so a user-space buffer would lose the tail of every run.
pub struct FileRecord<W: Write + Send> {
    writer: Mutex<W>,
    format: Format,
    start: Instant,
    /// The `--output` path, for the failure message.
    target: String,
    degraded: AtomicBool,
    /// Called once, with the complete message, on the first failed write.
    on_failure: Box<dyn Fn(&str) + Send + Sync>,
}

impl<W: Write + Send> FileRecord<W> {
    pub fn new(
        writer: W,
        format: Format,
        target: String,
        on_failure: Box<dyn Fn(&str) + Send + Sync>,
    ) -> Self {
        Self {
            writer: Mutex::new(writer),
            format,
            start: Instant::now(),
            target,
            degraded: AtomicBool::new(false),
            on_failure,
        }
    }
}

impl<W: Write + Send> RunRecorder for FileRecord<W> {
    fn record(&self, event: RecordEvent<'_>) {
        let result = {
            let mut writer = self.writer.lock().unwrap_or_else(|e| e.into_inner());
            // Stamped and rendered under the lock so records are written in
            // the order their timestamps claim.
            let elapsed_ms = self.start.elapsed().as_millis() as u64;
            let wall_secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            let text = match self.format {
                Format::Human => render_human(&event, elapsed_ms, wall_secs),
                Format::Ndjson => render_human(&event, elapsed_ms, wall_secs),
            };
            writer.write_all(text.as_bytes())
        };
        // Task 6 replaces this with the report-once latch.
        let _ = result;
        let _ = (&self.target, &self.degraded, &self.on_failure, Ordering::Relaxed);
    }
}

// ── Human-readable rendering (ADR-0018 §4a) ──────────────────────────────

/// Continuation-line indent. A record starts at column 0 with `[` or `#`;
/// everything else is indented, which makes truncation visible.
const INDENT: &str = "              ";

fn stamp(elapsed_ms: u64) -> String {
    format!("[{:>8.1}s] ", elapsed_ms as f64 / 1000.0)
}

fn esc(text: &str) -> String {
    escape_control_chars(text).into_owned()
}

fn esc_path(path: &Path) -> String {
    esc(&path.display().to_string())
}

/// `hard-linked: a, b` — representative plus siblings, byte-wise sorted, the
/// same row `report.rs` prints.
fn hard_link_row(note: &HardLinkNote) -> String {
    let mut set: Vec<&PathBuf> = std::iter::once(&note.representative)
        .chain(note.siblings.iter())
        .collect();
    set.sort();
    let joined = set.iter().map(|p| esc_path(p)).collect::<Vec<_>>().join(", ");
    format!("{INDENT}hard-linked: {joined}\n")
}

/// Render one event as a complete record. Pure: time is passed in.
fn render_human(event: &RecordEvent<'_>, elapsed_ms: u64, wall_secs: u64) -> String {
    let at = stamp(elapsed_ms);
    match event {
        RecordEvent::Header(inv) => {
            let roots = inv.roots.iter().map(|r| esc(r)).collect::<Vec<_>>().join(", ");
            let ext = inv.ext.as_deref().map_or_else(|| "<default>".to_owned(), esc);
            format!(
                "# imgdedup {version} run record\n\
                 # started:    {started}\n\
                 # invocation: roots=[{roots}] keep={keep} apply={apply} delete={delete} hidden={hidden}\n\
                 #             recurse={recurse} follow-symlinks={follow} ext={ext} exit-code={exit_code}\n",
                version = env!("CARGO_PKG_VERSION"),
                started = rfc3339_utc(wall_secs),
                keep = inv.keep,
                apply = inv.apply,
                delete = inv.delete,
                hidden = inv.hidden,
                recurse = !inv.no_recurse,
                follow = inv.follow_symlinks,
                exit_code = inv.exit_code,
            )
        }
        RecordEvent::Milestone { phase, current, total, percent, run_total } => {
            let label = phase.label();
            match (total, percent) {
                (Some(total), Some(pct)) if *phase == Phase::Verifying => format!(
                    "{at}{label}: {current} of {total} ({pct}%) ({run_total} candidates)\n"
                ),
                (Some(total), Some(pct)) => {
                    format!("{at}{label}: {current} of {total} ({pct}%)\n")
                }
                _ => format!("{at}{label}...\n"),
            }
        }
        RecordEvent::Group { group, notes } => {
            let reclaimable: u64 = group.removable.iter().map(|r| r.size).sum();
            let mut out = format!("{at}group ({reclaimable} bytes reclaimable):\n");
            out.push_str(&format!("{INDENT}KEEP    {}\n", esc_path(&group.keeper.path)));
            for r in &group.removable {
                out.push_str(&format!("{INDENT}REMOVE  {}\n", esc_path(&r.path)));
            }
            for p in &group.protected {
                out.push_str(&format!("{INDENT}PROTECT {}\n", esc_path(&p.path)));
            }
            for note in *notes {
                out.push_str(&hard_link_row(note));
            }
            out
        }
        RecordEvent::AlreadyDeduplicated(note) => format!(
            "{at}already deduplicated (hard-linked): {}\n{}",
            esc_path(&note.representative),
            hard_link_row(note)
        ),
        RecordEvent::Warning { stage, path, reason } => {
            format!("{at}warning: {stage}: {}: {}\n", esc_path(path), esc(reason))
        }
        RecordEvent::RootError { path, reason } => {
            format!("{at}error: {}: {}\n", esc_path(path), esc(reason))
        }
        RecordEvent::RunError { reason } => format!("{at}error: {}\n", esc(reason)),
        RecordEvent::Action { mode, path, skipped: None } => {
            format!("{at}{}: {}\n", mode.past_tense(), esc_path(path))
        }
        RecordEvent::Action { path, skipped: Some(reason), .. } => {
            format!("{at}warning: {}: {}\n", esc_path(path), esc(reason))
        }
        // The banner is FR-20's legibility mechanism: its absence means the
        // run did not finish. The bytes after it are stdout's, verbatim
        // (AC-26) — deliberately not escaped or re-wrapped.
        RecordEvent::Result { rendered, .. } => format!(
            "\n===== RESULT \u{2014} run completed {} after {:.1}s =====\n\n{rendered}",
            rfc3339_utc(wall_secs),
            elapsed_ms as f64 / 1000.0,
        ),
    }
}

// ── Timestamps ───────────────────────────────────────────────────────────

/// Format seconds since the Unix epoch as RFC 3339 UTC. Done by hand to
/// avoid a date-time dependency for one line of output; the day-to-date
/// conversion is the standard days-from-civil inverse (proleptic Gregorian).
pub(crate) fn rfc3339_utc(secs_since_epoch: u64) -> String {
    let days = secs_since_epoch / 86_400;
    let rem = secs_since_epoch % 86_400;
    let (hour, minute, second) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z % 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

// ── Test support ─────────────────────────────────────────────────────────

/// A recorder that keeps each event's human rendering (stamped at 0), for
/// other modules' tests to assert on what a stage recorded.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct Capture(Mutex<Vec<String>>);

#[cfg(test)]
impl Capture {
    pub(crate) fn lines(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

#[cfg(test)]
impl RunRecorder for Capture {
    fn record(&self, event: RecordEvent<'_>) {
        self.0.lock().unwrap().push(render_human(&event, 0, 0));
    }
}
```

`RecordEvent::Result` names `crate::json::JsonReport`, which is private until Task 7. To keep this task compiling on its own, make that one type visible now: in `src/json.rs` change `struct JsonReport` to `pub(crate) struct JsonReport`. Nothing else in `json.rs` changes here.

The module is not yet called from `main.rs`; put `#![allow(dead_code)]` as the first line of `src/record.rs` (below the module doc) and remove it in Task 10.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --bin imgdedup record:: && cargo clippy --all-targets -- -D warnings`
Expected: 10 passed; clippy clean.

- [ ] **Step 5: Commit**

```bash
git add src/record.rs src/main.rs src/json.rs
git commit -m "feat(record): run-record events, FileRecord and the human-readable format (FR-20)"
```

---

### Task 6: Record-write failure — continue, report once, flag degraded

**Files:**

- Modify: `src/record.rs` (`impl RunRecorder for FileRecord<W>`, tests)

**Interfaces:**

- Consumes: `FileRecord` (Task 5).
- Produces: `FileRecord::is_degraded()` returns `true` after the first failed write; `on_failure` is called exactly once with `error: could not write to <target>: <reason>` (both escaped).

- [ ] **Step 1: Write the failing tests**

Add to `src/record.rs`'s test module:

```rust
// ── Write failure (ADR-0018 §7, AC-28a) ──────────────────────────────

/// Fails every write, counting attempts.
#[derive(Clone, Default)]
struct Broken(Arc<std::sync::atomic::AtomicUsize>);

impl Write for Broken {
    fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(std::io::Error::other("No space left on device"))
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn broken_recorder(
    target: &str,
) -> (FileRecord<Broken>, Broken, Arc<Mutex<Vec<String>>>) {
    let writer = Broken::default();
    let messages: Arc<Mutex<Vec<String>>> = Arc::default();
    let sink = Arc::clone(&messages);
    let recorder = FileRecord::new(
        writer.clone(),
        Format::Human,
        target.to_owned(),
        Box::new(move |msg| sink.lock().unwrap().push(msg.to_owned())),
    );
    (recorder, writer, messages)
}

#[test]
fn a_failed_write_is_reported_once_and_degrades_the_recorder() {
    let (recorder, writer, messages) = broken_recorder("/mnt/nas/run.txt");
    assert!(!recorder.is_degraded());

    for _ in 0..5 {
        recorder.record(RecordEvent::RunError { reason: "x" });
    }

    assert!(recorder.is_degraded());
    assert_eq!(
        *messages.lock().unwrap(),
        vec!["error: could not write to /mnt/nas/run.txt: No space left on device".to_owned()]
    );
    assert_eq!(
        writer.0.load(Ordering::SeqCst),
        1,
        "once degraded, no further write is attempted"
    );
}

#[test]
fn the_failure_message_escapes_the_target_path() {
    let (recorder, _writer, messages) = broken_recorder("run\u{1b}[2J.txt");
    recorder.record(RecordEvent::RunError { reason: "x" });
    let messages = messages.lock().unwrap();
    assert!(!messages[0].contains('\u{1b}'), "{:?}", messages[0]);
}

/// Review Focus 4: rayon workers failing at the same instant.
#[test]
fn concurrent_write_failures_report_exactly_once() {
    let (recorder, _writer, messages) = broken_recorder("run.txt");
    std::thread::scope(|scope| {
        for _ in 0..16 {
            scope.spawn(|| {
                for _ in 0..200 {
                    recorder.record(RecordEvent::RunError { reason: "x" });
                }
            });
        }
    });
    assert_eq!(messages.lock().unwrap().len(), 1);
    assert!(recorder.is_degraded());
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --bin imgdedup record::tests::a_failed_write record::tests::concurrent record::tests::the_failure_message`
Expected: FAIL — `is_degraded()` is `false`, no message recorded.

- [ ] **Step 3: Implement**

Replace `impl<W: Write + Send> RunRecorder for FileRecord<W>` with:

```rust
impl<W: Write + Send> RunRecorder for FileRecord<W> {
    fn record(&self, event: RecordEvent<'_>) {
        // Checked before the mutex: once the file is unwritable every later
        // record costs one relaxed load, even on the rayon hot path.
        if self.degraded.load(Ordering::Relaxed) {
            return;
        }
        let result = {
            let mut writer = self.writer.lock().unwrap_or_else(|e| e.into_inner());
            // Re-checked under the lock: a thread that was waiting here when
            // another one failed must not attempt a second write.
            if self.degraded.load(Ordering::Relaxed) {
                return;
            }
            // Stamped and rendered under the lock so records are written in
            // the order their timestamps claim.
            let elapsed_ms = self.start.elapsed().as_millis() as u64;
            let wall_secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            let text = match self.format {
                Format::Human => render_human(&event, elapsed_ms, wall_secs),
                Format::Ndjson => render_human(&event, elapsed_ms, wall_secs),
            };
            let result = writer.write_all(text.as_bytes());
            if result.is_err() {
                self.degraded.store(true, Ordering::Relaxed);
            }
            result
        };
        // The file is left exactly as it is — partial, with no completion
        // document — and the run carries on (ADR-0018 §7). Only one thread
        // can observe `Err` (the flag is set under the lock above), so this
        // runs exactly once; the message is emitted outside the lock.
        if let Err(e) = result {
            (self.on_failure)(&format!(
                "error: could not write to {}: {}",
                esc(&self.target),
                esc(&e.to_string())
            ));
        }
    }

    fn is_degraded(&self) -> bool {
        self.degraded.load(Ordering::Relaxed)
    }
}
```

This sets the latch under the writer mutex rather than with ADR-0018 §7's `compare_exchange`; the guarantee is the same (exactly one reporter) and it additionally stops queued threads from retrying the write. Note it for the architect review.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --bin imgdedup record::`
Expected: 13 passed.

- [ ] **Step 5: Commit**

```bash
git add src/record.rs
git commit -m "feat(record): continue and report once when the record cannot be written (AC-28a)"
```

---

### Task 7: The NDJSON format

**Files:**

- Modify: `src/json.rs` (visibility, `document()`, `to_pretty()`, payload constructors)
- Modify: `src/record.rs` (`render_ndjson`, both `Format::Ndjson` arms, tests)

**Interfaces:**

- Consumes: `RecordEvent`, `Invocation`, `RECORD_VERSION` (Task 5).
- Produces, in `json.rs`:
  - `pub(crate) fn document(outcome: &DedupOutcome, enumerate_warnings: &[EnumerateWarning], sniff_warnings: &[SniffWarning], root_errors: &[EnumerateWarning], mode: ActionMode, applied: &ApplyOutcome) -> JsonReport`
  - `impl JsonReport { pub(crate) fn to_pretty(&self) -> String }`
  - `pub fn render(…) -> String` — unchanged signature and bytes
  - `pub(crate) fn group_payload(group: &DuplicateGroup, notes: &[HardLinkNote]) -> JsonGroup`
  - `pub(crate) fn hard_link_set_payload(note: &HardLinkNote) -> JsonHardLinkSet`
  - `pub(crate) fn warning_payload(stage: &'static str, path: &Path, reason: &str) -> JsonWarning`
  - `pub(crate) fn root_error_payload(path: &Path, reason: &str) -> JsonRootError`

- [ ] **Step 1: Write the failing tests**

In `src/json.rs`'s test module add (using its existing `outcome_from` fixture helper):

```rust
#[test]
fn document_to_pretty_is_byte_identical_to_render() {
    let dir = TempDir::new().unwrap();
    let outcome = outcome_from(&dir, &[("a.png", b"same"), ("b.png", b"same")]);
    let applied = ApplyOutcome::default();
    let pretty = document(&outcome, &[], &[], &[], ActionMode::DryRun, &applied).to_pretty();
    assert_eq!(pretty, render(&outcome, &[], &[], &[], ActionMode::DryRun, &applied));
}

#[test]
fn group_payload_equals_the_group_inside_the_document() {
    // The anti-drift rule (ADR-0018 §4b): a record `group` line is built by
    // the same code as the document's group, not a parallel copy.
    let dir = TempDir::new().unwrap();
    let outcome = outcome_from(&dir, &[("a.png", b"same"), ("b.png", b"same")]);
    let doc = document(&outcome, &[], &[], &[], ActionMode::DryRun, &ApplyOutcome::default());
    let from_doc = serde_json::to_value(&doc).unwrap()["groups"][0].clone();
    let payload = group_payload(&outcome.groups[0], &outcome.hard_link_notes);
    assert_eq!(serde_json::to_value(&payload).unwrap(), from_doc);
}
```

In `src/record.rs`'s test module add:

```rust
// ── NDJSON format (ADR-0018 §4b) ─────────────────────────────────────

fn parse(line: &str) -> serde_json::Value {
    assert!(line.ends_with('\n') && !line.trim_end().contains('\n'), "one line: {line:?}");
    serde_json::from_str(line).unwrap()
}

#[test]
fn ndjson_lines_start_with_type_then_elapsed_ms() {
    let line = render_ndjson(&RecordEvent::RunError { reason: "x" }, 1500, 0);
    assert!(line.starts_with(r#"{"type":"error","elapsed_ms":1500,"#), "{line}");
}

#[test]
fn ndjson_header_carries_version_tool_start_and_structured_invocation() {
    let v = parse(&render_ndjson(&RecordEvent::Header(&invocation()), 0, 1_786_015_353));
    assert_eq!(v["type"], "header");
    assert_eq!(v["record_version"], 1);
    assert_eq!(v["tool"], "imgdedup");
    assert_eq!(v["tool_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(v["started_at"], "2026-08-06T11:22:33Z");
    assert_eq!(v["invocation"]["roots"][0], "/photos");
    assert_eq!(v["invocation"]["keep"], "shortest");
    assert_eq!(v["invocation"]["ext"], serde_json::Value::Null);
    assert_eq!(v["invocation"]["output"], "run.txt");
}

#[test]
fn ndjson_milestone_uses_null_for_an_unknown_denominator() {
    let v = parse(&render_ndjson(
        &RecordEvent::Milestone {
            phase: Phase::Scanning,
            current: 0,
            total: None,
            percent: None,
            run_total: 0,
        },
        0,
        0,
    ));
    assert_eq!(v["type"], "milestone");
    assert_eq!(v["phase"], "scanning");
    assert!(v["total"].is_null() && v["percent"].is_null());

    let v = parse(&render_ndjson(
        &RecordEvent::Milestone {
            phase: Phase::Hashing,
            current: 1200,
            total: Some(28450),
            percent: Some(4),
            run_total: 28450,
        },
        12_040,
        0,
    ));
    assert_eq!((v["current"].as_u64(), v["total"].as_u64()), (Some(1200), Some(28450)));
    assert_eq!(v["percent"], 4);
    assert_eq!(v["run_total"], 28450);
}

#[test]
fn ndjson_group_has_the_json_group_shape() {
    let group = DuplicateGroup {
        keeper: rec("/photos/a.jpg", 4096),
        removable: vec![rec("/photos/b.jpg", 4096)],
        protected: vec![],
    };
    let v = parse(&render_ndjson(&RecordEvent::Group { group: &group, notes: &[] }, 9, 0));
    assert_eq!(v["type"], "group");
    assert_eq!(v["keeper"]["path"], "/photos/a.jpg");
    assert_eq!(v["keeper"]["size"], 4096);
    assert_eq!(v["removable"][0]["path"], "/photos/b.jpg");
    assert_eq!(v["reclaimable_bytes"], 4096);
    assert!(v["protected"].as_array().unwrap().is_empty());
    assert!(v["hard_link_sets"].as_array().unwrap().is_empty());
}

#[test]
fn ndjson_diagnostics_and_actions() {
    let p = PathBuf::from("/photos/x.jpg");
    let note = HardLinkNote {
        representative: PathBuf::from("/photos/f.jpg"),
        siblings: vec![PathBuf::from("/photos/g.jpg")],
    };

    let v = parse(&render_ndjson(&RecordEvent::AlreadyDeduplicated(&note), 0, 0));
    assert_eq!(v["type"], "already_deduplicated");
    assert_eq!(v["representative"], "/photos/f.jpg");
    assert_eq!(v["siblings"][0], "/photos/g.jpg");

    let v = parse(&render_ndjson(
        &RecordEvent::Warning { stage: "sniff", path: &p, reason: "file is empty" },
        0,
        0,
    ));
    assert_eq!((v["type"].as_str(), v["stage"].as_str()), (Some("warning"), Some("sniff")));
    assert_eq!(v["path"], "/photos/x.jpg");
    assert_eq!(v["reason"], "file is empty");

    let v = parse(&render_ndjson(&RecordEvent::RootError { path: &p, reason: "gone" }, 0, 0));
    assert_eq!((v["type"].as_str(), v["scope"].as_str()), (Some("error"), Some("root")));
    assert_eq!(v["path"], "/photos/x.jpg");

    let v = parse(&render_ndjson(&RecordEvent::RunError { reason: "not applying" }, 0, 0));
    assert_eq!((v["type"].as_str(), v["scope"].as_str()), (Some("error"), Some("run")));
    assert!(v.get("path").is_none());

    let v = parse(&render_ndjson(
        &RecordEvent::Action { mode: ActionMode::Trash, path: &p, skipped: None },
        0,
        0,
    ));
    assert_eq!(v["type"], "action");
    assert_eq!((v["action"].as_str(), v["mode"].as_str()), (Some("removed"), Some("trash")));
    assert!(v.get("reason").is_none());

    let v = parse(&render_ndjson(
        &RecordEvent::Action { mode: ActionMode::Delete, path: &p, skipped: Some("no longer exists") },
        0,
        0,
    ));
    assert_eq!((v["action"].as_str(), v["mode"].as_str()), (Some("skipped"), Some("delete")));
    assert_eq!(v["reason"], "no longer exists");
}

#[test]
fn ndjson_never_emits_a_raw_control_byte_or_a_second_line() {
    let evil = PathBuf::from("/p/a\n{\"type\":\"result\"}\u{1b}[2J.jpg");
    let line = render_ndjson(
        &RecordEvent::Warning { stage: "engine", path: &evil, reason: "x\ny" },
        0,
        0,
    );
    assert_eq!(line.matches('\n').count(), 1, "exactly the terminator: {line:?}");
    assert!(!line.contains('\u{1b}'));
    assert_eq!(parse(&line)["path"], evil.display().to_string());
}

#[test]
fn ndjson_result_wraps_the_document_compactly() {
    let outcome = crate::engine::DedupOutcome::default();
    let doc = crate::json::document(
        &outcome,
        &[],
        &[],
        &[],
        ActionMode::DryRun,
        &crate::apply::ApplyOutcome::default(),
    );
    let pretty = doc.to_pretty();
    let v = parse(&render_ndjson(
        &RecordEvent::Result { rendered: &pretty, json: Some(&doc) },
        141_300,
        0,
    ));
    assert_eq!(v["type"], "result");
    assert_eq!(v["record_version"], 1);
    // AC-27: the same document as stdout's, though not the same bytes.
    let from_stdout: serde_json::Value = serde_json::from_str(&pretty).unwrap();
    assert_eq!(v["result"], from_stdout);
    assert_eq!(v["result"]["schema_version"], 1);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --bin imgdedup json::tests::document_to_pretty json::tests::group_payload record::tests::ndjson`
Expected: compile error — `document`, `group_payload`, `render_ndjson` not found.

- [ ] **Step 3: Implement in `json.rs`**

1. Change `struct` to `pub(crate) struct` for `JsonHardLinkSet`, `JsonGroup`, `JsonWarning`, `JsonRootError` (their fields stay private; `JsonReport` was done in Task 5).
2. Rename the existing `pub fn render` to `pub(crate) fn document`, change its return type to `JsonReport`, and replace its last five lines (from the `// Serialization of these derive-only types cannot fail` comment through `out`) with returning the struct: end the function with the `JsonReport { … }` expression instead of binding it to `report`. Update its doc comment's first line to "Build the pipeline outcome as the stable JSON document (FR-14)."
3. Add directly below it:

```rust
impl JsonReport {
    /// The document as stdout prints it: pretty-printed, newline-terminated.
    pub(crate) fn to_pretty(&self) -> String {
        // Serialization of these derive-only types cannot fail; a pretty
        // document is friendlier to humans piping to files and equally
        // parseable.
        let mut out = serde_json::to_string_pretty(self)
            .expect("JSON serialization of plain structs cannot fail");
        out.push('\n');
        out
    }
}

/// Render the pipeline outcome as the stable JSON document (FR-14).
pub fn render(
    outcome: &DedupOutcome,
    enumerate_warnings: &[EnumerateWarning],
    sniff_warnings: &[SniffWarning],
    root_errors: &[EnumerateWarning],
    mode: ActionMode,
    applied: &ApplyOutcome,
) -> String {
    document(outcome, enumerate_warnings, sniff_warnings, root_errors, mode, applied).to_pretty()
}

// ── Payloads for the run record's NDJSON lines (ADR-0018 §4b) ────────────
//
// The record's `group`, `already_deduplicated`, `warning` and `error` lines
// are built from the very types the document uses, so a field added or
// renamed here reaches both surfaces at once.

/// One group, with the hard-link notes among `notes` that belong to it.
pub(crate) fn group_payload(group: &DuplicateGroup, notes: &[HardLinkNote]) -> JsonGroup {
    json_group(group, notes, &mut HashSet::new())
}

pub(crate) fn hard_link_set_payload(note: &HardLinkNote) -> JsonHardLinkSet {
    JsonHardLinkSet::from_note(note)
}

pub(crate) fn warning_payload(stage: &'static str, path: &Path, reason: &str) -> JsonWarning {
    JsonWarning { stage, path: lossy(path), reason: reason.to_owned() }
}

pub(crate) fn root_error_payload(path: &Path, reason: &str) -> JsonRootError {
    JsonRootError { path: lossy(path), reason: reason.to_owned() }
}
```

- [ ] **Step 4: Implement in `record.rs`**

Add below `render_human`:

```rust
// ── NDJSON rendering (ADR-0018 §4b) ──────────────────────────────────────

/// One line of the NDJSON record: `type` first, `elapsed_ms` second, then the
/// event's own fields.
#[derive(Serialize)]
struct Line<'a, T: Serialize> {
    #[serde(rename = "type")]
    kind: &'a str,
    elapsed_ms: u64,
    #[serde(flatten)]
    payload: T,
}

fn ndjson_line<T: Serialize>(kind: &str, elapsed_ms: u64, payload: T) -> String {
    // Compact, so it is one line; `serde_json` escapes every control
    // character, so a path can neither break the line nor reach a terminal.
    let mut line = serde_json::to_string(&Line { kind, elapsed_ms, payload })
        .expect("JSON serialization of plain structs cannot fail");
    line.push('\n');
    line
}

#[derive(Serialize)]
struct HeaderPayload<'a> {
    record_version: u32,
    tool: &'static str,
    tool_version: &'static str,
    started_at: String,
    invocation: &'a Invocation,
}

#[derive(Serialize)]
struct MilestonePayload {
    phase: &'static str,
    current: u64,
    total: Option<u64>,
    percent: Option<u64>,
    run_total: u64,
}

#[derive(Serialize)]
struct RootErrorPayload {
    scope: &'static str,
    #[serde(flatten)]
    error: crate::json::JsonRootError,
}

#[derive(Serialize)]
struct RunErrorPayload<'a> {
    scope: &'static str,
    reason: &'a str,
}

#[derive(Serialize)]
struct ActionPayload<'a> {
    action: &'static str,
    mode: &'static str,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'a str>,
}

#[derive(Serialize)]
struct ResultPayload<'a> {
    record_version: u32,
    result: &'a crate::json::JsonReport,
}

/// Under `--json` `main.rs` always supplies the document as a value. This
/// fallback only exists so a caller that does not still gets a parseable line.
#[derive(Serialize)]
struct ResultTextPayload<'a> {
    record_version: u32,
    result_text: &'a str,
}

/// Render one event as a single NDJSON line. Pure: time is passed in.
fn render_ndjson(event: &RecordEvent<'_>, elapsed_ms: u64, wall_secs: u64) -> String {
    use crate::json;
    match event {
        RecordEvent::Header(invocation) => ndjson_line(
            "header",
            elapsed_ms,
            HeaderPayload {
                record_version: RECORD_VERSION,
                tool: "imgdedup",
                tool_version: env!("CARGO_PKG_VERSION"),
                started_at: rfc3339_utc(wall_secs),
                invocation,
            },
        ),
        RecordEvent::Milestone { phase, current, total, percent, run_total } => ndjson_line(
            "milestone",
            elapsed_ms,
            MilestonePayload {
                phase: phase.label(),
                current: *current,
                total: *total,
                percent: *percent,
                run_total: *run_total,
            },
        ),
        RecordEvent::Group { group, notes } => {
            ndjson_line("group", elapsed_ms, json::group_payload(group, notes))
        }
        RecordEvent::AlreadyDeduplicated(note) => ndjson_line(
            "already_deduplicated",
            elapsed_ms,
            json::hard_link_set_payload(note),
        ),
        RecordEvent::Warning { stage, path, reason } => {
            ndjson_line("warning", elapsed_ms, json::warning_payload(stage, path, reason))
        }
        RecordEvent::RootError { path, reason } => ndjson_line(
            "error",
            elapsed_ms,
            RootErrorPayload { scope: "root", error: json::root_error_payload(path, reason) },
        ),
        RecordEvent::RunError { reason } => {
            ndjson_line("error", elapsed_ms, RunErrorPayload { scope: "run", reason })
        }
        RecordEvent::Action { mode, path, skipped } => ndjson_line(
            "action",
            elapsed_ms,
            ActionPayload {
                action: if skipped.is_some() { "skipped" } else { "removed" },
                mode: mode.as_str(),
                path: path.to_string_lossy().into_owned(),
                reason: *skipped,
            },
        ),
        RecordEvent::Result { json: Some(document), .. } => ndjson_line(
            "result",
            elapsed_ms,
            ResultPayload { record_version: RECORD_VERSION, result: document },
        ),
        RecordEvent::Result { rendered, json: None } => ndjson_line(
            "result",
            elapsed_ms,
            ResultTextPayload { record_version: RECORD_VERSION, result_text: rendered },
        ),
    }
}
```

In `FileRecord::record`, change the second match arm to `Format::Ndjson => render_ndjson(&event, elapsed_ms, wall_secs),`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: all pass — in particular every pre-existing `json.rs` and golden-set test, which is the evidence `--json`'s stdout bytes did not move.

- [ ] **Step 6: Commit**

```bash
git add src/json.rs src/record.rs
git commit -m "feat(record): NDJSON record format sharing json.rs's schema types (AC-27)"
```

---

### Task 8: `MilestoneSink` — progress milestones into the record

**Files:**

- Modify: `src/record.rs`

**Interfaces:**

- Consumes: `progress::{ProgressSink, Phase, MilestoneGate, percent, DEFAULT_REPAINT_INTERVAL_MS}` (Task 3); `RunRecorder`, `RecordEvent::Milestone`, `Capture` (Task 5).
- Produces: `pub struct MilestoneSink` with `pub fn new(recorder: Arc<dyn RunRecorder>) -> Self`, implementing `ProgressSink`.

- [ ] **Step 1: Write the failing tests**

Add to `src/record.rs`'s test module:

```rust
// ── MilestoneSink ────────────────────────────────────────────────────

fn milestones(drive: impl FnOnce(&MilestoneSink)) -> Vec<String> {
    let capture = Arc::new(Capture::default());
    let sink = MilestoneSink::with_interval_ms(capture.clone(), 0);
    drive(&sink);
    capture.lines()
}

#[test]
fn milestone_sink_records_phase_transitions_and_ten_percent_steps() {
    let lines = milestones(|s| {
        s.set_phase(Phase::Scanning);
        s.set_total(20);
        s.set_phase(Phase::Hashing);
        for _ in 0..20 {
            s.advance(1);
        }
        s.retotal(2);
        s.set_phase(Phase::Verifying);
        s.advance(2);
        s.finish();
    });
    assert_eq!(
        lines,
        vec![
            "[     0.0s] scanning...\n",
            "[     0.0s] hashing: 0 of 20 (0%)\n",
            "[     0.0s] hashing: 2 of 20 (10%)\n",
            "[     0.0s] hashing: 4 of 20 (20%)\n",
            "[     0.0s] hashing: 6 of 20 (30%)\n",
            "[     0.0s] hashing: 8 of 20 (40%)\n",
            "[     0.0s] hashing: 10 of 20 (50%)\n",
            "[     0.0s] hashing: 12 of 20 (60%)\n",
            "[     0.0s] hashing: 14 of 20 (70%)\n",
            "[     0.0s] hashing: 16 of 20 (80%)\n",
            "[     0.0s] hashing: 18 of 20 (90%)\n",
            "[     0.0s] hashing: 20 of 20 (100%)\n",
            "[     0.0s] verifying: 0 of 2 (0%) (20 candidates)\n",
            "[     0.0s] verifying: 2 of 2 (100%) (20 candidates)\n",
            "[     0.0s] done: 20 of 20 (100%)\n",
        ]
    );
}

#[test]
fn milestone_sink_is_bounded_not_one_line_per_file() {
    let lines = milestones(|s| {
        s.set_total(100_000);
        s.set_phase(Phase::Hashing);
        for _ in 0..100_000 {
            s.advance(1);
        }
        s.finish();
    });
    assert!(lines.len() <= 13, "0%..100% in 10% steps plus done; got {}", lines.len());
}

#[test]
fn milestone_sink_finish_is_idempotent() {
    let lines = milestones(|s| {
        s.set_total(1);
        s.finish();
        s.finish();
    });
    assert_eq!(lines, vec!["[     0.0s] done: 1 of 1 (100%)\n"]);
}

#[test]
fn milestone_sink_throttles_advance_between_checks() {
    // With the production interval, a burst of advances inside one interval
    // reaches the recorder at most once: the hot path stays lock-free.
    let capture = Arc::new(Capture::default());
    let sink = MilestoneSink::new(capture.clone());
    sink.set_total(1_000_000);
    sink.set_phase(Phase::Hashing); // forced: 1 line
    for _ in 0..1_000_000 {
        sink.advance(1);
    }
    assert!(capture.lines().len() <= 12, "got {}", capture.lines().len());
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --bin imgdedup record::tests::milestone_sink`
Expected: compile error — `MilestoneSink` not found.

- [ ] **Step 3: Implement**

Add to `src/record.rs` (extend the `use` lines with `crate::progress::{self, MilestoneGate, ProgressSink}`, `std::sync::Arc`, `std::sync::atomic::AtomicU64`):

```rust
// ── Milestones ───────────────────────────────────────────────────────────

struct MilestoneState {
    phase: Phase,
    run_total: u64,
    phase_total: u64,
    total_known: bool,
    gate: MilestoneGate,
    finished: bool,
}

/// A [`ProgressSink`] that turns the pipeline's progress notifications into
/// coarse [`RecordEvent::Milestone`]s, at the cadence of the non-TTY progress
/// fallback (the shared [`MilestoneGate`], AC-22). Teed alongside the
/// terminal renderer in `main.rs`, so the record keeps its milestones under
/// `--no-progress` and `--timings`.
///
/// Mirrors `progress::Progress`'s concurrency design: the numerator and the
/// rate-limit gate are lock-free atomics, so `advance` — called once per file
/// from rayon workers — takes no lock in the common case.
pub struct MilestoneSink {
    recorder: Arc<dyn RunRecorder>,
    state: Mutex<MilestoneState>,
    current: AtomicU64,
    start: Instant,
    next_check_millis: AtomicU64,
    interval_ms: u64,
}

impl MilestoneSink {
    pub fn new(recorder: Arc<dyn RunRecorder>) -> Self {
        Self::with_interval_ms(recorder, progress::DEFAULT_REPAINT_INTERVAL_MS)
    }

    pub(crate) fn with_interval_ms(recorder: Arc<dyn RunRecorder>, interval_ms: u64) -> Self {
        Self {
            recorder,
            state: Mutex::new(MilestoneState {
                phase: Phase::Scanning,
                run_total: 0,
                phase_total: 0,
                total_known: false,
                gate: MilestoneGate::new(),
                finished: false,
            }),
            current: AtomicU64::new(0),
            start: Instant::now(),
            next_check_millis: AtomicU64::new(0),
            interval_ms,
        }
    }

    fn now_millis(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    /// Record a milestone if one is due. Called with the state lock held.
    fn emit(&self, st: &mut MilestoneState, force: bool) {
        let now = self.now_millis();
        if !force && now < self.next_check_millis.load(Ordering::Relaxed) {
            return;
        }
        self.next_check_millis
            .store(now.saturating_add(self.interval_ms), Ordering::Relaxed);

        let raw = self.current.load(Ordering::Relaxed);
        let current = if st.total_known { raw.min(st.phase_total) } else { raw };
        let percent = st.total_known.then(|| progress::percent(current, st.phase_total));
        if st.gate.should_emit(st.phase, percent, force) {
            self.recorder.record(RecordEvent::Milestone {
                phase: st.phase,
                // An indeterminate milestone is written once, at phase entry;
                // a count there would be a stale zero (AC-22).
                current: if st.total_known { current } else { 0 },
                total: st.total_known.then_some(st.phase_total),
                percent,
                run_total: st.run_total,
            });
        }
    }
}

impl ProgressSink for MilestoneSink {
    fn set_total(&self, total: u64) {
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        st.run_total = total;
        st.phase_total = total;
        st.total_known = true;
        self.current.store(0, Ordering::Relaxed);
    }

    fn set_phase(&self, phase: Phase) {
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        st.phase = phase;
        self.emit(&mut st, true);
    }

    fn advance(&self, n: u64) {
        self.current.fetch_add(n, Ordering::Relaxed);
        if self.now_millis() < self.next_check_millis.load(Ordering::Relaxed) {
            return;
        }
        match self.state.try_lock() {
            Ok(mut st) => self.emit(&mut st, false),
            Err(std::sync::TryLockError::Poisoned(p)) => self.emit(&mut p.into_inner(), false),
            Err(std::sync::TryLockError::WouldBlock) => {}
        }
    }

    fn retotal(&self, total: u64) {
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        st.phase_total = total;
        st.total_known = true;
        self.current.store(0, Ordering::Relaxed);
    }

    fn finish(&self) {
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if st.finished {
            return;
        }
        st.finished = true;
        let run_total = st.run_total;
        st.phase = Phase::Done;
        st.phase_total = run_total;
        st.total_known = true;
        self.current.store(run_total, Ordering::Relaxed);
        self.emit(&mut st, true);
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --bin imgdedup record::`
Expected: all pass.

If `milestone_sink_records_phase_transitions_and_ten_percent_steps` shows a different line list, compare against `progress::Progress` driven the same way in non-TTY mode with interval 0 — the two must produce the same sequence of milestones (modulo the elapsed suffix and the stamp). A difference means `emit` has drifted from `Progress::paint`; fix `emit`, not the expectation.

- [ ] **Step 5: Commit**

```bash
git add src/record.rs
git commit -m "feat(record): MilestoneSink feeds progress milestones into the record"
```

---

### Task 9: Stages record warnings, errors and groups as they happen

**Files:**

- Modify: `src/enumerate.rs:261-262`, `:289-293`, `:318`, `:705-709`, `:744-748`, `:795-798`
- Modify: `src/sniff.rs:198-201`, `:248-251`, `:270-293`
- Modify: `src/engine.rs:273-282`, `:293-304`, `:369-375`, `:521-527`, `:573-581`
- Modify: `src/main.rs:121`, `:162`, `:178-182` (pass `&record::NoRecord` for now)
- Test: in-file unit tests of the three modules

**Interfaces:**

- Consumes: `record::{RunRecorder, RecordEvent, NoRecord, Capture}`.
- Produces (new trailing parameter on each):
  - `enumerate::enumerate_with_progress(paths: &[PathBuf], opts: &ScopeOptions, sink: &dyn ProgressSink, recorder: &dyn RunRecorder) -> CandidateSet`
  - `sniff::filter_candidates_with_progress(candidates: Vec<Candidate>, sink: &dyn ProgressSink, recorder: &dyn RunRecorder) -> (Vec<ConfirmedFile>, Vec<SniffWarning>)`
  - `engine::find_duplicates_with_progress<K>(files: Vec<ConfirmedFile>, keeper_order: K, sink: &dyn ProgressSink, recorder: &dyn RunRecorder) -> DedupOutcome`

- [ ] **Step 1: Thread the parameter through, with no behavior change**

Add `recorder: &dyn RunRecorder` as the last parameter of the three functions above and of the private `enumerate_root` and `dedup_core_with_sink`. Import `crate::record::{RecordEvent, RunRecorder}` in each module (plus `#[cfg(test)] use crate::record::NoRecord;` where the `#[cfg(test)]` wrappers live). Update the callers:

- `enumerate.rs:262` (`enumerate`): `enumerate_with_progress(paths, opts, &NoProgress, &NoRecord)`
- `sniff.rs:201` (`filter_candidates`): `filter_candidates_with_progress(candidates, &NoProgress, &NoRecord)`
- `engine.rs:281`: `dedup_core_with_sink(files, keeper_order, hash_file, true, sink, recorder)`
- `engine.rs:303` (`dedup_core`): `dedup_core_with_sink(files, keeper_order, hasher, size_prefilter, &NoProgress, &NoRecord)`
- `main.rs`: add `&record::NoRecord` as the last argument at the three call sites.
- Every test that calls one of the three `*_with_progress` functions directly (about 25 call sites; the compiler lists them): add `&NoRecord` as the last argument, importing `crate::record::NoRecord` in that test module.

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: all pass, nothing changed.

Commit:

```bash
git add src/enumerate.rs src/sniff.rs src/engine.rs src/main.rs
git commit -m "refactor(pipeline): thread a RunRecorder through enumerate, sniff and the engine"
```

- [ ] **Step 2: Write the failing tests**

`src/enumerate.rs` test module:

```rust
#[test]
fn a_root_error_is_recorded_when_it_happens() {
    let capture = crate::record::Capture::default();
    let missing = PathBuf::from("/definitely/not/here-imgdedup");
    let result = enumerate_with_progress(
        std::slice::from_ref(&missing),
        &ScopeOptions::default_scope(),
        &NoProgress,
        &capture,
    );
    assert_eq!(result.root_errors.len(), 1);
    let lines = capture.lines();
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].starts_with("[     0.0s] error: /definitely/not/here-imgdedup: "),
        "{lines:?}"
    );
}
```

`src/sniff.rs` test module (build `Candidate`s the way the neighbouring test `filter_candidates_with_progress_advances_once_per_candidate_regardless_of_outcome` does — one real image file, one text file with an image extension):

```rust
#[test]
fn a_rejected_candidate_is_recorded_as_a_sniff_warning() {
    let dir = tempfile::tempdir().unwrap();
    let fake = dir.path().join("notes.jpg");
    std::fs::write(&fake, b"just text").unwrap();
    let candidates = vec![Candidate { path: fake.clone(), size: 9, mtime: None }];

    let capture = crate::record::Capture::default();
    let (confirmed, warnings) =
        filter_candidates_with_progress(candidates, &NoProgress, &capture);

    assert!(confirmed.is_empty());
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        capture.lines(),
        vec![format!(
            "[     0.0s] warning: sniff: {}: content not recognized as a supported image format\n",
            fake.display()
        )]
    );
}
```

`src/engine.rs` test module (using its existing `cf(path: &Path) -> ConfirmedFile` and `lex_order` helpers):

```rust
#[test]
fn each_verified_group_is_recorded_in_canonical_member_order() {
    let dir = tempfile::tempdir().unwrap();
    let write = |name: &str, bytes: &[u8]| {
        let p = dir.path().join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    };
    // Two groups; the first has three members so member order is observable.
    let a = write("a.png", b"same-content-1");
    let b = write("b.png", b"same-content-1");
    let c = write("c.png", b"same-content-1");
    let x = write("x.png", b"same-content-2");
    let y = write("y.png", b"same-content-2");
    // Fed in reverse so emit-time sorting, not input order, is what's tested.
    let files = [&y, &x, &c, &b, &a].into_iter().map(|p| cf(p)).collect();

    let capture = crate::record::Capture::default();
    let outcome = find_duplicates_with_progress(files, lex_order, &NoProgress, &capture);

    assert_eq!(outcome.groups.len(), 2);
    let recorded = capture.lines();
    let groups: Vec<&String> = recorded.iter().filter(|l| l.contains("] group (")).collect();
    assert_eq!(groups.len(), 2, "one record per verified group: {recorded:?}");

    // Each recorded block lists exactly the final group's members, in the
    // final (canonical) order, with no index.
    for group in &outcome.groups {
        let mut expected = format!("              KEEP    {}\n", group.keeper.path.display());
        for r in &group.removable {
            expected.push_str(&format!("              REMOVE  {}\n", r.path.display()));
        }
        assert!(
            groups.iter().any(|block| block.ends_with(&expected)),
            "no recorded block matches {expected:?} in {recorded:?}"
        );
    }
    assert!(!recorded.iter().any(|l| l.contains("Group ")));
}

#[test]
fn a_hash_failure_is_recorded_as_an_engine_warning() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.png");
    let b = dir.path().join("b.png");
    std::fs::write(&a, b"12345").unwrap();
    std::fs::write(&b, b"12345").unwrap();
    let files = vec![cf(&a), cf(&b)];

    let capture = crate::record::Capture::default();
    let failing = |p: &Path| -> io::Result<[u8; 32]> {
        if p == a { Err(io::Error::other("boom")) } else { hash_file(p) }
    };
    let outcome =
        dedup_core_with_sink(files, lex_order, failing, true, &NoProgress, &capture);

    assert_eq!(outcome.warnings.len(), 1);
    assert_eq!(
        capture.lines(),
        vec![format!(
            "[     0.0s] warning: engine: {}: could not hash file: boom\n",
            a.display()
        )]
    );
}

#[test]
fn recording_never_changes_the_outcome() {
    // The AC-24 principle for the record: output-only.
    let dir = tempfile::tempdir().unwrap();
    let mut files: Vec<ConfirmedFile> = Vec::new();
    for name in ["a.png", "b.png", "c.png"] {
        let p = dir.path().join(name);
        std::fs::write(&p, b"identical").unwrap();
        files.push(cf(&p));
    }
    let capture = crate::record::Capture::default();
    let recorded = find_duplicates_with_progress(files.clone(), lex_order, &NoProgress, &capture);
    let plain = find_duplicates_with_progress(files, lex_order, &NoProgress, &NoRecord);
    assert_eq!(recorded, plain);
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test --bin imgdedup a_root_error_is_recorded a_rejected_candidate_is_recorded each_verified_group_is_recorded a_hash_failure_is_recorded`
Expected: FAIL — `capture.lines()` is empty in each. (`recording_never_changes_the_outcome` passes already and must keep passing.)

- [ ] **Step 4: Implement**

`enumerate.rs`, at the root-error push (`:744-748`) — record immediately after the existing `out.root_errors.push(…)`:

```rust
if let Some(e) = out.root_errors.last() {
    // Written when it happens: unlike stderr, the record has no live
    // progress line to corrupt (ADR-0018 §2).
    recorder.record(RecordEvent::RootError { path: &e.path, reason: &e.reason });
}
```

and at the metadata-failure warning (`:795-798`), immediately after `out.warnings.push(…)`:

```rust
if let Some(w) = out.warnings.last() {
    recorder.record(RecordEvent::Warning {
        stage: "enumerate",
        path: &w.path,
        reason: &w.reason,
    });
}
```

If `enumerate.rs` has any other `root_errors.push` / `warnings.push` site (`grep -n '\.push(EnumerateWarning' src/enumerate.rs`), give it the same treatment.

`sniff.rs`, inside the rayon closure, just before `sink.advance(1);`:

```rust
if let Err(w) = &result {
    recorder.record(RecordEvent::Warning {
        stage: "sniff",
        path: &w.path,
        reason: &w.reason,
    });
}
```

`engine.rs`, the hash-failure arm (`:521-527`), after `outcome.warnings.push(…)`:

```rust
if let Some(w) = outcome.warnings.last() {
    recorder.record(RecordEvent::Warning {
        stage: "engine",
        path: &w.path,
        reason: &w.reason,
    });
}
```

`engine.rs`, the verify loop (`:573-581`) — replace the loop body:

```rust
for group in pending_hash_groups {
    // Captured before the call because `process_hash_group`
    // consumes `group`; every member's disposition is decided by
    // the time it returns, regardless of which internal branch
    // (all-anchor group, verify pass/fail, ...) was taken.
    let group_len = group.len() as u64;
    let before = (
        outcome.groups.len(),
        outcome.hard_link_notes.len(),
        outcome.warnings.len(),
    );
    process_hash_group(group, &keeper_order, &mut outcome);
    if recorder.is_active() {
        record_decided(recorder, &outcome, before, &keeper_order);
    }
    sink.advance(group_len);
}
```

and add below `sort_canonical`:

```rust
/// Hand the run record whatever one `process_hash_group` call just decided:
/// its warnings, and either its duplicate group (with that group's hard-link
/// notes) or its already-deduplicated sets.
///
/// A group's **membership** is final here (ADR-0018 §1): the candidate set
/// closed before grouping began, a group cannot span size cohorts or hash
/// groups, and nothing later revisits it. Its position among the groups and
/// every run total are *not* final, so neither is recorded. Member order is
/// only fixed by `sort_canonical` later, so a copy is sorted here with the
/// same comparator — `sort_by` is stable and the order total, so this equals
/// the final order.
fn record_decided<K>(
    recorder: &dyn RunRecorder,
    outcome: &DedupOutcome,
    (groups_before, notes_before, warnings_before): (usize, usize, usize),
    keeper_order: &K,
) where
    K: Fn(&FileRec, &FileRec) -> Ordering,
{
    for w in &outcome.warnings[warnings_before..] {
        recorder.record(RecordEvent::Warning {
            stage: "engine",
            path: &w.path,
            reason: &w.reason,
        });
    }
    let mut notes = outcome.hard_link_notes[notes_before..].to_vec();
    notes.sort_by(|a, b| a.representative.cmp(&b.representative));
    match outcome.groups[groups_before..].first() {
        Some(group) => {
            let mut group = group.clone();
            group.removable.sort_by(keeper_order);
            group.protected.sort_by(keeper_order);
            recorder.record(RecordEvent::Group { group: &group, notes: &notes });
        }
        // No group emitted: any notes are all-anchor, already-deduplicated sets.
        None => {
            for note in &notes {
                recorder.record(RecordEvent::AlreadyDeduplicated(note));
            }
        }
    }
}
```

Update `dedup_core_with_sink`'s doc comment with one paragraph: "`recorder` is told about each warning and each verified group at the moment it is decided (FR-20, ADR-0018 §2). It is output-only: nothing it does can change the returned `DedupOutcome`."

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add src/enumerate.rs src/sniff.rs src/engine.rs
git commit -m "feat(record): record warnings, root errors and groups as they are decided"
```

**End of Batch B.** Reconcile `/usage` against the estimate, record it in `docs/planning/status.md`, re-size Batch C.

---

### Task 10: `--output` end to end — CLI flag, `main.rs` wiring, acceptance tests

**Files:**

- Modify: `src/cli.rs` (flag + tests)
- Modify: `src/main.rs` (`run`)
- Modify: `src/record.rs`, `src/progress.rs` (remove the `dead_code` allowances)
- Create: `tests/output_record.rs`

**Interfaces:**

- Consumes: everything from Tasks 1–9.
- Produces: the user-visible feature.

- [ ] **Step 1: Write the failing CLI unit tests**

Add to `src/cli.rs`'s test module:

```rust
// ── --output (FR-20) ──────────────────────────────────────────────────

#[test]
fn output_defaults_to_none() {
    assert_eq!(try_parse(&["imgdedup", "."]).unwrap().output, None);
}

#[test]
fn output_takes_a_path() {
    let cli = try_parse(&["imgdedup", ".", "--output", "run.txt"]).unwrap();
    assert_eq!(cli.output, Some(PathBuf::from("run.txt")));
}

#[test]
fn output_requires_a_value() {
    let err = try_parse(&["imgdedup", ".", "--output"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidValue);
}
```

Run: `cargo test --bin imgdedup cli::tests::output`
Expected: compile error — no field `output`.

- [ ] **Step 2: Add the flag**

In `src/cli.rs`, in the Output section after `no_progress`:

```rust
/// Write a durable record of the run to FILE, continuously (FR-20).
///
/// The file is written as the run progresses — a header, progress
/// milestones with elapsed time, each duplicate group as it is verified,
/// every warning and error, and each `--apply` action — and ends with the
/// complete report once the run finishes. If the run is interrupted, the
/// file keeps everything recorded up to that point and has no final
/// report. With `--json` the file is NDJSON (one JSON object per line).
/// An existing FILE is overwritten. This is in addition to the normal
/// output, which is unchanged.
#[arg(long, value_name = "FILE")]
pub output: Option<PathBuf>,
```

Run: `cargo test --bin imgdedup cli::`
Expected: PASS. If `output_requires_a_value` reports a different `ErrorKind`, assert the kind clap actually returns — the behavior under test is "this is a parse error".

- [ ] **Step 3: Write the failing integration tests**

Create `tests/output_record.rs`:

```rust
//! End-to-end tests for the durable run record (`--output`, FR-20,
//! ADR-0018): AC-25..AC-28a and the plan's Review Focus items.
mod support;

use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::Path;
use support::images::{ImageFormat, image_bytes};
use support::tree::{FileSpec, materialize};

const MTIME: i64 = 1_750_000_000;

fn cmd() -> Command {
    Command::cargo_bin("imgdedup").unwrap()
}

/// Three duplicate pairs: `dir{0,1,2}/a.<ext>` + `dir{0,1,2}/copy.<ext>`.
fn fixture(root: &Path) {
    let mut files = Vec::new();
    for (i, (format, seed)) in [
        (ImageFormat::Png, 1u8),
        (ImageFormat::Jpeg, 2u8),
        (ImageFormat::Bmp, 3u8),
    ]
    .into_iter()
    .enumerate()
    {
        let ext = format.extension();
        let bytes = image_bytes(format, seed, 64);
        for name in ["a", "copy"] {
            files.push(FileSpec {
                rel_path: format!("dir{i}/{name}.{ext}"),
                bytes: bytes.clone(),
                mtime: MTIME,
            });
        }
    }
    materialize(root, &files, &[]);
}

fn ndjson(path: &Path) -> Vec<Value> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("bad line {l:?}: {e}")))
        .collect()
}

// ── AC-26 / AC-28: human record ──────────────────────────────────────────

#[test]
fn human_record_ends_with_the_exact_stdout_report() {
    let tree = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    fixture(tree.path());
    let record = out_dir.path().join("run.txt");

    let out = cmd().arg(tree.path()).arg("--output").arg(&record).output().unwrap();
    assert!(out.status.success());

    let stdout = String::from_utf8(out.stdout).unwrap();
    let text = fs::read_to_string(&record).unwrap();
    assert!(text.starts_with("# imgdedup "), "{text}");
    assert!(text.contains("\n===== RESULT \u{2014} run completed "), "{text}");
    assert!(text.ends_with(&stdout), "AC-26: the record's tail is stdout, byte for byte");

    let transcript = text.split("\n===== RESULT").next().unwrap();
    assert_eq!(transcript.matches("] group (").count(), 3, "{transcript}");
    assert!(transcript.contains("] scanning..."));
    assert!(transcript.contains("] done: 6 of 6 (100%)"));
    assert!(!transcript.contains("Group 1/"), "no index in the transcript");
}

#[test]
fn stdout_is_byte_identical_with_and_without_output() {
    let tree = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    fixture(tree.path());

    for extra in [&[][..], &["--json"][..]] {
        let plain = cmd().arg(tree.path()).args(extra).output().unwrap();
        let recorded = cmd()
            .arg(tree.path())
            .args(extra)
            .arg("--output")
            .arg(out_dir.path().join("r"))
            .output()
            .unwrap();
        assert_eq!(plain.stdout, recorded.stdout, "AC-28 ({extra:?})");
        assert_eq!(plain.status.code(), recorded.status.code());
    }
}

#[test]
fn no_progress_does_not_silence_the_record() {
    let tree = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    fixture(tree.path());
    let record = out_dir.path().join("run.txt");

    let out = cmd()
        .arg(tree.path())
        .args(["--no-progress", "--output"])
        .arg(&record)
        .output()
        .unwrap();
    assert!(out.stderr.is_empty(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = fs::read_to_string(&record).unwrap();
    assert!(text.contains("] hashing: 0 of 6 (0%)"), "{text}");
    assert!(text.contains("] done: 6 of 6 (100%)"), "{text}");
}

// ── AC-27: NDJSON ────────────────────────────────────────────────────────

#[test]
fn ndjson_record_is_valid_per_line_and_ends_with_the_stdout_document() {
    let tree = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    fixture(tree.path());
    let record = out_dir.path().join("run.ndjson");

    let out = cmd()
        .arg(tree.path())
        .args(["--json", "--output"])
        .arg(&record)
        .output()
        .unwrap();
    assert!(out.status.success());

    let lines = ndjson(&record);
    let first = lines.first().unwrap();
    assert_eq!(first["type"], "header");
    assert_eq!(first["record_version"], 1);
    assert_eq!(first["invocation"]["json"], true);
    assert!(lines.iter().all(|l| l["type"].is_string() && l["elapsed_ms"].is_u64()));

    let last = lines.last().unwrap();
    assert_eq!(last["type"], "result");
    let stdout_doc: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(last["result"], stdout_doc, "AC-27: same document, not the same bytes");
    assert_eq!(lines.iter().filter(|l| l["type"] == "result").count(), 1);
}

#[test]
fn every_ndjson_group_line_equals_a_group_in_the_result() {
    // ADR-0018 §4b's anti-drift rule, end to end.
    let tree = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    fixture(tree.path());
    let record = out_dir.path().join("run.ndjson");
    cmd().arg(tree.path()).args(["--json", "--output"]).arg(&record).assert().success();

    let lines = ndjson(&record);
    let final_groups = lines.last().unwrap()["result"]["groups"].as_array().unwrap().clone();
    let streamed: Vec<Value> = lines
        .iter()
        .filter(|l| l["type"] == "group")
        .map(|l| {
            let mut g = l.clone();
            let obj = g.as_object_mut().unwrap();
            obj.remove("type");
            obj.remove("elapsed_ms");
            g
        })
        .collect();

    assert_eq!(streamed.len(), final_groups.len());
    for g in &streamed {
        assert!(final_groups.contains(g), "streamed group not in result: {g}");
    }
}

// ── AC-28: fail fast (Review Focus 2) ────────────────────────────────────

#[test]
fn unwritable_output_path_fails_before_scanning() {
    let tree = tempfile::tempdir().unwrap();
    fixture(tree.path());
    let a_directory = tree.path().join("dir0");
    let missing_parent = tree.path().join("no-such-dir").join("run.txt");

    for target in [a_directory, missing_parent] {
        let out = cmd().arg(tree.path()).arg("--output").arg(&target).output().unwrap();
        assert_eq!(out.status.code(), Some(2), "usage-level error for {target:?}");
        assert!(out.stdout.is_empty(), "no report is written");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.starts_with("error: cannot write to "), "{stderr}");
        assert!(!stderr.contains("scanning"), "must fail before scanning starts: {stderr}");
    }
}

// ── ADR-0018 §5: --output inside a scan root ─────────────────────────────

#[test]
fn an_output_file_inside_the_scan_root_is_never_grouped_or_removed() {
    let tree = tempfile::tempdir().unwrap();
    fixture(tree.path());
    // An image extension forces the record itself to become a candidate.
    let record = tree.path().join("rec.jpg");

    let out = cmd()
        .arg(tree.path())
        .args(["--apply", "--delete", "--json", "--output"])
        .arg(&record)
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(record.exists(), "the record must never be removed");

    let doc: Value = serde_json::from_slice(&out.stdout).unwrap();
    let rendered = doc.to_string();
    let grouped = doc["groups"].to_string() + &doc["actions"].to_string();
    assert!(!grouped.contains("rec.jpg"), "never grouped, never acted on: {rendered}");
    assert!(
        doc["warnings"].as_array().unwrap().iter().any(|w| {
            w["stage"] == "sniff" && w["path"].as_str().unwrap().ends_with("rec.jpg")
        }),
        "it is rejected by the magic-byte gate, with a warning: {rendered}"
    );
}

// ── apply-time actions ───────────────────────────────────────────────────

#[test]
fn apply_actions_are_recorded_as_they_happen() {
    let tree = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    fixture(tree.path());
    let record = out_dir.path().join("run.ndjson");

    cmd()
        .arg(tree.path())
        .args(["--apply", "--delete", "--json", "--output"])
        .arg(&record)
        .assert()
        .success();

    let lines = ndjson(&record);
    let actions: Vec<&Value> = lines.iter().filter(|l| l["type"] == "action").collect();
    assert_eq!(actions.len(), 3);
    assert!(actions.iter().all(|a| a["action"] == "removed" && a["mode"] == "delete"));
    let removed = &lines.last().unwrap()["result"]["actions"]["removed"];
    for a in &actions {
        assert!(removed.as_array().unwrap().contains(&a["path"]));
    }
    let position = |ty: &str| lines.iter().position(|l| l["type"] == ty).unwrap();
    assert!(position("action") < position("result"));
}

// ── Review Focus 3: truthful records at the edges ────────────────────────

#[test]
fn empty_run_still_writes_header_and_result() {
    let tree = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    let record = out_dir.path().join("run.txt");

    let out = cmd().arg(tree.path()).arg("--output").arg(&record).output().unwrap();
    assert!(out.status.success());
    let text = fs::read_to_string(&record).unwrap();
    assert!(text.starts_with("# imgdedup "));
    assert!(text.contains("] done: 0 of 0 (100%)"), "{text}");
    assert!(text.ends_with(&String::from_utf8(out.stdout).unwrap()));
    assert!(!text.contains("] group ("));
}

#[test]
fn refused_apply_leaves_errors_and_no_result() {
    let tree = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    fixture(tree.path());
    let record = out_dir.path().join("run.txt");

    let out = cmd()
        .arg(tree.path())
        .arg(tree.path().join("no-such-root"))
        .args(["--apply", "--output"])
        .arg(&record)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());

    let text = fs::read_to_string(&record).unwrap();
    assert!(text.contains("] error: ") && text.contains("no-such-root"), "{text}");
    assert!(text.contains("] error: not applying: 1 scan root(s) could not be read"), "{text}");
    assert!(
        !text.contains("===== RESULT"),
        "a run that printed no report has no completion document: {text}"
    );
}

// ── Review Focus 1: hostile filenames ────────────────────────────────────

#[cfg(unix)]
#[test]
fn record_cannot_be_forged_by_a_newline_in_a_filename() {
    let tree = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    let bytes = image_bytes(ImageFormat::Png, 7, 64);
    let evil = "x\n[     0.0s] trashed: FORGED.png";
    materialize(
        tree.path(),
        &[
            FileSpec { rel_path: "a.png".into(), bytes: bytes.clone(), mtime: MTIME },
            FileSpec { rel_path: evil.into(), bytes, mtime: MTIME },
        ],
        &[],
    );
    let record = out_dir.path().join("run.txt");
    cmd().arg(tree.path()).arg("--output").arg(&record).assert().success();

    let text = fs::read_to_string(&record).unwrap();
    let transcript = text.split("\n===== RESULT").next().unwrap();
    assert!(transcript.contains("] group ("), "the pair is still reported: {transcript}");
    assert!(
        !transcript.lines().any(|l| l.contains("] trashed: FORGED.png")),
        "a filename must not be able to start a record line: {transcript}"
    );
}

// ── AC-28a: mid-run write failure ────────────────────────────────────────

/// `/dev/full` opens for writing and fails every write with ENOSPC — a real
/// "disk full" without a full disk. Linux only; the cross-platform half of
/// AC-28a is pinned by `record.rs`'s injected-writer unit tests.
#[cfg(target_os = "linux")]
#[test]
fn a_record_that_cannot_be_written_does_not_stop_the_run() {
    let tree = tempfile::tempdir().unwrap();
    fixture(tree.path());

    let plain = cmd().arg(tree.path()).arg("--no-progress").output().unwrap();
    let out = cmd()
        .arg(tree.path())
        .args(["--no-progress", "--output", "/dev/full"])
        .output()
        .unwrap();

    assert_eq!(out.status.code(), Some(1), "exit 1 although the scan succeeded");
    assert_eq!(out.stdout, plain.stdout, "the report is exactly what it would have been");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        stderr.matches("error: could not write to /dev/full: ").count(),
        1,
        "reported exactly once: {stderr}"
    );
    assert_eq!(stderr.lines().count(), 1, "and nothing else: {stderr}");
}

#[cfg(target_os = "linux")]
#[test]
fn a_failed_record_never_interrupts_an_apply() {
    let tree = tempfile::tempdir().unwrap();
    fixture(tree.path());
    let out = cmd()
        .arg(tree.path())
        .args(["--apply", "--delete", "--no-progress", "--json", "--output", "/dev/full"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let doc: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(doc["actions"]["removed"].as_array().unwrap().len(), 3);
    for i in 0..3 {
        let left = fs::read_dir(tree.path().join(format!("dir{i}"))).unwrap().count();
        assert_eq!(left, 1, "dir{i}: the duplicate was removed, the keeper kept");
    }
}
```

`materialize` may reject a `rel_path` containing a newline or may not create parent directories for it; if `record_cannot_be_forged_by_a_newline_in_a_filename` fails in fixture setup rather than in an assertion, write those two files with `std::fs::write` directly instead.

- [ ] **Step 4: Run the tests to verify they fail**

Run: `cargo test --test output_record`
Expected: every test fails — `--output` is parsed but nothing writes the file (`No such file or directory` on read, or exit code 0 where 2 / 1 is expected).

- [ ] **Step 5: Wire `main.rs`**

Add `use std::sync::Arc;` at the top. Add this helper above `run`:

```rust
/// The invocation as the run record's header reports it (FR-20).
fn invocation(args: &cli::Cli, output: &std::path::Path) -> record::Invocation {
    record::Invocation {
        roots: args
            .paths
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect(),
        keep: match args.keep {
            cli::KeepRule::Shortest => "shortest",
            cli::KeepRule::Oldest => "oldest",
            cli::KeepRule::Newest => "newest",
        },
        apply: args.apply,
        delete: args.delete,
        json: args.json,
        no_recurse: args.no_recurse,
        hidden: args.hidden,
        follow_symlinks: args.follow_symlinks,
        ext: args.ext.clone(),
        exit_code: args.exit_code,
        no_progress: args.no_progress,
        timings: args.timings,
        output: output.to_string_lossy().into_owned(),
    }
}
```

In `run`, move the `let routing = streams::route(…);` statement (Task 4) up to directly after the `args.validate()` block, and insert after it:

```rust
// The run record (FR-20, ADR-0018). Opened before anything is scanned, so
// an unwritable path is a usage-level failure now rather than a surprise
// hours in (AC-28). `File::create` truncates an existing file — `--output`
// behaves like the `>` it stands in for (ADR-0018 §6). The file is written
// unbuffered: this function's caller ends in `process::exit`, which runs
// no destructors, so nothing may sit in a user-space buffer.
let recorder: Arc<dyn record::RunRecorder> = match &args.output {
    None => Arc::new(record::NoRecord),
    Some(path) => {
        let shown = path.display().to_string();
        let file = match std::fs::File::create(path) {
            Ok(file) => file,
            Err(e) => {
                eprintln!(
                    "error: cannot write to {}: {}",
                    safe_text::escape_control_chars(&shown),
                    safe_text::escape_control_chars(&e.to_string())
                );
                return exit_code::USAGE;
            }
        };
        // A write failure mid-run is the one stderr writer that can fire
        // while a live progress line is being redrawn; a leading newline
        // puts the message on a clean line (ADR-0018 §7).
        let live_line = routing.progress.is_some_and(|t| t.tty);
        let format = if args.json { record::Format::Ndjson } else { record::Format::Human };
        let file_record = record::FileRecord::new(
            file,
            format,
            shown,
            Box::new(move |message| {
                if live_line {
                    eprintln!();
                }
                eprintln!("{message}");
            }),
        );
        let recorder: Arc<dyn record::RunRecorder> = Arc::new(file_record);
        recorder.record(record::RecordEvent::Header(&invocation(&args, path)));
        recorder
    }
};
```

Replace the `SinkGuard::new(match routing.progress { … })` statement with:

```rust
let terminal_sink: Box<dyn progress::ProgressSink> = match routing.progress {
    None => Box::new(progress::NoProgress),
    Some(target) => Box::new(progress::Progress::new(
        streams::ProgressStream::open(target.stream),
        target.tty,
    )),
};
// Milestones reach the record through the same observer the pipeline
// already calls — independently of `--no-progress` and `--timings`, which
// only ever silence the terminal half (ADR-0018 §2).
let record_sink: Box<dyn progress::ProgressSink> = if recorder.is_active() {
    Box::new(record::MilestoneSink::new(Arc::clone(&recorder)))
} else {
    Box::new(progress::NoProgress)
};
let sink = progress::SinkGuard::new(Box::new(progress::Tee::new(terminal_sink, record_sink)));
```

Replace the three `&record::NoRecord` arguments (Task 9) with `&*recorder`.

In the `--apply` refusal block, record the failure before returning:

```rust
let reason = format!(
    "not applying: {} scan root(s) could not be read; \
     re-run once the paths are correct",
    candidate_set.root_errors.len()
);
eprintln!("error: {reason}");
recorder.record(record::RecordEvent::RunError { reason: &reason });
return exit_code::ERROR;
```

In the apply event closure, record each event (first statement of each arm):

```rust
apply::Event::Removed(path) => {
    recorder.record(record::RecordEvent::Action { mode, path, skipped: None });
    // … existing routing.status block …
}
apply::Event::Skipped(path, reason) => {
    let reason = reason.to_string();
    recorder.record(record::RecordEvent::Action {
        mode,
        path,
        skipped: Some(&reason),
    });
    eprintln!(
        "warning: {}: {}",
        safe_text::escape_control_chars(&path.display().to_string()),
        safe_text::escape_control_chars(&reason)
    );
}
```

Replace the body of the `rendered` block's `if args.json { … } else { … }` so the JSON document is kept as a value, and write the completion document after printing. The block becomes:

```rust
let (rendered, document) = {
    let render_span = tracing::info_span!(
        "render",
        format = if args.json { "json" } else { "human" },
        bytes = tracing::field::Empty,
    );
    let _render_guard = render_span.enter();
    let (rendered, document) = if args.json {
        let document = json::document(
            &dedup,
            &candidate_set.warnings,
            &sniff_warnings,
            &candidate_set.root_errors,
            mode,
            &applied,
        );
        (document.to_pretty(), Some(document))
    } else {
        (
            report::render(
                &dedup,
                &candidate_set.warnings,
                &sniff_warnings,
                mode,
                &applied,
            ),
            None,
        )
    };
    render_span.record("bytes", rendered.len());
    (rendered, document)
};
print!("{rendered}");

// The completion document: the same `rendered` string stdout just
// received (byte-identical in the human record, AC-26; the same document
// compacted onto one line under `--json`, AC-27). Its presence is what
// tells a reader the run finished.
recorder.record(record::RecordEvent::Result {
    rendered: &rendered,
    json: document.as_ref(),
});
```

Finally the exit code:

```rust
// A record that could not be written is a run-level failure even though
// the scan (and any apply) succeeded: the user asked for a durable record
// and did not get one (ADR-0018 §7, AC-28a).
exit_code::compute(
    !candidate_set.root_errors.is_empty() || recorder.is_degraded(),
    args.exit_code,
    !dedup.groups.is_empty(),
)
```

Update `exit_code::compute`'s doc for `run_level_error`: append "…or the `--output` record could not be written (ADR-0018 §7)."

Remove `#![allow(dead_code)]` from `src/record.rs` and the `#[allow(dead_code)]` attributes on `Tee` in `src/progress.rs`. If `json::render` is now unused outside tests, mark it `#[cfg(test)]` (it remains the entry point the existing `json.rs` and golden tests use).

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
Expected: all pass (the two `/dev/full` tests are compiled out on macOS; they run in CI's Linux leg).

- [ ] **Step 7: Verify AC-25 against a real kill**

```bash
cargo build --release
./target/release/imgdedup test-images --output /tmp/killed.txt & sleep 0.3; kill -9 $!
tail -5 /tmp/killed.txt; grep -c '===== RESULT' /tmp/killed.txt
```

Expected: the file exists, starts with `# imgdedup`, has milestone lines, and the `grep -c` prints `0`. If the run finishes before the kill lands, repeat with a shorter sleep; record the observed tail in the commit body. Then repeat with `--json --output /tmp/killed.ndjson` and check `while read -r l; do echo "$l" | jq -e . >/dev/null || echo BAD; done < <(sed '$d' /tmp/killed.ndjson)` prints nothing (every complete line parses; the possibly-truncated last line is dropped by `sed '$d'`).

- [ ] **Step 8: Commit**

```bash
git add src/cli.rs src/main.rs src/record.rs src/progress.rs src/json.rs src/exit_code.rs tests/output_record.rs
git commit -m "feat(output): --output writes a durable, continuously-updated run record (FR-20)"
```

---

### Task 11: Verification, docs, review, PR

**Files:**

- Modify: `README.md`, `docs/guide/output-and-exit-codes.md`, `docs/guide/getting-started.md` (product-owner)
- Modify: `claude-plugin/` skill text only if it tells the agent to read `trashed:`/`deleted:` lines from stderr (`grep -rn 'trashed\|deleted:\|stderr' claude-plugin`)
- Modify: `docs/adr/0018-durable-run-record-and-output-stream-routing.md` (status), `docs/planning/status.md`, `docs/planning/backlog.md`, `docs/planning/tasks/` (close-out)

- [ ] **Step 1: Tester pass (`tester` agent)**

Brief: verify AC-20..AC-32 + AC-28a against the branch, one row per criterion (test name or manual evidence). Known gaps to judge rather than rediscover: AC-29's stdout-TTY case and AC-21's in-place rendering on stdout have no automated pty test (covered by `streams::route`'s table, the flush pin, and Task 4 Step 7's manual check); AC-25 is covered by `records_are_visible_without_drop_or_flush` plus Task 10 Step 7's manual kill. Add tests for any criterion found uncovered. Run `make scale` as a regression check, then time the release binary on `test-images/` with and without `--output` and report whether the record adds measurable wall time.

- [ ] **Step 2: User docs (`product-owner` agent)**

Start from the reverted draft — still on GitHub though no longer in the local clone:

```bash
gh api repos/intruder1912/imgdedup/commits/13252110f81ec764acce1862083827d3d88a6cb5 \
  -H "Accept: application/vnd.github.patch" > "$TMPDIR/guide-draft.patch"
```

and apply its six review findings (recorded in `docs/planning/status.md`, session 33 window 2). The docs must state, in the user's terms:

- `--output <file>`: what it contains, that it is written as the run goes, that an existing file is overwritten, and that the file lists every scanned path (choose the location accordingly on a shared machine).
- How to tell a finished record from an interrupted one (the `===== RESULT` banner; under `--json`, a final `"type":"result"` line).
- The durability claim exactly as ADR-0018 §3 words it: survives the process dying (including `kill -9`), does **not** survive a host crash or power loss; groups appear during the verify phase, so a run killed while hashing leaves none.
- NDJSON: the event table, `record_version`, **explicitly unstable** — ignore unknown `type`s and fields — while the nested `result` keeps `schema_version: 1`'s guarantees. `jq -c 'select(.type=="result").result'` as the recovery one-liner.
- The stream model: where progress and `trashed:`/`deleted:` lines appear in each case; stderr carries failures only; `--json` without `--output` leaves no incremental trace, by design.
- Exit code `1` when the record could not be written; exit code `2` when `--output` cannot be opened.
- Elapsed time on progress lines.

- [ ] **Step 3: Planning and ADR close-out**

- ADR-0018: `Status: proposed` → `accepted`; add a short "Implementation notes (2026-11)" section listing the plan's five decisions (top of this document) as implemented, or as amended by review.
- `docs/planning/backlog.md`: TASK-0341 → `done (this PR)`; TASK-0340 → stays open for FR-22 only, description updated to say so.
- `docs/planning/status.md`: session entry with per-batch estimate vs. actual usage.

- [ ] **Step 4: Local validation**

Run: `make validate && cargo deny check`
Expected: `✓ local validation passed`; deny clean. Fix every finding before continuing.

- [ ] **Step 5: Architect review on the branch diff (`architect` agent)**

Brief: heavyweight code review of `git diff main...HEAD` against ADR-0018 and this plan. Ask specifically for a ruling on the five decisions at the top of this document, on the latch-under-mutex variant in Task 6, and a security read of: the new file-creating code path, escaping in both record formats, and the mid-run stderr writer. Iterate until converged; do not open the PR before that.

- [ ] **Step 6: Product-owner acceptance**

Brief: accept FR-20 / FR-21 / the FR-19 amendment against the built binary and the docs from Step 2.

- [ ] **Step 7: Push and open the PR (needs the owner's go-ahead)**

```bash
git push -u origin feat/output-record-and-stream-routing
gh pr create --title "feat(output): durable run record and stdout stream routing (TASK-0340/0341)" --body-file "$TMPDIR/pr-body.md"
```

The body records the review, the tester's AC table, and the manual-check evidence. Then: wait for `checks`, `ci-gate`, `security`; architect confirmation pass; the human owner merges.

---

## Self-review

**Spec coverage.**

| Requirement | Task |
|---|---|
| FR-21 routing table, all cases (ADR §8) | 1, 4 |
| `Stdout` flush is load-bearing (ADR §8 trap 1) | 4 Step 1 |
| `finish()` before the report (ADR §8 trap 2) | unchanged; comment updated in 4 |
| Apply removal lines: stdout-TTY or nowhere; skips stay on stderr (ADR §9 #6, #7) | 4 |
| Elapsed time everywhere, total on completion, `LineParams` (AC-32) | 2 |
| `MilestoneGate` shared cadence (ADR §2) | 3, 8 |
| `RunRecorder` / `NoRecord` / `FileRecord`, tee (ADR §2) | 5, 8, 10 |
| Group finality: no index, no totals, sorted at emit (ADR §1) | 5, 9 |
| Unbuffered, one write per record, no fsync (ADR §3) | 5 |
| Human format, banner, byte-identical tail (ADR §4a, AC-26) | 5, 10 |
| NDJSON vocabulary, wrapped result, `record_version`, shared serde types (ADR §4b–c, AC-27) | 7, 10 |
| `--output` inside a scan root (ADR §5) | 10 |
| Truncate; directory / unwritable → exit 2 before scanning (ADR §6, AC-28) | 10 |
| Mid-run failure: continue, once, exit 1, leading newline (ADR §7, AC-28a) | 6, 10 |
| stdout identical with and without `--output` (AC-28) | 10 |
| Survives a kill (AC-25) | 5, 10 Step 7 |
| User docs incl. the unstable-vocabulary and durability wording | 11 |

**Known gaps, stated rather than hidden.** No automated test drives a real pty, so "progress renders in place on stdout" rests on the routing table, the flush pin, and a manual check. The two `/dev/full` tests run only on Linux (CI), not on the macOS dev machine.

**Type consistency.** `RecordEvent` variants and field names are identical in Tasks 5, 7, 8, 9 and 10; `RunRecorder::{record, is_active, is_degraded}` in 5, 6, 9, 10; `MilestoneSink::{new, with_interval_ms}` in 8 and 10; `json::{document, group_payload, hard_link_set_payload, warning_payload, root_error_payload}` and `JsonReport::to_pretty` in 7 and 10; `streams::{route, Routing, Target, Stream, ProgressStream::open}` in 1, 4 and 10.
