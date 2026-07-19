/// Integration tests for the imgdedup CLI.
///
/// These tests run the real binary via `assert_cmd` and verify process exit
/// codes and stdout/stderr content.  They map directly to the acceptance
/// criteria in TASK-0301, ADR-0008, and PRD-0001 §8/§9.
use assert_cmd::Command;
use predicates::prelude::*;

fn cmd() -> Command {
    Command::cargo_bin("imgdedup").unwrap()
}

// ── Exit-code invariants (ADR-0008, AC-12) ────────────────────────────────

/// A successful invocation (valid path, no dedup logic yet) exits 0.
/// Maps to: AC-12 ("a successful run exits 0"), exit_code::SUCCESS.
#[test]
fn successful_invocation_exits_0() {
    cmd().arg(".").assert().code(0);
}

/// Invoking with no path argument errors with a non-zero exit.
/// clap exits 2 on parse errors.
/// Maps to: "invoking with none errors clearly", exit_code::CLAP (2).
#[test]
fn no_path_exits_nonzero() {
    cmd().assert().failure().code(2);
}

/// `--delete` without `--apply` exits 1 (run-level error, ADR-0008).
/// This is NOT a clap parse error (exit 2); it is a semantic/run-level error
/// caught by post-parse validation. Maps to: AC-12 ("--delete without --apply
/// is rejected"), exit_code::ERROR (1).
#[test]
fn delete_without_apply_exits_1() {
    cmd().arg(".").arg("--delete").assert().failure().code(1);
}

/// `--delete` without `--apply` prints a clear error message mentioning --apply.
/// Maps to: AC-12 ("with a clear error").
#[test]
fn delete_without_apply_prints_clear_error() {
    cmd()
        .arg(".")
        .arg("--delete")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--apply"));
}

/// An unknown flag exits 2 (clap parse error).
/// Maps to: "a clap parse error (unknown flag) exits 2".
#[test]
fn unknown_flag_exits_2() {
    cmd()
        .arg(".")
        .arg("--not-a-real-flag")
        .assert()
        .failure()
        .code(2);
}

/// A bad `--keep` value exits 2 (clap parse error).
/// Maps to: "`--keep` rejects values other than shortest|oldest|newest".
#[test]
fn bad_keep_value_exits_2() {
    cmd()
        .arg(".")
        .arg("--keep")
        .arg("invalid-strategy")
        .assert()
        .failure()
        .code(2);
}

// ── --help surface (TASK-0301 AC) ─────────────────────────────────────────

/// `--help` lists every PRD §8 flag.
/// Maps to: "imgdedup --help lists every PRD §8 flag".
#[test]
fn help_lists_all_flags() {
    let output = cmd().arg("--help").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Positional argument
    assert!(
        stdout.contains("PATH"),
        "--help must mention the PATH positional argument"
    );

    // Scope flags
    assert!(
        stdout.contains("--no-recurse"),
        "--help must list --no-recurse"
    );
    assert!(stdout.contains("--hidden"), "--help must list --hidden");
    assert!(
        stdout.contains("--follow-symlinks"),
        "--help must list --follow-symlinks"
    );
    assert!(stdout.contains("--ext"), "--help must list --ext");

    // Keeper selection
    assert!(stdout.contains("--keep"), "--help must list --keep");
    assert!(
        stdout.contains("shortest"),
        "--help must mention the 'shortest' keep rule"
    );
    assert!(
        stdout.contains("oldest"),
        "--help must mention the 'oldest' keep rule"
    );
    assert!(
        stdout.contains("newest"),
        "--help must mention the 'newest' keep rule"
    );

    // Actions
    assert!(stdout.contains("--apply"), "--help must list --apply");
    assert!(stdout.contains("--delete"), "--help must list --delete");

    // Output flags
    assert!(stdout.contains("--json"), "--help must list --json");
    assert!(
        stdout.contains("--exit-code"),
        "--help must list --exit-code"
    );
}

// ── Valid flag combinations ───────────────────────────────────────────────

/// `--apply --delete` is a valid combination and exits 0.
#[test]
fn apply_delete_together_exits_0() {
    cmd()
        .arg(".")
        .arg("--apply")
        .arg("--delete")
        .assert()
        .code(0);
}

/// `--keep newest` is accepted.
#[test]
fn keep_newest_is_accepted() {
    cmd().arg(".").arg("--keep").arg("newest").assert().code(0);
}

/// `--keep oldest` is accepted.
#[test]
fn keep_oldest_is_accepted() {
    cmd().arg(".").arg("--keep").arg("oldest").assert().code(0);
}

/// `--exit-code` flag is accepted (constant defined; behavior wired in TASK-0311).
#[test]
fn exit_code_flag_is_accepted() {
    cmd().arg(".").arg("--exit-code").assert().code(0);
}

/// `--json` flag is accepted.
#[test]
fn json_flag_is_accepted() {
    cmd().arg(".").arg("--json").assert().code(0);
}

/// Multiple paths are accepted.
#[test]
fn multiple_paths_are_accepted() {
    cmd().arg(".").arg(".").assert().code(0);
}

/// All scope flags together are accepted.
#[test]
fn all_scope_flags_accepted() {
    cmd()
        .arg(".")
        .arg("--no-recurse")
        .arg("--hidden")
        .arg("--follow-symlinks")
        .arg("--ext")
        .arg("jpg,png")
        .assert()
        .code(0);
}

/// A nonexistent scan root must exit 1 AND explain itself on stderr
/// (run-level error, ADR-0008; resilience review 2026-07 finding F1 —
/// previously the run failed silently).
#[test]
fn nonexistent_root_prints_error_to_stderr_and_exits_1() {
    cmd()
        .arg("/definitely/not/a/real/path-imgdedup-f1")
        .assert()
        .code(1)
        .stderr(predicates::str::contains("not/a/real/path-imgdedup-f1"));
}
