use clap::Parser;
use std::io::IsTerminal;
use std::process;

mod cli;
mod engine;
mod enumerate;
mod exit_code;
mod json;
mod keeper;
mod progress;
mod report;
mod sniff;

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
    // Post-parse semantic validation: --delete without --apply is a run-level
    // error (exit 1, ADR-0008), not a clap parse error (exit 2).
    if let Err(msg) = args.validate() {
        eprintln!("{msg}");
        return exit_code::ERROR;
    }

    // Progress sink (FR-19, TASK-0315). Default-on; --no-progress disables it
    // completely (AC-23), in which case `NoProgress` writes nothing on any
    // stream. Otherwise progress always targets stderr, and TTY-ness is
    // probed exactly once here — never inside `progress::Progress` itself —
    // so the renderer stays unit-testable with an injected boolean (AC-21 /
    // AC-22). `Box<dyn ProgressSink>` lets both arms share one call site
    // below regardless of which concrete sink was built.
    let stderr_is_tty = std::io::stderr().is_terminal();
    let sink: Box<dyn progress::ProgressSink> = if args.no_progress {
        Box::new(progress::NoProgress)
    } else {
        Box::new(progress::Progress::new(std::io::stderr(), stderr_is_tty))
    };
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

    // Stage 1 — Enumerate candidate files (ADR-0008, TASK-0302).
    let candidate_set = enumerate::enumerate(&args.paths, &scope);

    // Run-level errors (an unscannable root) must be *visible*, not just
    // reflected in the exit code (resilience review 2026-07, finding F1).
    // Printed to stderr so they survive stdout redirection and stay out of
    // the machine-readable stdout surface (--json, TASK-0308). The run still
    // continues over the remaining roots and exits 1 at the end (partial
    // results plus an explicit failure signal).
    for err in &candidate_set.root_errors {
        eprintln!("error: {}: {}", err.path.display(), err.reason);
    }

    // Stage 2 — Magic-byte confirmation (ADR-0008, FR-3, NFR-2, TASK-0303).
    // Hand the enumerated candidates (path + size) to the sniff predicate.
    // Candidates that fail confirmation are dropped and recorded as
    // SniffWarnings; they never enter the grouping pipeline. The size that
    // enumeration stat'd is threaded through into each ConfirmedFile.
    let (confirmed, sniff_warnings) = sniff::filter_candidates(candidate_set.candidates);

    // The denominator (y) is fixed the moment the confirmed set is known
    // (FR-19, AC-21): the count of candidate files still to be checked.
    sink.set_total(confirmed.len() as u64);

    // Stage 3 — Core dedup engine (ADR-0008, FR-4/5/17, NFR-1, TASK-0304/0305/0306).
    // Size pre-group → BLAKE3 pre-filter → hash group → identity collapse →
    // byte-for-byte verify against the keeper. Pure analysis: no filesystem
    // mutation. Keeper order comes from the real keeper rule (TASK-0305).
    // Hashing is parallelized via rayon (TASK-0306). The outcome is canonically
    // sorted before returning (ADR-0010). The engine drives the Hashing →
    // Verifying phase transitions itself and notifies `sink` as each
    // candidate file reaches its final disposition (FR-19).
    let dedup = engine::find_duplicates_with_progress(
        confirmed,
        keeper::build_comparator(args.keep),
        sink.as_ref(),
    );

    // Progress must be finished — and therefore fully written to stderr —
    // strictly before the stdout report/JSON is printed below, so progress
    // output can never interleave with stdout in any mode (AC-20).
    sink.finish();

    // Stage 4 — Output (TASK-0307 human / TASK-0308 JSON). Two formatters over
    // the same canonical model; no filesystem mutation in either — --apply /
    // --delete are TASK-0309/TASK-0310 and handled after this report.
    // JSON carries the diagnostics too (resilience F3): warnings and
    // root_errors are part of the document, while root errors additionally
    // went to stderr above (F1) so stdout stays one valid JSON document.
    let rendered = if args.json {
        json::render(
            &dedup,
            &candidate_set.warnings,
            &sniff_warnings,
            &candidate_set.root_errors,
        )
    } else {
        report::render(&dedup, &candidate_set.warnings, &sniff_warnings)
    };
    print!("{rendered}");

    // TODO(TASK-0309/0310): act on `dedup.groups` under --apply / --delete.

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
