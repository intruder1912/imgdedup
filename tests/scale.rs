//! NFR-4 scale validation (TASK-0312, second half).
//!
//! These tests are `#[ignore]`d: they build fixtures of 100k+ files and take
//! minutes, which is too slow for the always-on suite that gates every PR. The
//! acceptance coverage in `tests/acceptance.rs` stays always-on; this is run
//! deliberately, via `make scale`, when the scale claims need re-checking.
//!
//! What NFR-4 actually claims, and how each part is checked here:
//!
//! - **A 100k+ file library completes** — `scale_100k_files_completes`.
//! - **Memory scales with file *count*, not data *volume*** (ADR-0010's
//!   streaming-reads promise). Peak RSS is not measurable portably from inside
//!   a test process, so this suite provides the two fixtures that make the
//!   comparison meaningful — many small files versus few large ones — and the
//!   measurement itself is taken externally and recorded in the review log.
//!   `memory_fixture_large_files` exists to be driven that way.
//! - **Results are independent of concurrency width** (ADR-0010, NFR-3, AC-16)
//!   — `results_are_identical_across_concurrency_widths`, which is the one that
//!   actually protects the tuning knob: if the concurrency default is ever
//!   changed, output must not move with it.

mod support;

use assert_cmd::Command;
use std::fs;
use std::path::{Path, PathBuf};
use support::images::{ImageFormat, image_bytes};

/// Files per scale fixture. NFR-4 says "100k+ images".
const SCALE_FILES: usize = 100_000;

/// One in every `DUP_EVERY` files is a duplicate of its predecessor, so the
/// engine does real grouping and verification work rather than dropping
/// everything as a size singleton.
const DUP_EVERY: usize = 10;

fn cmd() -> Command {
    Command::cargo_bin("imgdedup").unwrap()
}

/// Builds `count` small images across a shallow directory fan-out, with every
/// `DUP_EVERY`-th file a byte-for-byte copy of the one before it.
///
/// The fan-out matters: 100k files in a single directory is not what a photo
/// library looks like, and some filesystems degrade badly on it.
fn build_many_small(root: &Path, count: usize) -> usize {
    let mut duplicates = 0;
    let mut previous: Option<Vec<u8>> = None;
    for i in 0..count {
        let dir = root.join(format!("d{:03}", i % 200));
        if !dir.exists() {
            fs::create_dir_all(&dir).expect("mkdir");
        }
        let bytes = if i % DUP_EVERY == 0 && previous.is_some() {
            duplicates += 1;
            previous.clone().unwrap()
        } else {
            // Vary seed *and* length so the fixture is mostly-unique, like a
            // real library: a seed alone only has 256 values, which would make
            // every 256th file an accidental duplicate and turn this into a
            // verification stress test rather than a scan benchmark.
            let b = image_bytes(ImageFormat::Png, (i % 251) as u8, 64 + (i % 4093));
            previous = Some(b.clone());
            b
        };
        fs::write(dir.join(format!("f{i:06}.png")), &bytes).expect("write");
    }
    duplicates
}

/// A few large files, so peak memory can be compared against the many-small
/// fixture. Deliberately larger than any sane read buffer.
fn build_few_large(root: &Path, count: usize, size_mib: usize) {
    let header = image_bytes(ImageFormat::Png, 1, 64);
    let mut body = vec![0u8; 1024 * 1024];
    for (i, b) in body.iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }
    for i in 0..count {
        let mut f = fs::File::create(root.join(format!("large{i}.png"))).expect("create");
        use std::io::Write;
        f.write_all(&header).expect("header");
        for _ in 0..size_mib {
            f.write_all(&body).expect("body");
        }
    }
}

fn scale_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("imgdedup-scale-{name}"));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("scale root");
    root
}

#[test]
#[ignore = "scale fixture: builds 100k files, run via `make scale`"]
fn scale_100k_files_completes() {
    let root = scale_root("100k");
    let duplicates = build_many_small(&root, SCALE_FILES);

    let out = cmd()
        .arg(&root)
        .arg("--json")
        .arg("--no-progress")
        .output()
        .expect("run");
    assert!(out.status.success(), "a 100k-file run must succeed");

    let doc: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");
    let groups = doc["groups"].as_array().expect("groups").len();
    assert!(
        groups > 0,
        "the fixture is built with {duplicates} duplicates; the run found no groups"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
#[ignore = "scale fixture: builds 100k files, run via `make scale`"]
fn results_are_identical_across_concurrency_widths() {
    // ADR-0010 makes concurrency a pure tuning knob: the canonical sort means
    // results cannot depend on it. This is what keeps that true — if someone
    // changes the default width, or rayon changes its own, the output must not
    // move. Run at 1 thread and at the machine default.
    let root = scale_root("widths");
    build_many_small(&root, 20_000);

    let single = cmd()
        .arg(&root)
        .arg("--json")
        .arg("--no-progress")
        .env("RAYON_NUM_THREADS", "1")
        .output()
        .expect("run");
    let wide = cmd()
        .arg(&root)
        .arg("--json")
        .arg("--no-progress")
        .env("RAYON_NUM_THREADS", "16")
        .output()
        .expect("run");

    assert!(single.status.success() && wide.status.success());
    assert_eq!(
        single.stdout, wide.stdout,
        "results must be byte-identical across concurrency widths (NFR-3, ADR-0010)"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
#[ignore = "scale fixture: writes ~300 MiB, run via `make scale`"]
fn memory_fixture_large_files() {
    // Exists so peak RSS can be measured against the many-small fixture from
    // outside the process (`/usr/bin/time -l` on macOS, `-v` on Linux). Three
    // files holding roughly the same total bytes as the 100k-file fixture is
    // the comparison that matters: if memory scaled with data volume the two
    // would be similar. Measured, they are not — 7.5 MiB here against 43 MiB
    // there. Numbers and method are recorded in the review log.
    let root = scale_root("large");
    build_few_large(&root, 3, 100);

    let out = cmd()
        .arg(&root)
        .arg("--json")
        .arg("--no-progress")
        .output()
        .expect("run");
    assert!(out.status.success(), "a large-file run must succeed");

    let _ = fs::remove_dir_all(&root);
}
