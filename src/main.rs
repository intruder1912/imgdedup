use clap::Parser;
use std::process;

mod cli;
mod engine;
mod enumerate;
mod exit_code;
mod keeper;
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

    // Stage 2 — Magic-byte confirmation (ADR-0008, FR-3, NFR-2, TASK-0303).
    // Hand the enumerated candidates (path + size) to the sniff predicate.
    // Candidates that fail confirmation are dropped and recorded as
    // SniffWarnings; they never enter the grouping pipeline. The size that
    // enumeration stat'd is threaded through into each ConfirmedFile.
    let (confirmed, sniff_warnings) = sniff::filter_candidates(candidate_set.candidates);

    // Stage 3 — Core dedup engine (ADR-0008, FR-4/5/17, NFR-1, TASK-0304/0305/0306).
    // Size pre-group → BLAKE3 pre-filter → hash group → identity collapse →
    // byte-for-byte verify against the keeper. Pure analysis: no filesystem
    // mutation. Keeper order comes from the real keeper rule (TASK-0305).
    // Hashing is parallelized via rayon (TASK-0306). The outcome is canonically
    // sorted before returning (ADR-0010).
    let dedup = engine::find_duplicates(confirmed, keeper::build_comparator(args.keep));

    // Stage 4 — Human-readable report (FR-9/FR-13/FR-17/NFR-6, TASK-0307).
    // Render a dry-run summary to stdout. No filesystem mutation here either —
    // --apply / --delete are TASK-0309/TASK-0310 and handled after this report.
    if !args.json {
        let rendered = report::render(&dedup, &candidate_set.warnings, &sniff_warnings);
        print!("{rendered}");
    }

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
