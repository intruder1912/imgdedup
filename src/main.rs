use clap::Parser;
use std::io::IsTerminal;
use std::process;

mod apply;
mod cli;
mod engine;
mod enumerate;
mod exit_code;
mod json;
mod keeper;
mod progress;
mod report;
mod safe_text;
mod sniff;
#[cfg(test)]
mod trace_test_support;

fn main() {
    let args = cli::Cli::parse();
    let code = run(args);
    process::exit(code);
}

/// Top-level dispatch.
///
/// Runs post-parse validation, then the enumeration stage (TASK-0302), then
/// the magic-byte confirmation stage (TASK-0303). Later Phase 3 tasks
/// (TASK-0304..TASK-0312) will extend the pipeline from the confirmed set.
fn run(args: cli::Cli) -> i32 {
    // Post-parse semantic validation: --delete without --apply is a usage
    // error (exit 2, TASK-0323) — not caught by clap's own parsing (it's a
    // valid flag combination syntactically), but the same code as if it had
    // been, since both mean "the invocation itself was wrong."
    if let Err(msg) = args.validate() {
        eprintln!("error: {msg}");
        return exit_code::USAGE;
    }

    // Performance instrumentation (ADR-0017, TASK-0338). `--timings`
    // installs a minimal `tracing-subscriber` `fmt` consumer over this run's
    // own tracing spans, writing one line per pipeline stage to stderr at
    // stage close. Off by default: with no subscriber installed, every span
    // macro below is a near-zero-cost no-op (measured ~0.15 ns/span,
    // ADR-0017). Installation is **non-fatal** on failure — instrumentation
    // must never be the reason the tool fails (the AC-24 principle again):
    // a failed `try_init()` just means no timing lines get printed.
    if args.timings {
        let _ = tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
            .with_max_level(tracing::Level::INFO)
            .with_target(false)
            .with_ansi(false)
            .try_init();
    }

    // The root `run` span (ADR-0017). `roots`/`apply`/`json` are known right
    // here, before any pipeline stage runs. **Hard constraint (easy to get
    // wrong):** `main()` ends in `process::exit(code)`, which runs no
    // destructors — this guard must therefore be created *and dropped
    // inside `run()`*, exactly like `SinkGuard` below, or the run-total
    // close line silently never fires. Held for the rest of this function,
    // so every span opened by a later pipeline stage nests inside it.
    let run_span = tracing::info_span!(
        "run",
        roots = args.paths.len(),
        apply = args.apply,
        json = args.json,
    );
    let _run_guard = run_span.enter();

    // Progress sink (FR-19, TASK-0315). Default-on; --no-progress disables it
    // completely (AC-23), in which case `NoProgress` writes nothing on any
    // stream. `--timings` also disables it (ADR-0017 §4): a stage-close
    // timing line written while the TTY progress line is live would corrupt
    // it mid-redraw (`progress.rs`'s module doc), so the two are mutually
    // exclusive and `--timings` always wins — no error, no second flag
    // needed. Otherwise progress always targets stderr, and TTY-ness is
    // probed exactly once here — never inside `progress::Progress` itself —
    // so the renderer stays unit-testable with an injected boolean (AC-21 /
    // AC-22). `Box<dyn ProgressSink>` lets both arms share one call site
    // below regardless of which concrete sink was built.
    //
    // Wrapped in `SinkGuard` (architect finding N4): its `Drop` calls
    // `finish()`, so progress is *always* terminated — a live TTY line
    // always ends cleanly — no matter how this function returns, including
    // an early `return` that later work (TASK-0309/TASK-0310's apply/delete
    // diagnostics land in exactly this window) might add below. `finish()`
    // is idempotent, so the explicit call further down (needed so progress
    // is fully flushed *before* the stdout report — AC-20, which `Drop`
    // alone cannot guarantee the ordering of) does not double-render the
    // completion line when `Drop` runs afterwards.
    let progress_enabled = !args.no_progress && !args.timings;
    let stderr_is_tty = std::io::stderr().is_terminal();
    let sink: progress::SinkGuard = progress::SinkGuard::new(if !progress_enabled {
        Box::new(progress::NoProgress)
    } else {
        Box::new(progress::Progress::new(std::io::stderr(), stderr_is_tty))
    });
    sink.set_phase(progress::Phase::Scanning);

    // Build scope options from parsed CLI flags (FR-1, FR-2, FR-6).
    // Start from the documented defaults and apply user overrides.
    let scope = {
        let mut s = enumerate::ScopeOptions::default_scope();
        s.no_recurse = args.no_recurse;
        s.include_hidden = args.hidden;
        s.follow_symlinks = args.follow_symlinks;
        s.extensions = args
            .ext
            .as_deref()
            .map(enumerate::ScopeOptions::parse_ext_list);
        s
    };

    // Stage 1 — Enumerate candidate files (ADR-0008, TASK-0302). Notifies
    // `sink` as each candidate is found (TASK-0317) so the Scanning phase
    // set above shows a live, growing count rather than a static
    // "scanning..." for the whole enumeration sub-phase.
    let candidate_set = enumerate::enumerate_with_progress(&args.paths, &scope, &*sink);

    // Stage 2 — Magic-byte confirmation (ADR-0008, FR-3, NFR-2, TASK-0303).
    // Hand the enumerated candidates (path + size) to the sniff predicate.
    // Candidates that fail confirmation are dropped and recorded as
    // SniffWarnings; they never enter the grouping pipeline. The size that
    // enumeration stat'd is threaded through into each ConfirmedFile.
    //
    // Scanning's second, determinate state (AC-21a, TASK-0328): the walk
    // above just finished, so `N` — the number of candidates it found — is
    // known exactly, and the indicator can leave the indeterminate
    // "scanning... N files found" state for a determinate
    // "scanning: x of N (p%)" one while each found file is examined here
    // (confirmed or rejected alike). `retotal` resets the numerator and
    // fixes the new denominator but never itself repaints (by design, see
    // `progress.rs`); `set_phase` with the *same* Phase::Scanning value —
    // the label does not change, per the product-owner ruling in PRD-0001's
    // 2026-08-05 amendment — is what forces the immediate `0 of N (0%)`
    // render AC-21a requires. `N == 0` skips this state entirely (no
    // `retotal`, no repaint), matching the zero-workload convention already
    // used for Hashing/Verifying in `engine.rs`.
    //
    // The state is bracketed by *two* forced repaints, not one (architect
    // review, TASK-0328). Repaints driven by `advance` are throttle-gated,
    // and the last candidate's advance almost never lands on a due tick, so
    // without the closing `set_phase` below AC-21a's "the state ends at
    // `scanning: N of N (100%)`" simply did not happen: measured on a
    // 20 000-file tree, the log's last scanning line was
    // `scanning: 18216 of 20000 (91%)` and then jumped straight to `done:`,
    // and on a TTY the frame left on screen when hashing took over was
    // likewise short of 100%. `filter_candidates_with_progress` has returned
    // by then, so every worker's `advance` is already visible and the forced
    // repaint renders exactly `N of N (100%)`. Cost: one extra repaint per
    // run — one extra line in a redirected log, and nothing on a TTY, where
    // it is overwritten in place by the next phase.
    let scanned_count = candidate_set.candidates.len() as u64;
    if scanned_count > 0 {
        sink.retotal(scanned_count);
        sink.set_phase(progress::Phase::Scanning);
    }
    let (confirmed, sniff_warnings) =
        sniff::filter_candidates_with_progress(candidate_set.candidates, &*sink);
    if scanned_count > 0 {
        sink.set_phase(progress::Phase::Scanning);
    }

    // Stage 3 — Core dedup engine (ADR-0008, FR-4/5/17, NFR-1, TASK-0304/0305/0306).
    // Size pre-group → BLAKE3 pre-filter → hash group → identity collapse →
    // byte-for-byte verify against the keeper. Pure analysis: no filesystem
    // mutation. Keeper order comes from the real keeper rule (TASK-0305).
    // Hashing is parallelized via rayon (TASK-0306). The outcome is canonically
    // sorted before returning (ADR-0010). The engine is the single source of
    // truth for the progress denominator — it calls `sink.set_total` itself
    // from `files.len()` (architect finding 11, so it can never desync from
    // what it actually iterates) — and drives the Hashing → Verifying phase
    // transitions, notifying `sink` as each candidate file reaches its final
    // disposition (FR-19). `main` only ever drives the Scanning phase, above.
    let dedup = engine::find_duplicates_with_progress(
        confirmed,
        keeper::build_comparator(args.keep),
        &*sink,
    );

    // Progress must be finished — and therefore fully written to stderr —
    // strictly before the stdout report/JSON is printed below, so progress
    // output can never interleave with stdout in any mode (AC-20).
    sink.finish();

    // Run-level errors (an unscannable root) must be *visible*, not just
    // reflected in the exit code (resilience review 2026-07, finding F1).
    // Printed to stderr so they survive stdout redirection and stay out of
    // the machine-readable stdout surface (--json, TASK-0308). The run still
    // continues over the remaining roots and exits 1 at the end (partial
    // results plus an explicit failure signal).
    //
    // Deliberately placed *after* `sink.finish()` above (architect finding
    // 1): on a TTY the progress line is redrawn in place and left
    // un-terminated between repaints, so writing anything else to stderr
    // while it is live corrupts it mid-line (e.g.
    // `scanning...error: /nonexistent: No such file or directory`).
    // `finish()` always leaves the cursor on a fresh line, so nothing here
    // (or any future stderr writer added during the run window — see the
    // rule in `progress.rs`'s module doc) may run before it returns.
    for err in &candidate_set.root_errors {
        eprintln!(
            "error: {}: {}",
            safe_text::escape_control_chars(&err.path.display().to_string()),
            safe_text::escape_control_chars(&err.reason)
        );
    }

    // Stage 4 — Destructive actions (TASK-0309 trash / TASK-0310 delete).
    //
    // Runs *before* the report so the output can describe what actually
    // happened rather than what was intended (FR-14). Progress and per-file
    // skips have already gone to stderr, so a run that dies mid-apply still
    // told the user something; stdout stays reserved for the one report/JSON
    // document (the S2 purity contract).
    //
    // Every candidate is re-validated immediately before its own removal
    // (FR-16) — the scan alone never authorizes anything — and a removal
    // failure is a per-file skip that never escalates to a different mechanism
    // (NFR-1, ADR-0009).
    let (mode, applied) = if args.apply {
        if !candidate_set.root_errors.is_empty() {
            // A root that could not be scanned means the scan the user asked
            // for did not happen. That is a poor basis for an irreversible
            // action — most often it is a typo in one of several paths — so the
            // run reports and exits rather than acting on a partial view.
            eprintln!(
                "error: not applying: {} scan root(s) could not be read; \
                 re-run once the paths are correct",
                candidate_set.root_errors.len()
            );
            return exit_code::ERROR;
        }

        // Mechanism and reported mode are chosen together, so the report can
        // never claim an action the tool did not perform.
        let (mode, remover): (apply::ActionMode, Box<dyn apply::Remover>) = if args.delete {
            (apply::ActionMode::Delete, Box::new(apply::DeleteRemover))
        } else {
            (
                apply::ActionMode::Trash,
                Box::new(apply::TrashRemover::new()),
            )
        };

        // Stream both outcomes as they happen. Removals matter most: if the
        // process is killed partway through a permanent delete, the report
        // below never prints, and without this the user would have no record of
        // what was destroyed.
        let verb = mode.past_tense();
        let outcome = apply::apply(
            &dedup.groups,
            mode,
            remover.as_ref(),
            &mut |event| match event {
                apply::Event::Removed(path) => {
                    eprintln!(
                        "{verb}: {}",
                        safe_text::escape_control_chars(&path.display().to_string())
                    )
                }
                apply::Event::Skipped(path, reason) => {
                    eprintln!(
                        "warning: {}: {}",
                        safe_text::escape_control_chars(&path.display().to_string()),
                        safe_text::escape_control_chars(&reason.to_string())
                    )
                }
            },
        );
        (mode, outcome)
    } else {
        (apply::ActionMode::DryRun, apply::ApplyOutcome::default())
    };

    // Stage 5 — Output (TASK-0307 human / TASK-0308 JSON). Two formatters over
    // the same canonical model, now including the performed actions.
    // JSON carries the diagnostics too (resilience F3): warnings and
    // root_errors are part of the document, while root errors additionally
    // went to stderr above (F1) so stdout stays one valid JSON document.
    //
    // ADR-0017 (TASK-0338): `render` is the other span `main.rs` owns
    // outright (alongside `run`). `format` is a fixed, small string — never
    // user-controlled data — and `bytes` is the size of the rendered
    // document, known only once rendering completes.
    //
    // The span value itself, not just its guard, is block-scoped (architect
    // review, TASK-0338): `FmtSpan::CLOSE` fires when the last `Span` handle
    // drops, not when the guard exits, so a function-scoped handle would
    // print `render`'s line at the very end of `run()` — after the stdout
    // document had already been printed — with a `time.idle` covering that
    // print. See the same note in `engine.rs`.
    let rendered = {
        let render_span = tracing::info_span!(
            "render",
            format = if args.json { "json" } else { "human" },
            bytes = tracing::field::Empty,
        );
        let _render_guard = render_span.enter();
        let rendered = if args.json {
            json::render(
                &dedup,
                &candidate_set.warnings,
                &sniff_warnings,
                &candidate_set.root_errors,
                mode,
                &applied,
            )
        } else {
            report::render(
                &dedup,
                &candidate_set.warnings,
                &sniff_warnings,
                mode,
                &applied,
            )
        };
        render_span.record("bytes", rendered.len());
        rendered
    };
    print!("{rendered}");

    // Stage 5 — Exit code (ADR-0008 §exit-code, TASK-0311).
    // A run-level error (non-empty root_errors) → 1. The --exit-code flag
    // returning 3 when duplicates are found → 3. All other successful runs → 0.
    // Per-file warnings alone never change the code from 0 (NFR-6).
    exit_code::compute(
        !candidate_set.root_errors.is_empty(),
        args.exit_code,
        !dedup.groups.is_empty(),
    )
}

// ── ADR-0017 §6 (security): span fields must never carry a path via `%` ────
//
// None of this crate's 11 real spans (see the table in ADR-0017) has a path
// field at all — every field is a count, a size, or an enum, by design. The
// two tests below are a *regression demonstration*, not a test of anything
// in this crate's own spans: they pin the underlying `tracing` behavior the
// rule depends on, the same way ADR-0017's own worked example does, so that
// a test shaped like this is what would catch it if a path field were ever
// added via `%` (Display) instead of `?` (Debug) or
// `safe_text::escape_control_chars`.
#[cfg(test)]
mod security_tests {
    use crate::trace_test_support::capture;

    /// `%` (Display) formatting writes a path's raw bytes straight into the
    /// formatted log line — the same terminal-injection channel TASK-0318
    /// and `safe_text.rs` exist to close elsewhere (a filename containing
    /// `ESC[2J` reaches the terminal raw, clearing the screen).
    #[test]
    fn percent_display_formatting_of_a_path_leaks_raw_control_bytes() {
        let evil = "evil\u{1b}[2Jname.jpg";

        let out = capture(|| {
            let span = tracing::info_span!("security_demo_percent", path = %evil);
            drop(span);
        });

        assert!(
            out.contains('\u{1b}'),
            "the raw ESC byte must reach the formatted line when a span \
             field is recorded via `%` (Display) — this is exactly the \
             injection ADR-0017 §6 forbids for any of this crate's real \
             spans; got: {out:?}"
        );
    }

    /// The safe alternative: `?` (Debug) escapes control characters via
    /// `escape_debug`, so the same hostile path never reaches the terminal
    /// raw. ADR-0017's rule: if a path ever must be recorded, use `?` —
    /// never `%`.
    #[test]
    fn debug_formatting_of_a_path_escapes_control_bytes() {
        let evil = "evil\u{1b}[2Jname.jpg";

        let out = capture(|| {
            let span = tracing::info_span!("security_demo_debug", path = ?evil);
            drop(span);
        });

        assert!(
            !out.contains('\u{1b}'),
            "the raw ESC byte must never reach the formatted line when a \
             span field is recorded via `?` (Debug); got: {out:?}"
        );
        assert!(
            out.contains("\\u{1b}"),
            "the ESC must render as a visible, escaped form instead; got: {out:?}"
        );
    }
}
