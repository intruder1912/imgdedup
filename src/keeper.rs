//! Keeper-selection comparators — TASK-0305, ADR-0008 §"Keeper rule".
//!
//! Provides the **single ordering source** for the dedup engine's keeper
//! designation. Given a [`KeepRule`] from the CLI, [`build_comparator`] returns
//! a total ordering function `Fn(&FileRec, &FileRec) -> Ordering` that the
//! engine passes to [`find_duplicates`] as its `keeper_order` argument.
//!
//! ## Total orders (ADR-0008)
//!
//! Each rule produces a **total** order — every pair of distinct files has an
//! unambiguous winner — by chaining primary, secondary, and final lexicographic
//! tie-break criteria:
//!
//! - **`shortest` (default):**
//!   1. Fewest path segments (shallowest).
//!   2. Fewest characters in the full path string (byte count).
//!   3. Oldest mtime (smallest Unix timestamp).
//!   4. Byte-wise lexicographic path order.
//!
//! - **`oldest`:**
//!   1. Oldest mtime (smallest Unix timestamp).
//!   2. Fewest path segments.
//!   3. Fewest path characters.
//!   4. Byte-wise lexicographic path order.
//!
//! - **`newest`:**
//!   1. Newest mtime (largest Unix timestamp — reversed).
//!   2. Fewest path segments.
//!   3. Fewest path characters.
//!   4. Byte-wise lexicographic path order.
//!
//! ## Path comparison
//!
//! The final tie-break is **byte-wise, case-sensitive** — `'A'` < `'a'`, no
//! locale-dependent case folding (ADR-0008 §NFR-3). This is implemented via
//! [`Path::cmp`], which compares OS-string bytes directly.
//!
//! ## mtime `None` placement
//!
//! Files whose mtime is `None` (unavailable) are treated as the **oldest
//! possible** (effectively timestamp `i64::MIN`). This is consistent and
//! documented: missing-mtime files sort before all files with a known mtime in
//! `oldest`/`shortest` order (where older = preferred = smaller), and after
//! all files with a known mtime in `newest` order (where newer = preferred =
//! larger). The placement is deterministic regardless of which files lack mtime.
//!
//! ## The engine picks the minimum
//!
//! [`find_duplicates`] selects the **minimum** under `keeper_order` as the
//! keeper. The comparators here therefore rank the *preferred* keeper as
//! *smaller*.

use crate::cli::KeepRule;
use crate::engine::FileRec;
use std::cmp::Ordering;
use std::path::Path;

// ── Public entry point ────────────────────────────────────────────────────────

/// Build a keeper comparator for `rule`.
///
/// Returns a `Box`ed `Fn` that implements the ADR-0008 total order for the
/// given [`KeepRule`]. Pass the result directly to
/// [`engine::find_duplicates`] as the `keeper_order` argument.
///
/// The engine selects the **minimum** under this order as the keeper.
///
/// # mtime `None` placement
///
/// `None` mtime is treated as `i64::MIN` — the oldest possible — consistently
/// across all rules. Under `shortest`/`oldest`, `None`-mtime files rank as
/// *older*, i.e. *preferred*; under `newest`, they rank as *less recent*, i.e.
/// *less preferred*.
pub fn build_comparator(rule: KeepRule) -> impl Fn(&FileRec, &FileRec) -> Ordering {
    move |a, b| match rule {
        KeepRule::Shortest => cmp_shortest(a, b),
        KeepRule::Oldest => cmp_oldest(a, b),
        KeepRule::Newest => cmp_newest(a, b),
    }
}

// ── Internal comparators ──────────────────────────────────────────────────────

/// Count the number of path segments (components) in a [`FileRec`] path.
fn segment_count(r: &FileRec) -> usize {
    r.path.components().count()
}

/// Return the byte length of the path's UTF-8 (or lossy) string representation.
fn path_char_count(r: &FileRec) -> usize {
    r.path.as_os_str().len()
}

/// Coerce `None` mtime to `i64::MIN` for comparison purposes (treated as the
/// oldest possible, ADR-0008 §mtime-None-placement).
fn mtime_or_oldest(r: &FileRec) -> i64 {
    r.mtime.unwrap_or(i64::MIN)
}

/// `--keep shortest` total order (ADR-0008):
/// segments → chars → oldest mtime → lexicographic path.
fn cmp_shortest(a: &FileRec, b: &FileRec) -> Ordering {
    cmp_shortest_by(&a.path, a.mtime, &b.path, b.mtime)
}

/// The `--keep shortest` order ([`cmp_shortest`]), applied directly to a
/// path/mtime pair rather than a full [`FileRec`].
///
/// Shared with `enumerate.rs`'s TASK-0320 symlink-alias tie-break (product
/// ruling: reuse the documented FR-7 `shortest` order rather than inventing a
/// second one) — this is the single definition both call sites use.
pub(crate) fn cmp_shortest_by(
    a_path: &Path,
    a_mtime: Option<i64>,
    b_path: &Path,
    b_mtime: Option<i64>,
) -> Ordering {
    fn segments(p: &Path) -> usize {
        p.components().count()
    }
    fn char_count(p: &Path) -> usize {
        p.as_os_str().len()
    }
    fn mtime_or_oldest(m: Option<i64>) -> i64 {
        m.unwrap_or(i64::MIN)
    }

    segments(a_path)
        .cmp(&segments(b_path))
        .then_with(|| char_count(a_path).cmp(&char_count(b_path)))
        .then_with(|| mtime_or_oldest(a_mtime).cmp(&mtime_or_oldest(b_mtime)))
        .then_with(|| a_path.cmp(b_path))
}

/// `--keep oldest` total order (ADR-0008):
/// oldest mtime → segments → chars → lexicographic path.
fn cmp_oldest(a: &FileRec, b: &FileRec) -> Ordering {
    mtime_or_oldest(a)
        .cmp(&mtime_or_oldest(b))
        .then_with(|| segment_count(a).cmp(&segment_count(b)))
        .then_with(|| path_char_count(a).cmp(&path_char_count(b)))
        .then_with(|| a.path.cmp(&b.path))
}

/// `--keep newest` total order (ADR-0008):
/// newest mtime (reversed) → segments → chars → lexicographic path.
fn cmp_newest(a: &FileRec, b: &FileRec) -> Ordering {
    // Reversed: larger mtime wins (is preferred = is smaller in the order).
    mtime_or_oldest(b)
        .cmp(&mtime_or_oldest(a))
        .then_with(|| segment_count(a).cmp(&segment_count(b)))
        .then_with(|| path_char_count(a).cmp(&path_char_count(b)))
        .then_with(|| a.path.cmp(&b.path))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{DedupOutcome, FileRec, find_duplicates};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    // ── Helpers ───────────────────────────────────────────────────────────────

    /// Write content bytes to `dir/name` and return an absolute PathBuf.
    fn write(dir: &TempDir, name: &str, bytes: &[u8]) -> PathBuf {
        let p = dir.path().join(name);
        fs::write(&p, bytes).unwrap();
        p
    }

    /// Build a synthetic `FileRec` without touching the filesystem — used for
    /// pure ordering tests where we control size / mtime directly.
    fn rec(path: &str, mtime: Option<i64>) -> FileRec {
        FileRec::new_for_test(PathBuf::from(path), 0, mtime)
    }

    /// Build a `ConfirmedFile` from a real on-disk file, threading through
    /// the mtime from metadata (same as the production enumeration path).
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

    /// Normalize outcome for order-insensitive assertions.
    fn normalize(mut o: DedupOutcome) -> DedupOutcome {
        for g in &mut o.groups {
            g.removable.sort_by(|x, y| x.path.cmp(&y.path));
        }
        o.groups.sort_by(|x, y| x.keeper.path.cmp(&y.keeper.path));
        o.warnings.sort_by(|x, y| x.path.cmp(&y.path));
        o
    }

    // ── AC-2: exactly one keeper per group ────────────────────────────────────

    #[test]
    fn shortest_gives_exactly_one_keeper_per_group() {
        // AC-2 (FR-4, FR-7): three byte-identical copies → one group → one keeper.
        let dir = tempfile::tempdir().unwrap();
        let a = write(&dir, "a.jpg", b"DUPLICATE");
        let b = write(&dir, "b.jpg", b"DUPLICATE");
        let c = write(&dir, "c.jpg", b"DUPLICATE");

        let cmp = build_comparator(KeepRule::Shortest);
        let out = find_duplicates(vec![confirmed(&a), confirmed(&b), confirmed(&c)], cmp);

        assert_eq!(out.groups.len(), 1, "exactly one group");
        assert_eq!(
            out.groups[0].removable.len(),
            2,
            "exactly two removable (= three total − one keeper)"
        );
        assert!(out.warnings.is_empty(), "no warnings expected");
    }

    #[test]
    fn oldest_gives_exactly_one_keeper_per_group() {
        // AC-2: oldest rule also produces exactly one keeper.
        let dir = tempfile::tempdir().unwrap();
        let a = write(&dir, "a.jpg", b"SAME");
        let b = write(&dir, "b.jpg", b"SAME");

        let cmp = build_comparator(KeepRule::Oldest);
        let out = find_duplicates(vec![confirmed(&a), confirmed(&b)], cmp);

        assert_eq!(out.groups.len(), 1, "exactly one group");
        assert_eq!(out.groups[0].removable.len(), 1);
    }

    #[test]
    fn newest_gives_exactly_one_keeper_per_group() {
        // AC-2: newest rule also produces exactly one keeper.
        let dir = tempfile::tempdir().unwrap();
        let a = write(&dir, "a.jpg", b"SAME");
        let b = write(&dir, "b.jpg", b"SAME");

        let cmp = build_comparator(KeepRule::Newest);
        let out = find_duplicates(vec![confirmed(&a), confirmed(&b)], cmp);

        assert_eq!(out.groups.len(), 1, "exactly one group");
        assert_eq!(out.groups[0].removable.len(), 1);
    }

    // ── AC-6: correct primary criterion per rule ──────────────────────────────

    #[test]
    fn shortest_keeps_shallowest_path() {
        // AC-6/FR-7/FR-8: `--keep shortest` keeps the file with fewest segments.
        // a.jpg (1 segment) beats sub/a.jpg (2 segments).
        let a = rec("/a.jpg", Some(100));
        let b = rec("/sub/a.jpg", Some(50)); // older mtime but deeper path
        let cmp = build_comparator(KeepRule::Shortest);

        assert_eq!(
            cmp(&a, &b),
            Ordering::Less,
            "shallower path must sort first (= preferred keeper)"
        );
    }

    #[test]
    fn shortest_breaks_segment_tie_by_char_count() {
        // AC-6/FR-7: equal segment count → fewer chars wins.
        let a = rec("/a.jpg", Some(100)); // short path
        let b = rec("/ab.jpg", Some(100)); // longer path, same segment count
        let cmp = build_comparator(KeepRule::Shortest);

        assert_eq!(
            cmp(&a, &b),
            Ordering::Less,
            "shorter path (char count) must sort first"
        );
    }

    #[test]
    fn shortest_breaks_char_tie_by_oldest_mtime() {
        // AC-6/FR-7: equal segments and chars → oldest mtime wins.
        let a = rec("/a.jpg", Some(100)); // older
        let b = rec("/b.jpg", Some(200)); // newer
        let cmp = build_comparator(KeepRule::Shortest);

        assert_eq!(
            cmp(&a, &b),
            Ordering::Less,
            "older mtime must sort first after segment/char tie"
        );
    }

    #[test]
    fn shortest_breaks_mtime_tie_by_lexicographic_path() {
        // AC-6/FR-7: equal segments, chars, and mtime → byte-wise path order.
        let a = rec("/a.jpg", Some(100));
        let b = rec("/b.jpg", Some(100)); // same mtime
        let cmp = build_comparator(KeepRule::Shortest);

        assert_eq!(
            cmp(&a, &b),
            Ordering::Less,
            "byte-wise path comparison must be the final tie-break"
        );
    }

    #[test]
    fn oldest_keeps_oldest_mtime() {
        // AC-6/FR-8: `--keep oldest` keeps the file with the smallest timestamp.
        let a = rec("/a.jpg", Some(50)); // older
        let b = rec("/zzzz.jpg", Some(200)); // newer (longer path, later alpha)
        let cmp = build_comparator(KeepRule::Oldest);

        assert_eq!(
            cmp(&a, &b),
            Ordering::Less,
            "oldest mtime must be the primary sort key for --keep oldest"
        );
    }

    #[test]
    fn newest_keeps_newest_mtime() {
        // AC-6/FR-8: `--keep newest` keeps the file with the largest timestamp.
        let a = rec("/b.jpg", Some(200)); // newer
        let b = rec("/a.jpg", Some(50)); // older
        let cmp = build_comparator(KeepRule::Newest);

        assert_eq!(
            cmp(&a, &b),
            Ordering::Less,
            "newest mtime must be the primary sort key for --keep newest (reversed)"
        );
    }

    // ── mtime None placement ──────────────────────────────────────────────────

    #[test]
    fn none_mtime_treated_as_oldest_under_shortest() {
        // None mtime is treated as i64::MIN (oldest) under --keep shortest.
        // A file with None mtime beats one with mtime = i64::MIN + 1.
        let a = rec("/a.jpg", None); // treated as i64::MIN
        let b = rec("/b.jpg", Some(0)); // mtime = 0 (Unix epoch = newer than i64::MIN)
        let cmp = build_comparator(KeepRule::Shortest);

        assert_eq!(
            cmp(&a, &b),
            Ordering::Less,
            "None mtime should rank as oldest (preferred) under shortest"
        );
    }

    #[test]
    fn none_mtime_treated_as_oldest_under_oldest() {
        // None mtime is preferred under --keep oldest (it's the "oldest").
        let a = rec("/a.jpg", None); // treated as i64::MIN
        let b = rec("/b.jpg", Some(0)); // Unix epoch
        let cmp = build_comparator(KeepRule::Oldest);

        assert_eq!(
            cmp(&a, &b),
            Ordering::Less,
            "None mtime should rank as oldest (preferred) under oldest"
        );
    }

    #[test]
    fn none_mtime_treated_as_oldest_under_newest() {
        // Under --keep newest, None mtime (= oldest) is NOT preferred —
        // the file with the real newer mtime wins.
        let a = rec("/a.jpg", None); // treated as i64::MIN
        let b = rec("/b.jpg", Some(0)); // Unix epoch = newer
        let cmp = build_comparator(KeepRule::Newest);

        assert_eq!(
            cmp(&a, &b),
            Ordering::Greater,
            "None mtime (treated as oldest) must NOT be preferred under newest"
        );
    }

    #[test]
    fn two_none_mtimes_fall_through_to_path_order() {
        // When both files have None mtime, the tie falls through to the
        // lexicographic path order under all rules.
        let a = rec("/a.jpg", None);
        let b = rec("/b.jpg", None);

        for rule in [KeepRule::Shortest, KeepRule::Oldest, KeepRule::Newest] {
            let cmp = build_comparator(rule);
            assert_eq!(
                cmp(&a, &b),
                Ordering::Less,
                "/a.jpg must sort before /b.jpg under {rule:?} with both mtimes None"
            );
        }
    }

    // ── AC-16: determinism / total order ─────────────────────────────────────

    #[test]
    fn ordering_is_total_no_equal_distinct_paths() {
        // AC-16 (NFR-3): for any pair of distinct-path FileRecs, the comparator
        // must not return Equal (it is a total order). Test all three rules.
        let pairs = [
            (rec("/a.jpg", Some(100)), rec("/b.jpg", Some(100))),
            (rec("/a/b.jpg", Some(100)), rec("/b.jpg", Some(100))),
            (rec("/same.jpg", Some(100)), rec("/same.jpg", Some(200))), // same path, different mtime
        ];

        for rule in [KeepRule::Shortest, KeepRule::Oldest, KeepRule::Newest] {
            let cmp = build_comparator(rule);
            for (a, b) in &pairs {
                // Same path → Equal is valid; distinct paths must not be Equal.
                if a.path != b.path {
                    let ord = cmp(a, b);
                    assert_ne!(
                        ord,
                        Ordering::Equal,
                        "distinct-path pair must not be Equal under {rule:?}: {:?} vs {:?}",
                        a.path,
                        b.path
                    );
                }
            }
        }
    }

    #[test]
    fn identical_runs_produce_identical_keeper() {
        // AC-16 (NFR-3): two separate invocations over the same files must
        // choose the same keeper. We run find_duplicates twice and compare.
        let dir = tempfile::tempdir().unwrap();
        let a = write(&dir, "aaa.jpg", b"CONTENT");
        let b = write(&dir, "bbb.jpg", b"CONTENT");
        let c = write(&dir, "ccc.jpg", b"CONTENT");

        let mk_cmp = || build_comparator(KeepRule::Shortest);
        let run1 = normalize(find_duplicates(
            vec![confirmed(&a), confirmed(&b), confirmed(&c)],
            mk_cmp(),
        ));
        let run2 = normalize(find_duplicates(
            vec![confirmed(&c), confirmed(&a), confirmed(&b)], // different input order
            mk_cmp(),
        ));

        assert_eq!(
            run1.groups[0].keeper.path, run2.groups[0].keeper.path,
            "keeper must be the same regardless of input order"
        );
        assert_eq!(
            run1, run2,
            "full outcome must be byte-identical across runs"
        );
    }

    // ── Case sensitivity: no case folding ────────────────────────────────────

    #[test]
    fn path_comparison_is_case_sensitive_no_folding() {
        // ADR-0008 §NFR-3: byte-wise path comparison; 'A' < 'a' (ASCII order).
        // /A.jpg must sort before /a.jpg because 'A' (0x41) < 'a' (0x61).
        let a = rec("/A.jpg", Some(100));
        let b = rec("/a.jpg", Some(100)); // same mtime, longer... no, same length

        let cmp = build_comparator(KeepRule::Shortest);
        // Segment count equal (1), char count equal, mtime equal →
        // byte-wise: 'A' (0x41) < 'a' (0x61), so /A.jpg < /a.jpg.
        assert_eq!(
            cmp(&a, &b),
            Ordering::Less,
            "uppercase 'A' (0x41) must sort before lowercase 'a' (0x61) — no case folding"
        );
    }

    // ── Segment and char count tie-breaks ────────────────────────────────────

    #[test]
    fn segment_count_is_primary_in_shortest() {
        // Under --keep shortest, the segment count is checked BEFORE char count.
        // A short path 5 chars but 2 segments loses to a 10-char path with 1 segment.
        let shallow = rec("/abcdefgh.jpg", Some(100)); // 1 segment, 13 chars
        let deep = rec("/ab/c.jpg", Some(100)); // 2 segments, 9 chars (fewer chars!)
        let cmp = build_comparator(KeepRule::Shortest);

        assert_eq!(
            cmp(&shallow, &deep),
            Ordering::Less,
            "fewer segments must win over fewer chars in --keep shortest"
        );
    }

    #[test]
    fn oldest_breaks_mtime_tie_by_segment_count() {
        // Under --keep oldest, mtime is primary. A tie falls to segment count.
        let a = rec("/a.jpg", Some(100)); // same mtime, 1 segment
        let b = rec("/sub/b.jpg", Some(100)); // same mtime, 2 segments
        let cmp = build_comparator(KeepRule::Oldest);

        assert_eq!(
            cmp(&a, &b),
            Ordering::Less,
            "mtime tie under oldest must break by segment count"
        );
    }

    #[test]
    fn newest_breaks_mtime_tie_by_segment_count() {
        // Under --keep newest, mtime is primary (reversed). Tie → segment count.
        let a = rec("/a.jpg", Some(100)); // same mtime, 1 segment
        let b = rec("/sub/b.jpg", Some(100)); // same mtime, 2 segments
        let cmp = build_comparator(KeepRule::Newest);

        assert_eq!(
            cmp(&a, &b),
            Ordering::Less,
            "mtime tie under newest must break by segment count"
        );
    }
}
