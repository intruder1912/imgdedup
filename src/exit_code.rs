//! Exit code constants — single source of truth for all of Phase 3.
//!
//! ADR-0008 exit-code scheme:
//! - `SUCCESS` (0): clean run, whether or not duplicates were found.
//! - `ERROR` (1): run-level error (invalid invocation, unreadable scan root, etc.).
//! - `CLAP` (2): reserved for clap's own argument-parsing errors.
//! - `DUPLICATES_FOUND` (3): opt-in via `--exit-code`; at least one duplicate
//!   group was found. Only emitted when `--exit-code` is supplied. Wired in
//!   TASK-0311; the constant is defined here so the rest of Phase 3 has a
//!   single reference point.

/// A clean, successful run.
pub const SUCCESS: i32 = 0;

/// Run-level error (e.g. `--delete` without `--apply`, unreadable scan root).
pub const ERROR: i32 = 1;

/// Reserved for clap's own argument-parsing errors. Clap exits with 2 by
/// default on a parse failure; this constant documents that value and prevents
/// other code from reusing exit code 2 for a different purpose.
// Used by later Phase 3 tasks; defined here as the single source of truth.
#[allow(dead_code)]
pub const CLAP: i32 = 2;

/// Opt-in duplicates-found signal (`--exit-code` flag, FR-18, ADR-0008).
/// Emitted instead of `SUCCESS` when `--exit-code` is set and at least one
/// duplicate group was found. Wired in TASK-0311.
// Used by TASK-0311; defined here as the single source of truth.
#[allow(dead_code)]
pub const DUPLICATES_FOUND: i32 = 3;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_have_correct_values() {
        assert_eq!(SUCCESS, 0);
        assert_eq!(ERROR, 1);
        assert_eq!(CLAP, 2);
        assert_eq!(DUPLICATES_FOUND, 3);
    }

    #[test]
    fn constants_are_distinct() {
        let codes = [SUCCESS, ERROR, CLAP, DUPLICATES_FOUND];
        for i in 0..codes.len() {
            for j in (i + 1)..codes.len() {
                assert_ne!(
                    codes[i], codes[j],
                    "exit codes {i} and {j} must be distinct"
                );
            }
        }
    }
}
