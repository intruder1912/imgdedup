//! Machine-readable JSON output — the `--json` formatter (TASK-0308, FR-14).
//!
//! A second formatter over the same canonical model as the human report
//! (TASK-0307): the engine's [`DedupOutcome`] plus the upstream enumeration,
//! sniff, and root-error diagnostics. It performs no filesystem I/O and no
//! re-computation — it only serializes what the pipeline already produced.
//!
//! ## Stable schema (documented contract, `docs/guide/output-and-exit-codes.md`)
//!
//! ```text
//! {
//!   "schema_version": 1,
//!   "mode": "dry-run",
//!   "groups": [
//!     {
//!       "keeper": { "path", "size", "mtime" },
//!       "removable": [ { "path", "size", "mtime" }, … ],
//!       "protected": [ { "path", "size", "mtime" }, … ],
//!       "hard_link_sets": [ { "representative", "siblings": […] }, … ],
//!       "reclaimable_bytes": u64
//!     }, …
//!   ],
//!   "already_deduplicated": [ { "representative", "siblings": […] }, … ],
//!   "totals": {
//!     "duplicate_groups", "removable_files", "reclaimable_bytes",
//!     "already_deduplicated_sets"
//!   },
//!   "warnings": [ { "stage": "enumerate"|"sniff"|"engine", "path", "reason" }, … ],
//!   "root_errors": [ { "path", "reason" }, … ]
//! }
//! ```
//!
//! Design rules (task notes + resilience review F3):
//!
//! - **No internal representations leak** — the engine's `hash` field is
//!   deliberately absent; the schema exposes only what scripts need.
//! - **Diagnostics are first-class** (resilience F3): per-file warnings from
//!   every stage and run-level `root_errors` are part of the document, so JSON
//!   consumers never inherit a silent failure. Root errors additionally go to
//!   stderr (F1) — stdout stays a single valid JSON document.
//! - **Determinism (NFR-3):** the `DedupOutcome` is already canonically sorted
//!   (ADR-0010); enumerate/sniff warnings are sorted here by path before
//!   serialization; `serde_json` preserves struct field order. Two runs over
//!   identical state emit byte-identical documents.
//! - **Paths** are serialized with `to_string_lossy` — invalid-UTF-8 path
//!   bytes are replaced (U+FFFD) rather than failing serialization; `mtime`
//!   is `null` when the platform did not provide it.
//! - `"mode"` is `"dry-run"` until the destructive slice (TASK-0309/0310)
//!   extends it (`"trash"`, `"delete"`) and adds performed-action reporting.

use crate::engine::{DedupOutcome, DuplicateGroup, FileRec, HardLinkNote};
use crate::enumerate::EnumerateWarning;
use crate::sniff::SniffWarning;
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;

/// Current schema version. Bump only on breaking changes to the contract.
pub const SCHEMA_VERSION: u32 = 1;

// ── Schema types (public contract, serde-serialized) ─────────────────────────

#[derive(Debug, Serialize)]
struct JsonFile {
    path: String,
    size: u64,
    mtime: Option<i64>,
}

impl JsonFile {
    fn from_rec(rec: &FileRec) -> Self {
        Self {
            path: lossy(&rec.path),
            size: rec.size,
            mtime: rec.mtime,
        }
    }
}

#[derive(Debug, Serialize)]
struct JsonHardLinkSet {
    representative: String,
    siblings: Vec<String>,
}

impl JsonHardLinkSet {
    fn from_note(note: &HardLinkNote) -> Self {
        Self {
            representative: lossy(&note.representative),
            siblings: note.siblings.iter().map(|p| lossy(p)).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct JsonGroup {
    keeper: JsonFile,
    removable: Vec<JsonFile>,
    protected: Vec<JsonFile>,
    hard_link_sets: Vec<JsonHardLinkSet>,
    reclaimable_bytes: u64,
}

#[derive(Debug, Serialize)]
struct JsonWarning {
    stage: &'static str,
    path: String,
    reason: String,
}

#[derive(Debug, Serialize)]
struct JsonRootError {
    path: String,
    reason: String,
}

#[derive(Debug, Serialize)]
struct JsonTotals {
    duplicate_groups: usize,
    removable_files: usize,
    reclaimable_bytes: u64,
    already_deduplicated_sets: usize,
}

#[derive(Debug, Serialize)]
struct JsonReport {
    schema_version: u32,
    mode: &'static str,
    groups: Vec<JsonGroup>,
    already_deduplicated: Vec<JsonHardLinkSet>,
    totals: JsonTotals,
    warnings: Vec<JsonWarning>,
    root_errors: Vec<JsonRootError>,
}

// ── Rendering ────────────────────────────────────────────────────────────────

/// Render the pipeline outcome as the stable JSON document (FR-14).
///
/// Mirrors [`crate::report::render`]'s note-partitioning: a [`HardLinkNote`]
/// whose representative appears among a group's paths (keeper, removable, or
/// protected) is attached to that group as a `hard_link_sets` entry; all
/// remaining notes are all-anchor "already deduplicated" sets, surfaced
/// top-level and excluded from totals — identical semantics to the human
/// report (parity, TASK-0308).
pub fn render(
    outcome: &DedupOutcome,
    enumerate_warnings: &[EnumerateWarning],
    sniff_warnings: &[SniffWarning],
    root_errors: &[EnumerateWarning],
) -> String {
    let mut attached: HashSet<usize> = HashSet::new();

    let groups: Vec<JsonGroup> = outcome
        .groups
        .iter()
        .map(|g| json_group(g, &outcome.hard_link_notes, &mut attached))
        .collect();

    let already_deduplicated: Vec<JsonHardLinkSet> = outcome
        .hard_link_notes
        .iter()
        .enumerate()
        .filter(|(i, _)| !attached.contains(i))
        .map(|(_, n)| JsonHardLinkSet::from_note(n))
        .collect();

    let totals = JsonTotals {
        duplicate_groups: groups.len(),
        removable_files: groups.iter().map(|g| g.removable.len()).sum(),
        reclaimable_bytes: groups.iter().map(|g| g.reclaimable_bytes).sum(),
        already_deduplicated_sets: already_deduplicated.len(),
    };

    // Enumerate/sniff warnings arrive in traversal order; sort by path for
    // determinism (NFR-3). Engine warnings are already canonically sorted.
    let mut warnings: Vec<JsonWarning> = Vec::new();
    collect_sorted(&mut warnings, "enumerate", enumerate_warnings, |w| {
        (&w.path, &w.reason)
    });
    collect_sorted(&mut warnings, "sniff", sniff_warnings, |w| {
        (&w.path, &w.reason)
    });
    for w in &outcome.warnings {
        warnings.push(JsonWarning {
            stage: "engine",
            path: lossy(&w.path),
            reason: w.reason.clone(),
        });
    }

    let root_errors: Vec<JsonRootError> = {
        let mut v: Vec<&EnumerateWarning> = root_errors.iter().collect();
        v.sort_by(|a, b| a.path.cmp(&b.path));
        v.into_iter()
            .map(|e| JsonRootError {
                path: lossy(&e.path),
                reason: e.reason.clone(),
            })
            .collect()
    };

    let report = JsonReport {
        schema_version: SCHEMA_VERSION,
        mode: "dry-run",
        groups,
        already_deduplicated,
        totals,
        warnings,
        root_errors,
    };

    // Serialization of these derive-only types cannot fail; a pretty document
    // is friendlier to humans piping to files and equally parseable.
    let mut out = serde_json::to_string_pretty(&report)
        .expect("JSON serialization of plain structs cannot fail");
    out.push('\n');
    out
}

/// Build one `JsonGroup`, attaching the hard-link notes whose representative
/// is among the group's paths and recording their indices in `attached`.
fn json_group(
    group: &DuplicateGroup,
    notes: &[HardLinkNote],
    attached: &mut HashSet<usize>,
) -> JsonGroup {
    let group_paths: HashSet<&Path> = std::iter::once(group.keeper.path.as_path())
        .chain(group.removable.iter().map(|r| r.path.as_path()))
        .chain(group.protected.iter().map(|r| r.path.as_path()))
        .collect();

    let hard_link_sets: Vec<JsonHardLinkSet> = notes
        .iter()
        .enumerate()
        .filter(|(_, n)| group_paths.contains(n.representative.as_path()))
        .map(|(i, n)| {
            attached.insert(i);
            JsonHardLinkSet::from_note(n)
        })
        .collect();

    JsonGroup {
        keeper: JsonFile::from_rec(&group.keeper),
        removable: group.removable.iter().map(JsonFile::from_rec).collect(),
        protected: group.protected.iter().map(JsonFile::from_rec).collect(),
        hard_link_sets,
        reclaimable_bytes: group.removable.iter().map(|r| r.size).sum(),
    }
}

/// Push `items` into `out` as `stage` warnings, sorted by (path, reason).
fn collect_sorted<'a, T, F>(out: &mut Vec<JsonWarning>, stage: &'static str, items: &'a [T], key: F)
where
    F: Fn(&'a T) -> (&'a std::path::PathBuf, &'a String),
{
    let mut sorted: Vec<(&std::path::PathBuf, &String)> = items.iter().map(key).collect();
    sorted.sort();
    for (path, reason) in sorted {
        out.push(JsonWarning {
            stage,
            path: lossy(path),
            reason: reason.clone(),
        });
    }
}

fn lossy(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::KeepRule;
    use crate::engine::find_duplicates;
    use crate::keeper::build_comparator;
    use crate::sniff::SniffWarning;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn outcome_from(dir: &TempDir, files: &[(&str, &[u8])]) -> DedupOutcome {
        let confirmed = files
            .iter()
            .map(|(name, bytes)| {
                let p = dir.path().join(name);
                fs::write(&p, bytes).unwrap();
                let meta = fs::metadata(&p).unwrap();
                crate::sniff::ConfirmedFile {
                    path: p,
                    size: meta.len(),
                    mtime: Some(1_700_000_000),
                }
            })
            .collect();
        find_duplicates(confirmed, build_comparator(KeepRule::Shortest))
    }

    fn parse(s: &str) -> serde_json::Value {
        serde_json::from_str(s).expect("output must be valid JSON")
    }

    #[test]
    fn output_is_valid_json_with_schema_version_and_mode() {
        let dir = TempDir::new().unwrap();
        let outcome = outcome_from(&dir, &[("a.jpg", b"same"), ("b.jpg", b"same")]);
        let v = parse(&render(&outcome, &[], &[], &[]));
        assert_eq!(v["schema_version"], 1);
        assert_eq!(v["mode"], "dry-run");
    }

    #[test]
    fn groups_carry_keeper_removable_and_reclaimable() {
        let dir = TempDir::new().unwrap();
        let outcome = outcome_from(&dir, &[("a.jpg", b"same"), ("bb.jpg", b"same")]);
        let v = parse(&render(&outcome, &[], &[], &[]));
        let groups = v["groups"].as_array().unwrap();
        assert_eq!(groups.len(), 1);
        let g = &groups[0];
        assert!(g["keeper"]["path"].as_str().unwrap().ends_with("a.jpg"));
        assert_eq!(g["removable"].as_array().unwrap().len(), 1);
        assert_eq!(g["reclaimable_bytes"], 4);
        assert_eq!(v["totals"]["duplicate_groups"], 1);
        assert_eq!(v["totals"]["removable_files"], 1);
        assert_eq!(v["totals"]["reclaimable_bytes"], 4);
    }

    #[test]
    fn parity_with_model_groups_and_keepers() {
        let dir = TempDir::new().unwrap();
        let outcome = outcome_from(
            &dir,
            &[
                ("a.jpg", b"alpha"),
                ("a2.jpg", b"alpha"),
                ("b.jpg", b"beta-x"),
                ("b2.jpg", b"beta-x"),
            ],
        );
        let v = parse(&render(&outcome, &[], &[], &[]));
        let groups = v["groups"].as_array().unwrap();
        assert_eq!(groups.len(), outcome.groups.len());
        for (jg, mg) in groups.iter().zip(outcome.groups.iter()) {
            assert_eq!(
                jg["keeper"]["path"].as_str().unwrap(),
                mg.keeper.path.to_string_lossy()
            );
            assert_eq!(
                jg["removable"].as_array().unwrap().len(),
                mg.removable.len()
            );
        }
    }

    #[test]
    fn output_is_deterministic_across_runs() {
        let dir = TempDir::new().unwrap();
        let outcome = outcome_from(
            &dir,
            &[("a.jpg", b"same"), ("b.jpg", b"same"), ("c.jpg", b"same")],
        );
        let warns = [SniffWarning {
            path: PathBuf::from("z.txt"),
            reason: "not an image".into(),
        }];
        let one = render(&outcome, &[], &warns, &[]);
        let two = render(&outcome, &[], &warns, &[]);
        assert_eq!(one, two);
    }

    #[test]
    fn warnings_and_root_errors_are_first_class_f3() {
        let dir = TempDir::new().unwrap();
        let outcome = outcome_from(&dir, &[]);
        let enum_warns = [EnumerateWarning {
            path: PathBuf::from("locked-dir/file.jpg"),
            reason: "permission denied".into(),
        }];
        let sniff_warns = [SniffWarning {
            path: PathBuf::from("fake.jpg"),
            reason: "not an image".into(),
        }];
        let roots = [EnumerateWarning {
            path: PathBuf::from("/no/such/root"),
            reason: "not found".into(),
        }];
        let v = parse(&render(&outcome, &enum_warns, &sniff_warns, &roots));
        let warnings = v["warnings"].as_array().unwrap();
        assert_eq!(warnings.len(), 2);
        assert_eq!(warnings[0]["stage"], "enumerate");
        assert_eq!(warnings[1]["stage"], "sniff");
        let roots_out = v["root_errors"].as_array().unwrap();
        assert_eq!(roots_out.len(), 1);
        assert_eq!(roots_out[0]["path"], "/no/such/root");
        assert_eq!(roots_out[0]["reason"], "not found");
    }

    #[test]
    fn mtime_serializes_as_null_when_unavailable() {
        let dir = TempDir::new().unwrap();
        let mut outcome = outcome_from(&dir, &[("a.jpg", b"same"), ("b.jpg", b"same")]);
        outcome.groups[0].keeper.mtime = None;
        let v = parse(&render(&outcome, &[], &[], &[]));
        assert!(v["groups"][0]["keeper"]["mtime"].is_null());
    }

    #[test]
    fn no_hash_field_leaks_into_schema() {
        let dir = TempDir::new().unwrap();
        let outcome = outcome_from(&dir, &[("a.jpg", b"same"), ("b.jpg", b"same")]);
        let rendered = render(&outcome, &[], &[], &[]);
        assert!(!rendered.contains("\"hash\""));
    }

    #[test]
    fn hard_link_set_attaches_to_group() {
        #[cfg(unix)]
        {
            let dir = TempDir::new().unwrap();
            let a = dir.path().join("a.jpg");
            fs::write(&a, b"same-bytes").unwrap();
            let link = dir.path().join("a-link.jpg");
            fs::hard_link(&a, &link).unwrap();
            let copy = dir.path().join("copy.jpg");
            fs::write(&copy, b"same-bytes").unwrap();
            let meta = fs::metadata(&a).unwrap();
            let confirmed = vec![
                crate::sniff::ConfirmedFile {
                    path: a,
                    size: meta.len(),
                    mtime: Some(1),
                },
                crate::sniff::ConfirmedFile {
                    path: link,
                    size: meta.len(),
                    mtime: Some(1),
                },
                crate::sniff::ConfirmedFile {
                    path: copy.clone(),
                    size: fs::metadata(&copy).unwrap().len(),
                    mtime: Some(1),
                },
            ];
            let outcome = find_duplicates(confirmed, build_comparator(KeepRule::Shortest));
            let v = parse(&render(&outcome, &[], &[], &[]));
            let groups = v["groups"].as_array().unwrap();
            assert_eq!(groups.len(), 1);
            assert_eq!(groups[0]["hard_link_sets"].as_array().unwrap().len(), 1);
            assert_eq!(v["already_deduplicated"].as_array().unwrap().len(), 0);
            assert_eq!(v["totals"]["already_deduplicated_sets"], 0);
        }
    }
}
