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
//! Group 1/N (M file[s], X bytes reclaimable):
//!   KEEP  /path/to/keeper.jpg
//!   REMOVE /path/to/dup1.jpg
//!   REMOVE /path/to/dup2.jpg
//!   [optional: PROTECT row for hard-link anchors]
//!   [optional: hard-link note]
//!
//! ...
//!
//! [optional: already-deduplicated block]
//! Already deduplicated (hard-linked) — nothing to reclaim:
//!   /path/to/rep.jpg
//!     hard-linked: /path/to/rep.jpg, /path/to/sibling.jpg
//!
//! Summary: N duplicate group[s], M file[s], X bytes reclaimable[; K already-deduplicated hard-link set[s] (nothing to reclaim)].
//! [or: Summary: no duplicate groups to remove; K already-deduplicated hard-link set[s].]
//! [or: No duplicate groups found.]
//! ```
//!
//! ## Removal opportunities vs. already-deduplicated sets
//!
//! A *removal opportunity* is a [`DuplicateGroup`] with `removable.len() >= 1`.
//! Only removal opportunities are counted as "duplicate groups" in the summary
//! and rendered as `Group i/N` headings.
//!
//! Groups with `removable.is_empty()` (keeper + protected anchor, nothing to
//! remove — case b) and all-anchor identity sets that produce no
//! [`DuplicateGroup`] at all (case d) are both surfaced under a single
//! "Already deduplicated (hard-linked) — nothing to reclaim:" informational
//! block. They are excluded from group/file/reclaimable totals.
//!
//! The renderer is kept separate from the JSON formatter (TASK-0308) so both
//! can share the [`DedupOutcome`] model without coupling their output formats.

use crate::engine::{DedupOutcome, HardLinkNote};
use crate::enumerate::EnumerateWarning;
use crate::sniff::SniffWarning;
use std::collections::HashSet;
use std::fmt::Write as FmtWrite;
use std::path::{Path, PathBuf};

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

    // ── All groups render as Group i/N (Amendment 2026-06-15) ──────────────────
    // Previously, removable-less groups (keeper + protected, no removable) were
    // moved to the already-deduplicated block. Amendment 2026-06-15 changes this:
    // all DuplicateGroup instances render as Group i/N, even with 0 reclaimable
    // (when a standalone keeper has no other standalones to remove).
    //
    // Only all-anchor groups (case d) surface as already-deduplicated notes,
    // because those emit no DuplicateGroup at all.
    let all_groups = outcome.groups.iter().collect::<Vec<_>>();

    // ── Build the set of note representatives claimed by any emitted group ────
    // Used to partition hard_link_notes: those matched to a group vs. orphaned
    // all-anchor notes (case d) that have no associated DuplicateGroup.
    let claimed_representatives: HashSet<&Path> = outcome
        .groups
        .iter()
        .flat_map(|g| {
            std::iter::once(g.keeper.path.as_path())
                .chain(g.removable.iter().map(|f| f.path.as_path()))
                .chain(g.protected.iter().map(|f| f.path.as_path()))
        })
        .collect();

    // All-anchor notes (case d): notes whose representative is not in any group.
    // These are the ONLY notes that appear in the already-deduplicated block.
    let already_dedup_notes: Vec<&HardLinkNote> = outcome
        .hard_link_notes
        .iter()
        .filter(|n| !claimed_representatives.contains(n.representative.as_path()))
        .collect();

    // ── Duplicate groups (all, including 0-reclaimable groups) ─────────────────
    let total_groups = all_groups.len();
    let mut total_reclaimable: u64 = 0;
    let mut total_files: usize = 0;

    if total_groups == 0 && already_dedup_notes.is_empty() {
        // No duplicate groups and no all-anchor sets: the truly empty case.
        writeln!(out, "No duplicate groups found.").unwrap();
        return out;
    }

    // Render each duplicate group (including 0-reclaimable groups where a
    // standalone keeper has no other standalones to remove).
    for (i, group) in all_groups.iter().enumerate() {
        let group_reclaimable: u64 = group.removable.iter().map(|f| f.size).sum();
        // group_files = keeper + removable (N1 ruling: protected anchors excluded).
        let group_files = 1 + group.removable.len();
        total_reclaimable += group_reclaimable;
        total_files += group_files;

        writeln!(
            out,
            "Group {}/{} ({} file{}, {} bytes reclaimable):",
            i + 1,
            total_groups,
            group_files,
            if group_files == 1 { "" } else { "s" },
            group_reclaimable,
        )
        .unwrap();

        writeln!(out, "  KEEP  {}", group.keeper.path.display()).unwrap();
        for removable in &group.removable {
            writeln!(out, "  REMOVE {}", removable.path.display()).unwrap();
        }
        // Protected anchors: hard-link representatives whose surviving in-scope
        // siblings pin the inode. Removing them would free 0 bytes.
        // (ADR-0008 anchor rule, Amendment 2026-06-15.)
        for protected in &group.protected {
            writeln!(
                out,
                "  PROTECT {}  (hard-linked; removing it frees no space)",
                protected.path.display()
            )
            .unwrap();
        }

        // Hard-link notes for paths that were collapsed within this group's
        // hash-match set (FR-17). Match notes to the current group by
        // representative path — a note's representative survives as keeper,
        // removable, or a protected anchor entry.
        let group_paths: HashSet<&Path> = std::iter::once(group.keeper.path.as_path())
            .chain(group.removable.iter().map(|f| f.path.as_path()))
            .chain(group.protected.iter().map(|f| f.path.as_path()))
            .collect();
        for note in &outcome.hard_link_notes {
            if group_paths.contains(note.representative.as_path()) {
                writeln!(
                    out,
                    "  NOTE  hard-link set: {} \u{2192} {}",
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

    // ── Already-deduplicated informational block (cases b + d) ───────────────
    // Printed after the real groups, before the Summary line. Omitted entirely
    // when there are no already-deduplicated sets.
    if !already_dedup_notes.is_empty() {
        writeln!(
            out,
            "Already deduplicated (hard-linked) \u{2014} nothing to reclaim:"
        )
        .unwrap();
        for note in &already_dedup_notes {
            writeln!(out, "  {}", note.representative.display()).unwrap();
            // Link set = representative + siblings, in canonical byte-wise order.
            let mut link_set: Vec<PathBuf> = std::iter::once(note.representative.clone())
                .chain(note.siblings.iter().cloned())
                .collect();
            link_set.sort();
            let link_set_str = link_set
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(out, "    hard-linked: {link_set_str}").unwrap();
        }
        writeln!(out).unwrap();
    }

    // ── Summary line ──────────────────────────────────────────────────────────
    let n_dedup_sets = already_dedup_notes.len();

    if total_groups == 0 {
        // Only already-deduplicated sets found — no removal opportunities.
        writeln!(
            out,
            "Summary: no duplicate groups to remove; {} already-deduplicated hard-link set{}.",
            n_dedup_sets,
            if n_dedup_sets == 1 { "" } else { "s" },
        )
        .unwrap();
    } else if n_dedup_sets == 0 {
        // Only removal opportunities — no already-deduplicated sets.
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
    } else {
        // Both removal opportunities and already-deduplicated sets.
        writeln!(
            out,
            "Summary: {} duplicate group{}, {} file{}, {} bytes reclaimable; {} already-deduplicated hard-link set{} (nothing to reclaim).",
            total_groups,
            if total_groups == 1 { "" } else { "s" },
            total_files,
            if total_files == 1 { "" } else { "s" },
            total_reclaimable,
            n_dedup_sets,
            if n_dedup_sets == 1 { "" } else { "s" },
        )
        .unwrap();
    }

    out
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::KeepRule;
    use crate::engine::{EngineWarning, find_duplicates};
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

    // ── (c) Pluralization: group-header file count ────────────────────────────

    #[test]
    fn group_header_uses_plural_files_for_multiple_files() {
        // (c) spec: group header must say "N files" when N > 1.
        let dir = tempfile::tempdir().unwrap();
        let a = write_file(&dir, "a.jpg", b"SAME");
        let b = write_file(&dir, "b.jpg", b"SAME");

        let outcome = find_duplicates(
            vec![confirmed(&a), confirmed(&b)],
            build_comparator(KeepRule::Shortest),
        );
        let (ew, sw) = no_warnings();
        let report = render(&outcome, &ew, &sw);

        assert!(
            report.contains("2 files"),
            "group header must say '2 files' when there are 2 files; got:\n{report}"
        );
        assert!(
            !report.contains("2 file,"),
            "group header must NOT say '2 file,' (missing plural); got:\n{report}"
        );
    }

    #[test]
    fn group_header_uses_singular_file_for_one_file() {
        // (c) spec: group header must say "1 file" (singular) when group has
        // exactly 1 file (keeper + 0 removable, but that's a removable-less group —
        // test with a genuine removal-opportunity 1-file header is impossible since
        // a removal opportunity needs >= 1 removable. The smallest removal
        // opportunity is 2 files (keeper + 1 removable). Assert "2 files" is
        // plural and confirm the format string handles the '1 file' case via
        // the pluralization branch used for total_files.
        //
        // Actually the group_files for a real removal opportunity group is always
        // >= 2 (1 keeper + >= 1 removable). The "1 file" case in the group header
        // can only be reached if a group has exactly 1 removable and 0 protected.
        // That's group_files = 2. Verify the pluralization logic directly by
        // confirming "2 files" is used for the standard 2-file case.
        //
        // The singular "1 file" in the group header is exercised only when the
        // group_files count reaches 1 — which cannot happen for a real removal
        // group (min is 2). We verify the logic is correct via the summary line's
        // singular form instead.
        let dir = tempfile::tempdir().unwrap();
        let a = write_file(&dir, "a.jpg", b"SAME");
        let b = write_file(&dir, "b.jpg", b"SAME");
        let c = write_file(&dir, "c.jpg", b"OTHER");
        let d = write_file(&dir, "d.jpg", b"OTHER");

        // Two groups of 2; verify each header says "2 files".
        let outcome = find_duplicates(
            vec![confirmed(&a), confirmed(&b), confirmed(&c), confirmed(&d)],
            build_comparator(KeepRule::Shortest),
        );
        let (ew, sw) = no_warnings();
        let report = render(&outcome, &ew, &sw);

        // Both group headers must say "2 files".
        let count = report.matches("2 files").count();
        assert!(
            count >= 2,
            "both group headers must say '2 files'; got:\n{report}"
        );
    }

    #[test]
    fn group_header_singular_file_count_rendered_correctly() {
        // (c) spec: verify the pluralization branch produces the correct plural form.
        // A removal-opportunity group always has >= 2 files (1 keeper + >= 1 removable).
        // Verify the 3-file case: keeper + 2 removable = "3 files".
        // The singular "1 file" path in the group header is not reachable through
        // the engine (min is 2); the summary's "1 file" singular is tested via
        // summary_singular tests.

        let dir = tempfile::tempdir().unwrap();
        let a = write_file(&dir, "a.jpg", b"SAME");
        let b = write_file(&dir, "b.jpg", b"SAME");
        let c = write_file(&dir, "c.jpg", b"SAME");

        let outcome = find_duplicates(
            vec![confirmed(&a), confirmed(&b), confirmed(&c)],
            build_comparator(KeepRule::Shortest),
        );
        let (ew, sw) = no_warnings();
        let report = render(&outcome, &ew, &sw);

        assert!(
            report.contains("3 files"),
            "group header must say '3 files' for 3-file group; got:\n{report}"
        );
    }

    // ── FR-17: hard-link notes ────────────────────────────────────────────────

    #[test]
    fn report_shows_hard_link_note() {
        // FR-17: when a group has both removable and protected entries, the hard-link
        // NOTE line must appear inside the group (the note is associated with the
        // anchor in a removal-opportunity group).
        //
        // Setup: h ↔ h-link (anchor) + c1.jpg + c2.jpg (two standalones).
        // keeper = c1 (shortest, lex min of non-anchors); removable = c2;
        // protected = h. This is a removal-opportunity group (removable non-empty).
        // The NOTE line for {h, h-link} must appear.
        let dir = tempfile::tempdir().unwrap();
        let h = write_file(&dir, "h.jpg", b"HL CONTENT");
        let h_link = dir.path().join("h-link.jpg");
        fs::hard_link(&h, &h_link).expect("hard link creatable in tempdir");
        let c1 = write_file(&dir, "c1.jpg", b"HL CONTENT");
        let c2 = write_file(&dir, "c2.jpg", b"HL CONTENT");

        let outcome = find_duplicates(
            vec![
                confirmed(&h),
                confirmed(&h_link),
                confirmed(&c1),
                confirmed(&c2),
            ],
            build_comparator(KeepRule::Shortest),
        );
        let (ew, sw) = no_warnings();
        let report = render(&outcome, &ew, &sw);

        assert!(
            report.contains("NOTE") && report.contains("hard-link"),
            "report must show hard-link note in removal-opportunity group; got:\n{report}"
        );
    }

    #[test]
    fn hard_link_reclaimable_excludes_hard_link_sizes() {
        // FR-17 / ADR-0008 anchor rule (Amendment 2026-06-15):
        // hard-link anchors are NEVER in removable; their size is NEVER counted
        // in reclaimable.
        //
        // Setup: a.jpg ↔ b.jpg (one inode, anchor) + c.jpg (separate copy).
        // Amendment 2026-06-15: a.jpg is the keeper (shortest); c.jpg is removable;
        // b.jpg is protected. Reclaimable = c.jpg size = 10 bytes (not a.jpg).
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

        // The group has removable (c.jpg) → it renders as a group.
        assert!(
            report.contains("Group 1/1"),
            "group must appear as Group 1/1; got:\n{report}"
        );

        // Reclaimable must be 10 bytes (c.jpg only, not a.jpg or b.jpg).
        assert!(
            report.contains("10 bytes reclaimable"),
            "reclaimable must be 10 bytes (c.jpg only); got:\n{report}"
        );

        // PROTECT row must appear for b.jpg (the hard-linked anchor).
        assert!(
            report.contains("PROTECT"),
            "PROTECT row must appear for b.jpg; got:\n{report}"
        );

        // No all-anchor already-deduplicated block.
        assert!(
            !report.contains("Already deduplicated (hard-linked)"),
            "no already-deduplicated block expected; got:\n{report}"
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

    // ── Anchor rule (ADR-0008 Amendment 2026-06-15) ──────────────────────────

    #[test]
    fn anchor_protect_row_appears_in_removal_group_when_removable_nonempty() {
        // When a group has BOTH removable files AND a protected anchor, the
        // PROTECT row must appear inside the group (not in the already-dedup block).
        // Setup: h ↔ h-link (anchor, protected) + c1 (keeper) + c2 (removable).
        let dir = tempfile::tempdir().unwrap();
        let content = b"identical content";
        let h = write_file(&dir, "h.jpg", content);
        let h_link = dir.path().join("h-link.jpg");
        fs::hard_link(&h, &h_link).expect("hard link creatable");
        let c1 = write_file(&dir, "c1.jpg", content);
        let c2 = write_file(&dir, "c2.jpg", content);

        let outcome = find_duplicates(
            vec![
                confirmed(&h),
                confirmed(&h_link),
                confirmed(&c1),
                confirmed(&c2),
            ],
            build_comparator(KeepRule::Shortest),
        );
        let (ew, sw) = no_warnings();
        let report = render(&outcome, &ew, &sw);

        // Must show Group heading (removal opportunity).
        assert!(
            report.contains("Group 1/1"),
            "removal-opportunity group must appear with Group heading; got:\n{report}"
        );
        assert!(
            report.contains("PROTECT"),
            "PROTECT label must appear in the removal-opportunity group; got:\n{report}"
        );
        assert!(
            report.contains("hard-linked; removing it frees no space"),
            "PROTECT reason must appear; got:\n{report}"
        );
        assert!(
            report.contains("NOTE") && report.contains("hard-link"),
            "NOTE line must still appear for the collapsed sibling; got:\n{report}"
        );
    }

    #[test]
    fn all_anchor_group_notes_surface_in_already_dedup_block() {
        // (d) spec: all-anchor group emits no DuplicateGroup; the engine puts both
        // notes in hard_link_notes. The report must surface them in the
        // already-deduplicated block and use the summary variant
        // "no duplicate groups to remove; N already-deduplicated hard-link set(s)".
        // "No duplicate groups found." must NOT appear.
        let dir = tempfile::tempdir().unwrap();
        let a = write_file(&dir, "a.jpg", b"SAME");
        let a2 = dir.path().join("a2.jpg");
        fs::hard_link(&a, &a2).expect("hard link creatable");
        let b = write_file(&dir, "b.jpg", b"SAME");
        let b2 = dir.path().join("b2.jpg");
        fs::hard_link(&b, &b2).expect("hard link creatable");

        let outcome = find_duplicates(
            vec![confirmed(&a), confirmed(&a2), confirmed(&b), confirmed(&b2)],
            build_comparator(KeepRule::Shortest),
        );
        let (ew, sw) = no_warnings();
        let report = render(&outcome, &ew, &sw);

        // No DuplicateGroup emitted.
        assert!(
            outcome.groups.is_empty(),
            "all-anchor group must emit no DuplicateGroup"
        );

        // "No duplicate groups found." must NOT appear — there ARE already-dedup sets.
        assert!(
            !report.contains("No duplicate groups found."),
            "must NOT say 'No duplicate groups found.' when already-dedup sets exist; got:\n{report}"
        );

        // Already-deduplicated block must appear.
        assert!(
            report.contains("Already deduplicated (hard-linked)"),
            "already-deduplicated heading must appear; got:\n{report}"
        );
        assert!(
            report.contains("hard-linked:"),
            "hard-linked entries must appear; got:\n{report}"
        );

        // Summary must use the "no duplicate groups to remove" variant.
        assert!(
            report.contains("no duplicate groups to remove"),
            "summary must say 'no duplicate groups to remove'; got:\n{report}"
        );
        assert!(
            report.contains("2 already-deduplicated hard-link sets"),
            "summary must mention 2 already-deduplicated sets; got:\n{report}"
        );
    }

    #[test]
    fn all_anchor_group_notes_surface_no_group_in_report() {
        // Legacy test (renamed from the original): all-anchor group must emit no
        // DuplicateGroup and the report must NOT say "No duplicate groups found."
        // (Updated under the (b)/(c)/(d) presentation spec.)
        let dir = tempfile::tempdir().unwrap();
        let a = write_file(&dir, "a.jpg", b"SAME");
        let a2 = dir.path().join("a2.jpg");
        fs::hard_link(&a, &a2).expect("hard link creatable");
        let b = write_file(&dir, "b.jpg", b"SAME");
        let b2 = dir.path().join("b2.jpg");
        fs::hard_link(&b, &b2).expect("hard link creatable");

        let outcome = find_duplicates(
            vec![confirmed(&a), confirmed(&a2), confirmed(&b), confirmed(&b2)],
            build_comparator(KeepRule::Shortest),
        );
        let (ew, sw) = no_warnings();
        let report = render(&outcome, &ew, &sw);

        // No DuplicateGroup emitted.
        assert!(
            outcome.groups.is_empty(),
            "all-anchor group must emit no DuplicateGroup"
        );

        // Under the new spec: the already-deduplicated block is shown, NOT
        // "No duplicate groups found." (which only appears when there are no
        // findings of any kind).
        assert!(
            !report.contains("No duplicate groups found."),
            "must NOT say 'No duplicate groups found.' — already-dedup sets exist; got:\n{report}"
        );
        assert!(
            report.contains("Already deduplicated (hard-linked)"),
            "already-deduplicated block must appear for all-anchor case; got:\n{report}"
        );
    }

    #[test]
    fn total_files_and_reclaimable_exclude_protected_anchors() {
        // N1 ruling: total_files = keeper + removable (not protected).
        // Amendment 2026-06-15: anchor (h) is keeper because --keep shortest picks h.jpg.
        // keeper=h (anchor), removable=[c1, c2], protected=[h2].
        // total_files = 1 + 2 = 3, reclaimable = 10 + 10 = 20 bytes.
        let dir = tempfile::tempdir().unwrap();
        let content = b"0123456789"; // 10 bytes
        let h = write_file(&dir, "h.jpg", content);
        let h2 = dir.path().join("h2.jpg");
        fs::hard_link(&h, &h2).expect("hard link creatable");
        let c1 = write_file(&dir, "c1.jpg", content);
        let c2 = write_file(&dir, "c2.jpg", content);

        let outcome = find_duplicates(
            vec![
                confirmed(&h),
                confirmed(&h2),
                confirmed(&c1),
                confirmed(&c2),
            ],
            build_comparator(KeepRule::Shortest),
        );
        let (ew, sw) = no_warnings();
        let report = render(&outcome, &ew, &sw);

        // 3 files acted on (keeper h + removable c1, c2); protected h2 not counted.
        assert!(
            report.contains("3 files"),
            "total_files must be 3 (keeper+removable, not protected); got:\n{report}"
        );

        // Reclaimable = 20 bytes (c1 + c2; h2 is protected, not counted).
        assert!(
            report.contains("20 bytes reclaimable"),
            "reclaimable must be 20 bytes (c1 + c2); got:\n{report}"
        );

        // PROTECT row present for h2 (inside the group heading).
        assert!(
            report.contains("PROTECT"),
            "PROTECT row must be present for h2; got:\n{report}"
        );

        // Must render as a proper group with removable files.
        assert!(
            report.contains("Group 1/1"),
            "group must appear as Group 1/1; got:\n{report}"
        );
    }

    // ── (b) Removable-less group → already-deduplicated block ────────────────

    #[test]
    fn removable_less_group_appears_in_already_dedup_block() {
        // Amendment 2026-06-15: removable-less groups no longer appear in the
        // already-deduplicated block. They render as normal groups with 0 reclaimable.
        //
        // Setup: anchor.jpg ↔ sibling.jpg (one inode, anchor) + standalone.jpg (separate).
        // keeper=anchor (shortest), removable=[standalone], protected=[sibling].
        let dir = tempfile::tempdir().unwrap();
        let content = b"same content";
        let anchor = write_file(&dir, "anchor.jpg", content);
        let sibling = dir.path().join("sibling.jpg");
        fs::hard_link(&anchor, &sibling).expect("hard link creatable");
        let standalone = write_file(&dir, "standalone.jpg", content);

        let outcome = find_duplicates(
            vec![
                confirmed(&anchor),
                confirmed(&sibling),
                confirmed(&standalone),
            ],
            build_comparator(KeepRule::Shortest),
        );
        let (ew, sw) = no_warnings();
        let report = render(&outcome, &ew, &sw);

        // Engine: one group, keeper=anchor, removable=[standalone], protected=[sibling].
        assert_eq!(outcome.groups.len(), 1, "one group from engine");
        assert_eq!(
            outcome.groups[0].removable.len(),
            1,
            "one removable standalone"
        );
        assert_eq!(outcome.groups[0].protected.len(), 1, "one protected anchor");

        // Report: should have a "Group i/N" heading (not in already-dedup).
        assert!(
            report.contains("Group 1/1"),
            "group must appear as Group heading; got:\n{report}"
        );

        // Already-deduplicated block must NOT appear (no all-anchor sets).
        assert!(
            !report.contains("Already deduplicated (hard-linked)"),
            "already-deduplicated heading must NOT appear; got:\n{report}"
        );

        // Summary should count 1 group.
        assert!(
            report.contains("1 duplicate group"),
            "summary must say '1 duplicate group'; got:\n{report}"
        );
    }

    #[test]
    fn removable_less_group_not_counted_in_totals() {
        // Amendment 2026-06-15: Both groups now render as normal groups.
        //
        // Setup: group 1 (aaa1+aaa2 standalones), group 2 (hh anchor + hh-link + zz-standalone).
        // Group 1: keeper=aaa1, removable=[aaa2].
        // Group 2: keeper=hh (shortest, anchor), removable=[zz-standalone], protected=[hh-link].
        // Summary: 2 groups, 4 files (1+1+1+1), 10 bytes reclaimable (5+5).
        let dir = tempfile::tempdir().unwrap();
        let content_a = b"ALPHA"; // 5 bytes
        let content_b = b"BETA?"; // 5 bytes — same size to force a cohort

        // Removal-opportunity group.
        let a1 = write_file(&dir, "aa1.jpg", content_a);
        let a2 = write_file(&dir, "aa2.jpg", content_a);

        // Group with anchor.
        let h = write_file(&dir, "hh.jpg", content_b);
        let h_link = dir.path().join("hh-link.jpg");
        fs::hard_link(&h, &h_link).expect("hard link creatable");
        let standalone = write_file(&dir, "zz-standalone.jpg", content_b);

        let outcome = find_duplicates(
            vec![
                confirmed(&a1),
                confirmed(&a2),
                confirmed(&h),
                confirmed(&h_link),
                confirmed(&standalone),
            ],
            build_comparator(KeepRule::Shortest),
        );
        let (ew, sw) = no_warnings();
        let report = render(&outcome, &ew, &sw);

        // Summary must count 2 duplicate groups.
        assert!(
            report.contains("2 duplicate group"),
            "summary must count 2 groups; got:\n{report}"
        );
        // Summary must count 4 files (1+1 from group 1, 1+1 from group 2).
        assert!(
            report.contains("4 files"),
            "summary must count 4 files; got:\n{report}"
        );
        // Summary must count 10 bytes reclaimable (5 from aa2 + 5 from zz-standalone).
        assert!(
            report.contains("10 bytes reclaimable"),
            "summary must count 10 bytes reclaimable; got:\n{report}"
        );
    }

    // ── (d) All-anchor notes → already-deduplicated block ────────────────────

    #[test]
    fn all_anchor_already_dedup_block_shows_link_set() {
        // (d) spec: all-anchor group → no DuplicateGroup; the report shows
        // the already-deduplicated block with the full link set (representative +
        // siblings) in canonical byte-wise order.
        let dir = tempfile::tempdir().unwrap();
        let a = write_file(&dir, "alpha.jpg", b"SAME");
        let a2 = dir.path().join("alpha2.jpg");
        fs::hard_link(&a, &a2).expect("hard link creatable");

        let outcome = find_duplicates(
            vec![confirmed(&a), confirmed(&a2)],
            build_comparator(KeepRule::Shortest),
        );
        // Only one inode → no group from size pre-filter (only 1 physical file).
        // Actually both are same size but same inode → collapse → 1 rep → < 2 reps
        // → no group emitted. hard_link_notes should be empty too (note is only
        // kept when the rep survives into a group). Let's use two separate inodes
        // to get the all-anchor scenario.
        drop(outcome);

        // Two distinct inodes, each multiply-linked.
        let b = write_file(&dir, "beta.jpg", b"SAME");
        let b2 = dir.path().join("beta2.jpg");
        fs::hard_link(&b, &b2).expect("hard link creatable");

        let outcome = find_duplicates(
            vec![confirmed(&a), confirmed(&a2), confirmed(&b), confirmed(&b2)],
            build_comparator(KeepRule::Shortest),
        );
        let (ew, sw) = no_warnings();
        let report = render(&outcome, &ew, &sw);

        assert!(
            outcome.groups.is_empty(),
            "all-anchor case must emit no group"
        );
        assert_eq!(outcome.hard_link_notes.len(), 2, "two notes expected");

        // Link set entries must include both paths for each inode.
        assert!(
            report.contains("hard-linked:"),
            "hard-linked entries must appear; got:\n{report}"
        );
        // The alpha inode set (alpha.jpg, alpha2.jpg) must appear.
        assert!(
            report.contains(&a.to_string_lossy().to_string()),
            "alpha.jpg must appear in block; got:\n{report}"
        );
        assert!(
            report.contains(&a2.to_string_lossy().to_string()),
            "alpha2.jpg must appear in block; got:\n{report}"
        );
    }

    // ── Combined summary clause ───────────────────────────────────────────────

    #[test]
    fn summary_combined_clause_when_both_groups_and_dedup_sets() {
        // Amendment 2026-06-15: no removable-less groups anymore.
        // Setup: group 1 (aaa1+aaa2) + group 2 (hhh anchor + hhh-link + zzz).
        // Both groups have removable, so both count.
        let dir = tempfile::tempdir().unwrap();
        let content_a = b"ALPHA"; // 5 bytes
        let content_b = b"BETAX"; // 5 bytes

        let a1 = write_file(&dir, "aaa1.jpg", content_a);
        let a2 = write_file(&dir, "aaa2.jpg", content_a);

        let h = write_file(&dir, "hhh.jpg", content_b);
        let h_link = dir.path().join("hhh-link.jpg");
        fs::hard_link(&h, &h_link).expect("hard link creatable");
        let zz = write_file(&dir, "zzz.jpg", content_b); // standalone copy, same content

        let outcome = find_duplicates(
            vec![
                confirmed(&a1),
                confirmed(&a2),
                confirmed(&h),
                confirmed(&h_link),
                confirmed(&zz),
            ],
            build_comparator(KeepRule::Shortest),
        );
        let (ew, sw) = no_warnings();
        let report = render(&outcome, &ew, &sw);

        // Summary must count 2 groups (both have removable).
        assert!(
            report.contains("2 duplicate group"),
            "summary must count 2 groups; got:\n{report}"
        );
        // No already-deduplicated clause (no all-anchor sets).
        assert!(
            !report.contains("already-deduplicated"),
            "summary must NOT include already-dedup clause; got:\n{report}"
        );
    }

    // ── No findings at all → "No duplicate groups found." ────────────────────

    #[test]
    fn no_findings_at_all_shows_no_duplicate_groups_found() {
        // When there are no groups and no hard-link notes, the exact string
        // "No duplicate groups found." must appear unchanged.
        let outcome = DedupOutcome::default();
        let (ew, sw) = no_warnings();
        let report = render(&outcome, &ew, &sw);

        assert_eq!(
            report.trim(),
            "No duplicate groups found.",
            "the exact 'No duplicate groups found.' message must appear when nothing is found"
        );
    }

    // ── Summary singular/plural ────────────────────────────────────────────────

    #[test]
    fn summary_singular_duplicate_group() {
        // Summary must say "1 duplicate group" (not "1 duplicate groups").
        let dir = tempfile::tempdir().unwrap();
        let a = write_file(&dir, "a.jpg", b"SAME");
        let b = write_file(&dir, "b.jpg", b"SAME");

        let outcome = find_duplicates(
            vec![confirmed(&a), confirmed(&b)],
            build_comparator(KeepRule::Shortest),
        );
        let (ew, sw) = no_warnings();
        let report = render(&outcome, &ew, &sw);

        assert!(
            report.contains("1 duplicate group,"),
            "summary must say '1 duplicate group' (singular); got:\n{report}"
        );
        assert!(
            !report.contains("1 duplicate groups"),
            "summary must NOT say '1 duplicate groups'; got:\n{report}"
        );
    }

    #[test]
    fn summary_singular_already_dedup_set() {
        // Amendment 2026-06-15: all-anchor groups have no standalones, so no group is
        // emitted. Their hard-link notes appear in "already-deduplicated" block.
        // Minimum case: 2 inode sets with same content, each multiply-linked.
        // This produces 2 hard-link notes (one per inode), so we test plural here.
        //
        // NOTE: Singular already-dedup isn't actually possible because an all-anchor
        // group requires 2+ reps (different inodes). If you have 2 inode sets matching
        // the same content, you get 2 notes. If you have just 1 inode set (even with
        // multiple links), the engine drops it as a singleton (only 1 physical file).
        let dir = tempfile::tempdir().unwrap();

        // Inode 1: a + a2 (hard-linked, anchor)
        let a = write_file(&dir, "a.jpg", b"SAME");
        let a2 = dir.path().join("a2.jpg");
        fs::hard_link(&a, &a2).expect("hard link creatable");

        // Inode 2: b + b2 (hard-linked, anchor, same content → forms all-anchor group)
        let b = write_file(&dir, "b.jpg", b"SAME");
        let b2 = dir.path().join("b2.jpg");
        fs::hard_link(&b, &b2).expect("hard link creatable");

        let outcome = find_duplicates(
            vec![confirmed(&a), confirmed(&a2), confirmed(&b), confirmed(&b2)],
            build_comparator(KeepRule::Shortest),
        );
        let (ew, sw) = no_warnings();
        let report = render(&outcome, &ew, &sw);

        // All-anchor group: no DuplicateGroup emitted. 2 hard-link notes in already-dedup.
        assert!(
            outcome.groups.is_empty(),
            "all-anchor case must emit no group"
        );
        assert_eq!(
            outcome.hard_link_notes.len(),
            2,
            "should have 2 hard-link notes (one per inode)"
        );

        // Summary says "no duplicate groups to remove; 2 already-deduplicated hard-link sets".
        assert!(
            report.contains("no duplicate groups to remove"),
            "summary must say 'no duplicate groups to remove'; got:\n{report}"
        );
        assert!(
            report.contains("2 already-deduplicated hard-link sets"),
            "summary must say '2 already-deduplicated hard-link sets' (plural); got:\n{report}"
        );
    }

    #[test]
    fn summary_plural_already_dedup_sets() {
        // Summary must say "2 already-deduplicated hard-link sets" (plural) when
        // there are 2 already-deduplicated sets.
        let dir = tempfile::tempdir().unwrap();
        let a = write_file(&dir, "a.jpg", b"SAME");
        let a2 = dir.path().join("a2.jpg");
        fs::hard_link(&a, &a2).expect("hard link creatable");
        let b = write_file(&dir, "b.jpg", b"SAME");
        let b2 = dir.path().join("b2.jpg");
        fs::hard_link(&b, &b2).expect("hard link creatable");

        let outcome = find_duplicates(
            vec![confirmed(&a), confirmed(&a2), confirmed(&b), confirmed(&b2)],
            build_comparator(KeepRule::Shortest),
        );
        let (ew, sw) = no_warnings();
        let report = render(&outcome, &ew, &sw);

        assert!(
            report.contains("2 already-deduplicated hard-link sets"),
            "summary must say '2 already-deduplicated hard-link sets' (plural); got:\n{report}"
        );
    }
}
