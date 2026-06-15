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
use rayon::prelude::*;
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
/// (ADR-0010): only the path, size, mtime, and the 32-byte BLAKE3 digest used
/// as a pre-filter. The identity key is computed transiently during collapse
/// and is not retained here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRec {
    /// Path to the file.
    pub path: PathBuf,
    /// Byte size, threaded through from enumeration via [`ConfirmedFile`].
    pub size: u64,
    /// Modification time in whole Unix seconds, threaded through from
    /// enumeration via [`ConfirmedFile`]. `None` when unavailable.
    /// Used by the keeper rule (TASK-0305).
    pub mtime: Option<i64>,
    /// 32-byte BLAKE3 digest (pre-filter only).
    hash: [u8; 32],
}

impl FileRec {
    /// Construct a synthetic `FileRec` for unit tests outside this module.
    ///
    /// The `hash` is zeroed — only use this constructor in test code where
    /// real file I/O and BLAKE3 hashing are not the subject under test.
    #[cfg(test)]
    pub fn new_for_test(path: PathBuf, size: u64, mtime: Option<i64>) -> Self {
        Self {
            path,
            size,
            mtime,
            hash: [0u8; 32],
        }
    }
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

/// An informational note about hard-linked files within a duplicate group
/// (FR-17, ADR-0008 §hard-link detection).
///
/// Hard-linked files share storage (same inode on POSIX, same file ID on
/// Windows). Within a hash group they are collapsed to a single representative
/// before keeper selection; the collapsed paths are surfaced here so the report
/// can show them as informational notes and **exclude them from the
/// reclaimable-space figure** (FR-13/FR-17 — they free no space when removed).
///
/// ## Canonical form (NFR-3)
///
/// `representative` is always the **byte-wise minimum path** among all paths
/// in the identity set (i.e. `min(representative, siblings[0], siblings[1], …)`).
/// `siblings` is always **sorted in ascending byte-wise path order**. This
/// canonical form is established at construction time in [`collapse_identity`]
/// so the invariant holds everywhere — two runs over identical state produce
/// byte-identical `HardLinkNote` values regardless of HashMap iteration order
/// or thread scheduling (AC-16 / NFR-3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardLinkNote {
    /// The representative path for the hard-link identity set: the
    /// **lexicographically smallest** path among all members of the set.
    /// This is also the path carried by the [`FileRec`] that flows forward
    /// into keeper selection and the emitted [`DuplicateGroup`].
    pub representative: PathBuf,
    /// The other paths collapsed into `representative` (hard-link siblings),
    /// in **sorted ascending byte-wise path order**.
    /// Non-empty — there is at least one sibling, otherwise there is no note.
    pub siblings: Vec<PathBuf>,
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

/// The result of [`find_duplicates`]: verified groups (canonically sorted) plus
/// any warnings.
///
/// The `groups` and `warnings` are in **canonical order** (ADR-0010): groups
/// by keeper path, files within each group by the TASK-0305 total order,
/// warnings by path. This canonical sort is the single determinism mechanism —
/// two runs over identical state produce byte-identical `DedupOutcome` values
/// regardless of thread scheduling or hash-map iteration order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DedupOutcome {
    /// Verified duplicate groups, in canonical order (keeper path ascending).
    pub groups: Vec<DuplicateGroup>,
    /// Non-fatal per-file warnings, ordered by path.
    pub warnings: Vec<EngineWarning>,
    /// Informational hard-link notes: paths that were collapsed within a hash
    /// group because they share the same filesystem identity. These are never
    /// in `removable` (FR-17) — they share storage with their representative
    /// and are excluded from the reclaimable-space figure. Ordered by
    /// representative path.
    pub hard_link_notes: Vec<HardLinkNote>,
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Analyse `files` and return verified duplicate groups.
///
/// `keeper_order` is a total ordering over [`FileRec`]s; within each verified
/// group the **minimum** under this ordering is chosen as the keeper and every
/// other member is a removable non-keeper. Build the correct ordering with
/// [`crate::keeper::build_comparator`] (TASK-0305).
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
    K: Fn(&FileRec, &FileRec) -> Ordering + Sync,
{
    dedup_core(files, keeper_order, hash_file, true)
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Core pipeline, parameterized over the hasher and whether size pre-grouping is
/// applied so tests can (a) force hash collisions to prove byte-verify is the
/// authority and (b) assert size pre-grouping is equivalent to hashing every file
/// (AC-9). Production callers go through [`find_duplicates`].
///
/// Hashing within each size cohort is executed with rayon parallel iterators
/// (ADR-0010, TASK-0306). The `hasher` function must be `Send + Sync` so it can
/// be called from rayon worker threads.
///
/// Before returning, the outcome is **canonically sorted** (ADR-0010): groups
/// by keeper path, files within each group by the keeper order, warnings by
/// path, hard-link notes by representative path. This canonical sort is the
/// single determinism mechanism — completion order never leaks into results.
fn dedup_core<H, K>(
    files: Vec<ConfirmedFile>,
    keeper_order: K,
    hasher: H,
    size_prefilter: bool,
) -> DedupOutcome
where
    H: Fn(&Path) -> io::Result<[u8; 32]> + Sync,
    K: Fn(&FileRec, &FileRec) -> Ordering + Sync,
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
        // Stage 2 — hash each file in the cohort in parallel (ADR-0010, rayon).
        // par_iter produces (ConfirmedFile, Result<hash>) pairs preserving no
        // particular order; we collect all results and then split into recs and
        // warnings so each per-file outcome is atomic.
        let hash_results: Vec<(ConfirmedFile, io::Result<[u8; 32]>)> = cohort
            .into_par_iter()
            .map(|cf| {
                let result = hasher(&cf.path);
                (cf, result)
            })
            .collect();

        let mut recs: Vec<FileRec> = Vec::with_capacity(hash_results.len());
        for (cf, result) in hash_results {
            match result {
                Ok(hash) => recs.push(FileRec {
                    path: cf.path,
                    size: cf.size,
                    mtime: cf.mtime,
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

    // Canonical sort (ADR-0010, TASK-0306): sort before returning so two runs
    // over identical state produce byte-identical DedupOutcome values regardless
    // of thread scheduling or hash-map iteration order.
    sort_canonical(&mut outcome, &keeper_order);

    outcome
}

/// Sort `outcome` into canonical order (ADR-0010).
///
/// - `groups`: by keeper path (ascending).
/// - Within each group's `removable`: by the `keeper_order` comparator.
/// - `warnings`: by path.
/// - `hard_link_notes`: by representative path.
fn sort_canonical<K>(outcome: &mut DedupOutcome, keeper_order: &K)
where
    K: Fn(&FileRec, &FileRec) -> Ordering,
{
    // Sort each group's removable list by the total order.
    for g in &mut outcome.groups {
        g.removable.sort_by(keeper_order);
    }
    // Sort groups by keeper path.
    outcome
        .groups
        .sort_by(|a, b| a.keeper.path.cmp(&b.keeper.path));
    // Sort warnings by path.
    outcome.warnings.sort_by(|a, b| a.path.cmp(&b.path));
    // Sort hard-link notes by representative path.
    outcome
        .hard_link_notes
        .sort_by(|a, b| a.representative.cmp(&b.representative));
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
/// Returns the deduplicated list of representative `FileRec`s and any
/// [`HardLinkNote`]s for collapsed sets (one note per set that had at least
/// one sibling collapsed). Notes are used by the report stage (TASK-0307, FR-17).
///
/// **Invariant (NFR-4):** `same-file::Handle` holds an open file descriptor for
/// its lifetime. Identity is therefore resolved here, **only within a single
/// hash-group** (a small set of same-size/same-hash candidates), and the handles
/// are dropped when this function returns. The full candidate set is never
/// handle-mapped.
fn collapse_identity(
    recs: Vec<FileRec>,
    warnings: &mut Vec<EngineWarning>,
) -> (Vec<FileRec>, Vec<HardLinkNote>) {
    // Each entry: the Handle (for equality), the representative FileRec, and
    // any siblings collapsed into it.
    let mut reps: Vec<(Handle, FileRec, Vec<PathBuf>)> = Vec::new();
    for rec in recs {
        match Handle::from_path(&rec.path) {
            Ok(handle) => {
                if let Some(slot) = reps.iter_mut().find(|(existing, _, _)| *existing == handle) {
                    // Same physical file as an existing representative (a hard
                    // link) — collapse it; never a removal candidate (AC-18).
                    slot.2.push(rec.path);
                } else {
                    reps.push((handle, rec, Vec::new()));
                }
            }
            Err(e) => warnings.push(EngineWarning {
                path: rec.path,
                reason: format!("could not determine file identity: {e}"),
            }),
        }
    }
    let mut notes: Vec<HardLinkNote> = Vec::new();
    let file_recs: Vec<FileRec> = reps
        .into_iter()
        .map(|(_, mut rec, mut siblings)| {
            if !siblings.is_empty() {
                // B1 (NFR-3): canonicalize each note so `representative` is
                // the **minimum path** (byte-wise) among all paths in the
                // identity set, and `siblings` are sorted. This makes the note
                // content identical regardless of HashMap iteration order or
                // thread scheduling.
                //
                // We also update `rec.path` to the minimum path so that the
                // `FileRec` flowing into keeper selection carries the same path
                // that the note's `representative` field names. All paths in an
                // identity set point to the same physical file (same inode), so
                // switching which path label the FileRec carries does not change
                // *which file* is kept — only which path name refers to it.
                // The report's matching logic (`note.representative ∈
                // group_paths`) depends on these two being in sync.
                siblings.push(rec.path.clone());
                siblings.sort();
                // `siblings` is now non-empty (we just pushed `rec.path`);
                // `remove(0)` extracts the minimum path.
                let min_path = siblings.remove(0);
                rec.path = min_path.clone();
                notes.push(HardLinkNote {
                    representative: min_path,
                    siblings,
                });
            }
            rec
        })
        .collect();
    (file_recs, notes)
}

/// Verify a single hash-group and, if it yields genuine duplicates, push a
/// [`DuplicateGroup`] onto `outcome`. Any hard-link notes are pushed to
/// `outcome.hard_link_notes` (FR-17).
///
/// Collapses identity sets, designates the keeper via `keeper_order`, then star
/// byte-verifies each non-keeper against the keeper. A non-keeper that fails
/// verification (a BLAKE3 pre-filter collision, or an I/O error) is excluded and
/// reported — never removed on a hash match alone (FR-4, NFR-1).
fn process_hash_group<K>(group: Vec<FileRec>, keeper_order: &K, outcome: &mut DedupOutcome)
where
    K: Fn(&FileRec, &FileRec) -> Ordering,
{
    let (mut reps, hl_notes) = collapse_identity(group, &mut outcome.warnings);
    outcome.hard_link_notes.extend(hl_notes);
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

    /// Lexicographic-by-path ordering used in engine tests that only care about
    /// engine mechanics (grouping, collapse, byte-verify) rather than the real
    /// keeper rule. Mirrors what `lex_order` used to do.
    fn lex_order(a: &FileRec, b: &FileRec) -> Ordering {
        a.path.cmp(&b.path)
    }

    fn confirmed(path: PathBuf, size: u64) -> ConfirmedFile {
        ConfirmedFile {
            path,
            size,
            mtime: None,
        }
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

    /// Build a `ConfirmedFile` for a real on-disk file (size and mtime from metadata).
    fn cf(path: &Path) -> ConfirmedFile {
        let meta = fs::metadata(path).unwrap();
        let size = meta.len();
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64);
        ConfirmedFile {
            path: path.to_path_buf(),
            size,
            mtime,
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

        let out = find_duplicates(vec![cf(&a), cf(&b), cf(&c)], lex_order);

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
        let out = find_duplicates(Vec::new(), lex_order);
        assert_eq!(out, DedupOutcome::default());
    }

    #[test]
    fn same_size_different_content_is_not_grouped() {
        // Two files of equal size but different content must not be grouped: the
        // hash separates them, and even a hash match would be caught by verify.
        let dir = tempfile::tempdir().unwrap();
        let a = write(&dir, "a.jpg", b"AAAAAAAAAA"); // 10 bytes
        let b = write(&dir, "b.jpg", b"BBBBBBBBBB"); // 10 bytes, same size

        let out = find_duplicates(vec![cf(&a), cf(&b)], lex_order);
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

        let with_prefilter = dedup_core(files.clone(), lex_order, hash_file, true);
        let without_prefilter = dedup_core(files, lex_order, hash_file, false);

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

        let out = dedup_core(vec![cf(&a), cf(&b)], lex_order, const_hasher, true);

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

        let out = dedup_core(vec![cf(&a), cf(&b)], lex_order, const_hasher, true);
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

        let out = find_duplicates(vec![cf(&a), cf(&b), cf(&c)], lex_order);

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
            mtime: None,
        };

        let out = find_duplicates(vec![cf(&a), cf(&b), ghost], lex_order);

        assert_eq!(out.groups.len(), 1, "the real duplicates still group");
        assert_eq!(out.warnings.len(), 1, "the unreadable file is reported");
        assert_eq!(
            out.warnings[0].path,
            PathBuf::from("/does/not/exist/ghost.jpg")
        );
    }

    // ── TASK-0306: canonical sort ─────────────────────────────────────────────

    #[test]
    fn outcome_groups_are_in_canonical_order_by_keeper_path() {
        // ADR-0010, TASK-0306: groups must be ordered by keeper path ascending
        // before find_duplicates returns, regardless of hash-map iteration order.
        let dir = tempfile::tempdir().unwrap();
        // Three independent duplicate pairs; whichever hashes first in rayon
        // must end up in keeper-path order in the output.
        let a1 = write(&dir, "aaa1.jpg", b"GROUP-A");
        let a2 = write(&dir, "aaa2.jpg", b"GROUP-A");
        let b1 = write(&dir, "bbb1.jpg", b"GROUP-B");
        let b2 = write(&dir, "bbb2.jpg", b"GROUP-B");
        let c1 = write(&dir, "ccc1.jpg", b"GROUP-C");
        let c2 = write(&dir, "ccc2.jpg", b"GROUP-C");

        // Supply in shuffled order to stress the sort.
        let out = find_duplicates(
            vec![cf(&c1), cf(&a1), cf(&b2), cf(&c2), cf(&b1), cf(&a2)],
            lex_order,
        );

        assert_eq!(out.groups.len(), 3, "three duplicate groups");
        // Canonical: groups sorted by keeper path. Under lex_order, keepers
        // are aaa1, bbb1, ccc1.
        let keeper_paths: Vec<_> = out.groups.iter().map(|g| &g.keeper.path).collect();
        assert_eq!(keeper_paths, vec![&a1, &b1, &c1]);
    }

    #[test]
    fn outcome_warnings_are_in_canonical_order_by_path() {
        // Warnings are sorted by path before returning (ADR-0010).
        let dir = tempfile::tempdir().unwrap();
        // Three ghost paths with the same declared size as a real pair.
        let a = write(&dir, "a.jpg", b"DUP");
        let b = write(&dir, "b.jpg", b"DUP");
        let ghost_z = ConfirmedFile {
            path: PathBuf::from("/zz/ghost.jpg"),
            size: 3,
            mtime: None,
        };
        let ghost_a = ConfirmedFile {
            path: PathBuf::from("/aa/ghost.jpg"),
            size: 3,
            mtime: None,
        };

        let out = find_duplicates(vec![cf(&a), cf(&b), ghost_z, ghost_a], lex_order);

        assert_eq!(out.warnings.len(), 2);
        // Canonical: warnings sorted by path ascending.
        assert!(
            out.warnings[0].path < out.warnings[1].path,
            "warnings must be in ascending path order; got {:?}",
            out.warnings.iter().map(|w| &w.path).collect::<Vec<_>>()
        );
    }

    #[test]
    fn canonical_sort_is_independent_of_input_order() {
        // AC-16 (NFR-3): two runs with shuffled input produce byte-identical
        // outcomes — the canonical sort is the single determinism mechanism.
        let dir = tempfile::tempdir().unwrap();
        let a1 = write(&dir, "aa.jpg", b"SAME");
        let a2 = write(&dir, "bb.jpg", b"SAME");
        let b1 = write(&dir, "cc.jpg", b"OTHER");
        let b2 = write(&dir, "dd.jpg", b"OTHER");

        let out1 = find_duplicates(vec![cf(&a1), cf(&a2), cf(&b1), cf(&b2)], lex_order);
        let out2 = find_duplicates(
            vec![cf(&b2), cf(&a2), cf(&b1), cf(&a1)], // reversed
            lex_order,
        );

        assert_eq!(
            out1, out2,
            "outcome must be identical regardless of input order"
        );
    }

    // ── TASK-0306: determinism at different concurrency widths (AC-16) ────────

    #[test]
    fn outcome_identical_at_single_and_multi_thread_widths() {
        // AC-16 (NFR-3): outcomes must be byte-identical when rayon uses 1
        // thread vs. the default pool (multiple threads). The canonical sort
        // after hashing ensures completion order never leaks into results.
        use rayon::ThreadPoolBuilder;

        let dir = tempfile::tempdir().unwrap();
        // Four duplicate pairs → four groups; distinct sizes so they go into
        // separate cohorts and the parallel + sequential paths both exercise grouping.
        let a1 = write(&dir, "a1.jpg", b"ALPHA!");
        let a2 = write(&dir, "a2.jpg", b"ALPHA!");
        let b1 = write(&dir, "b1.jpg", b"BETA!!");
        let b2 = write(&dir, "b2.jpg", b"BETA!!");
        let files = vec![cf(&a1), cf(&a2), cf(&b1), cf(&b2)];

        // Run in a single-thread pool.
        let out_single = ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap()
            .install(|| find_duplicates(files.clone(), lex_order));

        // Run in a 4-thread pool.
        let out_multi = ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap()
            .install(|| find_duplicates(files, lex_order));

        assert_eq!(
            out_single, out_multi,
            "outcome must be byte-identical at different thread-pool widths"
        );
    }

    // ── TASK-0306: hard-link notes in DedupOutcome ───────────────────────────

    #[test]
    fn hard_link_notes_surfaced_for_collapsed_sets() {
        // FR-17: when a hash-group contains hard links, the collapsed siblings
        // must be surfaced in DedupOutcome.hard_link_notes.
        let dir = tempfile::tempdir().unwrap();
        let a = write(&dir, "a.jpg", b"HARD_LINK_CONTENT");
        let b = dir.path().join("b.jpg"); // hard link to a
        fs::hard_link(&a, &b).expect("hard link creatable in tempdir");
        let c = write(&dir, "c.jpg", b"HARD_LINK_CONTENT"); // separate copy

        let out = find_duplicates(vec![cf(&a), cf(&b), cf(&c)], lex_order);

        // One duplicate group (a vs c); hard-link note for the (a,b) set.
        assert_eq!(out.groups.len(), 1);
        assert_eq!(
            out.hard_link_notes.len(),
            1,
            "one hard-link note expected for the collapsed (a,b) set"
        );
        let note = &out.hard_link_notes[0];
        // Representative is a (first in group-input order), sibling is b.
        assert_eq!(note.representative, a);
        assert_eq!(note.siblings, vec![b]);
    }

    #[test]
    fn no_hard_link_notes_when_no_hard_links() {
        // FR-17: when there are no hard links, hard_link_notes must be empty.
        let dir = tempfile::tempdir().unwrap();
        let a = write(&dir, "a.jpg", b"CONTENT");
        let b = write(&dir, "b.jpg", b"CONTENT");

        let out = find_duplicates(vec![cf(&a), cf(&b)], lex_order);

        assert!(
            out.hard_link_notes.is_empty(),
            "no hard-link notes when there are no hard links"
        );
    }

    // ── B1: hard-link note content is deterministic (NFR-3 / AC-16) ─────────

    #[test]
    fn hard_link_note_representative_is_min_path_and_siblings_are_sorted() {
        // B1 / NFR-3 / AC-16: regardless of HashMap iteration order or thread
        // scheduling, the note's representative must be the lexicographic minimum
        // among the identity set and siblings must be in sorted order.
        //
        // Use a 3-member hard-link set (all same inode) plus a distinct copy so
        // a group is emitted. The "natural" insertion order may vary; the note
        // must always be canonical: representative = min(hhh,aaa,mmm) = aaa,
        // siblings = [hhh, mmm] in sorted order.
        let dir = tempfile::tempdir().unwrap();
        // Create three hard-link paths to the same inode. The first created is
        // "hhh" (not lexicographically smallest) to expose ordering bugs.
        let hhh = write(&dir, "hhh.jpg", b"INODE_CONTENT");
        let aaa = dir.path().join("aaa.jpg");
        fs::hard_link(&hhh, &aaa).expect("hard link creatable");
        let mmm = dir.path().join("mmm.jpg");
        fs::hard_link(&hhh, &mmm).expect("hard link creatable");
        // A separate physical copy so a duplicate group is emitted.
        let zzz = write(&dir, "zzz.jpg", b"INODE_CONTENT");

        // Run 1: input order where hhh appears first (old code would make hhh
        // the representative).
        let out1 = find_duplicates(vec![cf(&hhh), cf(&aaa), cf(&mmm), cf(&zzz)], lex_order);
        // Run 2: input in a different order (aaa first).
        let out2 = find_duplicates(vec![cf(&zzz), cf(&mmm), cf(&aaa), cf(&hhh)], lex_order);

        // Both runs must produce exactly one hard-link note.
        assert_eq!(
            out1.hard_link_notes.len(),
            1,
            "expected one hard-link note (run 1); got {:?}",
            out1.hard_link_notes
        );
        assert_eq!(
            out2.hard_link_notes.len(),
            1,
            "expected one hard-link note (run 2); got {:?}",
            out2.hard_link_notes
        );

        let note1 = &out1.hard_link_notes[0];
        let note2 = &out2.hard_link_notes[0];

        // Representative must be the minimum path among {aaa, hhh, mmm}.
        assert_eq!(
            note1.representative, aaa,
            "representative must be min path (aaa), not first-seen; run 1 got {:?}",
            note1.representative
        );
        assert_eq!(
            note2.representative, aaa,
            "representative must be min path (aaa), not first-seen; run 2 got {:?}",
            note2.representative
        );

        // Siblings must be sorted: [hhh, mmm].
        let expected_siblings = vec![hhh.clone(), mmm.clone()];
        assert_eq!(
            note1.siblings, expected_siblings,
            "siblings must be sorted (run 1); got {:?}",
            note1.siblings
        );
        assert_eq!(
            note2.siblings, expected_siblings,
            "siblings must be sorted (run 2); got {:?}",
            note2.siblings
        );

        // The two outcomes must be byte-identical (NFR-3).
        assert_eq!(
            out1, out2,
            "hard-link note must be identical regardless of input order"
        );
    }

    #[test]
    fn hard_link_notes_identical_at_different_thread_pool_widths() {
        // B1 / NFR-3 / AC-16: hard-link notes must be byte-identical when rayon
        // uses 1 thread vs. 4 threads (completion order must not affect notes).
        use rayon::ThreadPoolBuilder;

        let dir = tempfile::tempdir().unwrap();
        // 3-member hard-link set: ppp, aaa, zzz_link (min = aaa).
        let ppp = write(&dir, "ppp.jpg", b"THREAD_CONTENT");
        let aaa = dir.path().join("aaa.jpg");
        fs::hard_link(&ppp, &aaa).expect("hard link creatable");
        let zzz_link = dir.path().join("zzz.jpg");
        fs::hard_link(&ppp, &zzz_link).expect("hard link creatable");
        // Separate copy for a group to be emitted.
        let sep = write(&dir, "sep.jpg", b"THREAD_CONTENT");

        let files = vec![cf(&ppp), cf(&aaa), cf(&zzz_link), cf(&sep)];

        let out_single = ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap()
            .install(|| find_duplicates(files.clone(), lex_order));

        let out_multi = ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap()
            .install(|| find_duplicates(files, lex_order));

        assert_eq!(
            out_single.hard_link_notes, out_multi.hard_link_notes,
            "hard-link notes must be byte-identical at 1 vs 4 threads"
        );

        // Also verify the note is canonical.
        assert_eq!(out_single.hard_link_notes.len(), 1);
        let note = &out_single.hard_link_notes[0];
        assert_eq!(
            note.representative, aaa,
            "representative must be min path (aaa)"
        );
        let mut expected = vec![ppp.clone(), zzz_link.clone()];
        expected.sort();
        assert_eq!(note.siblings, expected, "siblings must be sorted");
    }

    // ── S1: no orphaned hard-link note when representative excluded ──────────

    #[test]
    fn hard_link_note_dropped_when_representative_excluded_by_verify() {
        // S1: if the representative of a hard-link set fails byte-verify (forced
        // by const_hasher here), its note must be dropped — it must NOT appear
        // as an orphan in outcome.hard_link_notes.
        //
        // Reverse-lex keeper order makes "zzz" the keeper. The hard-link set
        // {aaa, bbb} collapses to representative "aaa" (min path). "aaa" becomes
        // the non-keeper and fails byte-verify against "zzz" (different content).
        // The note for {aaa, bbb} must therefore be dropped.
        let dir = tempfile::tempdir().unwrap();
        let aaa = write(&dir, "aaa.jpg", b"CONTENT-A");
        let bbb = dir.path().join("bbb.jpg");
        fs::hard_link(&aaa, &bbb).expect("hard link creatable");
        // Different content, same size (9 bytes). Sorts last → becomes keeper.
        let zzz = write(&dir, "zzz.jpg", b"CONTENT-Z");

        fn const_hasher(_p: &Path) -> io::Result<[u8; 32]> {
            Ok([0u8; 32])
        }

        let out = dedup_core(
            vec![cf(&aaa), cf(&bbb), cf(&zzz)],
            |x, y| y.path.cmp(&x.path), // reverse lex: zzz is keeper
            const_hasher,
            true,
        );

        // No group is emitted (byte-verify rejects the only non-keeper).
        assert!(
            out.groups.is_empty(),
            "no group when non-keeper fails verify; got {:?}",
            out.groups
        );

        // The note for {aaa, bbb} must be absent — representative was excluded.
        assert!(
            out.hard_link_notes.is_empty(),
            "orphaned hard-link note must be dropped when representative is excluded; \
             got {:?}",
            out.hard_link_notes
        );

        // Exactly one warning for the verification failure.
        assert_eq!(
            out.warnings.len(),
            1,
            "exactly one warning expected for the failed verify; got {:?}",
            out.warnings
        );
    }

    #[test]
    fn hard_link_note_retained_when_representative_in_group() {
        // S1 complement: when the representative IS in the final group (as
        // keeper or removable), the note must be retained.
        let dir = tempfile::tempdir().unwrap();
        let a = write(&dir, "a.jpg", b"SAME_CONTENT");
        let b = dir.path().join("b.jpg"); // hard link to a
        fs::hard_link(&a, &b).expect("hard link creatable");
        let c = write(&dir, "c.jpg", b"SAME_CONTENT"); // separate copy

        let out = find_duplicates(vec![cf(&a), cf(&b), cf(&c)], lex_order);

        // One group (a vs c); note for {a, b} must still be present.
        assert_eq!(out.groups.len(), 1);
        assert_eq!(
            out.hard_link_notes.len(),
            1,
            "note must be retained when representative is in the group; got {:?}",
            out.hard_link_notes
        );
        let note = &out.hard_link_notes[0];
        // a is the min path among {a, b} → representative.
        assert_eq!(note.representative, a);
    }
}
