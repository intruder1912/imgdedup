use clap::Parser;
use std::process;

mod cli;
mod exit_code;

fn main() {
    let args = cli::Cli::parse();
    let code = run(args);
    process::exit(code);
}

/// Top-level dispatch.  Stub handlers return `exit_code::SUCCESS`; later Phase
/// 3 tasks (TASK-0302..TASK-0312) fill in the real logic.
fn run(args: cli::Cli) -> i32 {
    // Post-parse semantic validation: --delete without --apply is a run-level
    // error (exit 1, ADR-0008), not a clap parse error (exit 2).
    if let Err(msg) = args.validate() {
        eprintln!("{msg}");
        return exit_code::ERROR;
    }

    let _ = args; // placeholder until TASK-0302 wires the pipeline
    exit_code::SUCCESS
}
