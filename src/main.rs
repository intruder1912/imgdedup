use clap::Parser;
use std::process;

mod cli;
mod engine;
mod enumerate;
mod exit_code;
mod keeper;
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
    let (confirmed, _sniff_warnings) = sniff::filter_candidates(candidate_set.candidates);

    // Stage 3 — Core dedup engine (ADR-0008, FR-4/5/17, NFR-1, TASK-0304/0305).
    // Size pre-group → BLAKE3 pre-filter → hash group → identity collapse →
    // byte-for-byte verify against the keeper. Pure analysis: no filesystem
    // mutation. Keeper order comes from the real keeper rule (TASK-0305).
    let _dedup = engine::find_duplicates(confirmed, keeper::build_comparator(args.keep));

    // TODO(TASK-0307): surface `_dedup.groups` + `_dedup.warnings`,
    //                  `_sniff_warnings`, `candidate_set.warnings`, and
    //                  `candidate_set.root_errors` in the report.
    // TODO(TASK-0309/0310): act on `_dedup.groups` under --apply / --delete.

    exit_code::SUCCESS
}
