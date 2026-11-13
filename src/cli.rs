use clap::{ArgAction, Parser, ValueEnum};
use std::path::PathBuf;

/// The GNU-conventional license/copyright block printed by `imgdedup
/// --version` (ADR-0014's TASK-0219 amendment). `-V` keeps printing exactly
/// `imgdedup <version>` — see [`Cli`]'s `long_version`/`version` wiring.
///
/// clap prepends the binary name itself (`imgdedup `) when it renders
/// `long_version`, so this constant deliberately starts with the bare
/// version number, not `"imgdedup "` — verified by direct experiment against
/// clap 4, not assumed off the docs (a leading `"imgdedup "` here would
/// double-print the binary name).
///
/// Wording is pinned by the ADR, not left to taste: ASCII `(C)` (not `©`, for
/// legacy Windows console code pages), the SPDX identifier `GPL-3.0-only`
/// rather than GNU's bare `GPLv3`/`+`-suffix convention, every line ≤ 80
/// columns (hence `gnu.org` rather than `www.gnu.org`), no trailing period on
/// the three metadata lines (`Copyright`/`License`/`Source`, so a URL is
/// never selected with a stray `.`), and the two warranty sentences verbatim
/// from GNU's own convention. The `Source:` URL comes from
/// `CARGO_PKG_REPOSITORY` (Cargo metadata) so it cannot drift from
/// `Cargo.toml`'s `repository` field.
const LONG_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "\nCopyright (C) 2026 the imgdedup authors\n",
    "License GPL-3.0-only: GNU GPL version 3 <https://gnu.org/licenses/gpl-3.0.html>\n",
    "This is free software: you are free to change and redistribute it.\n",
    "There is NO WARRANTY, to the extent permitted by law.\n",
    "Source: ",
    env!("CARGO_PKG_REPOSITORY"),
);

/// Find and remove duplicate images.
///
/// By default runs as a dry run: prints each duplicate group with its keeper
/// and the files that would be removed. Nothing on disk changes unless
/// `--apply` is passed.
#[derive(Debug, Parser)]
#[command(name = "imgdedup", version, long_version = LONG_VERSION, about, long_about = None)]
pub struct Cli {
    // `///` comments on the fields below are the `--help` text. They are
    // user-facing: internal identifiers (FR-/AC-/ADR-/TASK-) belong in `//`
    // comments, never in the doc comment (TASK-0344; pinned by the
    // `help_text_cites_no_internal_identifiers` integration test).
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
    /// together with `--apply`; exits 2 without it.
    // AC-12.
    #[arg(long)]
    pub delete: bool,

    // ── Output ─────────────────────────────────────────────────────────────
    /// Emit machine-readable JSON output.
    // FR-14.
    #[arg(long, action = ArgAction::SetTrue)]
    pub json: bool,

    /// Exit 3 if duplicate groups were found, 0 if none.
    /// Without this flag, a successful run always exits 0.
    // FR-18, ADR-0008.
    #[arg(long, action = ArgAction::SetTrue)]
    pub exit_code: bool,

    /// Disable the live progress indicator. Progress is shown by
    /// default on the terminal — on stdout when stdout is an interactive
    /// terminal, otherwise on stderr; under `--json` always on stderr. This
    /// flag turns it off completely: nothing progress-related is written to
    /// any stream, and the report is unaffected either way.
    // FR-19, AC-23.
    #[arg(long, action = ArgAction::SetTrue)]
    pub no_progress: bool,

    /// Write a durable record of the run to FILE, continuously.
    ///
    /// The file is written as the run progresses — a header, progress
    /// milestones with elapsed time, each duplicate group as it is verified,
    /// every warning and error, and each `--apply` action — and ends with the
    /// complete report once the run finishes. If the run is interrupted, the
    /// file keeps everything recorded up to that point and has no final
    /// report. With `--json` the file is NDJSON (one JSON object per line).
    /// An earlier run record at FILE is kept as FILE.bak and replaced; any
    /// other existing file is refused unless `--overwrite-output` is given.
    /// This is in addition to the normal output, which is unchanged.
    // FR-20, ADR-0018; the overwrite rule is TASK-0342.
    #[arg(long, value_name = "FILE")]
    pub output: Option<PathBuf>,

    /// Replace an existing FILE given to `--output` even if it is not an
    /// imgdedup run record.
    ///
    /// Without this flag, `--output` replaces only an earlier run record or
    /// an empty file and refuses anything else. Whatever is replaced is kept
    /// as FILE.bak. A FILE.bak that is not an imgdedup run record is never
    /// replaced, with or without this flag. Requires `--output`.
    // TASK-0342.
    #[arg(long, action = ArgAction::SetTrue)]
    pub overwrite_output: bool,

    // ── Diagnostics (ADR-0017, TASK-0338) ──────────────────────────────────
    /// Print per-stage timing lines to stderr (diagnostic).
    ///
    /// Writes one line per pipeline stage to stderr as that stage finishes,
    /// with the time it took. Implies `--no-progress`: timing lines and the
    /// live progress indicator cannot share a terminal, so `--timings` wins, and
    /// passing both is not an error. Off by default; stdout is never
    /// changed, so `--timings --json` is the way to measure a scripted run.
    /// The output is diagnostic, not a stable interface.
    // ADR-0017.
    #[arg(long, action = ArgAction::SetTrue)]
    pub timings: bool,
}

impl Cli {
    /// Post-parse semantic validation.
    ///
    /// Returns an error string if the parsed arguments form an invalid
    /// combination that clap's type system cannot catch. The caller is
    /// responsible for printing the message to stderr and exiting with
    /// `exit_code::USAGE` (2).
    ///
    /// Currently enforced: `--delete` requires `--apply` (AC-12, ADR-0008).
    /// This is a *usage* error (exit 2, TASK-0323) — not caught by clap's
    /// own parser (the combination is syntactically valid), but the same
    /// code clap's own parse errors use, since both mean "the invocation
    /// itself was wrong."
    ///
    /// Returns the bare diagnostic, with no `"error: "` prefix — `main.rs`
    /// adds it uniformly at the single stderr call site, the same way every
    /// other diagnostic in the run does (TASK-0217 consistency pass).
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.delete && !self.apply {
            return Err(
                "--delete requires --apply (refusing to permanently delete without an explicit --apply)",
            );
        }
        if self.overwrite_output && self.output.is_none() {
            return Err("--overwrite-output requires --output");
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
        // AC-12 / TASK-0323: --delete without --apply is a *usage* error
        // (exit 2), but NOT one clap itself catches — the combination is
        // syntactically valid. Clap must accept it; post-parse validate()
        // is responsible for the rejection (main.rs maps it to the same
        // exit_code::USAGE clap's own parse errors use).
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
        assert!(
            !msg.starts_with("error:"),
            "validate() must return the bare message — main.rs adds the \
             \"error: \" prefix uniformly at the one stderr call site, got: {msg}"
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

    // ── --timings (ADR-0017, TASK-0338) ───────────────────────────────────

    #[test]
    fn timings_defaults_false() {
        let cli = try_parse(&["imgdedup", "."]).unwrap();
        assert!(!cli.timings);
    }

    #[test]
    fn timings_parses() {
        let cli = try_parse(&["imgdedup", ".", "--timings"]).unwrap();
        assert!(cli.timings);
    }

    // ── Unknown flag ──────────────────────────────────────────────────────

    #[test]
    fn unknown_flag_is_a_parse_error() {
        let err = try_parse(&["imgdedup", ".", "--unknown-flag"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::UnknownArgument);
    }

    // ── --version / -V (ADR-0014 TASK-0219 amendment) ──────────────────────
    //
    // clap prepends the binary name itself when rendering `long_version` /
    // `version`, verified by direct experiment (a scratch clap 4.6.3 crate)
    // rather than assumed off the docs: `render_long_version()` and
    // `render_version()` both come back already prefixed with `"imgdedup "`.
    // The exact block below is pinned by literal string, not reconstructed
    // from the constant under test, and the `Source:` line's repo URL is a
    // literal here (not `env!("CARGO_PKG_REPOSITORY")`) precisely so a
    // `Cargo.toml` `repository` field change is *caught* by this test rather
    // than silently mirrored by it.

    /// The exact 6-line GNU-style block, version interpolated.
    fn expected_long_version_block() -> String {
        format!(
            "imgdedup {}\n\
             Copyright (C) 2026 the imgdedup authors\n\
             License GPL-3.0-only: GNU GPL version 3 <https://gnu.org/licenses/gpl-3.0.html>\n\
             This is free software: you are free to change and redistribute it.\n\
             There is NO WARRANTY, to the extent permitted by law.\n\
             Source: https://github.com/intruder1912/imgdedup\n",
            env!("CARGO_PKG_VERSION")
        )
    }

    #[test]
    fn long_version_is_the_exact_gnu_style_block() {
        use clap::CommandFactory;
        let rendered = Cli::command().render_long_version();
        assert_eq!(rendered, expected_long_version_block());
    }

    #[test]
    fn long_version_is_ascii() {
        use clap::CommandFactory;
        let rendered = Cli::command().render_long_version();
        assert!(
            rendered.is_ascii(),
            "the --version block must stay ASCII (renders on legacy Windows \
             console code pages); got: {rendered:?}"
        );
    }

    #[test]
    fn long_version_lines_are_at_most_80_columns() {
        use clap::CommandFactory;
        let rendered = Cli::command().render_long_version();
        for line in rendered.lines() {
            assert!(
                line.chars().count() <= 80,
                "line exceeds 80 columns ({} chars): {line:?}",
                line.chars().count()
            );
        }
    }

    #[test]
    fn long_version_first_line_matches_short_version() {
        // Drift guard: the first line of the long block must be exactly what
        // `-V` prints (minus its own trailing newline), so the two can never
        // silently diverge on the version number itself.
        use clap::CommandFactory;
        let long = Cli::command().render_long_version();
        let short = Cli::command().render_version();
        let first_line = long.lines().next().unwrap();
        assert_eq!(format!("{first_line}\n"), short);
    }

    #[test]
    fn short_version_is_exactly_imgdedup_and_version() {
        // Regression guard for an existing contract, not new behavior:
        // scripts/build-release-artifact.sh asserts this exact one-liner.
        use clap::CommandFactory;
        let rendered = Cli::command().render_version();
        assert_eq!(
            rendered,
            format!("imgdedup {}\n", env!("CARGO_PKG_VERSION"))
        );
    }

    // ── --output (FR-20) ──────────────────────────────────────────────────

    #[test]
    fn output_defaults_to_none() {
        assert_eq!(try_parse(&["imgdedup", "."]).unwrap().output, None);
    }

    #[test]
    fn output_takes_a_path() {
        let cli = try_parse(&["imgdedup", ".", "--output", "run.txt"]).unwrap();
        assert_eq!(cli.output, Some(PathBuf::from("run.txt")));
    }

    #[test]
    fn output_requires_a_value() {
        let err = try_parse(&["imgdedup", ".", "--output"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidValue);
    }
}
