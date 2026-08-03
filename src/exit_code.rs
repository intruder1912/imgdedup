//! Exit code constants and computation — single source of truth (TASK-0311).
//!
//! ADR-0008 exit-code scheme:
//! - `SUCCESS` (0): clean run, whether or not duplicates were found.
//! - `ERROR` (1): run-level error (unreadable/nonexistent scan root, etc.).
//! - `USAGE` (2): usage error — clap's own parse-time argument errors, and
//!   semantic validation failures this binary catches post-parse (e.g.
//!   `--delete` without `--apply`, TASK-0323).
//! - `DUPLICATES_FOUND` (3): opt-in via `--exit-code`; at least one duplicate
//!   group was found. Only emitted when `--exit-code` is supplied.
//!
//! [`compute`] is the single pure function of (run outcome, flags,
//! duplicates-found) → exit code. It is directly testable without
//! process-exit side effects.

/// A clean, successful run.
pub const SUCCESS: i32 = 0;

/// Run-level error (e.g. an unreadable/nonexistent scan root) — the
/// invocation itself was fine, but something about the state of the
/// filesystem prevented the run from completing cleanly.
pub const ERROR: i32 = 1;

/// Usage error: the invocation itself was wrong. Covers both clap's own
/// parse-time argument errors (clap exits 2 by default; this constant
/// documents that value and prevents other code from reusing it for a
/// different purpose) and semantic validation failures this binary catches
/// itself post-parse — e.g. `--delete` without `--apply` (TASK-0323, owner
/// ruling 2026-08-03: a refused flag *combination* is a usage error, not a
/// run-level one, the same bucket as an invalid flag value).
pub const USAGE: i32 = 2;

/// Opt-in duplicates-found signal (`--exit-code` flag, FR-18, ADR-0008).
/// Emitted instead of `SUCCESS` when `--exit-code` is set and at least one
/// duplicate group was found.
pub const DUPLICATES_FOUND: i32 = 3;

// ── Pure computation ──────────────────────────────────────────────────────────

/// Compute the process exit code from the run outcome.
///
/// This is a **pure function** of its arguments with no side effects — it can
/// be unit-tested directly without invoking the process. The caller is
/// responsible for passing the right arguments.
///
/// # Arguments
///
/// - `run_level_error` — `true` when there is at least one run-level error
///   (non-empty `CandidateSet.root_errors`, ADR-0008). A run-level error always
///   exits `ERROR` (1), taking precedence over `--exit-code`.
/// - `exit_code_flag` — `true` when the `--exit-code` CLI flag was supplied
///   (FR-18). Without this flag a successful run always returns `SUCCESS` (0),
///   even when duplicates are present.
/// - `duplicates_found` — `true` when the engine returned at least one
///   verified duplicate group.
///
/// # Mapping (ADR-0008 §exit-code)
///
/// | Condition                                    | Code |
/// |----------------------------------------------|------|
/// | Run-level error                              | `1`  |
/// | `--exit-code` set + duplicates found         | `3`  |
/// | All other successful runs                    | `0`  |
///
/// Per-file skip/error warnings alone **never** change the code from `0`
/// (NFR-6). `--exit-code` only affects the exit status, never what is scanned,
/// grouped, or removed.
pub fn compute(run_level_error: bool, exit_code_flag: bool, duplicates_found: bool) -> i32 {
    if run_level_error {
        return ERROR;
    }
    if exit_code_flag && duplicates_found {
        return DUPLICATES_FOUND;
    }
    SUCCESS
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Constant values ───────────────────────────────────────────────────────

    #[test]
    fn constants_have_correct_values() {
        assert_eq!(SUCCESS, 0);
        assert_eq!(ERROR, 1);
        assert_eq!(USAGE, 2);
        assert_eq!(DUPLICATES_FOUND, 3);
    }

    #[test]
    fn constants_are_distinct() {
        let codes = [SUCCESS, ERROR, USAGE, DUPLICATES_FOUND];
        for i in 0..codes.len() {
            for j in (i + 1)..codes.len() {
                assert_ne!(
                    codes[i], codes[j],
                    "exit codes {i} and {j} must be distinct"
                );
            }
        }
    }

    // ── AC-12: success/error mapping ──────────────────────────────────────────

    #[test]
    fn successful_run_no_flags_exits_0() {
        // AC-12: a clean run exits 0.
        assert_eq!(
            compute(false, false, false),
            SUCCESS,
            "no error, no flag, no dups → 0"
        );
    }

    #[test]
    fn successful_run_with_duplicates_no_flag_exits_0() {
        // AC-12 / AC-19: a run that found duplicates exits 0 WITHOUT --exit-code.
        assert_eq!(
            compute(false, false, true),
            SUCCESS,
            "no error, no --exit-code, dups found → 0"
        );
    }

    #[test]
    fn run_level_error_exits_1() {
        // AC-12: a run-level error (e.g. an unreadable root) exits 1
        // regardless of other flags. `--delete` without `--apply` is a
        // *usage* error (exit 2, TASK-0323) handled by `cli::validate()`
        // before `compute()` is ever called — not exercised here.
        assert_eq!(compute(true, false, false), ERROR, "run-level error → 1");
    }

    #[test]
    fn run_level_error_takes_precedence_over_exit_code_flag() {
        // AC-12: run-level error (1) takes precedence over --exit-code (3).
        assert_eq!(
            compute(true, true, true),
            ERROR,
            "run-level error + --exit-code + dups → still 1"
        );
    }

    // ── AC-19: --exit-code flag ────────────────────────────────────────────────

    #[test]
    fn exit_code_flag_with_duplicates_exits_3() {
        // AC-19 (FR-18): --exit-code + dups found → 3.
        assert_eq!(
            compute(false, true, true),
            DUPLICATES_FOUND,
            "--exit-code set, dups found → 3"
        );
    }

    #[test]
    fn exit_code_flag_without_duplicates_exits_0() {
        // AC-19 (FR-18): --exit-code + no dups → 0 (not 3).
        assert_eq!(
            compute(false, true, false),
            SUCCESS,
            "--exit-code set, no dups → 0"
        );
    }

    #[test]
    fn exit_code_flag_affects_only_status_not_3_when_error() {
        // The duplicates-found 3 is ONLY for successful runs; a run-level
        // error always returns 1 regardless of --exit-code.
        assert_eq!(
            compute(true, true, false),
            ERROR,
            "run-level error with --exit-code → 1, not 3"
        );
    }

    // ── NFR-6: per-file warnings do not change exit code ─────────────────────

    #[test]
    fn per_file_warning_alone_does_not_change_exit_code() {
        // NFR-6: per-file skip/error warnings are not run-level errors; they
        // must not change the exit code from 0. (Per-file errors are not passed
        // to compute() at all — only CandidateSet.root_errors cause run_level_error=true.)
        // This test documents the contract: compute(false, …) always gives 0 or 3.
        assert_eq!(
            compute(false, false, false),
            SUCCESS,
            "no run-level error → 0 regardless of per-file skips"
        );
    }
}
