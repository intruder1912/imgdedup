//! Core dedup engine — the safety heart (TASK-0304, ADR-0008/ADR-0010).
//!
//! Turns confirmed candidates into **verified duplicate groups** via the
//! ADR-0008 pipeline:
//!
//! ```text
//! confirmed files (TASK-0303)
//!   → group by exact byte size (drop singletons, no hashing)   [AC-9, FR-5]
//!   → BLAKE3 hash within each size group (pre-filter only)     [FR-4]
//!   → group by hash (drop singletons)
//!   → collapse hard-link / identity sets to one representative [AC-18, FR-17]
//!   → star byte-for-byte verify each non-keeper vs the keeper  [FR-4, NFR-1]
//!   → emit { keeper, removable } groups
//! ```
//!
//! ## Safety invariants
//!
//! - **Byte comparison is the sole authority for removal.** A BLAKE3 match alone
//!   never authorizes removal; every non-keeper is byte-for-byte verified against
//!   the keeper, and any file that fails verification is excluded and reported
//!   (FR-4, NFR-1). BLAKE3 is a *pre-filter only* — correctness does not depend on
//!   it being collision-proof.
//! - **No whole-file buffering.** Hashing and verification both stream through a
//!   fixed-size buffer and never hold file contents in memory; in-memory state is
//!   per-file metadata only (path, size, identity, 32-byte hash) — ADR-0010.
//! - **Hard links are never collateral.** Files sharing an identity key
//!   (`same-file::Handle` ≡ POSIX `(st_dev, st_ino)` / Windows volume-serial +
//!   file-id) are collapsed to a single representative; identical-identity files
//!   are never proposed for removal (AC-18, FR-17).
//! - **Pure analysis.** This stage performs **no filesystem mutation**.

use crate::sniff::ConfirmedFile;
use same_file::Handle;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// Size of the fixed streaming buffer used for hashing and byte-verification.
///
/// Reads stream through a buffer of this size so file contents are never held
/// whole in memory (ADR-0010, NFR-1).
const STREAM_BUF_LEN: usize = 64 * 1024;

// ── Public types ──────────────────────────────────────────────────────────────

/// Per-file metadata carried through the engine. **Never** holds file contents
/// (ADR-0010): only the path, size, and the 32-byte BLAKE3 digest used as a
/// pre-filter. The identity key is computed transiently during collapse and is
/// not retained here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRec {
    /// Path to the file.
    pub path: PathBuf,
    /// Byte size, threaded through from enumeration via [`ConfirmedFile`].
    pub size: u64,
    /// 32-byte BLAKE3 digest (pre-filter only).
    hash: [u8; 32],
}

/// A verified duplicate group: one keeper plus the non-keepers confirmed
/// byte-identical to it and therefore safe to remove.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateGroup {
    /// The file to keep (chosen by the injected keeper ordering).
    pub keeper: FileRec,
    /// Non-keepers byte-for-byte verified identical to `keeper`.
    pub removable: Vec<FileRec>,
}

/// A non-fatal problem encountered while analysing a file (I/O error during
/// hashing/verification, identity could not be determined, or a hash-collision
/// that byte-verification rejected). Surfaced by the report stage (TASK-0307).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineWarning {
    /// The file the warning concerns.
    pub path: PathBuf,
    /// Human-readable reason.
    pub reason: String,
}

/// The result of [`find_duplicates`]: verified groups plus any warnings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DedupOutcome {
    /// Verified duplicate groups.
    pub groups: Vec<DuplicateGroup>,
    /// Non-fatal per-file warnings.
    pub warnings: Vec<EngineWarning>,
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Analyse `files` and return verified duplicate groups.
///
/// `keeper_order` is a total ordering over [`FileRec`]s; within each verified
/// group the **minimum** under this ordering is chosen as the keeper and every
/// other member is a removable non-keeper. The keeper *rule* itself is owned by
/// TASK-0305; this engine only consumes an ordering, so the two plug together
/// cleanly. Use [`placeholder_keeper_order`] until TASK-0305 lands.
///
/// `keeper_order` should be a **total** ordering. If it is not, the keeper among
/// tie-comparing files is unspecified (it falls out of internal grouping order)
/// but always **safe** — every non-keeper is still byte-verified, so removal
/// correctness never depends on the ordering being total. Canonical-sort
/// determinism is TASK-0306's concern.
///
/// This is a **pure analysis** stage: it performs no filesystem mutation. BLAKE3
/// is used only as a pre-filter; byte-for-byte verification is the sole authority
/// for placing a file in `removable` (FR-4, NFR-1).
pub fn find_duplicates<K>(files: Vec<ConfirmedFile>, keeper_order: K) -> DedupOutcome
where
    K: Fn(&FileRec, &FileRec) -> Ordering,
{
    dedup_core(files, keeper_order, hash_file, true)
}

/// Deterministic placeholder keeper ordering used until TASK-0305 defines the
/// real keeper rule: order lexicographically by path. Keeps engine output stable
/// for tests and for the not-yet-built report.
pub fn placeholder_keeper_order(a: &FileRec, b: &FileRec) -> Ordering {
    a.path.cmp(&b.path)
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Core pipeline, parameterized over the hasher and whether size pre-grouping is
/// applied so tests can (a) force hash collisions to prove byte-verify is the
/// authority and (b) assert size pre-grouping is equivalent to hashing every file
/// (AC-9). Production callers go through [`find_duplicates`].
fn dedup_core<H, K>(
    files: Vec<ConfirmedFile>,
    keeper_order: K,
    hasher: H,
    size_prefilter: bool,
) -> DedupOutcome
where
    H: Fn(&Path) -> io::Result<[u8; 32]>,
    K: Fn(&FileRec, &FileRec) -> Ordering,
{
    let mut outcome = DedupOutcome::default();

    // Stage 1 — size pre-grouping (FR-5, AC-9). Singleton sizes are dropped
    // without hashing. The non-prefilter path (one cohort of everything) exists
    // only so the equivalence with hashing-everything can be asserted (AC-9).
    let cohorts = if size_prefilter {
        group_by_size(files)
    } else if files.is_empty() {
        Vec::new()
    } else {
        vec![files]
    };

    for cohort in cohorts {
        // Stage 2 — hash each file in the cohort (pre-filter). Hash I/O errors
        // drop the file with a warning; analysis of the rest continues.
        let mut recs: Vec<FileRec> = Vec::with_capacity(cohort.len());
        for cf in cohort {
            match hasher(&cf.path) {
                Ok(hash) => recs.push(FileRec {
                    path: cf.path,
                    size: cf.size,
                    hash,
                }),
                Err(e) => outcome.warnings.push(EngineWarning {
                    path: cf.path,
                    reason: format!("could not hash file: {e}"),
                }),
            }
        }

        // Stage 3 — group by hash (drop singletons), then verify each group.
        for group in group_by_hash(recs) {
            process_hash_group(group, &keeper_order, &mut outcome);
        }
    }

    outcome
}

/// Group hashed records by their digest, dropping any digest with only one file.
fn group_by_hash(recs: Vec<FileRec>) -> Vec<Vec<FileRec>> {
    let mut by_hash: HashMap<[u8; 32], Vec<FileRec>> = HashMap::new();
    for r in recs {
        by_hash.entry(r.hash).or_default().push(r);
    }
    by_hash.into_values().filter(|g| g.len() > 1).collect()
}

/// Collapse files that share an identity key (hard links / the same physical
/// file) to a single representative, preserving input order. Files whose
/// identity cannot be determined are dropped with a warning — the safe direction
/// (we never propose removing a file we could not identify).
///
/// **Invariant (NFR-4):** `same-file::Handle` holds an open file descriptor for
/// its lifetime. Identity is therefore resolved here, **only within a single
/// hash-group** (a small set of same-size/same-hash candidates), and the handles
/// are dropped when this function returns. The full candidate set is never
/// handle-mapped.
fn collapse_identity(recs: Vec<FileRec>, warnings: &mut Vec<EngineWarning>) -> Vec<FileRec> {
    let mut reps: Vec<(Handle, FileRec)> = Vec::new();
    for rec in recs {
        match Handle::from_path(&rec.path) {
            Ok(handle) => {
                if reps.iter().any(|(existing, _)| *existing == handle) {
                    // Same physical file as an existing representative (a hard
                    // link) — collapse it; never a removal candidate (AC-18).
                    continue;
                }
                reps.push((handle, rec));
            }
            Err(e) => warnings.push(EngineWarning {
                path: rec.path,
                reason: format!("could not determine file identity: {e}"),
            }),
        }
    }
    reps.into_iter().map(|(_, rec)| rec).collect()
}

/// Verify a single hash-group and, if it yields genuine duplicates, push a
/// [`DuplicateGroup`] onto `outcome`.
///
/// Collapses identity sets, designates the keeper via `keeper_order`, then star
/// byte-verifies each non-keeper against the keeper. A non-keeper that fails
/// verification (a BLAKE3 pre-filter collision, or an I/O error) is excluded and
/// reported — never removed on a hash match alone (FR-4, NFR-1).
fn process_hash_group<K>(group: Vec<FileRec>, keeper_order: &K, outcome: &mut DedupOutcome)
where
    K: Fn(&FileRec, &FileRec) -> Ordering,
{
    let mut reps = collapse_identity(group, &mut outcome.warnings);
    if reps.len() < 2 {
        // Fewer than two distinct physical files → nothing to remove.
        return;
    }

    // Keeper = minimum under the injected ordering.
    let keeper_idx = (0..reps.len())
        .min_by(|&i, &j| keeper_order(&reps[i], &reps[j]))
        .expect("reps is non-empty");
    let keeper = reps.remove(keeper_idx);

    let mut removable = Vec::new();
    for rec in reps {
        match files_equal(&keeper.path, &rec.path) {
            Ok(true) => removable.push(rec),
            Ok(false) => outcome.warnings.push(EngineWarning {
                path: rec.path,
                reason: "content differs from keeper despite a matching hash \
                         (BLAKE3 pre-filter collision); not removed"
                    .to_owned(),
            }),
            Err(e) => outcome.warnings.push(EngineWarning {
                path: rec.path,
                reason: format!("could not byte-verify against keeper: {e}"),
            }),
        }
    }

    if !removable.is_empty() {
        outcome.groups.push(DuplicateGroup { keeper, removable });
    }
}

/// Group files by exact byte size, dropping any size that has only one file.
///
/// Singleton size groups cannot contain a duplicate, so they are dropped here
/// **without hashing** (FR-5, AC-9). The relative order of files within a group
/// is preserved.
fn group_by_size(files: Vec<ConfirmedFile>) -> Vec<Vec<ConfirmedFile>> {
    let mut by_size: HashMap<u64, Vec<ConfirmedFile>> = HashMap::new();
    for f in files {
        by_size.entry(f.size).or_default().push(f);
    }
    by_size.into_values().filter(|g| g.len() > 1).collect()
}

/// Stream `path` through a fixed buffer and return its BLAKE3 digest.
///
/// Never holds the whole file in memory (ADR-0010).
fn hash_file(path: &Path) -> io::Result<[u8; 32]> {
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; STREAM_BUF_LEN];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(*hasher.finalize().as_bytes())
}

/// Return `true` if the two files have byte-for-byte identical contents.
///
/// Streams both files through fixed buffers and short-circuits on the first
/// differing byte or a length mismatch (ADR-0010). Never buffers a whole file.
fn files_equal(a: &Path, b: &Path) -> io::Result<bool> {
    let mut fa = File::open(a)?;
    let mut fb = File::open(b)?;
    let mut ba = vec![0u8; STREAM_BUF_LEN];
    let mut bb = vec![0u8; STREAM_BUF_LEN];
    loop {
        let na = fill(&mut fa, &mut ba)?;
        let nb = fill(&mut fb, &mut bb)?;
        if na != nb {
            // Length mismatch — short-circuit (cannot be equal).
            return Ok(false);
        }
        if na == 0 {
            // Both reached EOF together with no difference seen.
            return Ok(true);
        }
        if ba[..na] != bb[..nb] {
            return Ok(false);
        }
    }
}

/// Read repeatedly until `buf` is full or EOF, returning the number of bytes
/// read. Collapses short reads so that equal-length files compare chunk-for-chunk
/// regardless of how the OS splits reads.
fn fill(file: &mut File, buf: &mut [u8]) -> io::Result<usize> {
    let mut total = 0;
    while total < buf.len() {
        let n = file.read(&mut buf[total..])?;
        if n == 0 {
            break;
        }
        total += n;
    }
    Ok(total)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write(dir: &TempDir, name: &str, bytes: &[u8]) -> PathBuf {
        let p = dir.path().join(name);
        fs::write(&p, bytes).unwrap();
        p
    }

    fn confirmed(path: PathBuf, size: u64) -> ConfirmedFile {
        ConfirmedFile { path, size }
    }

    // ── group_by_size ─────────────────────────────────────────────────────────

    #[test]
    fn group_by_size_groups_equal_sizes_and_drops_singletons() {
        // FR-5/AC-9: files of equal size are grouped; a unique-size file is
        // dropped (it cannot be a duplicate) without being hashed.
        let files = vec![
            confirmed(PathBuf::from("a"), 100),
            confirmed(PathBuf::from("b"), 100),
            confirmed(PathBuf::from("c"), 200), // singleton size → dropped
            confirmed(PathBuf::from("d"), 200), // pairs with c
            confirmed(PathBuf::from("e"), 999), // singleton size → dropped
        ];

        let mut groups = group_by_size(files);
        // Normalize for assertion: sort each group's paths and the groups.
        for g in &mut groups {
            g.sort_by(|x, y| x.path.cmp(&y.path));
        }
        groups.sort_by(|x, y| x[0].path.cmp(&y[0].path));

        let shapes: Vec<Vec<String>> = groups
            .iter()
            .map(|g| {
                g.iter()
                    .map(|c| c.path.to_string_lossy().into_owned())
                    .collect()
            })
            .collect();

        assert_eq!(
            shapes,
            vec![
                vec!["a".to_owned(), "b".to_owned()],
                vec!["c".to_owned(), "d".to_owned()],
            ],
            "only the two same-size pairs should survive; singletons dropped"
        );
    }

    // ── hash_file ─────────────────────────────────────────────────────────────

    #[test]
    fn hash_file_differs_on_different_content() {
        // BLAKE3 must distinguish different content (the pre-filter must filter).
        let dir = tempfile::tempdir().unwrap();
        let a = write(&dir, "a.bin", b"hello world");
        let b = write(&dir, "b.bin", b"goodbye world");

        let ha = hash_file(&a).unwrap();
        let hb = hash_file(&b).unwrap();
        assert_ne!(ha, hb, "different content must hash differently");
    }

    #[test]
    fn hash_file_same_for_identical_content() {
        let dir = tempfile::tempdir().unwrap();
        let a = write(&dir, "a.bin", b"same bytes here");
        let b = write(&dir, "b.bin", b"same bytes here");

        assert_eq!(
            hash_file(&a).unwrap(),
            hash_file(&b).unwrap(),
            "identical content must hash identically"
        );
    }

    // ── files_equal ───────────────────────────────────────────────────────────

    #[test]
    fn files_equal_true_for_identical_bytes() {
        let dir = tempfile::tempdir().unwrap();
        // Larger than one stream buffer to exercise multi-chunk comparison.
        let payload: Vec<u8> = (0..200_000u32).map(|i| i as u8).collect();
        let a = write(&dir, "a.bin", &payload);
        let b = write(&dir, "b.bin", &payload);
        assert!(files_equal(&a, &b).unwrap());
    }

    #[test]
    fn files_equal_false_for_same_length_different_bytes() {
        // Critical for FR-4: same length, one byte differs deep in the file.
        let dir = tempfile::tempdir().unwrap();
        let pa: Vec<u8> = vec![7u8; 150_000];
        let mut pb = pa.clone();
        pb[149_999] = 8; // differ in the final byte
        let a = write(&dir, "a.bin", &pa);
        let b = write(&dir, "b.bin", &pb);
        assert!(
            !files_equal(&a, &b).unwrap(),
            "a one-byte difference must be detected"
        );
    }

    #[test]
    fn files_equal_false_for_different_length() {
        let dir = tempfile::tempdir().unwrap();
        let a = write(&dir, "a.bin", b"short");
        let b = write(&dir, "b.bin", b"a much longer set of bytes");
        assert!(!files_equal(&a, &b).unwrap());
    }

    // ── find_duplicates: behavioral ───────────────────────────────────────────

    /// Build a `ConfirmedFile` for a real on-disk file (size from metadata).
    fn cf(path: &Path) -> ConfirmedFile {
        let size = fs::metadata(path).unwrap().len();
        ConfirmedFile {
            path: path.to_path_buf(),
            size,
        }
    }

    /// Normalize an outcome's groups for order-insensitive comparison: sort each
    /// group's removable by path and sort groups by keeper path.
    fn normalize(mut o: DedupOutcome) -> DedupOutcome {
        for g in &mut o.groups {
            g.removable.sort_by(|x, y| x.path.cmp(&y.path));
        }
        o.groups.sort_by(|x, y| x.keeper.path.cmp(&y.keeper.path));
        o.warnings.sort_by(|x, y| x.path.cmp(&y.path));
        o
    }

    #[test]
    fn identical_files_form_one_group_unique_file_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let a = write(&dir, "a.jpg", b"DUPLICATE CONTENT");
        let b = write(&dir, "b.jpg", b"DUPLICATE CONTENT");
        let c = write(&dir, "c.jpg", b"a wholly unique file"); // different size+content

        let out = find_duplicates(vec![cf(&a), cf(&b), cf(&c)], placeholder_keeper_order);

        assert_eq!(out.groups.len(), 1, "exactly one duplicate group");
        assert!(
            out.warnings.is_empty(),
            "no warnings expected: {:?}",
            out.warnings
        );
        let g = &out.groups[0];
        assert_eq!(
            g.keeper.path, a,
            "lexicographically-smallest path is keeper"
        );
        assert_eq!(g.removable.len(), 1);
        assert_eq!(g.removable[0].path, b);
    }

    #[test]
    fn empty_input_yields_empty_outcome() {
        let out = find_duplicates(Vec::new(), placeholder_keeper_order);
        assert_eq!(out, DedupOutcome::default());
    }

    #[test]
    fn same_size_different_content_is_not_grouped() {
        // Two files of equal size but different content must not be grouped: the
        // hash separates them, and even a hash match would be caught by verify.
        let dir = tempfile::tempdir().unwrap();
        let a = write(&dir, "a.jpg", b"AAAAAAAAAA"); // 10 bytes
        let b = write(&dir, "b.jpg", b"BBBBBBBBBB"); // 10 bytes, same size

        let out = find_duplicates(vec![cf(&a), cf(&b)], placeholder_keeper_order);
        assert!(out.groups.is_empty(), "different content must not group");
    }

    // ── AC-9: size pre-grouping ≡ hashing every candidate ─────────────────────

    #[test]
    fn size_prefilter_equivalent_to_hashing_everything() {
        // AC-9: the groups produced with size pre-grouping must be identical to
        // those produced by hashing every candidate (no size pre-filter).
        let dir = tempfile::tempdir().unwrap();
        let a1 = write(&dir, "a1", b"alpha");
        let a2 = write(&dir, "a2", b"alpha"); // dup of a1
        let b1 = write(&dir, "b1", b"beta-longer"); // different size
        let b2 = write(&dir, "b2", b"beta-longer"); // dup of b1
        let c = write(&dir, "c", b"unique-gamma!"); // unique
                                                    // Two same-size-but-different-content files (size 5 like alpha):
        let d = write(&dir, "d", b"delta"); // 5 bytes, != alpha
        let files = vec![cf(&a1), cf(&a2), cf(&b1), cf(&b2), cf(&c), cf(&d)];

        let with_prefilter = dedup_core(files.clone(), placeholder_keeper_order, hash_file, true);
        let without_prefilter = dedup_core(files, placeholder_keeper_order, hash_file, false);

        assert_eq!(
            normalize(with_prefilter),
            normalize(without_prefilter),
            "size pre-grouping must not change the result vs hashing everything"
        );
    }

    // ── FR-4 / NFR-1: byte-verify is the sole authority ───────────────────────

    #[test]
    fn forced_hash_collision_is_rejected_by_byte_verify() {
        // FR-4/NFR-1: with a constant hasher, two same-size, different-content
        // files land in one hash group. Byte-verify MUST reject them — no group
        // is emitted and the mismatch is reported. This proves correctness does
        // not depend on BLAKE3 being collision-proof.
        let dir = tempfile::tempdir().unwrap();
        let a = write(&dir, "a.jpg", b"CONTENT-ONE"); // 11 bytes
        let b = write(&dir, "b.jpg", b"CONTENT-TWO"); // 11 bytes, same size

        fn const_hasher(_p: &Path) -> io::Result<[u8; 32]> {
            Ok([0u8; 32])
        }

        let out = dedup_core(
            vec![cf(&a), cf(&b)],
            placeholder_keeper_order,
            const_hasher,
            true,
        );

        assert!(
            out.groups.is_empty(),
            "a hash collision with differing content must NOT be grouped"
        );
        assert_eq!(
            out.warnings.len(),
            1,
            "the rejected non-keeper must be reported"
        );
        assert_eq!(out.warnings[0].path, b);
    }

    #[test]
    fn forced_hash_collision_still_groups_genuine_duplicates() {
        // The other half of the collision story: when the colliding files really
        // are byte-identical, byte-verify confirms and they ARE grouped.
        let dir = tempfile::tempdir().unwrap();
        let a = write(&dir, "a.jpg", b"REAL DUPLICATE");
        let b = write(&dir, "b.jpg", b"REAL DUPLICATE");

        fn const_hasher(_p: &Path) -> io::Result<[u8; 32]> {
            Ok([0u8; 32])
        }

        let out = dedup_core(
            vec![cf(&a), cf(&b)],
            placeholder_keeper_order,
            const_hasher,
            true,
        );
        assert_eq!(out.groups.len(), 1);
        assert!(out.warnings.is_empty());
    }

    // ── AC-18 / FR-17: hard links collapsed, never removed ────────────────────

    #[test]
    fn hard_links_are_collapsed_and_never_removed() {
        // AC-18/FR-17: two hard links to the same content are one physical file;
        // they must collapse to a single representative and never be proposed for
        // removal. A genuine separate copy IS a removal candidate.
        let dir = tempfile::tempdir().unwrap();
        let a = write(&dir, "aaa.jpg", b"SHARED INODE CONTENT");
        let b = dir.path().join("bbb.jpg");
        fs::hard_link(&a, &b).expect("hard link should be creatable in tempdir");
        let c = write(&dir, "ccc.jpg", b"SHARED INODE CONTENT"); // separate copy

        let out = find_duplicates(vec![cf(&a), cf(&b), cf(&c)], placeholder_keeper_order);

        assert_eq!(
            out.groups.len(),
            1,
            "one group from the distinct-identity pair"
        );
        let g = &out.groups[0];
        // a and c are the two distinct identities; a sorts first → keeper.
        assert_eq!(g.keeper.path, a);
        assert_eq!(g.removable.len(), 1, "only the separate copy is removable");
        assert_eq!(g.removable[0].path, c);
        // The hard-link sibling b must never appear as keeper or removable.
        assert_ne!(g.keeper.path, b);
        assert!(
            g.removable.iter().all(|r| r.path != b),
            "the hard-link sibling must never be proposed for removal"
        );
    }

    // ── Keeper designation via injected ordering ──────────────────────────────

    #[test]
    fn keeper_chosen_by_injected_ordering() {
        // The engine consumes an injected ordering; swapping it changes the keeper.
        let dir = tempfile::tempdir().unwrap();
        let a = write(&dir, "a.jpg", b"SAME");
        let b = write(&dir, "b.jpg", b"SAME");

        // Reverse-lexicographic ordering → the largest path becomes the keeper.
        let out = find_duplicates(vec![cf(&a), cf(&b)], |x, y| y.path.cmp(&x.path));

        assert_eq!(out.groups.len(), 1);
        assert_eq!(
            out.groups[0].keeper.path, b,
            "reverse ordering keeps the largest path"
        );
        assert_eq!(out.groups[0].removable[0].path, a);
    }

    // ── I/O error handling ────────────────────────────────────────────────────

    #[test]
    fn unreadable_file_is_reported_not_panicked() {
        // A file that cannot be hashed (does not exist) is reported as a warning;
        // the rest of the analysis proceeds. We pass two real duplicates plus a
        // ghost path declared to share their size so it enters a size cohort.
        let dir = tempfile::tempdir().unwrap();
        let a = write(&dir, "a.jpg", b"DUP");
        let b = write(&dir, "b.jpg", b"DUP");
        let ghost = ConfirmedFile {
            path: PathBuf::from("/does/not/exist/ghost.jpg"),
            size: 3, // same size as "DUP" so it joins the cohort and is then hashed
        };

        let out = find_duplicates(vec![cf(&a), cf(&b), ghost], placeholder_keeper_order);

        assert_eq!(out.groups.len(), 1, "the real duplicates still group");
        assert_eq!(out.warnings.len(), 1, "the unreadable file is reported");
        assert_eq!(
            out.warnings[0].path,
            PathBuf::from("/does/not/exist/ghost.jpg")
        );
    }
}
