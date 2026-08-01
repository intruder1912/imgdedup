use clap::{ArgAction, Parser, ValueEnum};
use std::path::PathBuf;

/// Find and remove duplicate images.
///
/// By default runs as a dry run: prints each duplicate group with its keeper
/// and the files that would be removed. Nothing on disk changes unless
/// `--apply` is passed.
#[derive(Debug, Parser)]
#[command(name = "imgdedup", version, about, long_about = None)]
pub struct Cli {
    /// One or more paths to scan for duplicate images.
    #[arg(required = true, value_name = "PATH")]
    pub paths: Vec<PathBuf>,

    // ── Scope ──────────────────────────────────────────────────────────────
    /// Do not descend into subdirectories (scan top level of each path only).
    #[arg(long)]
    pub no_recurse: bool,

    /// Include hidden files and directories (those starting with `.`).
    #[arg(long)]
    pub hidden: bool,

    /// Follow symbolic links during traversal.
    #[arg(long)]
    pub follow_symlinks: bool,

    /// Override the image extension allow-list (comma- or space-separated,
    /// e.g. `jpg,png,gif`).
    #[arg(long, value_name = "LIST")]
    pub ext: Option<String>,

    // ── Keeper selection ───────────────────────────────────────────────────
    /// Rule for selecting the keeper within each duplicate group.
    ///
    /// `shortest` (default): fewest path segments, then fewest characters,
    /// then oldest mtime, then lexicographic order. `oldest`/`newest`: oldest
    /// or newest mtime first, then shortest, then lexicographic.
    #[arg(long, value_enum, default_value_t = KeepRule::Shortest, value_name = "RULE")]
    pub keep: KeepRule,

    // ── Actions ────────────────────────────────────────────────────────────
    /// Move non-keepers to the OS trash (recoverable). Without this flag the
    /// run is a dry run only.
    #[arg(long)]
    pub apply: bool,

    /// Permanently delete non-keepers instead of trashing them. Only valid
    /// together with `--apply`; exits 1 without it (AC-12).
    #[arg(long)]
    pub delete: bool,

    // ── Output ─────────────────────────────────────────────────────────────
    /// Emit machine-readable JSON output (FR-14).
    #[arg(long, action = ArgAction::SetTrue)]
    pub json: bool,

    /// Exit 3 if duplicate groups were found, 0 if none (FR-18, ADR-0008).
    /// Without this flag, a successful run always exits 0.
    #[arg(long, action = ArgAction::SetTrue)]
    pub exit_code: bool,

    /// Disable the live progress indicator (FR-19). Progress is shown on
    /// stderr by default (auto-adapting to whether stderr is a terminal);
    /// this flag turns it off completely — nothing progress-related is
    /// written to any stream, and stdout is unaffected either way (AC-23).
    #[arg(long, action = ArgAction::SetTrue)]
    pub no_progress: bool,
}

impl Cli {
    /// Post-parse semantic validation.
    ///
    /// Returns an error string if the parsed arguments form an invalid
    /// combination that clap's type system cannot catch. The caller is
    /// responsible for printing the message to stderr and exiting with
    /// `exit_code::ERROR` (1).
    ///
    /// Currently enforced: `--delete` requires `--apply` (AC-12, ADR-0008).
    /// This is a *run-level* error (exit 1), not a parse error (exit 2).
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.delete && !self.apply {
            return Err(
                "error: --delete requires --apply (refusing to permanently delete without an explicit --apply)",
            );
        }
        Ok(())
    }
}

/// Keeper-selection rule (FR-7/FR-8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum KeepRule {
    /// Keep the file with the shortest path (fewest segments, then fewest chars).
    Shortest,
    /// Keep the file with the oldest modification time.
    Oldest,
    /// Keep the file with the newest modification time.
    Newest,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::error::ErrorKind;

    // Helper: parse args via clap's try_parse_from (no process::exit).
    fn try_parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(args)
    }

    // Helper: parse and then run semantic validation.
    fn parse_and_validate(args: &[&str]) -> Result<Cli, String> {
        let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;
        cli.validate().map_err(|e| e.to_string())?;
        Ok(cli)
    }

    // ── Positional path tests ──────────────────────────────────────────────

    #[test]
    fn rejects_no_path_args() {
        // AC-12 / "at least one path required"
        let err = try_parse(&["imgdedup"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn accepts_single_path() {
        let cli = try_parse(&["imgdedup", "/tmp/photos"]).unwrap();
        assert_eq!(cli.paths, vec![PathBuf::from("/tmp/photos")]);
    }

    #[test]
    fn accepts_multiple_paths() {
        let cli = try_parse(&["imgdedup", "/tmp/a", "/tmp/b", "/tmp/c"]).unwrap();
        assert_eq!(cli.paths.len(), 3);
    }

    // ── Scope flags ───────────────────────────────────────────────────────

    #[test]
    fn no_recurse_defaults_false() {
        let cli = try_parse(&["imgdedup", "."]).unwrap();
        assert!(!cli.no_recurse);
    }

    #[test]
    fn no_recurse_parses() {
        let cli = try_parse(&["imgdedup", ".", "--no-recurse"]).unwrap();
        assert!(cli.no_recurse);
    }

    #[test]
    fn hidden_defaults_false() {
        let cli = try_parse(&["imgdedup", "."]).unwrap();
        assert!(!cli.hidden);
    }

    #[test]
    fn hidden_parses() {
        let cli = try_parse(&["imgdedup", ".", "--hidden"]).unwrap();
        assert!(cli.hidden);
    }

    #[test]
    fn follow_symlinks_defaults_false() {
        let cli = try_parse(&["imgdedup", "."]).unwrap();
        assert!(!cli.follow_symlinks);
    }

    #[test]
    fn follow_symlinks_parses() {
        let cli = try_parse(&["imgdedup", ".", "--follow-symlinks"]).unwrap();
        assert!(cli.follow_symlinks);
    }

    #[test]
    fn ext_defaults_none() {
        let cli = try_parse(&["imgdedup", "."]).unwrap();
        assert!(cli.ext.is_none());
    }

    #[test]
    fn ext_parses_value() {
        let cli = try_parse(&["imgdedup", ".", "--ext", "jpg,png"]).unwrap();
        assert_eq!(cli.ext.as_deref(), Some("jpg,png"));
    }

    // ── Keeper selection ──────────────────────────────────────────────────

    #[test]
    fn keep_defaults_to_shortest() {
        let cli = try_parse(&["imgdedup", "."]).unwrap();
        assert_eq!(cli.keep, KeepRule::Shortest);
    }

    #[test]
    fn keep_parses_shortest_explicit() {
        let cli = try_parse(&["imgdedup", ".", "--keep", "shortest"]).unwrap();
        assert_eq!(cli.keep, KeepRule::Shortest);
    }

    #[test]
    fn keep_parses_oldest() {
        let cli = try_parse(&["imgdedup", ".", "--keep", "oldest"]).unwrap();
        assert_eq!(cli.keep, KeepRule::Oldest);
    }

    #[test]
    fn keep_parses_newest() {
        let cli = try_parse(&["imgdedup", ".", "--keep", "newest"]).unwrap();
        assert_eq!(cli.keep, KeepRule::Newest);
    }

    #[test]
    fn keep_rejects_invalid_value() {
        // Bad value for --keep should produce a clap parse error (exit 2).
        let err = try_parse(&["imgdedup", ".", "--keep", "random"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidValue);
    }

    // ── Action flags ──────────────────────────────────────────────────────

    #[test]
    fn apply_defaults_false() {
        let cli = try_parse(&["imgdedup", "."]).unwrap();
        assert!(!cli.apply);
    }

    #[test]
    fn apply_parses() {
        let cli = try_parse(&["imgdedup", ".", "--apply"]).unwrap();
        assert!(cli.apply);
    }

    #[test]
    fn delete_without_apply_parses_ok_at_clap_level() {
        // AC-12 / ADR-0008: --delete without --apply is a *run-level* error
        // (exit 1), NOT a clap parse error (exit 2). Clap must accept this
        // combination; post-parse validate() is responsible for the rejection.
        let cli = try_parse(&["imgdedup", ".", "--delete"]).unwrap();
        assert!(cli.delete);
        assert!(!cli.apply);
    }

    #[test]
    fn validate_rejects_delete_without_apply() {
        // validate() must return Err when --delete is set without --apply.
        let cli = try_parse(&["imgdedup", ".", "--delete"]).unwrap();
        let result = cli.validate();
        assert!(
            result.is_err(),
            "validate() must reject --delete without --apply"
        );
        let msg = result.unwrap_err();
        assert!(
            msg.contains("--apply"),
            "error message must mention --apply, got: {msg}"
        );
    }

    #[test]
    fn validate_accepts_apply_with_delete() {
        // --apply --delete together is valid.
        let cli = try_parse(&["imgdedup", ".", "--apply", "--delete"]).unwrap();
        assert!(cli.apply);
        assert!(cli.delete);
        assert!(cli.validate().is_ok());
    }

    #[test]
    fn validate_accepts_apply_without_delete() {
        // --apply alone (trash mode) is valid.
        let cli = try_parse(&["imgdedup", ".", "--apply"]).unwrap();
        assert!(cli.validate().is_ok());
    }

    #[test]
    fn validate_accepts_normal_dry_run() {
        // No --apply, no --delete: dry run is always valid.
        let cli = try_parse(&["imgdedup", "."]).unwrap();
        assert!(cli.validate().is_ok());
    }

    #[test]
    fn parse_and_validate_rejects_delete_without_apply() {
        // End-to-end: parse + validate pipeline rejects --delete without --apply.
        assert!(
            parse_and_validate(&["imgdedup", ".", "--delete"]).is_err(),
            "parse_and_validate must return Err for --delete without --apply"
        );
    }

    // ── Output flags ──────────────────────────────────────────────────────

    #[test]
    fn json_defaults_false() {
        let cli = try_parse(&["imgdedup", "."]).unwrap();
        assert!(!cli.json);
    }

    #[test]
    fn json_parses() {
        let cli = try_parse(&["imgdedup", ".", "--json"]).unwrap();
        assert!(cli.json);
    }

    #[test]
    fn exit_code_defaults_false() {
        let cli = try_parse(&["imgdedup", "."]).unwrap();
        assert!(!cli.exit_code);
    }

    #[test]
    fn exit_code_parses() {
        let cli = try_parse(&["imgdedup", ".", "--exit-code"]).unwrap();
        assert!(cli.exit_code);
    }

    // ── Progress (FR-19) ──────────────────────────────────────────────────

    #[test]
    fn no_progress_defaults_false() {
        // Progress is default-on (AC-21/AC-22); the flag is an opt-out.
        let cli = try_parse(&["imgdedup", "."]).unwrap();
        assert!(!cli.no_progress);
    }

    #[test]
    fn no_progress_parses() {
        let cli = try_parse(&["imgdedup", ".", "--no-progress"]).unwrap();
        assert!(cli.no_progress);
    }

    // ── Unknown flag ──────────────────────────────────────────────────────

    #[test]
    fn unknown_flag_is_a_parse_error() {
        let err = try_parse(&["imgdedup", ".", "--unknown-flag"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::UnknownArgument);
    }
}
