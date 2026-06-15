//! Human-readable dry-run report — TASK-0307, FR-9/FR-13/FR-17/NFR-6.
//!
//! Renders a [`DedupOutcome`] (plus upstream warnings from enumeration and
//! sniffing) into a human-readable summary string. The renderer writes to any
//! `Write` sink and does **not** hard-code stdout, so it is fully unit-testable.
//!
//! ## Design
//!
//! The report is a **pure formatter** over the canonical [`DedupOutcome`] that
//! the engine already produced. It performs no filesystem I/O, no re-scanning,
//! and no mutation (FR-9, AC-1). The `DedupOutcome` is already in canonical
//! order (ADR-0010, TASK-0306), so the report is deterministic without any
//! additional sorting here.
//!
//! ## Output structure
//!
//! ```text
//! [optional: per-file warnings from enumeration/sniff/engine]
//!
//! Group 1/N (M files, X bytes reclaimable):
//!   KEEP  /path/to/keeper.jpg
//!   REMOVE /path/to/dup1.jpg
//!   REMOVE /path/to/dup2.jpg
//!   [optional: hard-link note]
//!
//! ...
//!
//! Summary: N duplicate groups, M files, X bytes reclaimable.
//! [or: No duplicate groups found.]
//! ```
//!
//! The renderer is kept separate from the JSON formatter (TASK-0308) so both
//! can share the [`DedupOutcome`] model without coupling their output formats.

use crate::engine::DedupOutcome;
use crate::enumerate::EnumerateWarning;
use crate::sniff::SniffWarning;
use std::fmt::Write as FmtWrite;

// ── Public entry point ────────────────────────────────────────────────────────

/// Render a human-readable dry-run report into a `String`.
///
/// # Arguments
///
/// - `outcome` — the canonical [`DedupOutcome`] from the engine (already
///   sorted: groups by keeper path, warnings by path).
/// - `enumerate_warnings` — per-entry warnings from the enumeration stage.
/// - `sniff_warnings` — per-file warnings from the magic-byte sniff stage.
///
/// Engine warnings are included in `outcome.warnings`. Enumeration and sniff
/// warnings are passed separately so the renderer can surface them alongside
/// the engine warnings without the engine needing to know about those stages.
///
/// # Returns
///
/// A `String` ready to be written to stdout (or any sink). The string ends
/// with a single newline. It never panics — formatting to a `String` is
/// infallible.
pub fn render(
    outcome: &DedupOutcome,
    enumerate_warnings: &[EnumerateWarning],
    sniff_warnings: &[SniffWarning],
) -> String {
    let mut out = String::new();

    // ── Warnings section ──────────────────────────────────────────────────────
    // Surface all upstream + engine warnings (NFR-6). Listed before the groups
    // so the user sees them before the main report.
    let has_warnings = !enumerate_warnings.is_empty()
        || !sniff_warnings.is_empty()
        || !outcome.warnings.is_empty();

    if has_warnings {
        writeln!(out, "Warnings:").unwrap();
        for w in enumerate_warnings {
            writeln!(out, "  SKIP  {}: {}", w.path.display(), w.reason).unwrap();
        }
        for w in sniff_warnings {
            writeln!(out, "  SKIP  {}: {}", w.path.display(), w.reason).unwrap();
        }
        for w in &outcome.warnings {
            writeln!(out, "  SKIP  {}: {}", w.path.display(), w.reason).unwrap();
        }
        writeln!(out).unwrap();
    }

    // ── Duplicate groups ──────────────────────────────────────────────────────
    let total_groups = outcome.groups.len();

    if total_groups == 0 {
        writeln!(out, "No duplicate groups found.").unwrap();
        return out;
    }

    // Compute reclaimable space for each group: sum of non-keeper sizes.
    // Hard-link siblings are excluded from reclaimable-space (FR-17, ADR-0008):
    // removing one hard-link path frees no storage. Their sizes are already
    // excluded from the DuplicateGroup (they were collapsed to one representative
    // in the engine); the HardLinkNote paths are informational only.
    let mut total_reclaimable: u64 = 0;
    let mut total_files: usize = 0;

    for (i, group) in outcome.groups.iter().enumerate() {
        let group_reclaimable: u64 = group.removable.iter().map(|f| f.size).sum();
        let group_files = 1 + group.removable.len(); // keeper + removable
        total_reclaimable += group_reclaimable;
        total_files += group_files;

        writeln!(
            out,
            "Group {}/{} ({} files, {} bytes reclaimable):",
            i + 1,
            total_groups,
            group_files,
            group_reclaimable,
        )
        .unwrap();

        writeln!(out, "  KEEP  {}", group.keeper.path.display()).unwrap();
        for removable in &group.removable {
            writeln!(out, "  REMOVE {}", removable.path.display()).unwrap();
        }

        // Hard-link notes for paths that were collapsed within this group's
        // hash-match set (FR-17). Match notes to the current group by
        // representative path — a note's representative is always the file
        // that survived into the group's keeper or removable set.
        let group_paths: std::collections::HashSet<_> = std::iter::once(&group.keeper.path)
            .chain(group.removable.iter().map(|f| &f.path))
            .collect();
        for note in &outcome.hard_link_notes {
            if group_paths.contains(&note.representative) {
                writeln!(
                    out,
                    "  NOTE  hard-link set: {} → {}",
                    note.representative.display(),
                    note.siblings
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", "),
                )
                .unwrap();
            }
        }

        writeln!(out).unwrap();
    }

    // ── Summary line ──────────────────────────────────────────────────────────
    writeln!(
        out,
        "Summary: {} duplicate group{}, {} file{}, {} bytes reclaimable.",
        total_groups,
        if total_groups == 1 { "" } else { "s" },
        total_files,
        if total_files == 1 { "" } else { "s" },
        total_reclaimable,
    )
    .unwrap();

    out
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::KeepRule;
    use crate::engine::{find_duplicates, EngineWarning};
    use crate::keeper::build_comparator;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn write_file(dir: &TempDir, name: &str, bytes: &[u8]) -> PathBuf {
        let p = dir.path().join(name);
        fs::write(&p, bytes).unwrap();
        p
    }

    fn confirmed(path: &PathBuf) -> crate::sniff::ConfirmedFile {
        let meta = fs::metadata(path).unwrap();
        crate::sniff::ConfirmedFile {
            path: path.clone(),
            size: meta.len(),
            mtime: meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64),
        }
    }

    fn no_warnings() -> (Vec<EnumerateWarning>, Vec<SniffWarning>) {
        (Vec::new(), Vec::new())
    }

    // ── AC-1: dry run mutates nothing ────────────────────────────────────────

    #[test]
    fn dry_run_does_not_modify_any_file() {
        // AC-1 (FR-9, NFR-1): after render() the fixture files must be unchanged.
        let dir = tempfile::tempdir().unwrap();
        let a = write_file(&dir, "a.jpg", b"DUP CONTENT");
        let b = write_file(&dir, "b.jpg", b"DUP CONTENT");

        let outcome = find_duplicates(
            vec![confirmed(&a), confirmed(&b)],
            build_comparator(KeepRule::Shortest),
        );
        let (ew, sw) = no_warnings();
        let _ = render(&outcome, &ew, &sw);

        // Both files must still exist with their original content.
        assert_eq!(
            fs::read(&a).unwrap(),
            b"DUP CONTENT",
            "a.jpg must be untouched after render()"
        );
        assert_eq!(
            fs::read(&b).unwrap(),
            b"DUP CONTENT",
            "b.jpg must be untouched after render()"
        );
    }

    // ── AC-13: keeper, removed files, counts, reclaimable ────────────────────

    #[test]
    fn report_shows_keeper_and_removed_files() {
        // AC-13 (FR-13): the rendered report must mention the keeper path and
        // each removable path.
        let dir = tempfile::tempdir().unwrap();
        let a = write_file(&dir, "aaa.jpg", b"CONTENT");
        let b = write_file(&dir, "bbb.jpg", b"CONTENT");

        let outcome = find_duplicates(
            vec![confirmed(&a), confirmed(&b)],
            build_comparator(KeepRule::Shortest),
        );
        let (ew, sw) = no_warnings();
        let report = render(&outcome, &ew, &sw);

        assert!(
            report.contains("KEEP"),
            "report must show KEEP label; got:\n{report}"
        );
        assert!(
            report.contains("REMOVE"),
            "report must show REMOVE label; got:\n{report}"
        );
        assert!(
            report.contains(&a.to_string_lossy().to_string()),
            "report must mention keeper path; got:\n{report}"
        );
        assert!(
            report.contains(&b.to_string_lossy().to_string()),
            "report must mention removable path; got:\n{report}"
        );
    }

    #[test]
    fn report_shows_group_and_file_counts() {
        // AC-13 (FR-13): counts must appear in the summary.
        let dir = tempfile::tempdir().unwrap();
        let a = write_file(&dir, "a.jpg", b"SAME");
        let b = write_file(&dir, "b.jpg", b"SAME");

        let outcome = find_duplicates(
            vec![confirmed(&a), confirmed(&b)],
            build_comparator(KeepRule::Shortest),
        );
        let (ew, sw) = no_warnings();
        let report = render(&outcome, &ew, &sw);

        // Summary must mention counts.
        assert!(
            report.contains("1 duplicate group"),
            "summary must show group count; got:\n{report}"
        );
        assert!(
            report.contains("2 files"),
            "summary must show file count; got:\n{report}"
        );
    }

    #[test]
    fn report_shows_reclaimable_space() {
        // AC-13 (FR-13): reclaimable bytes must appear in the report.
        let dir = tempfile::tempdir().unwrap();
        // Write 10 bytes of content so we can assert the reclaimable figure.
        let content = b"0123456789"; // 10 bytes
        let a = write_file(&dir, "a.jpg", content);
        let b = write_file(&dir, "b.jpg", content);

        let outcome = find_duplicates(
            vec![confirmed(&a), confirmed(&b)],
            build_comparator(KeepRule::Shortest),
        );
        let (ew, sw) = no_warnings();
        let report = render(&outcome, &ew, &sw);

        // 1 removable × 10 bytes = 10 bytes reclaimable.
        assert!(
            report.contains("10 bytes reclaimable"),
            "report must show reclaimable space; got:\n{report}"
        );
    }

    // ── FR-17: hard-link notes ────────────────────────────────────────────────

    #[test]
    fn report_shows_hard_link_note() {
        // FR-17: hard-link siblings must appear as an informational note.
        let dir = tempfile::tempdir().unwrap();
        let a = write_file(&dir, "a.jpg", b"HL CONTENT");
        let b = dir.path().join("b.jpg");
        fs::hard_link(&a, &b).expect("hard link creatable in tempdir");
        let c = write_file(&dir, "c.jpg", b"HL CONTENT"); // separate copy

        let outcome = find_duplicates(
            vec![confirmed(&a), confirmed(&b), confirmed(&c)],
            build_comparator(KeepRule::Shortest),
        );
        let (ew, sw) = no_warnings();
        let report = render(&outcome, &ew, &sw);

        assert!(
            report.contains("NOTE") && report.contains("hard-link"),
            "report must show hard-link note; got:\n{report}"
        );
    }

    #[test]
    fn hard_link_reclaimable_excludes_hard_link_sizes() {
        // FR-17: hard-link siblings are excluded from reclaimable-space.
        // The only removable in this test is the separate copy (c); the
        // hard-link sibling (b) is only in the NOTE line.
        let dir = tempfile::tempdir().unwrap();
        let content = b"0123456789"; // 10 bytes
        let a = write_file(&dir, "a.jpg", content);
        let b = dir.path().join("b.jpg");
        fs::hard_link(&a, &b).expect("hard link creatable in tempdir");
        let c = write_file(&dir, "c.jpg", content); // separate copy

        let outcome = find_duplicates(
            vec![confirmed(&a), confirmed(&b), confirmed(&c)],
            build_comparator(KeepRule::Shortest),
        );
        let (ew, sw) = no_warnings();
        let report = render(&outcome, &ew, &sw);

        // Reclaimable must be 10 (the separate copy c), NOT 20 (which would
        // incorrectly include the hard-link sibling b).
        assert!(
            report.contains("10 bytes reclaimable"),
            "reclaimable must exclude hard-link sibling; got:\n{report}"
        );
    }

    // ── NFR-6: warnings surfaced ──────────────────────────────────────────────

    #[test]
    fn enumerate_warnings_are_surfaced_in_report() {
        // NFR-6: per-file skip warnings from enumeration must appear in report.
        let outcome = DedupOutcome::default();
        let ew = vec![EnumerateWarning {
            path: PathBuf::from("/some/file.jpg"),
            reason: "could not read: permission denied".to_owned(),
        }];
        let sw: Vec<SniffWarning> = vec![];

        let report = render(&outcome, &ew, &sw);

        assert!(
            report.contains("Warnings:"),
            "report must show Warnings section; got:\n{report}"
        );
        assert!(
            report.contains("/some/file.jpg"),
            "warning path must appear; got:\n{report}"
        );
        assert!(
            report.contains("permission denied"),
            "warning reason must appear; got:\n{report}"
        );
    }

    #[test]
    fn sniff_warnings_are_surfaced_in_report() {
        // NFR-6: magic-byte rejection warnings must appear in report.
        let outcome = DedupOutcome::default();
        let ew: Vec<EnumerateWarning> = vec![];
        let sw = vec![SniffWarning {
            path: PathBuf::from("/fake.jpg"),
            reason: "content not recognized as a supported image format".to_owned(),
        }];

        let report = render(&outcome, &ew, &sw);

        assert!(
            report.contains("/fake.jpg"),
            "sniff warning path must appear; got:\n{report}"
        );
        assert!(
            report.contains("not recognized"),
            "sniff warning reason must appear; got:\n{report}"
        );
    }

    #[test]
    fn engine_warnings_are_surfaced_in_report() {
        // NFR-6: engine warnings (hash I/O errors) must appear in report.
        let outcome = DedupOutcome {
            warnings: vec![EngineWarning {
                path: PathBuf::from("/bad.jpg"),
                reason: "could not hash file: permission denied".to_owned(),
            }],
            ..DedupOutcome::default()
        };
        let (ew, sw) = no_warnings();

        let report = render(&outcome, &ew, &sw);

        assert!(
            report.contains("/bad.jpg"),
            "engine warning must appear; got:\n{report}"
        );
        assert!(
            report.contains("permission denied"),
            "engine warning reason must appear; got:\n{report}"
        );
    }

    #[test]
    fn per_file_skip_alone_still_shows_no_duplicate_groups() {
        // NFR-6: when only warnings exist (no duplicate groups), the report
        // shows the warning section AND the "No duplicate groups found." message.
        let outcome = DedupOutcome::default();
        let ew = vec![EnumerateWarning {
            path: PathBuf::from("/skip.jpg"),
            reason: "skipped".to_owned(),
        }];
        let sw: Vec<SniffWarning> = vec![];

        let report = render(&outcome, &ew, &sw);

        assert!(
            report.contains("Warnings:"),
            "warnings section must appear; got:\n{report}"
        );
        assert!(
            report.contains("No duplicate groups found."),
            "must still say no groups; got:\n{report}"
        );
    }

    // ── No duplicates ─────────────────────────────────────────────────────────

    #[test]
    fn no_duplicates_shows_appropriate_message() {
        let outcome = DedupOutcome::default();
        let (ew, sw) = no_warnings();
        let report = render(&outcome, &ew, &sw);
        assert!(
            report.contains("No duplicate groups found."),
            "empty outcome should say no groups; got:\n{report}"
        );
    }

    // ── Canonical ordering ────────────────────────────────────────────────────

    #[test]
    fn report_groups_appear_in_canonical_order() {
        // Output ordering is the canonical order (already sorted in engine).
        // Groups appear in keeper-path ascending order.
        let dir = tempfile::tempdir().unwrap();
        let a1 = write_file(&dir, "aaa1.jpg", b"GROUP-A");
        let a2 = write_file(&dir, "aaa2.jpg", b"GROUP-A");
        let b1 = write_file(&dir, "bbb1.jpg", b"GROUP-B");
        let b2 = write_file(&dir, "bbb2.jpg", b"GROUP-B");

        let outcome = find_duplicates(
            vec![
                confirmed(&b1),
                confirmed(&a1),
                confirmed(&b2),
                confirmed(&a2),
            ],
            build_comparator(KeepRule::Shortest),
        );
        let (ew, sw) = no_warnings();
        let report = render(&outcome, &ew, &sw);

        // aaa1 (group A) must appear before bbb1 (group B) in the text.
        let pos_a = report
            .find(&a1.to_string_lossy().to_string())
            .expect("a1 path must appear in report");
        let pos_b = report
            .find(&b1.to_string_lossy().to_string())
            .expect("b1 path must appear in report");
        assert!(
            pos_a < pos_b,
            "group A (keeper aaa1) must appear before group B (keeper bbb1) in report"
        );
    }

    // ── Multiple groups summary ───────────────────────────────────────────────

    #[test]
    fn summary_counts_multiple_groups_correctly() {
        // Two groups, three total files (1 keeper + 1 removable each).
        let dir = tempfile::tempdir().unwrap();
        let a1 = write_file(&dir, "aa1.jpg", b"ALPHA");
        let a2 = write_file(&dir, "aa2.jpg", b"ALPHA");
        let b1 = write_file(&dir, "bb1.jpg", b"BETA_");
        let b2 = write_file(&dir, "bb2.jpg", b"BETA_");

        let outcome = find_duplicates(
            vec![
                confirmed(&a1),
                confirmed(&a2),
                confirmed(&b1),
                confirmed(&b2),
            ],
            build_comparator(KeepRule::Shortest),
        );
        let (ew, sw) = no_warnings();
        let report = render(&outcome, &ew, &sw);

        assert!(
            report.contains("2 duplicate groups"),
            "summary must show 2 groups; got:\n{report}"
        );
        assert!(
            report.contains("4 files"),
            "summary must show 4 files; got:\n{report}"
        );
    }
}
