use clap::Parser;
use std::process;

mod cli;
mod enumerate;
mod exit_code;

fn main() {
    let args = cli::Cli::parse();
    let code = run(args);
    process::exit(code);
}

/// Top-level dispatch.
///
/// Runs post-parse validation, then the enumeration stage (TASK-0302).
/// Later Phase 3 tasks (TASK-0303..TASK-0312) will extend the pipeline from
/// the candidate set returned here.
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

    // Enumerate candidate files — first pipeline stage (ADR-0008).
    // The result feeds TASK-0303 (magic-byte confirm) onward.
    // Fields are annotated #[allow(dead_code)] in enumerate.rs until each
    // downstream stage is wired (TASK-0303/0304/0307/0311).
    let _candidate_set = enumerate::enumerate(&args.paths, &scope);

    exit_code::SUCCESS
}
