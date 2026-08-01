/// Integration tests for the imgdedup CLI.
///
/// These tests run the real binary via `assert_cmd` and verify process exit
/// codes and stdout/stderr content.  They map directly to the acceptance
/// criteria in TASK-0301, ADR-0008, and PRD-0001 §8/§9.
mod support;

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::PathBuf;
use support::images::{ImageFormat, image_bytes};
use support::tree::{FileSpec, materialize};

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

// ── Destructive actions (slice S3) ───────────────────────────────────────────
//
// **Every test in this section scans a temporary directory it created.** One of
// these used to run `imgdedup . --apply --delete` against the repository
// working directory; that was harmless only while both flags were inert, and
// became a real removal the moment TASK-0309 wired `--apply` up. No test may
// point a destructive flag at a directory it does not own.

/// Builds a tree with one duplicate pair (`a.png` ≡ `b.png`) plus a unique
/// file, and returns the temp dir with the three paths. `a.png` is the keeper
/// under the default `shortest` rule (equal depth, equal length, lexicographic
/// tie-break).
fn duplicate_tree() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let dup_bytes = image_bytes(ImageFormat::Png, 21, 64);
    let other = image_bytes(ImageFormat::Png, 22, 64);
    let a = dir.path().join("a.png");
    let b = dir.path().join("b.png");
    let u = dir.path().join("u.png");
    fs::write(&a, &dup_bytes).unwrap();
    fs::write(&b, &dup_bytes).unwrap();
    fs::write(&u, &other).unwrap();
    (dir, a, b, u)
}

/// AC-7 (FR-11): `--apply --delete` permanently removes the non-keeper. Uses
/// the delete path rather than the trash path because deletion is deterministic
/// on every platform, so "it is really gone" can be asserted strictly.
#[test]
fn apply_delete_permanently_removes_non_keepers_and_keeps_the_keeper() {
    let (dir, a, b, u) = duplicate_tree();

    cmd()
        .arg(dir.path())
        .arg("--apply")
        .arg("--delete")
        .arg("--no-progress")
        .assert()
        .code(0)
        .stdout(predicate::str::contains("Deleted 1 file."));

    assert!(a.exists(), "the keeper must survive (NFR-1)");
    assert!(!b.exists(), "the non-keeper must be gone (AC-7)");
    assert!(u.exists(), "an unrelated file must not be touched");
}

/// AC-3 (FR-10, NFR-1): `--apply` moves the non-keeper to the OS trash.
///
/// The safety invariants are asserted unconditionally; whether trashing
/// *succeeds* is environment-dependent by design (ADR-0009 — a box with no
/// usable trash reports a per-file skip and the run continues), so the
/// non-keeper is allowed to be either trashed or reported as skipped. What is
/// never allowed is the keeper disappearing, an unrelated file being touched,
/// or a trash failure escalating into a deletion.
#[test]
fn apply_trashes_non_keepers_or_reports_why_not() {
    let (dir, a, b, u) = duplicate_tree();

    let out = cmd()
        .arg(dir.path())
        .arg("--apply")
        .arg("--no-progress")
        .output()
        .expect("run");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(a.exists(), "the keeper must survive (NFR-1)");
    assert!(u.exists(), "an unrelated file must not be touched");

    if b.exists() {
        // Trash unavailable: it must have been reported, not silently ignored,
        // and above all not deleted instead.
        assert!(
            stdout.contains("Trashed 0 files.") && stdout.contains("Skipped (left in place):"),
            "an untrashed file must be reported as skipped; stdout:\n{stdout}\nstderr:\n{stderr}"
        );
    } else {
        assert!(
            stdout.contains("Trashed 1 file."),
            "a trashed file must be reported; stdout:\n{stdout}"
        );
    }
}

/// AC-5 (FR-12): re-running `--apply` on an already-deduplicated tree takes
/// zero actions and changes nothing.
#[test]
fn a_second_apply_run_is_a_no_op() {
    let (dir, a, b, u) = duplicate_tree();

    cmd()
        .arg(dir.path())
        .arg("--apply")
        .arg("--delete")
        .arg("--no-progress")
        .assert()
        .code(0);
    assert!(!b.exists());

    // Second run: nothing left to group, so nothing to do.
    cmd()
        .arg(dir.path())
        .arg("--apply")
        .arg("--delete")
        .arg("--no-progress")
        .assert()
        .code(0)
        .stdout(predicate::str::contains("No duplicate groups found."))
        .stdout(predicate::str::contains("Deleted 0 files."));

    assert!(a.exists(), "the keeper still stands after a second run");
    assert!(u.exists());
}

/// AC-11 (FR-9 → FR-10 parity): for an unchanged tree, what the dry run says it
/// would remove is exactly what the apply removes.
#[test]
fn the_dry_run_predicts_exactly_what_apply_removes() {
    let (dir, _a, b, _u) = duplicate_tree();

    let dry = cmd()
        .arg(dir.path())
        .arg("--json")
        .arg("--no-progress")
        .output()
        .expect("run");
    let dry: serde_json::Value = serde_json::from_slice(&dry.stdout).expect("valid JSON");
    assert_eq!(dry["mode"], "dry-run", "a plain run must not claim to act");
    let intended: Vec<String> = dry["groups"][0]["removable"]
        .as_array()
        .expect("removable array")
        .iter()
        .map(|f| f["path"].as_str().unwrap().to_owned())
        .collect();

    let applied = cmd()
        .arg(dir.path())
        .arg("--json")
        .arg("--apply")
        .arg("--delete")
        .arg("--no-progress")
        .output()
        .expect("run");
    let applied: serde_json::Value = serde_json::from_slice(&applied.stdout).expect("valid JSON");
    assert_eq!(applied["mode"], "delete", "the mode reports what happened");
    let removed: Vec<String> = applied["actions"]["removed"]
        .as_array()
        .expect("actions.removed array")
        .iter()
        .map(|p| p.as_str().unwrap().to_owned())
        .collect();

    assert_eq!(
        intended, removed,
        "the dry run's intended removals must match what apply actually removed"
    );
    assert!(!b.exists());
}

/// A removal that *fails* is reported and the file is left in place — and the
/// run never falls back to a different mechanism (NFR-1, ADR-0009).
///
/// Genuine end-to-end coverage of the failure path: the duplicate lives in a
/// sub-directory made read-only after the files are written, so the scan can
/// still read it (read permission on the directory and the file) while
/// `remove_file` cannot unlink it (that needs write permission on the parent).
/// Unix-only because Windows permissions do not model this the same way.
#[cfg(unix)]
#[test]
fn a_removal_that_fails_is_reported_and_the_file_survives() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("tempdir");
    let bytes = image_bytes(ImageFormat::Png, 24, 64);
    let keeper = dir.path().join("a.png");
    let locked_dir = dir.path().join("sub");
    fs::create_dir(&locked_dir).unwrap();
    let victim = locked_dir.join("b.png");
    fs::write(&keeper, &bytes).unwrap();
    fs::write(&victim, &bytes).unwrap();

    fs::set_permissions(&locked_dir, fs::Permissions::from_mode(0o555)).unwrap();

    let out = cmd()
        .arg(dir.path())
        .arg("--apply")
        .arg("--delete")
        .arg("--no-progress")
        .output()
        .expect("run");
    let stdout = String::from_utf8_lossy(&out.stdout);

    // Restore permissions so the temp dir can be cleaned up.
    fs::set_permissions(&locked_dir, fs::Permissions::from_mode(0o755)).unwrap();

    assert!(
        victim.exists(),
        "a file whose removal failed must still be there; stdout:\n{stdout}"
    );
    assert!(keeper.exists(), "the keeper is untouched");
    assert!(
        stdout.contains("Deleted 0 files.") && stdout.contains("Skipped (left in place):"),
        "the failure must be reported, not swallowed; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("removal failed"),
        "the reason must say the removal failed; stdout:\n{stdout}"
    );
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

/// `--json` must emit a single valid JSON document on stdout (FR-14, AC-8),
/// with the stable top-level fields present.
#[test]
fn json_flag_emits_valid_json_document() {
    let out = cmd().arg(".").arg("--json").output().unwrap();
    assert!(out.status.success());
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("--json stdout must parse as JSON");
    assert_eq!(v["schema_version"], 1);
    assert!(v["groups"].is_array());
    assert!(v["totals"].is_object());
    assert!(v["warnings"].is_array());
    assert!(v["root_errors"].is_array());
}

// ── Progress indication (FR-19, TASK-0315, AC-20..AC-24) ──────────────────
//
// `src/progress.rs` unit tests already cover the renderer in isolation
// (AC-21 TTY, AC-22 non-TTY, AC-23 disabled) with an injected writer/tty
// flag; `src/engine.rs` unit tests already cover the "advances sum to the
// total" invariant. These tests instead exercise the real binary end to
// end: the stdout-purity contract (AC-20), the output-only guarantee
// (AC-24), and the non-TTY fallback / opt-out as observed through an actual
// child process's stdio pipes (AC-22 / AC-23).

/// Fixed mtime for the progress fixture tree — arbitrary but constant, so
/// the tree (and therefore every run over it) is fully deterministic.
const PROGRESS_MTIME: i64 = 1_700_000_000;

/// Materialize a small tree with three genuine duplicate pairs across three
/// distinct image formats (six confirmed files total). This is large enough
/// to drive the pipeline through Scanning -> Hashing -> Verifying with more
/// than one file per phase (so a real run actually produces progress
/// output and multiple duplicate groups), while staying small enough that
/// the exact stderr shape stays predictable across platforms/CI. Every file
/// is a genuinely valid, sniff-confirmed image, and the tree contains no
/// unreadable files, symlinks, or nonexistent paths — so a run over it is
/// guaranteed (by construction) to produce zero warnings and zero root
/// errors, which is exactly what the AC-23 test below relies on.
fn materialize_progress_fixture(root: &std::path::Path) {
    let pairs = [
        (ImageFormat::Png, 1u8),
        (ImageFormat::Jpeg, 2u8),
        (ImageFormat::Bmp, 3u8),
    ];
    let mut files = Vec::new();
    for (i, (format, seed)) in pairs.into_iter().enumerate() {
        let ext = format.extension();
        let bytes = image_bytes(format, seed, 64);
        files.push(FileSpec {
            rel_path: format!("dir{i}/a.{ext}"),
            bytes: bytes.clone(),
            mtime: PROGRESS_MTIME,
        });
        files.push(FileSpec {
            rel_path: format!("dir{i}/copy.{ext}"),
            bytes,
            mtime: PROGRESS_MTIME,
        });
    }
    materialize(root, &files, &[]);
}

/// Progress phase labels (`src/progress.rs` `Phase::label`), used to detect
/// any progress text that leaked onto a stream it must not touch.
const PROGRESS_MARKERS: [&str; 3] = ["scanning", "hashing", "verifying"];

/// AC-20 (FR-19, stdout purity, default report): with progress enabled (the
/// default), a completed run's stdout is **exactly** the human report — no
/// progress text anywhere in it — while the progress phases appear on
/// stderr instead (positive control proving progress actually ran, so the
/// stdout-purity assertion above cannot pass vacuously because progress was
/// silent for some unrelated reason).
#[test]
fn progress_enabled_default_report_stdout_has_no_progress_text() {
    let dir = tempfile::tempdir().expect("tempdir");
    materialize_progress_fixture(dir.path());

    let out = cmd().arg(dir.path()).output().expect("imgdedup must spawn");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    // Positive control: the tree really was scanned and really did produce
    // a report (three duplicate pairs, one per format).
    assert!(
        stdout.contains("Group 1/3") && stdout.contains("Summary: 3 duplicate groups"),
        "expected the real report on stdout; got:\n{stdout}"
    );

    for marker in PROGRESS_MARKERS.iter().copied().chain(["\r", "\x1b"]) {
        assert!(
            !stdout.contains(marker),
            "stdout must not contain progress marker {marker:?} (AC-20); got:\n{stdout:?}"
        );
    }

    assert!(
        PROGRESS_MARKERS.iter().any(|m| stderr.contains(m)),
        "expected progress phase text on stderr instead (positive control); got:\n{stderr}"
    );
}

/// AC-20 (FR-19, stdout purity, `--json`): the same guarantee holds for
/// `--json` — stdout is a single valid JSON document with no progress text
/// mixed in, verified by actually parsing it rather than string-matching.
#[test]
fn progress_enabled_json_stdout_is_single_valid_json_with_no_progress_text() {
    let dir = tempfile::tempdir().expect("tempdir");
    materialize_progress_fixture(dir.path());

    let out = cmd()
        .arg(dir.path())
        .arg("--json")
        .output()
        .expect("imgdedup must spawn");
    assert!(out.status.success());

    let doc: serde_json::Value = serde_json::from_slice(&out.stdout)
        .expect("--json stdout must parse as a single JSON document even with progress enabled");
    let groups = doc["groups"]
        .as_array()
        .expect("groups must be a JSON array");
    assert_eq!(
        groups.len(),
        3,
        "positive control: 3 duplicate groups expected"
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    for marker in PROGRESS_MARKERS.iter().copied().chain(["\r", "\x1b"]) {
        assert!(
            !stdout.contains(marker),
            "--json stdout must not contain progress marker {marker:?} (AC-20); got:\n{stdout:?}"
        );
    }

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        PROGRESS_MARKERS.iter().any(|m| stderr.contains(m)),
        "expected progress phase text on stderr even in --json mode; got:\n{stderr}"
    );
}

/// AC-24 (FR-19, output-only), default report: for identical inputs/flags,
/// stdout is **byte-identical** whether progress is on (default) or off
/// (`--no-progress`). This is the load-bearing test for the whole unit —
/// progress must perturb neither results nor stdout ordering. It would
/// catch, for example: progress writes mistakenly routed to stdout, a
/// shared buffer/lock accidentally serializing output differently under
/// the two sinks, or a progress call site that (bug) mutates pipeline state
/// as a side effect instead of being a pure observer.
#[test]
fn progress_on_off_default_report_stdout_is_byte_identical() {
    let dir = tempfile::tempdir().expect("tempdir");
    materialize_progress_fixture(dir.path());

    let with_progress = cmd().arg(dir.path()).output().expect("imgdedup must spawn");
    let without_progress = cmd()
        .arg(dir.path())
        .arg("--no-progress")
        .output()
        .expect("imgdedup must spawn");

    assert!(with_progress.status.success());
    assert!(without_progress.status.success());
    assert_eq!(
        with_progress.stdout, without_progress.stdout,
        "AC-24: default-report stdout must be byte-identical whether progress is on or off"
    );
}

/// AC-24 (FR-19, output-only), `--json`: the same byte-identical guarantee
/// holds for the machine-readable format.
#[test]
fn progress_on_off_json_stdout_is_byte_identical() {
    let dir = tempfile::tempdir().expect("tempdir");
    materialize_progress_fixture(dir.path());

    let with_progress = cmd()
        .arg(dir.path())
        .arg("--json")
        .output()
        .expect("imgdedup must spawn");
    let without_progress = cmd()
        .arg(dir.path())
        .arg("--json")
        .arg("--no-progress")
        .output()
        .expect("imgdedup must spawn");

    assert!(with_progress.status.success());
    assert!(without_progress.status.success());
    assert_eq!(
        with_progress.stdout, without_progress.stdout,
        "AC-24: --json stdout must be byte-identical whether progress is on or off"
    );
}

/// AC-23 (FR-19, opt-out) through the real binary: with `--no-progress`,
/// nothing progress-related reaches stderr.
///
/// Honesty note (per the task brief): stderr is also where legitimate
/// diagnostics land (per-file warnings, `error:` root-error lines), so a
/// blanket "stderr is empty" assertion would be dishonest for an arbitrary
/// tree — it could pass merely because *nothing at all* went wrong, telling
/// us nothing about progress specifically. We deliberately use
/// `materialize_progress_fixture`, which by construction (see its doc
/// comment) is a warning-free, root-error-free tree — every file is a
/// genuinely valid, sniff-confirmed image, nothing is unreadable, and the
/// root path exists. That lets us assert stderr is fully empty here as the
/// strongest possible check for "nothing progress-related", while the
/// comment above documents why this specific assertion doesn't generalize
/// to arbitrary trees with real diagnostics.
#[test]
fn no_progress_flag_produces_no_progress_text_on_stderr() {
    let dir = tempfile::tempdir().expect("tempdir");
    materialize_progress_fixture(dir.path());

    let out = cmd()
        .arg(dir.path())
        .arg("--no-progress")
        .output()
        .expect("imgdedup must spawn");

    assert!(out.status.success());
    assert!(
        out.stderr.is_empty(),
        "expected empty stderr with --no-progress on a warning-free tree (AC-23); got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// AC-22 (FR-19, non-TTY fallback) through the real binary. A test
/// process's stderr is a pipe, so `assert_cmd`/`std::process::Command`
/// naturally drives the binary down the non-TTY path with no PTY emulation
/// needed — exactly the surface the task brief asks us to assert on
/// honestly. No in-place redraw (carriage return) or ANSI escape sequence
/// may appear, and the number of progress lines must be small and bounded —
/// a future regression that logs per-file instead of at coarse milestones
/// must fail this test.
///
/// `\r` handling / Windows note: we search for a bare `\r` byte in the raw
/// piped stderr bytes. This is safe on Linux, macOS, and Windows CI alike
/// because (a) `assert_cmd` captures the child's stderr as raw bytes with
/// no OS- or libc-level newline translation on any of the three platforms
/// (that translation only happens for files explicitly opened in "text
/// mode" by some runtimes, not for pipes), and (b) `writeln!`/`write!` in
/// Rust's std always emit a bare `\n`, never `\r\n`, regardless of target
/// OS. So on the non-TTY fallback path — which per `src/progress.rs` never
/// legitimately emits `\r` at all (that byte is reserved for the TTY
/// in-place-redraw path, AC-21) — a lone `\r` search needs no `\r\n`
/// normalization first; there is no legitimate `\r\n` in this output to
/// confuse it with.
#[test]
fn piped_stderr_has_no_redraw_or_ansi_and_bounded_line_count() {
    let dir = tempfile::tempdir().expect("tempdir");
    materialize_progress_fixture(dir.path());

    let out = cmd().arg(dir.path()).output().expect("imgdedup must spawn");
    assert!(out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        !stderr.contains('\r'),
        "non-TTY stderr must never contain a carriage return (AC-22); got:\n{stderr:?}"
    );
    assert!(
        !stderr.contains('\x1b'),
        "non-TTY stderr must never contain an ANSI escape sequence (AC-22); got:\n{stderr:?}"
    );

    // Positive control: progress did produce some output (otherwise the
    // absence assertions above would pass vacuously).
    let line_count = stderr.lines().count();
    assert!(
        line_count > 0,
        "expected some non-TTY progress output for a positive control; got empty stderr"
    );

    // Bounded: coarse milestones only (phase transitions + 10%-point
    // buckets), never one line per file. Over this 6-file / 3-phase fixture
    // a correct implementation emits ~7 lines (measured); a regression that
    // logs per candidate file, per hash, or per byte would blow well past a
    // generous upper bound long before file counts got large. 20 gives
    // headroom for legitimate variance (throttling/thread scheduling)
    // without hiding a per-file-logging regression.
    assert!(
        line_count <= 20,
        "expected a small, bounded number of non-TTY progress lines, got {line_count}:\n{stderr}"
    );
}
