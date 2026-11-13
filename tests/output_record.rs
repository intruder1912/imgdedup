//! End-to-end tests for the durable run record (`--output`, FR-20,
//! ADR-0018): AC-25..AC-28a and the plan's Review Focus items.
mod support;

use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::Path;
use support::images::{ImageFormat, image_bytes};
use support::tree::{FileSpec, materialize};

const MTIME: i64 = 1_750_000_000;

fn cmd() -> Command {
    Command::cargo_bin("imgdedup").unwrap()
}

/// Three duplicate pairs: `dir{0,1,2}/a.<ext>` + `dir{0,1,2}/copy.<ext>`.
fn fixture(root: &Path) {
    let mut files = Vec::new();
    for (i, (format, seed)) in [
        (ImageFormat::Png, 1u8),
        (ImageFormat::Jpeg, 2u8),
        (ImageFormat::Bmp, 3u8),
    ]
    .into_iter()
    .enumerate()
    {
        let ext = format.extension();
        let bytes = image_bytes(format, seed, 64);
        for name in ["a", "copy"] {
            files.push(FileSpec {
                rel_path: format!("dir{i}/{name}.{ext}"),
                bytes: bytes.clone(),
                mtime: MTIME,
            });
        }
    }
    materialize(root, &files, &[]);
}

fn ndjson(path: &Path) -> Vec<Value> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("bad line {l:?}: {e}")))
        .collect()
}

// ── AC-26 / AC-28: human record ──────────────────────────────────────────

#[test]
fn human_record_ends_with_the_exact_stdout_report() {
    let tree = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    fixture(tree.path());
    let record = out_dir.path().join("run.txt");

    let out = cmd()
        .arg(tree.path())
        .arg("--output")
        .arg(&record)
        .output()
        .unwrap();
    assert!(out.status.success());

    let stdout = String::from_utf8(out.stdout).unwrap();
    let text = fs::read_to_string(&record).unwrap();
    assert!(text.starts_with("# imgdedup "), "{text}");
    assert!(
        text.contains("\n===== RESULT \u{2014} run completed "),
        "{text}"
    );
    assert!(
        text.ends_with(&stdout),
        "AC-26: the record's tail is stdout, byte for byte"
    );

    let transcript = text.split("\n===== RESULT").next().unwrap();
    assert_eq!(transcript.matches("] group (").count(), 3, "{transcript}");
    assert!(transcript.contains("] scanning..."));
    assert!(transcript.contains("] done: 6 of 6 (100%)"));
    assert!(
        !transcript.contains("Group 1/"),
        "no index in the transcript"
    );
}

#[test]
fn stdout_is_byte_identical_with_and_without_output() {
    let tree = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    fixture(tree.path());

    for extra in [&[][..], &["--json"][..]] {
        let plain = cmd().arg(tree.path()).args(extra).output().unwrap();
        let recorded = cmd()
            .arg(tree.path())
            .args(extra)
            .arg("--output")
            .arg(out_dir.path().join("r"))
            .output()
            .unwrap();
        assert_eq!(plain.stdout, recorded.stdout, "AC-28 ({extra:?})");
        assert_eq!(plain.status.code(), recorded.status.code());
    }
}

#[test]
fn no_progress_does_not_silence_the_record() {
    let tree = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    fixture(tree.path());
    let record = out_dir.path().join("run.txt");

    let out = cmd()
        .arg(tree.path())
        .args(["--no-progress", "--output"])
        .arg(&record)
        .output()
        .unwrap();
    assert!(
        out.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = fs::read_to_string(&record).unwrap();
    assert!(text.contains("] hashing: 0 of 6 (0%)"), "{text}");
    assert!(text.contains("] done: 6 of 6 (100%)"), "{text}");
}

// ── AC-27: NDJSON ────────────────────────────────────────────────────────

#[test]
fn ndjson_record_is_valid_per_line_and_ends_with_the_stdout_document() {
    let tree = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    fixture(tree.path());
    let record = out_dir.path().join("run.ndjson");

    let out = cmd()
        .arg(tree.path())
        .args(["--json", "--output"])
        .arg(&record)
        .output()
        .unwrap();
    assert!(out.status.success());

    let lines = ndjson(&record);
    let first = lines.first().unwrap();
    assert_eq!(first["type"], "header");
    assert_eq!(first["record_version"], 1);
    assert_eq!(first["invocation"]["json"], true);
    assert!(
        lines
            .iter()
            .all(|l| l["type"].is_string() && l["elapsed_ms"].is_u64())
    );

    let last = lines.last().unwrap();
    assert_eq!(last["type"], "result");
    let stdout_doc: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        last["result"], stdout_doc,
        "AC-27: same document, not the same bytes"
    );
    assert_eq!(lines.iter().filter(|l| l["type"] == "result").count(), 1);
}

#[test]
fn every_ndjson_group_line_equals_a_group_in_the_result() {
    // ADR-0018 §4b's anti-drift rule, end to end.
    let tree = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    fixture(tree.path());
    let record = out_dir.path().join("run.ndjson");
    cmd()
        .arg(tree.path())
        .args(["--json", "--output"])
        .arg(&record)
        .assert()
        .success();

    let lines = ndjson(&record);
    let final_groups = lines.last().unwrap()["result"]["groups"]
        .as_array()
        .unwrap()
        .clone();
    let streamed: Vec<Value> = lines
        .iter()
        .filter(|l| l["type"] == "group")
        .map(|l| {
            let mut g = l.clone();
            let obj = g.as_object_mut().unwrap();
            obj.remove("type");
            obj.remove("elapsed_ms");
            g
        })
        .collect();

    assert_eq!(streamed.len(), final_groups.len());
    for g in &streamed {
        assert!(
            final_groups.contains(g),
            "streamed group not in result: {g}"
        );
    }
}

// ── AC-28: fail fast (Review Focus 2) ────────────────────────────────────

#[test]
fn unwritable_output_path_fails_before_scanning() {
    let tree = tempfile::tempdir().unwrap();
    fixture(tree.path());
    let a_directory = tree.path().join("dir0");
    let missing_parent = tree.path().join("no-such-dir").join("run.txt");

    for target in [a_directory, missing_parent] {
        let out = cmd()
            .arg(tree.path())
            .arg("--output")
            .arg(&target)
            .output()
            .unwrap();
        assert_eq!(
            out.status.code(),
            Some(2),
            "usage-level error for {target:?}"
        );
        assert!(out.stdout.is_empty(), "no report is written");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.starts_with("error: cannot open output file "),
            "{stderr}"
        );
        assert!(
            !stderr.contains("scanning"),
            "must fail before scanning starts: {stderr}"
        );
    }
}

// ── ADR-0018 §5: --output inside a scan root ─────────────────────────────

#[test]
fn an_output_file_inside_the_scan_root_is_never_grouped_or_removed() {
    let tree = tempfile::tempdir().unwrap();
    fixture(tree.path());
    // An image extension forces the record itself to become a candidate.
    let record = tree.path().join("rec.jpg");

    let out = cmd()
        .arg(tree.path())
        .args(["--apply", "--delete", "--json", "--output"])
        .arg(&record)
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(record.exists(), "the record must never be removed");

    let doc: Value = serde_json::from_slice(&out.stdout).unwrap();
    let rendered = doc.to_string();
    let grouped = doc["groups"].to_string() + &doc["actions"].to_string();
    assert!(
        !grouped.contains("rec.jpg"),
        "never grouped, never acted on: {rendered}"
    );
    assert!(
        doc["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| { w["stage"] == "sniff" && w["path"].as_str().unwrap().ends_with("rec.jpg") }),
        "it is rejected by the magic-byte gate, with a warning: {rendered}"
    );
}

// ── apply-time actions ───────────────────────────────────────────────────

#[test]
fn apply_actions_are_recorded_as_they_happen() {
    let tree = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    fixture(tree.path());
    let record = out_dir.path().join("run.ndjson");

    cmd()
        .arg(tree.path())
        .args(["--apply", "--delete", "--json", "--output"])
        .arg(&record)
        .assert()
        .success();

    let lines = ndjson(&record);
    let actions: Vec<&Value> = lines.iter().filter(|l| l["type"] == "action").collect();
    assert_eq!(actions.len(), 3);
    assert!(
        actions
            .iter()
            .all(|a| a["action"] == "removed" && a["mode"] == "delete")
    );
    let removed = &lines.last().unwrap()["result"]["actions"]["removed"];
    for a in &actions {
        assert!(removed.as_array().unwrap().contains(&a["path"]));
    }
    let position = |ty: &str| lines.iter().position(|l| l["type"] == ty).unwrap();
    assert!(position("action") < position("result"));
}

// ── Review Focus 3: truthful records at the edges ────────────────────────

#[test]
fn empty_run_still_writes_header_and_result() {
    let tree = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    let record = out_dir.path().join("run.txt");

    let out = cmd()
        .arg(tree.path())
        .arg("--output")
        .arg(&record)
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = fs::read_to_string(&record).unwrap();
    assert!(text.starts_with("# imgdedup "));
    assert!(text.contains("] done: 0 of 0 (100%)"), "{text}");
    assert!(text.ends_with(&String::from_utf8(out.stdout).unwrap()));
    assert!(!text.contains("] group ("));
}

#[test]
fn refused_apply_leaves_errors_and_no_result() {
    let tree = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    fixture(tree.path());
    let record = out_dir.path().join("run.txt");

    let out = cmd()
        .arg(tree.path())
        .arg(tree.path().join("no-such-root"))
        .args(["--apply", "--output"])
        .arg(&record)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());

    let text = fs::read_to_string(&record).unwrap();
    assert!(
        text.contains("] error: ") && text.contains("no-such-root"),
        "{text}"
    );
    assert!(
        text.contains("] error: not applying: 1 scan root(s) could not be read"),
        "{text}"
    );
    assert!(
        !text.contains("===== RESULT"),
        "a run that printed no report has no completion document: {text}"
    );
}

// ── Review Focus 1: hostile filenames ────────────────────────────────────

#[cfg(unix)]
#[test]
fn record_cannot_be_forged_by_a_newline_in_a_filename() {
    let tree = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    let bytes = image_bytes(ImageFormat::Png, 7, 64);
    let evil = "x\n[     0.0s] trashed: FORGED.png";
    materialize(
        tree.path(),
        &[
            FileSpec {
                rel_path: "a.png".into(),
                bytes: bytes.clone(),
                mtime: MTIME,
            },
            FileSpec {
                rel_path: evil.into(),
                bytes,
                mtime: MTIME,
            },
        ],
        &[],
    );
    let record = out_dir.path().join("run.txt");
    cmd()
        .arg(tree.path())
        .arg("--output")
        .arg(&record)
        .assert()
        .success();

    let text = fs::read_to_string(&record).unwrap();
    let transcript = text.split("\n===== RESULT").next().unwrap();
    assert!(
        transcript.contains("] group ("),
        "the pair is still reported: {transcript}"
    );
    // The newline is escaped, so the hostile name stays on its indented
    // `REMOVE` row: no line of the transcript *starts* a record with it.
    assert!(
        !transcript
            .lines()
            .any(|l| l.starts_with('[') && l.contains("] trashed: FORGED.png")),
        "a filename must not be able to start a record line: {transcript}"
    );
    assert!(
        transcript.contains(r"x\n[     0.0s] trashed: FORGED.png"),
        "the newline is shown escaped, in place: {transcript}"
    );
}

// ── AC-28a: mid-run write failure ────────────────────────────────────────

/// `/dev/full` opens for writing and fails every write with ENOSPC — a real
/// "disk full" without a full disk. Linux only; the cross-platform half of
/// AC-28a is pinned by `record.rs`'s injected-writer unit tests.
#[cfg(target_os = "linux")]
#[test]
fn a_record_that_cannot_be_written_does_not_stop_the_run() {
    let tree = tempfile::tempdir().unwrap();
    fixture(tree.path());

    let plain = cmd()
        .arg(tree.path())
        .arg("--no-progress")
        .output()
        .unwrap();
    let out = cmd()
        .arg(tree.path())
        .args(["--no-progress", "--output", "/dev/full"])
        .output()
        .unwrap();

    assert_eq!(
        out.status.code(),
        Some(1),
        "exit 1 although the scan succeeded"
    );
    assert_eq!(
        out.stdout, plain.stdout,
        "the report is exactly what it would have been"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        stderr
            .matches("error: could not write to /dev/full: ")
            .count(),
        1,
        "reported exactly once: {stderr}"
    );
    assert_eq!(stderr.lines().count(), 1, "and nothing else: {stderr}");
}

#[cfg(target_os = "linux")]
#[test]
fn a_failed_record_never_interrupts_an_apply() {
    let tree = tempfile::tempdir().unwrap();
    fixture(tree.path());
    let out = cmd()
        .arg(tree.path())
        .args([
            "--apply",
            "--delete",
            "--no-progress",
            "--json",
            "--output",
            "/dev/full",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let doc: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(doc["actions"]["removed"].as_array().unwrap().len(), 3);
    for i in 0..3 {
        let left = fs::read_dir(tree.path().join(format!("dir{i}")))
            .unwrap()
            .count();
        assert_eq!(
            left, 1,
            "dir{i}: the duplicate was removed, the keeper kept"
        );
    }
}

/// The cross-platform twin of the `/dev/full` tests above: a real write
/// failure (`EFBIG`) produced by the file-size resource limit. `SIGXFSZ` is
/// ignored (an ignored signal survives `exec`), so the over-limit write fails
/// instead of killing the process. Only the record file is limited: stdout and
/// stderr are pipes, which `RLIMIT_FSIZE` does not constrain.
#[cfg(unix)]
fn run_with_file_size_limit(args: &[&std::ffi::OsStr]) -> std::process::Output {
    std::process::Command::new("sh")
        .args([
            "-c",
            r#"ulimit -f 1; trap '' XFSZ; exec "$0" "$@""#,
            env!("CARGO_BIN_EXE_imgdedup"),
        ])
        .args(args)
        .output()
        .unwrap()
}

#[cfg(unix)]
#[test]
fn a_record_that_hits_the_file_size_limit_does_not_stop_the_run() {
    // AC-28a on every Unix: same product behavior as the `/dev/full` test,
    // but runnable on macOS. `ulimit -f 1` is one block: 512 bytes under dash,
    // 1024 under bash-as-sh. The three-group fixture's complete record is
    // larger than either, so some write must fail. Which one depends on the
    // shell and on the length of the temporary paths, so nothing below assumes
    // the failure lands before the result.
    use std::ffi::OsStr;
    let tree = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    fixture(tree.path());
    let record = out_dir.path().join("run.txt");

    let plain = cmd()
        .arg(tree.path())
        .arg("--no-progress")
        .output()
        .unwrap();
    let out = run_with_file_size_limit(&[
        tree.path().as_os_str(),
        OsStr::new("--no-progress"),
        OsStr::new("--output"),
        record.as_os_str(),
    ]);

    assert_eq!(
        out.status.code(),
        Some(1),
        "exit 1 although the scan succeeded"
    );
    assert_eq!(
        out.stdout, plain.stdout,
        "stdout is exactly the plain report"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        stderr.matches("error: could not write to ").count(),
        1,
        "reported exactly once: {stderr}"
    );
    assert_eq!(stderr.lines().count(), 1, "and nothing else: {stderr}");

    // Bytes, not a string: the cut can land inside a multi-byte character.
    let bytes = fs::read(&record).unwrap();
    assert!(bytes.starts_with(b"# imgdedup "), "partial record kept");
    assert!(
        !bytes.ends_with(&plain.stdout),
        "a failed record never ends with the complete report"
    );
}

#[cfg(unix)]
#[test]
fn a_record_write_failure_never_interrupts_an_apply_on_any_unix() {
    use std::ffi::OsStr;
    let tree = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    fixture(tree.path());
    let record = out_dir.path().join("run.ndjson");

    let out = run_with_file_size_limit(&[
        tree.path().as_os_str(),
        OsStr::new("--apply"),
        OsStr::new("--delete"),
        OsStr::new("--no-progress"),
        OsStr::new("--json"),
        OsStr::new("--output"),
        record.as_os_str(),
    ]);

    assert_eq!(out.status.code(), Some(1));
    let doc: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(doc["actions"]["removed"].as_array().unwrap().len(), 3);
    for i in 0..3 {
        let left = fs::read_dir(tree.path().join(format!("dir{i}")))
            .unwrap()
            .count();
        assert_eq!(left, 1, "dir{i}: duplicate removed, keeper kept");
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        stderr.matches("error: could not write to ").count(),
        1,
        "{stderr}"
    );
}

// ── TASK-0342: `--output` never destroys a file unasked ───────────────────

#[test]
fn rerunning_with_the_same_output_keeps_the_previous_record_as_bak() {
    let tree = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    fixture(tree.path());
    let record = out_dir.path().join("run.txt");
    let bak = out_dir.path().join("run.txt.bak");

    let run = || {
        cmd()
            .arg(tree.path())
            .arg("--no-progress")
            .arg("--output")
            .arg(&record)
            .output()
            .unwrap()
    };
    assert_eq!(run().status.code(), Some(0));
    let first = fs::read(&record).unwrap();
    assert!(!bak.exists(), "nothing to back up on the first run");

    assert_eq!(run().status.code(), Some(0), "a re-run needs no flag");
    assert_eq!(fs::read(&bak).unwrap(), first, "previous record kept");
    assert!(
        fs::read_to_string(&record)
            .unwrap()
            .contains("===== RESULT")
    );

    // One generation: the third run replaces the backup with the second.
    let second = fs::read(&record).unwrap();
    assert_eq!(run().status.code(), Some(0));
    assert_eq!(fs::read(&bak).unwrap(), second);
}

#[test]
fn a_json_rerun_backs_up_a_human_record() {
    let tree = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    fixture(tree.path());
    let record = out_dir.path().join("run.out");

    let human = cmd()
        .arg(tree.path())
        .args(["--no-progress", "--output"])
        .arg(&record)
        .output()
        .unwrap();
    assert_eq!(human.status.code(), Some(0));
    let first = fs::read(&record).unwrap();

    let json = cmd()
        .arg(tree.path())
        .args(["--no-progress", "--json", "--output"])
        .arg(&record)
        .output()
        .unwrap();
    assert_eq!(json.status.code(), Some(0));
    assert_eq!(fs::read(out_dir.path().join("run.out.bak")).unwrap(), first);
    assert_eq!(ndjson(&record).last().unwrap()["type"], "result");
}

#[test]
fn refusing_leaves_an_existing_image_intact() {
    // The mistake this exists for: `--output` names one of the photos.
    let tree = tempfile::tempdir().unwrap();
    fixture(tree.path());
    let image = tree.path().join("dir0/a.png");
    let before = fs::read(&image).unwrap();

    let out = cmd()
        .arg(tree.path())
        .arg("--output")
        .arg(&image)
        .output()
        .unwrap();

    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty(), "no report");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.starts_with("error: output file "), "{stderr}");
    assert!(stderr.contains("--overwrite-output"), "{stderr}");
    assert!(!stderr.contains("scanning"), "refused before scanning");
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert_eq!(fs::read(&image).unwrap(), before, "the image is intact");
    assert!(!tree.path().join("dir0/a.png.bak").exists());
}

#[test]
fn overwrite_output_replaces_a_foreign_file_and_keeps_it_as_bak() {
    let tree = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    fixture(tree.path());
    let record = out_dir.path().join("notes.txt");
    fs::write(&record, b"shopping list\n").unwrap();

    let out = cmd()
        .arg(tree.path())
        .args(["--no-progress", "--overwrite-output", "--output"])
        .arg(&record)
        .output()
        .unwrap();

    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        fs::read(out_dir.path().join("notes.txt.bak")).unwrap(),
        b"shopping list\n"
    );
    let text = fs::read_to_string(&record).unwrap();
    assert!(text.ends_with(&String::from_utf8(out.stdout).unwrap()));
}

#[test]
fn a_foreign_bak_refuses_the_run() {
    let tree = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    fixture(tree.path());
    let record = out_dir.path().join("run.txt");
    let bak = out_dir.path().join("run.txt.bak");
    fs::write(&record, b"# imgdedup 0.1.0 run record\n").unwrap();
    fs::write(&bak, b"not ours\n").unwrap();

    for extra in [&[][..], &["--overwrite-output"][..]] {
        let out = cmd()
            .arg(tree.path())
            .args(extra)
            .arg("--output")
            .arg(&record)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2), "{extra:?}");
        assert!(out.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.starts_with("error: cannot back up "), "{stderr}");
        assert_eq!(fs::read(&bak).unwrap(), b"not ours\n");
        assert_eq!(fs::read(&record).unwrap(), b"# imgdedup 0.1.0 run record\n");
    }
}
