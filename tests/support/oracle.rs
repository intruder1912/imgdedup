//! Golden-set oracle harness (TASK-0313).
//!
//! Runs the **real built binary** with `--json` on a materialized tree,
//! parses stdout, relativizes every path against the scan root, and compares
//! the whole document to a hand-derived expected manifest. The harness never
//! re-implements pipeline logic — expected values come from the scenario's
//! construction, which is what makes it an oracle rather than a mirror.

use std::path::Path;
use std::process::Output;

use assert_cmd::Command;
use serde_json::Value;

/// Run `imgdedup <root> <extra_args> --json`, assert exit success, and return
/// the raw process output (for byte-level determinism assertions).
pub fn run_json_raw(root: &Path, extra_args: &[&str]) -> Output {
    let output = Command::cargo_bin("imgdedup")
        .expect("imgdedup binary must build")
        .arg(root)
        .args(extra_args)
        .arg("--json")
        .output()
        .expect("imgdedup must spawn");
    assert!(
        output.status.success(),
        "imgdedup exited {:?}; stderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

/// Run `imgdedup <root> <extra_args> --json` and return the parsed document
/// with every path string relativized against `root` (`/`-separated), so
/// expected manifests are platform- and tempdir-independent.
pub fn run_json(root: &Path, extra_args: &[&str]) -> Value {
    let output = run_json_raw(root, extra_args);
    let mut doc: Value =
        serde_json::from_slice(&output.stdout).expect("--json stdout must be valid JSON");
    let prefix = root
        .to_str()
        .expect("test tempdir paths are valid UTF-8")
        .to_owned();
    relativize(&mut doc, &prefix);
    doc
}

/// Rewrite, in place, every string value that starts with the scan-root
/// prefix into a `/`-separated root-relative path. Non-path strings never
/// carry the tempdir prefix, so they pass through untouched.
fn relativize(value: &mut Value, prefix: &str) {
    match value {
        Value::String(s) => {
            if let Some(rest) = s.strip_prefix(prefix) {
                *s = rest.trim_start_matches(['/', '\\']).replace('\\', "/");
            }
        }
        Value::Array(items) => {
            for item in items {
                relativize(item, prefix);
            }
        }
        Value::Object(map) => {
            for (_, v) in map.iter_mut() {
                relativize(v, prefix);
            }
        }
        _ => {}
    }
}

/// Like [`run_json_raw`], but for more than one scan root: runs
/// `imgdedup <roots...> <extra_args> --json` and returns the parsed document
/// with paths left absolute. Multiple roots need not share one common
/// prefix, so there is no single string [`run_json`]'s relativization could
/// strip; callers compare against roots' own joined paths instead.
pub fn run_multi_root_json(roots: &[&Path], extra_args: &[&str]) -> Value {
    let output = Command::cargo_bin("imgdedup")
        .expect("imgdedup binary must build")
        .args(roots)
        .args(extra_args)
        .arg("--json")
        .output()
        .expect("imgdedup must spawn");
    assert!(
        output.status.success(),
        "imgdedup exited {:?}; stderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("--json stdout must be valid JSON")
}

/// Assert the actual (relativized) document equals the expected manifest,
/// failing with both documents pretty-printed for a readable diff.
pub fn assert_json_eq(actual: &Value, expected: &Value) {
    assert_eq!(
        actual,
        expected,
        "--json output diverged from the golden manifest\n\n-- actual --\n{}\n\n-- expected --\n{}\n",
        serde_json::to_string_pretty(actual).expect("actual document serializes"),
        serde_json::to_string_pretty(expected).expect("expected manifest serializes"),
    );
}
