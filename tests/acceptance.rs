//! End-to-end acceptance suite — TASK-0312 (AC half; NFR-4 scale/benchmark is a
//! later session, out of scope here).
//!
//! Drives the **real compiled `imgdedup` binary** (via `assert_cmd`) over
//! `tempfile::tempdir()` fixtures built with the shared `tests/support`
//! helpers (`support::images`, `support::tree`, `support::oracle`). One test
//! (or a small, clearly-labeled group of tests) maps to each PRD-0001 §9
//! acceptance criterion, AC-1..AC-19 plus AC-18a and AC-20..AC-24, so the
//! mapping from requirement to verifying test is traceable without opening the
//! PRD. Each test's doc comment quotes the criterion it pins.
//!
//! ## Rules this suite follows (task brief)
//!
//! 1. Every fixture is a fresh `tempfile::tempdir()` that this suite created;
//!    no test ever points a destructive flag (`--apply`, `--delete`) at a
//!    directory it did not create itself.
//! 2. Anything relying on symlinks, hard links, or Unix permission bits that
//!    is not portable to Windows is `#[cfg(unix)]`-gated. Hard links
//!    (`std::fs::hard_link`) work on all three NFR-5 platforms (Windows NTFS
//!    supports them too — see `tests/golden_set.rs`, which uses them without a
//!    cfg gate), so AC-18/AC-18a are portable.
//! 3. Path-separator / line-ending portability: assertions on stdout use
//!    `support::oracle::run_json`'s relativized (`/`-separated) paths wherever
//!    possible, or `Path::join`/`display()` rather than hard-coded
//!    separators, mirroring `tests/cli_integration.rs`.
//!
//! ## AC-20..AC-24 (progress indication, FR-19)
//!
//! These are already covered end to end in `tests/cli_integration.rs`
//! (`progress_enabled_default_report_stdout_has_no_progress_text` /
//! `progress_enabled_json_stdout_is_single_valid_json_with_no_progress_text`
//! for AC-20, `piped_stderr_has_no_redraw_or_ansi_and_bounded_line_count` for
//! AC-22, `no_progress_flag_produces_no_progress_text_on_stderr` for AC-23,
//! `progress_on_off_default_report_stdout_is_byte_identical` /
//! `progress_on_off_json_stdout_is_byte_identical` for AC-24; AC-21's TTY path
//! is covered by `src/progress.rs` unit tests with an injected TTY flag, since
//! a real PTY is not needed to exercise the renderer logic). Not duplicated
//! here; every other test in this suite passes `--no-progress` so its
//! assertions are not perturbed by the progress stream (AC-24 already proves
//! that choice is inert).

mod support;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::PathBuf;
use support::images::{ImageFormat, image_bytes};
use support::oracle::{run_json, run_json_raw, run_multi_root_json};
use support::tree::{FileSpec, LinkSpec, materialize};

fn cmd() -> Command {
    Command::cargo_bin("imgdedup").unwrap()
}

fn spec(rel_path: &str, bytes: Vec<u8>, mtime: i64) -> FileSpec {
    FileSpec {
        rel_path: rel_path.to_owned(),
        bytes,
        mtime,
    }
}

/// Base mtime for fixtures whose timestamp is not scenario-relevant.
const T: i64 = 1_650_000_000;

// ─────────────────────────────────────────────────────────────────────────
// AC-1
// ─────────────────────────────────────────────────────────────────────────

/// AC-1 (FR-9, NFR-1): "Given a directory containing duplicate images, when
/// run with no action flag, then every duplicate group is listed and no file
/// on disk is modified, moved, or deleted."
#[test]
fn ac1_dry_run_lists_every_group_and_touches_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bytes_a = image_bytes(ImageFormat::Png, 1, 40);
    let bytes_b = image_bytes(ImageFormat::Jpeg, 2, 48);
    materialize(
        dir.path(),
        &[
            spec("a1.png", bytes_a.clone(), T),
            spec("a2.png", bytes_a.clone(), T),
            spec("b1.jpg", bytes_b.clone(), T),
            spec("b2.jpg", bytes_b.clone(), T),
        ],
        &[],
    );
    // Snapshot every file's bytes and mtime before the run so "unmodified" is
    // asserted directly, not inferred.
    let before: Vec<(PathBuf, Vec<u8>, std::time::SystemTime)> =
        ["a1.png", "a2.png", "b1.jpg", "b2.jpg"]
            .iter()
            .map(|n| {
                let p = dir.path().join(n);
                let bytes = fs::read(&p).unwrap();
                let mtime = fs::metadata(&p).unwrap().modified().unwrap();
                (p, bytes, mtime)
            })
            .collect();

    cmd()
        .arg(dir.path())
        .arg("--no-progress")
        .assert()
        .code(0)
        .stdout(predicate::str::contains("Group 1/2"))
        .stdout(predicate::str::contains("Group 2/2"))
        .stdout(predicate::str::contains("Summary: 2 duplicate groups"));

    for (path, bytes, mtime) in before {
        assert_eq!(fs::read(&path).unwrap(), bytes, "{path:?} content changed");
        assert_eq!(
            fs::metadata(&path).unwrap().modified().unwrap(),
            mtime,
            "{path:?} mtime changed"
        );
    }
    for n in ["a1.png", "a2.png", "b1.jpg", "b2.jpg"] {
        assert!(dir.path().join(n).exists(), "{n} must still exist");
    }
}

// ─────────────────────────────────────────────────────────────────────────
// AC-2
// ─────────────────────────────────────────────────────────────────────────

/// AC-2 (FR-4, FR-7): "Given three byte-identical copies of an image, when
/// scanned, then they form exactly one group with exactly one keeper chosen
/// per the deterministic rule."
#[test]
fn ac2_three_byte_identical_copies_form_exactly_one_group_one_keeper() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bytes = image_bytes(ImageFormat::Bmp, 7, 32);
    materialize(
        dir.path(),
        &[
            spec("one.bmp", bytes.clone(), T),
            spec("two.bmp", bytes.clone(), T),
            spec("three.bmp", bytes, T),
        ],
        &[],
    );

    let doc = run_json(dir.path(), &["--no-progress"]);
    let groups = doc["groups"].as_array().unwrap();
    assert_eq!(groups.len(), 1, "exactly one group; got {doc:#}");
    // One keeper (a single object, not an array) + exactly two removable —
    // together the three copies, no more, no fewer.
    assert!(groups[0]["keeper"].is_object());
    // The criterion quotes a *deterministic* rule, so pin which copy wins:
    // default `--keep shortest`, equal depth and length, lexicographic
    // tie-break. Set membership alone would pass under a random keeper.
    assert_eq!(
        groups[0]["keeper"]["path"], "one.bmp",
        "the shortest-path rule must select one.bmp; got {doc:#}"
    );
    assert_eq!(groups[0]["removable"].as_array().unwrap().len(), 2);
    let mut names: Vec<&str> = std::iter::once(groups[0]["keeper"]["path"].as_str().unwrap())
        .chain(
            groups[0]["removable"]
                .as_array()
                .unwrap()
                .iter()
                .map(|f| f["path"].as_str().unwrap()),
        )
        .collect();
    names.sort_unstable();
    assert_eq!(names, ["one.bmp", "three.bmp", "two.bmp"]);
}

// ─────────────────────────────────────────────────────────────────────────
// AC-3
// ─────────────────────────────────────────────────────────────────────────

/// AC-3 (FR-10, NFR-1): "Given a duplicate group, when run with `--apply`,
/// then the non-keepers are in the OS trash and recoverable, and the keeper
/// remains in place."
///
/// The safety half (keeper survives, unrelated files untouched, and an
/// unsuccessful trash is reported rather than silently dropped or escalated)
/// is asserted unconditionally, matching
/// `cli_integration.rs::apply_trashes_non_keepers_or_reports_why_not`
/// (trashing itself is environment-dependent by design — ADR-0009).
///
/// **Recoverability gap, disclosed:** this test additionally tries to prove
/// the trashed file is actually recoverable, using the `trash` crate's own
/// `os_limited::list()` API (our production dependency, not new test
/// machinery) — available on Linux and Windows, where it returns every
/// trashed item's original parent directory and name, letting us confirm our
/// specific file is really there. **macOS has no portable answer**: the
/// `trash` crate's own docs state `os_limited` is "on Linux and on Windows"
/// only, and there is no dependency-free, non-brittle way to enumerate
/// `~/.Trash` (or the per-volume `.Trashes/<uid>` NSFileManager may use) and
/// prove a *specific* file is the one this test trashed. On macOS this test
/// therefore only proves the safety invariants and that trashing occurred;
/// it does not independently verify recoverability. This gap is called out
/// in the tester's report rather than papered over with a brittle path guess.
#[test]
fn ac3_apply_trashes_non_keeper_recoverably_and_keeper_survives() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bytes = image_bytes(ImageFormat::Png, 31, 64);
    let other = image_bytes(ImageFormat::Png, 32, 64);
    let keeper = dir.path().join("a.png");
    let victim = dir.path().join("b.png");
    let unrelated = dir.path().join("u.png");
    fs::write(&keeper, &bytes).unwrap();
    fs::write(&victim, &bytes).unwrap();
    fs::write(&unrelated, &other).unwrap();

    let out = cmd()
        .arg(dir.path())
        .arg("--apply")
        .arg("--no-progress")
        .output()
        .expect("run");

    // Unconditional safety invariants (NFR-1).
    assert!(keeper.exists(), "the keeper must survive");
    assert!(unrelated.exists(), "an unrelated file must not be touched");

    if victim.exists() {
        // Trash unavailable in this environment: must be reported, not
        // silently ignored (ADR-0009).
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            stdout.contains("Skipped (left in place):"),
            "an untrashed file must be reported as skipped; stdout:\n{stdout}"
        );
        return;
    }

    // Trashing succeeded: try to prove it is recoverable.
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    {
        match trash::os_limited::list() {
            Ok(items) => {
                // Match on the file name plus the tempdir's own unique
                // directory component rather than comparing whole paths. The
                // parent path here comes back from the platform's trash
                // metadata, and on Windows that routinely differs textually
                // from `tempdir()`'s path (8.3 short names like RUNNER~1, or a
                // `\\?\` prefix) while pointing at the same directory. Whole-path
                // equality would turn this into a red Windows job for a reason
                // that has nothing to do with the tool.
                let unique = dir
                    .path()
                    .file_name()
                    .expect("tempdir has a final component")
                    .to_owned();
                let found = items.iter().any(|it| {
                    // Compare the *stem*, not the full name: the Windows
                    // Recycle Bin reports an item's display name, which hides
                    // known extensions, so "b.png" comes back as "b". The
                    // unique tempdir component is what keeps this specific.
                    std::path::Path::new(&it.name).file_stem() == Some(std::ffi::OsStr::new("b"))
                        && it.original_parent.file_name() == Some(unique.as_os_str())
                });
                assert!(
                    found,
                    "b.png was trashed but not found via trash::os_limited::list(); items: {:?}",
                    items
                        .iter()
                        .map(|i| (i.original_parent.clone(), i.name.clone()))
                        .collect::<Vec<_>>()
                );
            }
            Err(e) => {
                // The trash backend could not be enumerated here. That is the
                // environment variance ADR-0009 already expects, so it is not
                // by itself a failure — but if *every* platform degrades to
                // this branch, the "trash silently became a delete" defect
                // class is covered nowhere, and nobody finds out. CI sets
                // IMGDEDUP_REQUIRE_TRASH_CHECK so at least one matrix job is
                // proven to have executed the assertion.
                if std::env::var_os("IMGDEDUP_REQUIRE_TRASH_CHECK").is_some() {
                    panic!(
                        "AC-3 recoverability could not be verified and this job \
                         requires it: trash::os_limited::list() failed: {e}"
                    );
                }
                eprintln!(
                    "note: trash::os_limited::list() unavailable ({e}); \
                     AC-3 recoverability not verified in this run"
                );
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        eprintln!(
            "note: macOS has no trash::os_limited support (see this test's \
             doc comment); AC-3 recoverability is not independently verified \
             on this platform by this suite"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────
// AC-4
// ─────────────────────────────────────────────────────────────────────────

/// AC-4 (FR-3, NFR-2): "Given a file named `photo.jpg` that is actually plain
/// text, when scanned, then it is never grouped with images and never
/// trashed or deleted."
#[test]
fn ac4_mislabeled_text_file_is_never_grouped_or_removed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let real = image_bytes(ImageFormat::Jpeg, 9, 80);
    // Deliberately the same size as the real pair, so a bug that grouped by
    // size alone (skipping magic-byte confirmation) would be caught too.
    let fake: Vec<u8> = b"just plain text, not an image at all!! "
        .iter()
        .copied()
        .cycle()
        .take(80)
        .collect();
    materialize(
        dir.path(),
        &[
            spec("real1.jpg", real.clone(), T),
            spec("real2.jpg", real, T),
            spec("photo.jpg", fake.clone(), T),
        ],
        &[],
    );
    let photo_path = dir.path().join("photo.jpg");

    let out = cmd()
        .arg(dir.path())
        .arg("--apply")
        .arg("--delete")
        .arg("--no-progress")
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);

    // Positive control: the real pair really was scanned and one copy really
    // was removed, so the absence of photo.jpg from that removal cannot pass
    // vacuously because nothing happened at all.
    assert!(
        stdout.contains("Deleted 1 file:") && stdout.contains("real2.jpg"),
        "expected the genuine duplicate to be removed as a positive control; stdout:\n{stdout}"
    );

    // photo.jpg is untouched, unchanged, and never removed.
    assert_eq!(
        fs::read(&photo_path).unwrap(),
        fake,
        "photo.jpg content changed"
    );
    // Assert on the JSON document, not on report prose. The previous stdout
    // checks here could not fail: the report always emits a "Warnings:" header
    // when any warning exists, and it prints absolute paths, so a literal
    // "REMOVE photo.jpg" was unsatisfiable regardless of behavior.
    let doc = run_json(dir.path(), &["--no-progress"]);
    for group in doc["groups"].as_array().expect("groups") {
        let mut paths = vec![group["keeper"]["path"].as_str().unwrap_or_default()];
        for f in group["removable"].as_array().expect("removable") {
            paths.push(f["path"].as_str().unwrap_or_default());
        }
        for f in group["protected"].as_array().expect("protected") {
            paths.push(f["path"].as_str().unwrap_or_default());
        }
        assert!(
            !paths.iter().any(|p| p.ends_with("photo.jpg")),
            "a mislabeled non-image must never appear in any group; group: {group:#}"
        );
    }

    // It is reported as a warning, not silently dropped (NFR-6/FR-3).
    let doc: Value = serde_json::from_slice(
        &cmd()
            .arg(dir.path())
            .arg("--json")
            .arg("--no-progress")
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let warnings = doc["warnings"].as_array().unwrap();
    assert!(
        warnings
            .iter()
            .any(|w| w["path"].as_str().unwrap().ends_with("photo.jpg") && w["stage"] == "sniff"),
        "photo.jpg must be reported as a sniff-stage warning; got: {warnings:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// AC-5
// ─────────────────────────────────────────────────────────────────────────

/// AC-5 (FR-12): "Given a tree already deduplicated, when run again with
/// `--apply`, then zero actions are taken and nothing changes."
#[test]
fn ac5_second_apply_run_on_a_deduplicated_tree_is_a_no_op() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bytes = image_bytes(ImageFormat::Gif89a, 5, 24);
    materialize(
        dir.path(),
        &[spec("a.gif", bytes.clone(), T), spec("b.gif", bytes, T)],
        &[],
    );
    let keeper = dir.path().join("a.gif");

    cmd()
        .arg(dir.path())
        .arg("--apply")
        .arg("--delete")
        .arg("--no-progress")
        .assert()
        .code(0);
    assert!(!dir.path().join("b.gif").exists());
    let keeper_bytes_after_first = fs::read(&keeper).unwrap();

    cmd()
        .arg(dir.path())
        .arg("--apply")
        .arg("--delete")
        .arg("--no-progress")
        .assert()
        .code(0)
        .stdout(predicate::str::contains("No duplicate groups found."))
        .stdout(predicate::str::contains("Deleted 0 files."));

    assert!(keeper.exists(), "the keeper still stands");
    assert_eq!(
        fs::read(&keeper).unwrap(),
        keeper_bytes_after_first,
        "unchanged"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// AC-6
// ─────────────────────────────────────────────────────────────────────────

/// AC-6 (FR-8): "Given `--keep newest`, when a group is resolved, then the
/// file with the newest modification time is the keeper."
#[test]
fn ac6_keep_newest_selects_the_newest_mtime_as_keeper() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bytes = image_bytes(ImageFormat::Webp, 11, 40);
    materialize(
        dir.path(),
        &[
            spec("old.webp", bytes.clone(), T),
            spec("mid.webp", bytes.clone(), T),
            spec("new.webp", bytes, T + 1000),
        ],
        &[],
    );

    let doc = run_json(dir.path(), &["--keep", "newest", "--no-progress"]);
    let groups = doc["groups"].as_array().unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(
        groups[0]["keeper"]["path"], "new.webp",
        "the newest-mtime file must be the keeper; got {doc:#}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// AC-7
// ─────────────────────────────────────────────────────────────────────────

/// AC-7 (FR-11): "Given `--apply --delete`, when run on a duplicate group,
/// then the non-keepers are permanently removed and are not present in the
/// trash."
#[test]
fn ac7_apply_delete_permanently_removes_non_keepers_not_via_trash() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bytes = image_bytes(ImageFormat::TiffLe, 13, 32);
    materialize(
        dir.path(),
        &[spec("a.tif", bytes.clone(), T), spec("b.tif", bytes, T)],
        &[],
    );
    let keeper = dir.path().join("a.tif");
    let victim = dir.path().join("b.tif");

    cmd()
        .arg(dir.path())
        .arg("--apply")
        .arg("--delete")
        .arg("--no-progress")
        .assert()
        .code(0)
        .stdout(predicate::str::contains("Deleted 1 file:"))
        .stdout(predicate::str::contains("b.tif"));

    assert!(keeper.exists());
    assert!(!victim.exists(), "permanently gone, not relocated");

    // "not present in the trash": on Linux/Windows the trash crate's own
    // listing API confirms it directly rather than merely inferring absence
    // from a missing file (which could also mean "trashed to somewhere this
    // suite does not know to look").
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    {
        if let Ok(items) = trash::os_limited::list() {
            // Same unique-component match as AC-3: whole-path equality against
            // platform trash metadata is unreliable on Windows, and here a
            // textual mismatch would silently turn a negative assertion into a
            // vacuous one rather than failing loudly.
            let unique = dir
                .path()
                .file_name()
                .expect("tempdir has a final component")
                .to_owned();
            assert!(
                !items.iter().any(|it| {
                    // Stem, not full name — see AC-3: Windows reports display
                    // names with known extensions hidden.
                    std::path::Path::new(&it.name).file_stem() == Some(std::ffi::OsStr::new("b"))
                        && it.original_parent.file_name() == Some(unique.as_os_str())
                }),
                "a permanently deleted file must not appear in the trash listing"
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
// AC-8
// ─────────────────────────────────────────────────────────────────────────

/// AC-8 (FR-14): "Given `--json`, when run, then output is valid JSON
/// containing each group, its keeper, the intended or performed actions, and
/// totals."
#[test]
fn ac8_json_output_contains_groups_keeper_actions_and_totals() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bytes = image_bytes(ImageFormat::Png, 17, 40);
    materialize(
        dir.path(),
        &[spec("a.png", bytes.clone(), T), spec("b.png", bytes, T)],
        &[],
    );

    let doc = run_json(dir.path(), &["--no-progress"]);
    assert_eq!(doc["schema_version"], 1);
    assert_eq!(doc["mode"], "dry-run");
    let groups = doc["groups"].as_array().expect("groups array");
    assert_eq!(groups.len(), 1);
    assert!(groups[0]["keeper"]["path"].is_string());
    assert!(groups[0]["removable"].is_array());
    assert!(doc["actions"]["removed"].is_array());
    assert!(doc["actions"]["skipped"].is_array());
    let totals = &doc["totals"];
    assert_eq!(totals["duplicate_groups"], 1);
    assert_eq!(totals["removable_files"], 1);
    assert_eq!(totals["reclaimable_bytes"], 40);
}

// ─────────────────────────────────────────────────────────────────────────
// AC-9
// ─────────────────────────────────────────────────────────────────────────

/// AC-9 (FR-5): "Given a candidate set, when scanned, then the groups
/// produced with the size pre-grouping optimization are identical to those
/// produced by hashing every candidate."
///
/// Observable claim (per the task brief): files of a unique size never
/// group, and grouping within a shared-size cohort matches exactly what
/// content-byte equality alone would produce. This test builds several
/// same-size cohorts, each containing one genuine duplicate pair plus one
/// same-size-but-different-content singleton, computes the expected grouping
/// by **raw content-byte equality** independently of the product (a trivial
/// `HashMap<Vec<u8>, …>` bucketing done here in the test, not by invoking any
/// `imgdedup` code), and asserts the real `--json` output's group membership
/// is *exactly* that — proving the pre-grouping optimization changed nothing
/// about the result.
#[test]
fn ac9_size_pregrouping_produces_exactly_the_content_equality_groups() {
    let dir = tempfile::tempdir().expect("tempdir");
    let sizes: [u64; 4] = [16, 24, 32, 48];
    let mut files = Vec::new();
    // relpath -> raw content, so the expected grouping can be derived by pure
    // content-byte equality, independent of the product.
    let mut content_of: HashMap<String, Vec<u8>> = HashMap::new();

    for (i, &size) in sizes.iter().enumerate() {
        let pair_bytes = image_bytes(ImageFormat::Png, (10 + i) as u8, size as usize);
        let singleton_bytes = image_bytes(ImageFormat::Png, (200 + i) as u8, size as usize);
        for (name, bytes) in [
            (format!("s{i}/pair_a.png"), pair_bytes.clone()),
            (format!("s{i}/pair_b.png"), pair_bytes),
            (format!("s{i}/singleton.png"), singleton_bytes),
        ] {
            content_of.insert(name.clone(), bytes.clone());
            files.push(spec(&name, bytes, T));
        }
    }
    materialize(dir.path(), &files, &[]);

    // Expected grouping: bucket relpaths by raw content; any bucket with 2+
    // members is an expected duplicate group (singletons never appear).
    let mut buckets: HashMap<&Vec<u8>, BTreeSet<String>> = HashMap::new();
    for (name, bytes) in &content_of {
        buckets.entry(bytes).or_default().insert(name.clone());
    }
    let mut expected: Vec<BTreeSet<String>> = buckets
        .into_values()
        .filter(|members| members.len() >= 2)
        .collect();
    expected.sort();
    assert_eq!(
        expected.len(),
        sizes.len(),
        "one pair expected per size cohort"
    );

    let doc = run_json(dir.path(), &["--no-progress"]);
    let mut actual: Vec<BTreeSet<String>> = doc["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| {
            let mut set: BTreeSet<String> = BTreeSet::new();
            set.insert(g["keeper"]["path"].as_str().unwrap().to_owned());
            for r in g["removable"].as_array().unwrap() {
                set.insert(r["path"].as_str().unwrap().to_owned());
            }
            set
        })
        .collect();
    actual.sort();

    assert_eq!(
        actual, expected,
        "the real grouping must exactly match content-byte-equality grouping \
         (no unique-size file ever grouped; no same-size-different-content \
         file grouped; every genuine same-content pair grouped)"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// AC-10
// ─────────────────────────────────────────────────────────────────────────

/// AC-10 (NFR-6): "Given an unreadable file in the scan path, when run, then
/// the file is reported and skipped and the run completes successfully."
///
/// Unix-only: an unreadable *file* (as opposed to an unreadable directory,
/// already covered by `enumerate.rs`'s unit tests) is simulated with `chmod
/// 000`, which has no portable Windows equivalent.
#[cfg(unix)]
#[test]
fn ac10_unreadable_file_is_reported_and_skipped_and_the_run_still_succeeds() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("tempdir");
    let bytes = image_bytes(ImageFormat::Png, 19, 40);
    materialize(
        dir.path(),
        &[
            spec("good1.png", bytes.clone(), T),
            spec("good2.png", bytes, T),
            spec("locked.png", image_bytes(ImageFormat::Png, 20, 40), T),
        ],
        &[],
    );
    let locked = dir.path().join("locked.png");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();

    let out = cmd()
        .arg(dir.path())
        .arg("--no-progress")
        .output()
        .expect("run");

    fs::set_permissions(&locked, fs::Permissions::from_mode(0o644)).unwrap();

    assert!(
        out.status.success(),
        "the run must complete successfully despite the unreadable file; \
         stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Positive control: the readable duplicate pair was still found.
    assert!(
        stdout.contains("Summary: 1 duplicate group"),
        "expected the genuine pair to still be found; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("locked.png"),
        "the unreadable file must be reported, not silently dropped; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("Warnings:"),
        "the report must show a Warnings section; stdout:\n{stdout}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// AC-11
// ─────────────────────────────────────────────────────────────────────────

/// AC-11 (FR-9 → FR-10 parity): "Given a tree left unchanged between runs,
/// the groups and intended removals reported by a dry run match what a
/// subsequent `--apply` performs."
///
/// A tighter, CLI-level re-assertion of `cli_integration.rs`'s
/// `the_dry_run_predicts_exactly_what_apply_removes`, using a multi-group
/// tree so the comparison is not trivially a single-file coincidence.
#[test]
fn ac11_dry_run_intended_removals_match_what_apply_actually_removes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let a = image_bytes(ImageFormat::Png, 21, 32);
    let b = image_bytes(ImageFormat::Jpeg, 22, 40);
    materialize(
        dir.path(),
        &[
            spec("a1.png", a.clone(), T),
            spec("a2.png", a.clone(), T),
            spec("a3.png", a, T),
            spec("b1.jpg", b.clone(), T),
            spec("b2.jpg", b, T),
        ],
        &[],
    );

    let dry = run_json(dir.path(), &["--no-progress"]);
    assert_eq!(dry["mode"], "dry-run");
    let mut intended: Vec<String> = dry["groups"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|g| g["removable"].as_array().unwrap())
        .map(|f| f["path"].as_str().unwrap().to_owned())
        .collect();
    intended.sort();

    let applied = run_json(dir.path(), &["--apply", "--delete", "--no-progress"]);
    assert_eq!(applied["mode"], "delete");
    let mut removed: Vec<String> = applied["actions"]["removed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap().to_owned())
        .collect();
    removed.sort();

    assert_eq!(intended.len(), 3, "sanity: 3 removable across two groups");
    assert_eq!(intended, removed);
}

// ─────────────────────────────────────────────────────────────────────────
// AC-12
// ─────────────────────────────────────────────────────────────────────────

/// AC-12 (FR-15, §8): "Given `--delete` without `--apply`, when run, then the
/// tool exits non-zero with a clear error and removes nothing; given a
/// successful run it exits `0`, and given a run-level failure it exits with a
/// distinct non-zero code."
#[test]
fn ac12_exit_code_mapping_error_success_and_run_level_failure() {
    // `--delete` without `--apply`: non-zero, clear error, removes nothing.
    // Exit 2 (usage error, TASK-0323, owner ruling 2026-08-03): a refused
    // flag *combination* is a usage error, not a run-level one.
    let dir = tempfile::tempdir().expect("tempdir");
    let bytes = image_bytes(ImageFormat::Png, 23, 32);
    materialize(
        dir.path(),
        &[spec("a.png", bytes.clone(), T), spec("b.png", bytes, T)],
        &[],
    );
    cmd()
        .arg(dir.path())
        .arg("--delete")
        .arg("--no-progress")
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("--apply"));
    assert!(dir.path().join("a.png").exists());
    assert!(dir.path().join("b.png").exists(), "nothing removed");

    // A successful run (no findings needed) exits 0.
    let empty = tempfile::tempdir().expect("tempdir");
    cmd()
        .arg(empty.path())
        .arg("--no-progress")
        .assert()
        .code(0);

    // A run-level failure (unreadable/nonexistent scan root) exits a distinct
    // non-zero code (1) — distinct from 0 above and from clap's 2 / the
    // opt-in 3.
    cmd()
        .arg("/definitely/does/not/exist/imgdedup-ac12")
        .arg("--no-progress")
        .assert()
        .failure()
        .code(1);
}

// ─────────────────────────────────────────────────────────────────────────
// AC-13
// ─────────────────────────────────────────────────────────────────────────

/// AC-13 (FR-13): "Given duplicate groups, when run, then the summary
/// reports each group's keeper and removed files, the group and file counts,
/// and the reclaimable space."
#[test]
fn ac13_summary_reports_keeper_removed_files_counts_and_reclaimable_space() {
    let dir = tempfile::tempdir().expect("tempdir");
    let a = image_bytes(ImageFormat::Png, 25, 16); // 16 bytes
    let b = image_bytes(ImageFormat::Jpeg, 26, 24); // 24 bytes
    materialize(
        dir.path(),
        &[
            spec("keep-a.png", a.clone(), T),
            spec("remove-a.png", a, T),
            spec("keep-b.jpg", b.clone(), T),
            spec("remove-b.jpg", b, T),
        ],
        &[],
    );

    let out = cmd().arg(dir.path()).arg("--no-progress").output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(
        stdout.contains("KEEP"),
        "keeper must be labeled; got:\n{stdout}"
    );
    assert!(
        stdout.contains("REMOVE"),
        "removed files must be labeled; got:\n{stdout}"
    );
    // Assert the *association*, not just that both labels appear somewhere:
    // swapping the KEEP and REMOVE labels in the renderer would pass a check
    // that only looked for the words. The keeper is deterministic here under
    // the default `shortest` rule.
    assert!(
        stdout.contains(&format!(
            "KEEP  {}",
            dir.path().join("keep-a.png").display()
        )),
        "keep-a.png must be labeled KEEP; got:\n{stdout}"
    );
    assert!(
        stdout.contains(&format!(
            "REMOVE {}",
            dir.path().join("remove-a.png").display()
        )),
        "remove-a.png must be labeled REMOVE; got:\n{stdout}"
    );
    // Group + file counts.
    assert!(
        stdout.contains("Summary: 2 duplicate groups, 4 files"),
        "expected group/file counts in the summary; got:\n{stdout}"
    );
    // Reclaimable space: 16 (remove-a) + 24 (remove-b) = 40 bytes.
    assert!(
        stdout.contains("40 bytes reclaimable"),
        "expected total reclaimable bytes; got:\n{stdout}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// AC-14
// ─────────────────────────────────────────────────────────────────────────

/// AC-14 (FR-6, NFR-2), hidden half: "Given a directory containing … a hidden
/// file, when run with defaults, then … [it is not] included; when run with
/// `--hidden`, then [it is] included."
#[test]
fn ac14_hidden_files_excluded_by_default_included_with_hidden_flag() {
    let dir = tempfile::tempdir().expect("tempdir");
    let visible = image_bytes(ImageFormat::Png, 27, 32);
    let hidden = image_bytes(ImageFormat::Png, 28, 40);
    materialize(
        dir.path(),
        &[
            spec("visible1.png", visible.clone(), T),
            spec("visible2.png", visible, T),
            spec(".hidden1.png", hidden.clone(), T),
            spec(".hidden2.png", hidden, T),
        ],
        &[],
    );

    let default_doc = run_json(dir.path(), &["--no-progress"]);
    assert_eq!(
        default_doc["totals"]["duplicate_groups"], 1,
        "hidden pair excluded by default"
    );
    // Membership, not just the count: an inverted filter that included the
    // hidden pair and excluded the visible one would also yield exactly 1.
    let default_paths: Vec<&str> = default_doc["groups"][0]["removable"]
        .as_array()
        .expect("removable")
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .chain(std::iter::once(
            default_doc["groups"][0]["keeper"]["path"].as_str().unwrap(),
        ))
        .collect();
    assert!(
        default_paths.iter().all(|p| p.starts_with("visible")),
        "the default group must be the visible pair, not the hidden one; got {default_paths:?}"
    );

    let hidden_doc = run_json(dir.path(), &["--hidden", "--no-progress"]);
    assert_eq!(
        hidden_doc["totals"]["duplicate_groups"], 2,
        "--hidden includes both pairs"
    );
}

/// AC-14 (FR-6, NFR-2), symlink half: "Given a directory containing a
/// symlink …, when run with defaults, then [it is] not followed …; when run
/// with `--follow-symlinks`, then [it is] included."
///
/// Unix-only: symlink creation via `std::os::unix::fs::symlink`; the
/// equivalent on Windows requires elevated privileges in many configurations
/// and is already covered non-destructively at the `enumerate.rs` unit level
/// for that platform.
#[cfg(unix)]
#[test]
fn ac14_symlinks_not_followed_by_default_followed_with_flag() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().expect("tempdir");
    let outside = tempfile::tempdir().expect("outside tempdir");
    let content = image_bytes(ImageFormat::Png, 29, 40);

    let real = dir.path().join("real.png");
    fs::write(&real, &content).unwrap();
    let target = outside.path().join("target.png");
    fs::write(&target, &content).unwrap();
    let link = dir.path().join("link.png");
    symlink(&target, &link).unwrap();

    let default_doc = run_json(dir.path(), &["--no-progress"]);
    assert_eq!(
        default_doc["totals"]["duplicate_groups"], 0,
        "the symlink target must not be followed by default; got {default_doc:#}"
    );

    let followed_doc = run_json(dir.path(), &["--follow-symlinks", "--no-progress"]);
    assert_eq!(
        followed_doc["totals"]["duplicate_groups"], 1,
        "got {followed_doc:#}"
    );
    let members: BTreeSet<&str> = std::iter::once(
        followed_doc["groups"][0]["keeper"]["path"]
            .as_str()
            .unwrap(),
    )
    .chain(
        followed_doc["groups"][0]["removable"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["path"].as_str().unwrap()),
    )
    .collect();
    assert_eq!(members, BTreeSet::from(["real.png", "link.png"]));
}

/// AC-14a (FR-6, FR-13, FR-17, TASK-0320): "Given `--follow-symlinks` and a
/// tree where one physical file is reachable by two paths (one through a
/// symlinked ancestor), when scanned, then the file appears exactly once, at
/// its non-symlink-routed path; it is not reported as a hard-link set; and a
/// genuine byte-identical copy elsewhere in the tree is still reported as
/// removable with its true reclaimable size."
///
/// Reproduces the exact defect found while investigating this task: without
/// the fix, the symlink-routed alias was mislabeled as a hard-link set and,
/// worse, protected a genuinely reclaimable duplicate as though it were an
/// unremovable anchor — reporting 0 reclaimable bytes when there really are
/// `content.len()` of them.
#[cfg(unix)]
#[test]
fn ac14a_follow_symlinks_alias_is_counted_once_and_never_masks_a_real_duplicate() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().expect("tempdir");
    let content = image_bytes(ImageFormat::Png, 31, 486);

    materialize(
        dir.path(),
        &[
            spec("real_dir/photo.png", content.clone(), T),
            // A genuine second physical copy elsewhere in the tree — this is
            // the real, reclaimable duplicate the fix must still find.
            spec("genuine_dup.png", content, T),
        ],
        &[],
    );
    // A symlinked ancestor directory gives real_dir/photo.png a second route:
    // link_dir/photo.png. Same physical file, not a second copy.
    symlink(dir.path().join("real_dir"), dir.path().join("link_dir")).unwrap();

    let doc = run_json(dir.path(), &["--follow-symlinks", "--no-progress"]);

    assert_eq!(
        doc["already_deduplicated"].as_array().unwrap().len(),
        0,
        "the alias must not be reported as a hard-link set; got {doc:#}"
    );
    assert_eq!(
        doc["groups"].as_array().unwrap().len(),
        1,
        "the genuine duplicate must still form exactly one group; got {doc:#}"
    );
    let group = &doc["groups"][0];
    assert_eq!(
        group["hard_link_sets"].as_array().unwrap().len(),
        0,
        "no hard-link set belongs to this group; got {doc:#}"
    );
    assert_eq!(
        group["removable"].as_array().unwrap().len(),
        1,
        "exactly one file must be removable: the real duplicate; got {doc:#}"
    );
    assert_eq!(
        group["reclaimable_bytes"], 486,
        "the 486 real bytes must be reported reclaimable, not masked as 0; got {doc:#}"
    );

    // Every path named anywhere in the group must be the non-symlink-routed
    // one — link_dir/photo.png must never appear.
    let mut all_paths: Vec<&str> = vec![group["keeper"]["path"].as_str().unwrap()];
    all_paths.extend(
        group["removable"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["path"].as_str().unwrap()),
    );
    assert!(
        all_paths.iter().all(|p| !p.starts_with("link_dir")),
        "the symlink-routed alias must never appear in the report; got {all_paths:?}"
    );
    assert_eq!(
        BTreeSet::from_iter(all_paths),
        BTreeSet::from(["real_dir/photo.png", "genuine_dup.png"]),
    );
}

/// AC-14b (FR-6, FR-13, FR-17; TASK-0329): "Given `--follow-symlinks` and two
/// or more scan roots, where a symlink under one root gives a physical file
/// under another root a second route, when scanned, then that file appears
/// exactly once, at its non-symlink-routed path; it is not reported as a
/// hard-link set; and a genuine byte-identical copy elsewhere in the scan is
/// still reported as removable with its true reclaimable size."
///
/// Reproduces the exact defect TASK-0320's own architect review found left
/// open by its root-scoped fix: `A/l -> B/deep` gives `B/deep/photo.png` a
/// second route via root `A`, and the anchor rule protected both aliased
/// representatives, masking a genuine duplicate (`A/g.png`) as unreclaimable.
#[cfg(unix)]
#[test]
fn ac14b_follow_symlinks_alias_across_two_roots_is_counted_once_and_never_masks_a_real_duplicate() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().expect("tempdir");
    let content = image_bytes(ImageFormat::Png, 31, 498);

    materialize(
        dir.path(),
        &[
            // A genuine second physical copy, under root A — the real,
            // reclaimable duplicate the fix must still find.
            spec("A/g.png", content.clone(), T),
            spec("B/deep/photo.png", content, T),
        ],
        &[],
    );
    // A symlink under root A gives B/deep/photo.png a second route,
    // crossing from one scan root into another.
    symlink(dir.path().join("B/deep"), dir.path().join("A/l")).unwrap();

    let root_a = dir.path().join("A");
    let root_b = dir.path().join("B");
    let doc = run_multi_root_json(&[&root_a, &root_b], &["--follow-symlinks", "--no-progress"]);

    assert_eq!(
        doc["already_deduplicated"].as_array().unwrap().len(),
        0,
        "the cross-root alias must not be reported as a hard-link set; got {doc:#}"
    );
    assert_eq!(
        doc["groups"].as_array().unwrap().len(),
        1,
        "the genuine duplicate must still form exactly one group; got {doc:#}"
    );
    let group = &doc["groups"][0];
    assert_eq!(
        group["hard_link_sets"].as_array().unwrap().len(),
        0,
        "no hard-link set belongs to this group; got {doc:#}"
    );
    assert_eq!(
        group["removable"].as_array().unwrap().len(),
        1,
        "exactly one file must be removable: the real duplicate; got {doc:#}"
    );
    assert_eq!(
        group["reclaimable_bytes"], 498,
        "the 498 real bytes must be reported reclaimable, not masked as 0; got {doc:#}"
    );

    let mut all_paths: Vec<String> = vec![group["keeper"]["path"].as_str().unwrap().to_string()];
    all_paths.extend(
        group["removable"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["path"].as_str().unwrap().to_string()),
    );
    let symlink_routed = root_a.join("l/photo.png").to_str().unwrap().to_string();
    assert!(
        all_paths.iter().all(|p| *p != symlink_routed),
        "the symlink-routed alias must never appear in the report; got {all_paths:?}"
    );
    assert_eq!(
        BTreeSet::from_iter(all_paths),
        BTreeSet::from([
            root_a.join("g.png").to_str().unwrap().to_string(),
            root_b.join("deep/photo.png").to_str().unwrap().to_string(),
        ]),
    );
}

/// AC-14b / NFR-3 (AC-16): the cross-root alias result — including which
/// physical path survives — does not depend on the order the roots are
/// given on the command line.
#[cfg(unix)]
#[test]
fn ac14b_cross_root_alias_result_is_independent_of_root_argument_order() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().expect("tempdir");
    let content = image_bytes(ImageFormat::Png, 31, 498);

    materialize(
        dir.path(),
        &[
            spec("A/g.png", content.clone(), T),
            spec("B/deep/photo.png", content, T),
        ],
        &[],
    );
    symlink(dir.path().join("B/deep"), dir.path().join("A/l")).unwrap();

    let root_a = dir.path().join("A");
    let root_b = dir.path().join("B");
    let extra = ["--follow-symlinks", "--no-progress"];

    let forward = run_multi_root_json(&[&root_a, &root_b], &extra);
    let backward = run_multi_root_json(&[&root_b, &root_a], &extra);

    assert_eq!(
        forward, backward,
        "root argument order must not change the result"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// AC-15
// ─────────────────────────────────────────────────────────────────────────

/// AC-15 (FR-1): "Given duplicates nested in subdirectories, when run with
/// defaults they are found; when run with `--no-recurse` only top-level files
/// of each path are considered."
#[test]
fn ac15_recursion_finds_nested_duplicates_no_recurse_limits_to_top_level() {
    let dir = tempfile::tempdir().expect("tempdir");
    let top = image_bytes(ImageFormat::Png, 33, 32);
    let nested = image_bytes(ImageFormat::Png, 34, 40);
    materialize(
        dir.path(),
        &[
            spec("top_a.png", top.clone(), T),
            spec("top_b.png", top, T),
            spec("sub/deep/nested_a.png", nested.clone(), T),
            spec("sub/deep/nested_b.png", nested, T),
        ],
        &[],
    );

    let default_doc = run_json(dir.path(), &["--no-progress"]);
    assert_eq!(
        default_doc["totals"]["duplicate_groups"], 2,
        "both pairs found by default"
    );

    let no_recurse_doc = run_json(dir.path(), &["--no-recurse", "--no-progress"]);
    assert_eq!(
        no_recurse_doc["totals"]["duplicate_groups"], 1,
        "only the top-level pair"
    );
    assert_eq!(
        no_recurse_doc["groups"][0]["keeper"]["path"]
            .as_str()
            .unwrap(),
        "top_a.png"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// AC-16
// ─────────────────────────────────────────────────────────────────────────

/// AC-16 (NFR-3): "Given the same tree and flags, two separate runs produce
/// identical groups and keeper choices."
#[test]
fn ac16_two_runs_over_the_same_tree_produce_byte_identical_stdout() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut files = Vec::new();
    for (i, fmt) in [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::Bmp]
        .into_iter()
        .enumerate()
    {
        let bytes = image_bytes(fmt, (40 + i) as u8, 32);
        files.push(spec(
            &format!("g{i}/a.{}", fmt.extension()),
            bytes.clone(),
            T,
        ));
        files.push(spec(&format!("g{i}/b.{}", fmt.extension()), bytes, T));
    }
    materialize(dir.path(), &files, &[]);

    // Default report mode.
    let first_report = cmd().arg(dir.path()).arg("--no-progress").output().unwrap();
    let second_report = cmd().arg(dir.path()).arg("--no-progress").output().unwrap();
    assert!(!first_report.stdout.is_empty());
    assert_eq!(first_report.stdout, second_report.stdout);

    // --json mode (also exercised via the raw byte comparison helper).
    let first_json = run_json_raw(dir.path(), &["--no-progress"]);
    let second_json = run_json_raw(dir.path(), &["--no-progress"]);
    assert!(!first_json.stdout.is_empty());
    assert_eq!(first_json.stdout, second_json.stdout);
}

// ─────────────────────────────────────────────────────────────────────────
// AC-17
// ─────────────────────────────────────────────────────────────────────────

/// AC-17 (FR-16, NFR-6): "Given a non-keeper that is modified or replaced
/// between the scan and `--apply`, when applied, then that file is skipped
/// with a warning and not removed, and the remaining groups are still
/// applied normally."
///
/// A genuine scan-then-mutate race cannot be driven from outside the process
/// (the mutation would have to land in the gap between two internal pipeline
/// stages); per-branch TOCTOU coverage for that lives in `src/apply.rs`'s
/// unit tests (`skips_a_candidate_whose_content_changed_since_the_scan` and
/// neighbors). What *is* reachable end to end is a removal that fails
/// outright: a read-only parent directory on Unix makes `remove_file` fail
/// while the scan (which only needs read access) still succeeds — and the
/// spec's "the remaining groups are still applied normally" clause is
/// asserted by including an independent, writable second group in the same
/// run.
#[cfg(unix)]
#[test]
fn ac17_a_failed_removal_is_reported_and_survives_other_groups_still_apply() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("tempdir");
    let locked_bytes = image_bytes(ImageFormat::Png, 41, 32);
    let ok_bytes = image_bytes(ImageFormat::Png, 42, 40);

    let keeper_locked = dir.path().join("keep.png");
    let locked_subdir = dir.path().join("locked");
    fs::create_dir(&locked_subdir).unwrap();
    let victim_locked = locked_subdir.join("dup.png");
    fs::write(&keeper_locked, &locked_bytes).unwrap();
    fs::write(&victim_locked, &locked_bytes).unwrap();

    // Equal path length and segment count, so the deterministic keeper rule's
    // final lexicographic tie-break decides unambiguously: "ok1.png" keeps,
    // "ok2.png" is removed (ADR-0008 total order) — asserted directly below
    // rather than assumed.
    let keeper_ok = dir.path().join("ok1.png");
    let victim_ok = dir.path().join("ok2.png");
    fs::write(&keeper_ok, &ok_bytes).unwrap();
    fs::write(&victim_ok, &ok_bytes).unwrap();

    fs::set_permissions(&locked_subdir, fs::Permissions::from_mode(0o555)).unwrap();

    let out = cmd()
        .arg(dir.path())
        .arg("--apply")
        .arg("--delete")
        .arg("--no-progress")
        .output()
        .expect("run");

    fs::set_permissions(&locked_subdir, fs::Permissions::from_mode(0o755)).unwrap();

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        victim_locked.exists(),
        "the removal that failed must leave the file in place; stdout:\n{stdout}"
    );
    assert!(keeper_locked.exists());
    assert!(
        stdout.contains("removal failed"),
        "the skip reason must say the removal failed; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("Skipped (left in place):"),
        "must be reported as a skip with a warning; stdout:\n{stdout}"
    );

    // The independent, writable group is still applied normally.
    assert!(
        !victim_ok.exists(),
        "the unrelated group's duplicate must still be removed"
    );
    assert!(
        keeper_ok.exists(),
        "ok1.png is the deterministic keeper (lexicographic tie-break)"
    );
    assert!(
        stdout.contains("Deleted 1 file:") && stdout.contains("ok2.png"),
        "the successful removal must still be reported; stdout:\n{stdout}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// AC-18 / AC-18a
// ─────────────────────────────────────────────────────────────────────────

/// AC-18 (FR-17): "Given two hard links to the same content, when scanned,
/// then neither is proposed for removal and they do not contribute to
/// reclaimable space."
///
/// Portable (no `cfg(unix)` gate): `std::fs::hard_link` works on all three
/// NFR-5 platforms (Windows NTFS supports hard links too), matching
/// `tests/golden_set.rs`'s hard-link scenarios, which are likewise
/// ungated.
#[test]
fn ac18_hard_linked_siblings_are_never_removable_and_excluded_from_reclaimable() {
    let dir = tempfile::tempdir().expect("tempdir");
    let anchored = image_bytes(ImageFormat::Png, 51, 32);
    let separate_copy = anchored.clone();
    materialize(
        dir.path(),
        &[
            spec("h/h1.png", anchored, T),
            spec("copy.png", separate_copy, T),
        ],
        &[LinkSpec {
            existing: "h/h1.png".to_owned(),
            new: "h/h2.png".to_owned(),
        }],
    );

    let doc = run_json(dir.path(), &["--no-progress"]);
    let groups = doc["groups"].as_array().unwrap();
    assert_eq!(
        groups.len(),
        1,
        "the anchor set + the separate copy form one group"
    );
    let group = &groups[0];

    let removable: Vec<&str> = group["removable"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();
    assert!(
        !removable.contains(&"h/h1.png") && !removable.contains(&"h/h2.png"),
        "neither hard-linked sibling may ever be removable; got {removable:?}"
    );
    // The hard-linked pair is protected (kept), the separate copy is the one
    // genuinely reclaimable file — reclaimable_bytes must reflect only that.
    assert_eq!(
        group["reclaimable_bytes"], 0,
        "removing the anchor set frees nothing"
    );
    assert_eq!(doc["totals"]["reclaimable_bytes"], 0);
}

/// AC-18a (FR-17, TASK-0316): "Given two or more scanned hard links to the
/// same content and no other file in scope with that content, when scanned,
/// then the set is reported as already deduplicated … while
/// `duplicate_groups`, `removable_files` and `reclaimable_bytes` all stay
/// `0`. A single file with no hard-linked sibling is not such a set and is
/// reported as before."
#[test]
fn ac18a_lone_hard_link_set_is_already_deduplicated_single_file_is_not() {
    let dir = tempfile::tempdir().expect("tempdir");
    let linked = image_bytes(ImageFormat::Png, 52, 32);
    let solo = image_bytes(ImageFormat::Png, 53, 40);
    materialize(
        dir.path(),
        &[
            spec("j1.png", linked, T),
            spec("solo.png", solo, T), // no sibling of any kind
        ],
        &[LinkSpec {
            existing: "j1.png".to_owned(),
            new: "j2.png".to_owned(),
        }],
    );

    let doc = run_json(dir.path(), &["--no-progress"]);

    assert_eq!(
        doc["groups"].as_array().unwrap().len(),
        0,
        "no removal opportunity exists"
    );
    let already = doc["already_deduplicated"].as_array().unwrap();
    assert_eq!(
        already.len(),
        1,
        "the lone hard-link set must be reported; got {doc:#}"
    );
    assert_eq!(already[0]["representative"], "j1.png");
    assert_eq!(already[0]["siblings"].as_array().unwrap().len(), 1);

    let totals = &doc["totals"];
    assert_eq!(totals["duplicate_groups"], 0);
    assert_eq!(totals["removable_files"], 0);
    assert_eq!(totals["reclaimable_bytes"], 0);
    assert_eq!(totals["already_deduplicated_sets"], 1);

    // solo.png (no hard-linked sibling, no content match) is not such a set:
    // a standalone file with no duplicate of any kind is not part of the
    // dedup document at all (neither `groups` nor `already_deduplicated`
    // mentions files that have nothing to report about them — confirmed
    // above via the exact `already_deduplicated` length of 1). Positive
    // control that the file really was scanned rather than silently
    // dropped: it is confirmable and counted, just via the human report's
    // absence-of-findings framing, not a JSON per-file listing.
    let already_dedup_str = serde_json::to_string(already).unwrap();
    assert!(
        !already_dedup_str.contains("solo.png"),
        "a lone standalone file must never be reported as already-deduplicated"
    );

    // Human report parity: the block name and the "not counted" framing.
    let report = cmd().arg(dir.path()).arg("--no-progress").output().unwrap();
    let report_stdout = String::from_utf8_lossy(&report.stdout);
    assert!(
        report_stdout.contains("Already deduplicated (hard-linked)"),
        "report:\n{report_stdout}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// AC-19
// ─────────────────────────────────────────────────────────────────────────

/// AC-19 (FR-18): "Given `--exit-code`, when a scan finds duplicate groups
/// the tool exits `3`; when it finds none it exits `0`; and without
/// `--exit-code` a successful run exits `0` even when duplicates are
/// present."
///
/// Deliberately deferred to this task (per the task file): `exit_code.rs`'s
/// unit tests already pin the pure `compute()` mapping, but no prior test
/// drove `--exit-code` through the real binary end to end. This is that test.
#[test]
fn ac19_exit_code_flag_semantics_through_the_real_binary() {
    let dupes = tempfile::tempdir().expect("tempdir");
    let bytes = image_bytes(ImageFormat::Png, 61, 32);
    materialize(
        dupes.path(),
        &[spec("a.png", bytes.clone(), T), spec("b.png", bytes, T)],
        &[],
    );

    // --exit-code + duplicates found → 3.
    cmd()
        .arg(dupes.path())
        .arg("--exit-code")
        .arg("--no-progress")
        .assert()
        .code(3);

    // --exit-code + no duplicates → 0.
    let empty = tempfile::tempdir().expect("tempdir");
    cmd()
        .arg(empty.path())
        .arg("--exit-code")
        .arg("--no-progress")
        .assert()
        .code(0);

    // Without --exit-code, a successful run exits 0 even with duplicates present.
    cmd()
        .arg(dupes.path())
        .arg("--no-progress")
        .assert()
        .code(0);
}
