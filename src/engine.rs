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

#[cfg(test)]
use crate::progress::NoProgress;
use crate::progress::{Phase, ProgressSink};
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

/// Files at or above this size are hashed via BLAKE3's rayon-parallel
/// `update_rayon` ([`RAYON_HASH_BUF_LEN`]-sized reads) instead of serial
/// `update()` ([`STREAM_BUF_LEN`]-sized reads).
///
/// TASK-0322 (see the ADR-0010 amendment for the full measurement): below
/// this size, `update_rayon`'s per-call join overhead is a wash at best and a
/// real regression at small sizes (measured ~2x slower at 32 KiB in
/// isolation); at and above it, `update_rayon` wins whenever the *outer*
/// per-cohort rayon parallelism (the file-level loop in
/// [`dedup_core_with_sink`]) has spare cores — up to ~3.9x in isolation, and
/// 1.5–3.2x for the smallest cohort the engine can actually hash (two
/// same-size files; [`group_by_size`] drops size-singletons without hashing
/// them, so a cohort of one never reaches here) — and even in the worst
/// measured case (hundreds of same-size files
/// saturating every core at once, so the outer level already has no spare
/// capacity) the regression stays within a few percent, never a real loss.
/// 1 MiB sits comfortably past the measured breakeven point (~128 KiB) with
/// margin.
const RAYON_HASH_THRESHOLD: u64 = 1024 * 1024;

/// Upper bound on the streaming buffer used by the `update_rayon` path (see
/// [`RAYON_HASH_THRESHOLD`]). Chosen from the measured sweet spot: throughput
/// on a single large file climbs steeply to 2–4 MiB and then flattens (3.0x at
/// 2 MiB, 3.3x at 4 MiB, 3.6x at 8 MiB on a 200 MiB file), while *contended*
/// cohorts get slightly worse as the buffer grows (0.97x at 4 MiB, 0.90x at
/// 8 MiB with 10 same-size files on 10 cores) — 4 MiB is the knee.
///
/// [`hash_file`] allocates `min(file size, RAYON_HASH_BUF_LEN)`, never the
/// full constant for a file smaller than it: a 1 MiB file (exactly at the
/// threshold) would otherwise pay a 4 MiB buffer to hash 1 MiB of data, which
/// measured both slower under cohort contention and ~2.8x heavier in peak RSS.
///
/// **Memory (ADR-0010, NFR-4).** The bound stays proportional to the *worker
/// pool width*, never to data volume — but it is *not* exactly one buffer per
/// worker: a rayon worker blocked inside `update_rayon`'s internal `join` may
/// steal another file's outer task and hold a second buffer live, so the live
/// buffer count runs above the pool width (measured ~1.5–2x it). Budget
/// `O(2 × pool width × 4 MiB)` — about 80–90 MB at 10 workers, and
/// proportionally more on wider machines. See the 2026-08-04 amendment to
/// ADR-0010.
const RAYON_HASH_BUF_LEN: usize = 4 * 1024 * 1024;

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
/// byte-identical to it and therefore safe to remove, and zero or more
/// protected hard-link anchors (ADR-0008 anchor rule, Amendment 2026-06-15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateGroup {
    /// The file to keep (chosen by the injected keeper ordering among
    /// genuinely-removable candidates; never an anchor).
    pub keeper: FileRec,
    /// Non-keepers byte-for-byte verified identical to `keeper`.
    /// Never contains an anchor (a representative with surviving in-scope
    /// siblings). Removing these frees real storage.
    pub removable: Vec<FileRec>,
    /// Hard-link anchors: representatives whose identity sets had ≥2 scanned
    /// members (non-empty `HardLinkNote.siblings`). These are forced to the
    /// kept side but are **not** the keeper. Removing them would free 0 bytes
    /// (surviving siblings still pin the inode), so they are never in
    /// `removable` and their sizes are never counted in reclaimable.
    /// (ADR-0008 anchor rule, Amendment 2026-06-15.)
    pub protected: Vec<FileRec>,
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
/// `representative` is always the **shortest path** among all paths in the
/// identity set, with ascending byte-wise path order as the tie-break (S2
/// amendment: the keeper-preferred path, so hard-link sets respect `--keep
/// shortest`).
/// `siblings` keeps the same **(path length, byte-wise path)** ordering that
/// selected the representative. This
/// canonical form is established at construction time in [`collapse_identity`]
/// so the invariant holds everywhere — two runs over identical state produce
/// byte-identical `HardLinkNote` values regardless of HashMap iteration order
/// or thread scheduling (AC-16 / NFR-3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardLinkNote {
    /// The representative path for the hard-link identity set: the
    /// **shortest** path among all members of the set (lexicographic
    /// order as the tie-break).
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

/// The result of [`find_duplicates_with_progress`]: verified groups
/// (canonically sorted) plus any warnings.
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
///
/// Progress-free variant of [`find_duplicates_with_progress`], kept
/// `#[cfg(test)]`-only (architect finding 6): [`find_duplicates_with_progress`]
/// is the crate's **sole production entry point** — the compiler proves it
/// (no production code path can reach this function or [`dedup_core`] at
/// all), rather than relying on a lint suppression. `pub(crate)` (rather
/// than fully private) because it is also used as a test fixture builder by
/// other modules' unit tests (`json.rs`, `keeper.rs`, `report.rs`), not just
/// this module's own ~1800 lines of pre-existing tests — both keep
/// compiling unchanged.
#[cfg(test)]
pub(crate) fn find_duplicates<K>(files: Vec<ConfirmedFile>, keeper_order: K) -> DedupOutcome
where
    K: Fn(&FileRec, &FileRec) -> Ordering + Sync,
{
    dedup_core(files, keeper_order, hash_file, true)
}

/// Analyse `files` and return verified duplicate groups, notifying `sink` as
/// candidate files reach their final disposition (TASK-0315, FR-19). This is
/// the crate's sole production entry point into the engine (finding 6); pass
/// [`NoProgress`] for callers that don't care about progress — it behaves
/// identically to the pipeline having no progress-aware code at all (AC-24).
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
pub fn find_duplicates_with_progress<K>(
    files: Vec<ConfirmedFile>,
    keeper_order: K,
    sink: &dyn ProgressSink,
) -> DedupOutcome
where
    K: Fn(&FileRec, &FileRec) -> Ordering + Sync,
{
    dedup_core_with_sink(files, keeper_order, hash_file, true, sink)
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Progress-free variant of [`dedup_core_with_sink`] (see there for the
/// pipeline description), `#[cfg(test)]`-only for the same reason as
/// [`find_duplicates`] — kept with this exact 4-argument signature so tests
/// that call it directly (to force hash collisions, or to assert size
/// pre-grouping is equivalent to hashing every file, AC-9) keep compiling
/// unchanged.
#[cfg(test)]
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
    dedup_core_with_sink(files, keeper_order, hasher, size_prefilter, &NoProgress)
}

/// Core pipeline, additionally notifying `sink` as each candidate file
/// reaches its final disposition (TASK-0315, FR-19).
///
/// Hashing within each size cohort is executed with rayon parallel iterators
/// (ADR-0010, TASK-0306). The `hasher` function must be `Send + Sync` so it can
/// be called from rayon worker threads.
///
/// ## Progress accounting (FR-19)
///
/// `sink.set_total(files.len())` is called here, once, as the very first
/// action — the engine is the single source of truth for the run-wide
/// denominator (architect finding 11: previously `main.rs` computed it
/// separately from `confirmed.len()`, which could desync from what the
/// engine actually iterates if the two were ever refactored independently).
///
/// The pipeline is then walked in two passes so the phase toggles **at most
/// once** each way ([`Phase::Hashing`], only if there is anything to hash at
/// all, then [`Phase::Verifying`], only if there is any byte-verification
/// workload at all) regardless of how many size cohorts or hash groups
/// exist, which keeps both the TTY redraw cadence and the non-TTY
/// milestone-line count bounded. An empty candidate set enters neither phase
/// (architect finding N1: the same "skip a zero-workload phase entirely"
/// rule that finding 2 established for Verifying) — the run goes straight
/// to [`ProgressSink::finish`]'s terminal `Done`.
///
/// 1. **Hashing pass** — every size-singleton dropped by the size
///    pre-grouping is advanced up front (its disposition is final: it is
///    never hashed). Then, for every size cohort, **every file is advanced
///    by 1 the instant it is hashed** (success *or* failure), from inside
///    the rayon closure — hashing is the slow, dominant phase on a real
///    photo library, so it must animate smoothly from 0% to `y` as files
///    complete, not jump in one bulk step once the whole pass is done. Files
///    are then grouped by hash; non-singleton groups are deferred to the
///    verify pass (their disposition is *not yet* final — a hash match is
///    only a pre-filter, FR-4 — but they were already counted the moment
///    they were hashed, so they must not be counted again).
/// 2. **Verify pass** — the files entering this pass (the deferred
///    non-singleton hash groups) are typically a much smaller workload than
///    `y`; reusing `y` as the denominator here would either overrun it or
///    pin the display at a misleading "100%" while still working. So the
///    counter is [`ProgressSink::retotal`]-ed to the count of files actually
///    entering verification, then advanced by each group's size right after
///    [`process_hash_group`] returns (every member's disposition is decided
///    by then, regardless of which internal branch — all-anchor group,
///    verify pass/fail, ... — was taken). **If that count is zero, the phase
///    is skipped entirely** — no `retotal`, no `set_phase` — rather than
///    showing a degenerate `0 of 0` (architect finding 2; a duplicate-free
///    tree now goes straight from Hashing to [`ProgressSink::finish`]'s
///    terminal `Done`).
///
/// This guarantees the **per-phase counting invariant**: every file is
/// counted exactly once during the hashing pass (summing to `y`, the input
/// length) and, when the phase runs at all, exactly once during the verify
/// pass (summing to the number of files that entered it) — see the
/// `find_duplicates_with_progress_*` tests below, including a regression
/// test asserting hashing advances arrive incrementally rather than in one
/// bulk jump.
///
/// Before returning, the outcome is **canonically sorted** (ADR-0010): groups
/// by keeper path, files within each group by the keeper order, warnings by
/// path, hard-link notes by representative path. This canonical sort is the
/// single determinism mechanism — completion order (including the two-pass
/// restructuring above) never leaks into results.
fn dedup_core_with_sink<H, K>(
    files: Vec<ConfirmedFile>,
    keeper_order: K,
    hasher: H,
    size_prefilter: bool,
    sink: &dyn ProgressSink,
) -> DedupOutcome
where
    H: Fn(&Path) -> io::Result<[u8; 32]> + Sync,
    K: Fn(&FileRec, &FileRec) -> Ordering + Sync,
{
    let mut outcome = DedupOutcome::default();

    // The engine is the single source of truth for the run-wide denominator
    // (finding 11) — set once, here, before anything else.
    sink.set_total(files.len() as u64);
    // Finding N1: a phase with zero workload is skipped entirely, not shown
    // as a degenerate `0 of 0` — the same rule already applied to Verifying
    // (finding 2) applies here too. An empty candidate set has nothing to
    // hash, so Hashing is never entered; the run goes straight from
    // Scanning (main.rs) to `ProgressSink::finish`'s terminal `Done`, which
    // *does* still render `done: 0 of 0 (100%)` — the AC explicitly
    // sanctions the completion line against the full total even when it is
    // zero.
    if !files.is_empty() {
        sink.set_phase(Phase::Hashing);
    }

    // Stage 1 — size pre-grouping (FR-5, AC-9). Singleton sizes are dropped
    // without hashing — their disposition is final now, so they are counted
    // here. The non-prefilter path (one cohort of everything) exists only so
    // the equivalence with hashing-everything can be asserted (AC-9); it
    // never drops a size-singleton. `saturating_sub` (finding 9): a future
    // change to cohort construction that ever produced more cohort members
    // than input files would be a logic bug, but progress accounting must
    // not be where it panics (debug) or silently wraps (release).
    let total_files = files.len();
    let cohorts = if size_prefilter {
        group_by_size(files)
    } else if files.is_empty() {
        Vec::new()
    } else {
        vec![files]
    };
    let cohort_files: usize = cohorts.iter().map(Vec::len).sum();
    let size_singletons = total_files.saturating_sub(cohort_files);
    if size_singletons > 0 {
        sink.advance(size_singletons as u64);
    }

    // Deferred hash groups (size > 1), collected across every cohort so the
    // verify pass below runs once, after all hashing is done.
    let mut pending_hash_groups: Vec<Vec<FileRec>> = Vec::new();

    for cohort in cohorts {
        // Stage 2 — hash each file in the cohort in parallel (ADR-0010, rayon).
        // par_iter's collect into a Vec actually does preserve input order
        // (indexed collect splits the destination by index, regardless of
        // completion order — see sniff.rs's TASK-0321 doc comment for the
        // full argument); the safety property here doesn't rely on that,
        // though, since the outcome is canonically sorted below anyway. We
        // collect all results and then split into recs and warnings so each
        // per-file outcome is atomic. Every file's
        // hashing disposition is final the instant it is hashed — success or
        // failure — so it is advanced right here, inside the parallel
        // closure, via the thread-safe sink (FR-19: this is what makes the
        // hashing phase animate incrementally instead of jumping once at the
        // end).
        let hash_results: Vec<(ConfirmedFile, io::Result<[u8; 32]>)> = cohort
            .into_par_iter()
            .map(|cf| {
                let result = hasher(&cf.path);
                sink.advance(1);
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

        // Stage 3 — group by hash (drop singletons). Every one of these
        // files was already advanced above when it was hashed, so grouping
        // itself advances nothing further; non-singleton groups are deferred
        // to the verify pass.
        pending_hash_groups.extend(group_by_hash(recs));
    }

    // Stage 4 — verify every deferred hash group (identity collapse + keeper
    // selection + byte-verify). This phase's workload is typically much
    // smaller than `y`, so it gets its own denominator (FR-19) rather than
    // continuing to count against the hashing-phase total — otherwise the
    // display would either overrun `y` or sit at a misleading "100%" while
    // still working. `retotal` is paired immediately with `set_phase` so the
    // label and the new denominator change together in the rendered output.
    // When there is nothing to verify, the phase is skipped entirely
    // (finding 2) — `pending_hash_groups` is empty in that case by
    // construction (every pushed group has length >= 2), so there is
    // nothing left to iterate either way.
    let verify_total: u64 = pending_hash_groups.iter().map(|g| g.len() as u64).sum();
    if verify_total > 0 {
        sink.retotal(verify_total);
        sink.set_phase(Phase::Verifying);
        for group in pending_hash_groups {
            // Captured before the call because `process_hash_group` consumes
            // `group`; every member's disposition is decided by the time it
            // returns, regardless of which internal branch (all-anchor group,
            // verify pass/fail, ...) was taken.
            let group_len = group.len() as u64;
            process_hash_group(group, &keeper_order, &mut outcome);
            sink.advance(group_len);
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
/// - Within each group's `protected`: by the `keeper_order` comparator.
/// - `warnings`: by path.
/// - `hard_link_notes`: by representative path.
fn sort_canonical<K>(outcome: &mut DedupOutcome, keeper_order: &K)
where
    K: Fn(&FileRec, &FileRec) -> Ordering,
{
    // Sort each group's removable and protected lists by the total order.
    for g in &mut outcome.groups {
        g.removable.sort_by(keeper_order);
        g.protected.sort_by(keeper_order);
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
/// Returns the deduplicated list of `(FileRec, is_anchor)` pairs and any
/// [`HardLinkNote`]s for collapsed sets (one note per set that had at least
/// one sibling collapsed). Notes are used by the report stage (TASK-0307, FR-17).
///
/// `is_anchor` is `true` when the representative absorbed ≥1 in-scope sibling
/// (non-empty siblings list). An anchor must never be a removal candidate
/// (ADR-0008 anchor rule, Amendment 2026-06-15).
///
/// **Invariant (NFR-4):** `same-file::Handle` holds an open file descriptor for
/// its lifetime. Identity is therefore resolved here, **only within a single
/// hash-group** (a small set of same-size/same-hash candidates), and the handles
/// are dropped when this function returns. The full candidate set is never
/// handle-mapped.
fn collapse_identity(
    recs: Vec<FileRec>,
    warnings: &mut Vec<EngineWarning>,
) -> (Vec<(FileRec, bool)>, Vec<HardLinkNote>) {
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
    let recs_with_anchor: Vec<(FileRec, bool)> = reps
        .into_iter()
        .map(|(_, mut rec, mut siblings)| {
            let is_anchor = !siblings.is_empty();
            if is_anchor {
                // B1 (NFR-3): canonicalize each note so `representative` is
                // preferred by keeper selection among all paths in the identity
                // set. With anchors now as keeper candidates (S2 amendment),
                // pick the shortest path by character count as the representative,
                // with lexicographic path order as the deterministic tie-break.
                // This ensures the keeper-preferred representative flows into
                // selection and makes hard-link sets respect --keep shortest.
                //
                // We also update `rec.path` to this preferred path so that the
                // `FileRec` flowing into keeper selection carries the same path
                // that the note's `representative` field names. All paths in an
                // identity set point to the same physical file (same inode), so
                // switching which path label the FileRec carries does not change
                // *which file* is kept — only which path name refers to it.
                // The report's matching logic (`note.representative ∈
                // group_paths`) depends on these two being in sync.
                siblings.push(rec.path.clone());
                // Sort by (char_count, lexicographic_path) to prefer shortest path
                siblings.sort_by(|a, b| {
                    a.as_os_str()
                        .len()
                        .cmp(&b.as_os_str().len())
                        .then_with(|| a.cmp(b))
                });
                // `siblings` is now non-empty (we just pushed `rec.path`);
                // `remove(0)` extracts the shortest path (or first by path order if tied).
                let best_path = siblings.remove(0);
                rec.path = best_path.clone();
                notes.push(HardLinkNote {
                    representative: best_path,
                    siblings,
                });
            }
            (rec, is_anchor)
        })
        .collect();
    (recs_with_anchor, notes)
}

/// Verify a single hash-group and, if it yields genuine duplicates, push a
/// [`DuplicateGroup`] onto `outcome`. Any hard-link notes are pushed to
/// `outcome.hard_link_notes` (FR-17).
///
/// Implements the **anchor rule** (ADR-0008 Amendment 2026-06-15):
///
/// 1. Collapse identity sets → list of `(FileRec, is_anchor)`.
/// 2. Partition into `anchors` (is_anchor) and `candidates` (non-anchors).
/// 3. If `candidates` is non-empty: keeper = `min_by(keeper_order)` over
///    candidates; remaining candidates are byte-verified and become `removable`;
///    all `anchors` go to `protected` (never byte-verified, never removable).
/// 4. If `candidates` is empty (all-anchor group): emit **no** `DuplicateGroup`;
///    retain hard-link notes so the report can surface the already-deduplicated
///    note via the `hard_link_notes` path.
///
/// A non-keeper that fails verification (BLAKE3 pre-filter collision or I/O
/// error) is excluded and reported — never removed on a hash match alone (FR-4,
/// NFR-1).
fn process_hash_group<K>(group: Vec<FileRec>, keeper_order: &K, outcome: &mut DedupOutcome)
where
    K: Fn(&FileRec, &FileRec) -> Ordering,
{
    let (reps_with_anchor, hl_notes) = collapse_identity(group, &mut outcome.warnings);

    // Partition into anchors (is_anchor = true) and standalones (non-anchors).
    // Anchors are representatives whose identity set had ≥2 in-scope members.
    //
    // TASK-0316: this partition must happen *before* any count-based early
    // return. A single anchor is still an all-anchor case under ADR-0008 edge
    // case 1 — one inode trivially satisfies "every distinct inode is
    // multiply-linked in-scope" — so bailing out on fewer than two
    // representatives silently discarded the note for a lone hard-link set and
    // printed "No duplicate groups found." for a tree the guide documents as
    // already-deduplicated.
    let (anchors, standalones): (Vec<_>, Vec<_>) = reps_with_anchor
        .into_iter()
        .partition(|(_, is_anchor)| *is_anchor);

    // Early return for all-anchor groups (no keeper to choose)
    if standalones.is_empty() {
        // Edge case 1 (ADR-0008): all representatives are anchors — every
        // distinct inode is multiply-linked in-scope. Nothing is removable.
        // Retain all hard-link notes so the report can show the
        // already-deduplicated note (no DuplicateGroup emitted). Reached with
        // one anchor (a lone hard-link set) as well as with several.
        for note in hl_notes {
            outcome.hard_link_notes.push(note);
        }
        return;
    }

    if standalones.len() == 1 && anchors.is_empty() {
        // A single distinct physical file with no hard-linked siblings: not a
        // duplicate of anything and not already-deduplicated. Nothing to report.
        return;
    }

    // Keeper from all representatives (anchors + standalones).
    // Amendment 2026-06-15: anchors are included in keeper ranking to honor --keep
    // semantics and maximize safe reclamation.
    let all_reps: Vec<(&FileRec, bool)> = anchors
        .iter()
        .map(|(rec, is_anchor)| (rec, *is_anchor))
        .chain(standalones.iter().map(|(rec, is_anchor)| (rec, *is_anchor)))
        .collect();

    let keeper_idx = (0..all_reps.len())
        .min_by(|&i, &j| keeper_order(all_reps[i].0, all_reps[j].0))
        .expect("all_reps is non-empty");

    let (keeper_rec, keeper_is_anchor) = all_reps[keeper_idx];
    let keeper = keeper_rec.clone();

    // Partition protected/removable based on keeper type (Amendment 2026-06-15)
    let (protected, removable) = if keeper_is_anchor {
        // Keeper is an anchor: protect other anchors and their hard-link siblings,
        // remove all standalones.
        let mut protected: Vec<FileRec> = Vec::new();

        // Add non-keeper anchors
        for (rec, _) in anchors.iter() {
            if rec.path != keeper.path {
                protected.push(rec.clone());
            }
        }

        // Add hard-link siblings of ALL anchors (including keeper)
        for note in &hl_notes {
            if let Some(rep) = anchors.iter().find(|(r, _)| r.path == note.representative) {
                for sibling_path in &note.siblings {
                    protected.push(FileRec {
                        path: sibling_path.clone(),
                        size: rep.0.size,
                        mtime: rep.0.mtime,
                        hash: rep.0.hash,
                    });
                }
            }
        }

        let removable: Vec<FileRec> = standalones.into_iter().map(|(rec, _)| rec).collect();
        (protected, removable)
    } else {
        // Keeper is a standalone: protect all anchors and their hard-link siblings,
        // remove other standalones.
        let mut protected: Vec<FileRec> = Vec::new();

        // Add all anchors
        for (rec, _) in anchors.iter() {
            protected.push(rec.clone());
        }

        // Add hard-link siblings of all anchors
        for note in &hl_notes {
            if let Some(rep) = anchors.iter().find(|(r, _)| r.path == note.representative) {
                for sibling_path in &note.siblings {
                    protected.push(FileRec {
                        path: sibling_path.clone(),
                        size: rep.0.size,
                        mtime: rep.0.mtime,
                        hash: rep.0.hash,
                    });
                }
            }
        }

        let removable: Vec<FileRec> = standalones
            .into_iter()
            .filter(|(rec, _)| rec.path != keeper.path)
            .map(|(rec, _)| rec)
            .collect();
        (protected, removable)
    };

    // Byte-verify each candidate removable vs the keeper
    let verified_removable: Vec<FileRec> = removable
        .into_iter()
        .filter(|rec| match files_equal(&keeper.path, &rec.path) {
            Ok(true) => true,
            Ok(false) => {
                outcome.warnings.push(EngineWarning {
                    path: rec.path.clone(),
                    reason: "content differs from keeper despite a matching hash \
                                 (BLAKE3 pre-filter collision); not removed"
                        .to_owned(),
                });
                false
            }
            Err(e) => {
                outcome.warnings.push(EngineWarning {
                    path: rec.path.clone(),
                    reason: format!("could not byte-verify against keeper: {e}"),
                });
                false
            }
        })
        .collect();

    // Only emit a DuplicateGroup when there is something to report.
    // Note: with anchors present we always have at least one protected entry,
    // so the group is meaningful even with empty removable (the PROTECT row
    // informs the user about the hard-linked anchor).
    let has_content = !verified_removable.is_empty() || !protected.is_empty();
    if has_content {
        // Extend note retention (S1 fix + anchor extension): retain notes
        // whose representative is the keeper, a removable, OR a protected
        // anchor in the emitted group. The report's group_paths set must
        // include all three categories for note matching to work.
        let group_paths: std::collections::HashSet<&Path> = std::iter::once(keeper.path.as_path())
            .chain(verified_removable.iter().map(|r| r.path.as_path()))
            .chain(protected.iter().map(|r| r.path.as_path()))
            .collect();
        for note in hl_notes {
            if group_paths.contains(note.representative.as_path()) {
                outcome.hard_link_notes.push(note);
            }
            // Notes whose representative was excluded by byte-verify failure
            // are silently dropped: they have no group to attach to.
        }
        outcome.groups.push(DuplicateGroup {
            keeper,
            removable: verified_removable,
            protected,
        });
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
/// Never holds the whole file in memory (ADR-0010). Files at or above
/// [`RAYON_HASH_THRESHOLD`] are hashed via BLAKE3's intra-file parallel
/// `update_rayon` (TASK-0322); smaller files use the plain serial `update()`
/// path, where `update_rayon`'s per-call overhead is not worth paying. Both
/// paths are equivalent by BLAKE3's own guarantee: the digest depends only on
/// the bytes, never on how they were chunked (pinned by
/// `hash_file_matches_reference_hash_across_threshold_boundary`).
///
/// The size used to pick the branch is read from the **already-open handle**
/// (`File::metadata`), not from the path a second time, so it cannot be
/// re-resolved to a different file. It selects a strategy and a buffer length
/// and nothing else: if the file changes size between that stat and the read
/// loop, both paths still stream to EOF and hash exactly the bytes they read.
/// A file mutated mid-run may therefore hash to something that no longer
/// matches its enumerated size — a pre-existing scan-time TOCTOU that
/// ADR-0008's byte-for-byte verification (and TASK-0309's apply-time
/// re-validation) remains the authority over; hashing is only a pre-filter.
fn hash_file(path: &Path) -> io::Result<[u8; 32]> {
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    let mut hasher = blake3::Hasher::new();
    if size >= RAYON_HASH_THRESHOLD {
        // Never allocate more buffer than the file can fill: a file just over
        // the threshold would otherwise pay the full 4 MiB. `size` is at least
        // RAYON_HASH_THRESHOLD here, so the buffer is never empty (an empty
        // buffer would make `read` return 0 and hash nothing). The cap is a
        // memory/throughput optimization only — if the file grows between this
        // stat and the reads below, the loop simply takes more iterations and
        // still hashes every byte it can read.
        let buf_len = RAYON_HASH_BUF_LEN.min(usize::try_from(size).unwrap_or(usize::MAX));
        let mut buf = vec![0u8; buf_len];
        loop {
            let n = file.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update_rayon(&buf[..n]);
        }
    } else {
        let mut buf = vec![0u8; STREAM_BUF_LEN];
        loop {
            let n = file.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
    }
    Ok(*hasher.finalize().as_bytes())
}

/// Return `true` if the two files have byte-for-byte identical contents.
///
/// Streams both files through fixed buffers and short-circuits on the first
/// differing byte or a length mismatch (ADR-0010). Never buffers a whole file.
pub(crate) fn files_equal(a: &Path, b: &Path) -> io::Result<bool> {
    let mut fa = File::open(a)?;
    let mut fb = File::open(b)?;
    files_equal_handles(&mut fa, &mut fb)
}

/// Byte-compare two files that are **already open**.
///
/// The apply stage needs this (TASK-0309): re-opening by path there would mean
/// the bytes it compares and the metadata/identity it checked could belong to
/// different files, since a path can be re-pointed between the two. Comparing
/// through handles it already holds keeps every check about one file.
pub(crate) fn files_equal_handles(fa: &mut File, fb: &mut File) -> io::Result<bool> {
    let mut ba = vec![0u8; STREAM_BUF_LEN];
    let mut bb = vec![0u8; STREAM_BUF_LEN];
    loop {
        let na = fill(fa, &mut ba)?;
        let nb = fill(fb, &mut bb)?;
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

    // ── TASK-0322: intra-file parallel hashing via update_rayon ───────────────

    /// Deterministic pseudo-random payload of `len` bytes (xorshift), so large
    /// test payloads don't depend on any external RNG crate.
    fn pseudo_random_payload(len: usize) -> Vec<u8> {
        let mut state: u64 = 0x9E3779B97F4A7C15;
        let mut out = Vec::with_capacity(len);
        while out.len() < len {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            out.extend_from_slice(&state.to_le_bytes());
        }
        out.truncate(len);
        out
    }

    /// `hash_file` must agree with BLAKE3's own one-shot hasher regardless of
    /// which internal streaming path (serial `update()` below the threshold,
    /// `update_rayon()` at/above it) handles a given size — BLAKE3 guarantees
    /// the same digest for the same bytes no matter how they are chunked, so
    /// any mismatch here would point at this module's own buffering logic,
    /// not the library. Covers just below, exactly at, and just above
    /// `RAYON_HASH_THRESHOLD` to pin the branch boundary, plus a size past
    /// `RAYON_HASH_BUF_LEN` so the parallel path is exercised with **more than
    /// one** `update_rayon` call and an unaligned tail — the buffer is capped
    /// at the file size, so every smaller case fills it in a single read and
    /// would not catch a multi-chunk buffering bug on its own.
    #[test]
    fn hash_file_matches_reference_hash_across_threshold_boundary() {
        let dir = tempfile::tempdir().unwrap();
        let sizes = [
            0usize,
            1,
            (RAYON_HASH_THRESHOLD as usize) - 1,
            RAYON_HASH_THRESHOLD as usize,
            (RAYON_HASH_THRESHOLD as usize) + 1,
            (RAYON_HASH_THRESHOLD as usize) * 2 + 12345, // above threshold, not buffer-aligned
            RAYON_HASH_BUF_LEN + 12345, // > one full rayon buffer: multi-chunk + tail
        ];
        for (i, &size) in sizes.iter().enumerate() {
            let payload = pseudo_random_payload(size);
            let path = write(&dir, &format!("payload_{i}.bin"), &payload);

            let expected = *blake3::hash(&payload).as_bytes();
            let actual = hash_file(&path).unwrap();
            assert_eq!(
                actual, expected,
                "hash_file must match the reference one-shot hash at size {size}"
            );
        }
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
            g.protected.sort_by(|x, y| x.path.cmp(&y.path));
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
        // removal. Under S2 amendment (anchor-keeper extension), if the anchor
        // wins the keeper order, it becomes the keeper and other anchors (siblings)
        // go to protected.
        //
        // Previously (pre-amendment) this test asserted `c` (the standalone) was
        // the keeper. Under the amendment, with all representatives competing,
        // `a` (the hard-link representative, shortest path) becomes the keeper
        // (anchor, same inode as `b`), and `c` becomes removable. The sibling
        // `b` goes to protected.
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

        // Under the amendment: anchor `a` wins by lex_order (aaa < ccc) → keeper.
        assert_eq!(
            g.keeper.path, a,
            "anchor (shortest/first lex) must be keeper"
        );

        // Removable is the standalone `c`.
        assert_eq!(g.removable.len(), 1, "one removable: the standalone copy");
        assert_eq!(g.removable[0].path, c, "standalone must be removable");

        // Protected contains the sibling `b` of the anchor keeper.
        assert_eq!(g.protected.len(), 1, "one protected anchor (sibling)");
        assert_eq!(
            g.protected[0].path, b,
            "hard-link sibling must be protected"
        );

        // The hard-link sibling `b` must never be the keeper.
        assert_ne!(g.keeper.path, b, "sibling must not be keeper");
        // And must never be removable.
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
        // S1 (updated for anchor rule): previously this tested that a note was
        // dropped when its representative failed byte-verify as a non-keeper.
        // Under the anchor rule (ADR-0008 Amendment 2026-06-15) the scenario
        // changes: the hard-link representative `aaa` is now an **anchor**
        // (absorbed sibling `bbb`) and goes to `protected`, never byte-verified.
        // The anchor IS in `group_paths`, so the note is RETAINED. The original
        // S1 concern (no orphaned notes) still holds: the note is retained
        // because the anchor is always present in the group's group_paths set.
        //
        // The note-dropping behavior only fires when the representative is a
        // non-anchor non-keeper that fails byte-verify.  That sub-case is
        // exercised by `hard_link_note_dropped_for_non_anchor_verify_failure`
        // below.
        //
        // Here we verify the updated behavior: reverse-lex keeper order makes
        // "zzz" the keeper. The hard-link set {aaa, bbb} collapses to anchor
        // representative "aaa" (min path). Under the anchor rule, "aaa" is
        // placed in `protected` (not byte-verified). The note for {aaa, bbb}
        // is retained because "aaa" is in `group_paths` as a protected entry.
        let dir = tempfile::tempdir().unwrap();
        let aaa = write(&dir, "aaa.jpg", b"CONTENT-A");
        let bbb = dir.path().join("bbb.jpg");
        fs::hard_link(&aaa, &bbb).expect("hard link creatable");
        // Different content, same size (9 bytes). Sorts last under reverse-lex.
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

        // Under the anchor rule: group IS emitted (anchor in protected).
        assert_eq!(
            out.groups.len(),
            1,
            "group must be emitted with anchor in protected; got {:?}",
            out.groups
        );
        let g = &out.groups[0];
        assert_eq!(g.keeper.path, zzz, "zzz must be keeper");
        assert!(g.removable.is_empty(), "no removable (anchor not verified)");
        // With S2 amendment: protected includes both the anchor representative
        // and its hard-link sibling.
        assert_eq!(g.protected.len(), 2, "anchor representative and sibling");
        assert!(
            g.protected.iter().any(|r| r.path == aaa),
            "aaa must be protected"
        );
        assert!(
            g.protected.iter().any(|r| r.path == bbb),
            "bbb sibling must be protected"
        );

        // Note IS retained because anchor is in group_paths.
        assert_eq!(
            out.hard_link_notes.len(),
            1,
            "note must be retained (anchor in group_paths); got {:?}",
            out.hard_link_notes
        );
        assert_eq!(out.hard_link_notes[0].representative, aaa);

        // No warnings: anchor is not byte-verified, no failure.
        assert!(
            out.warnings.is_empty(),
            "no warnings expected (anchor skips verify); got {:?}",
            out.warnings
        );
    }

    #[test]
    fn hard_link_note_dropped_for_non_anchor_verify_failure() {
        // S1 original concern: a non-anchor non-keeper that fails byte-verify
        // is excluded from the group. If that rep had a note, the note is
        // dropped (no orphan). Here we set up a scenario where:
        // - `aaa.jpg` is a lone single-link rep (no siblings → NOT an anchor)
        //   with content "CONTENT-A"
        // - `zzz.jpg` is the keeper (reverse-lex → zzz first) with content
        //   "CONTENT-Z" (different)
        // const_hasher forces them into one hash group. Byte-verify rejects
        // `aaa` (content differs). No note for `aaa` (it was never an anchor),
        // so no orphan risk — but the group itself should be empty (no
        // removable, no protected → no group emitted). No warnings.
        //
        // Actually, both are non-anchors (single-link). zzz is keeper; aaa
        // fails verify → warning. No group emitted.
        let dir = tempfile::tempdir().unwrap();
        let aaa = write(&dir, "aaa.jpg", b"CONTENT-A");
        let zzz = write(&dir, "zzz.jpg", b"CONTENT-Z"); // same size, different content

        fn const_hasher(_p: &Path) -> io::Result<[u8; 32]> {
            Ok([0u8; 32])
        }

        let out = dedup_core(
            vec![cf(&aaa), cf(&zzz)],
            |x, y| y.path.cmp(&x.path), // reverse lex: zzz is keeper
            const_hasher,
            true,
        );

        // No group (only candidate failed verify, nothing in removable or protected).
        assert!(
            out.groups.is_empty(),
            "no group when non-anchor non-keeper fails verify; got {:?}",
            out.groups
        );

        // No hard-link notes (no anchors, no notes).
        assert!(
            out.hard_link_notes.is_empty(),
            "no hard-link notes expected; got {:?}",
            out.hard_link_notes
        );

        // One warning for the verification failure.
        assert_eq!(
            out.warnings.len(),
            1,
            "one warning for the rejected non-keeper; got {:?}",
            out.warnings
        );
        assert_eq!(out.warnings[0].path, aaa);
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

    // ── Anchor rule (ADR-0008 Amendment 2026-06-15) ──────────────────────────

    #[test]
    fn anchor_representative_is_protected_not_removable() {
        // ADR-0008 anchor rule with S2 amendment: hard-link anchor can be keeper
        // if it wins by keeper_order.
        //
        // `aaa.jpg` and `bbb.jpg` are hard links (same inode); `ccc.jpg` is a
        // separate copy. Under lex_order and shortest-path rep selection,
        // `aaa` becomes the representative for the hard-link set and wins as keeper
        // (aaa < ccc lexicographically). `ccc` becomes removable, and `bbb`
        // (the sibling) appears in protected.
        let dir = tempfile::tempdir().unwrap();
        let aaa = write(&dir, "aaa.jpg", b"SHARED INODE CONTENT");
        let bbb = dir.path().join("bbb.jpg");
        fs::hard_link(&aaa, &bbb).expect("hard link should be creatable in tempdir");
        let ccc = write(&dir, "ccc.jpg", b"SHARED INODE CONTENT"); // separate copy

        let out = find_duplicates(vec![cf(&aaa), cf(&bbb), cf(&ccc)], lex_order);

        assert_eq!(out.groups.len(), 1, "one group from distinct-identity pair");
        let g = &out.groups[0];

        // The anchor representative (aaa) wins by lex_order → keeper.
        assert_eq!(
            g.keeper.path, aaa,
            "anchor (shortest/first lex) must be keeper"
        );

        // Removable is the standalone (ccc).
        assert_eq!(g.removable.len(), 1, "one removable: the standalone");
        assert_eq!(g.removable[0].path, ccc, "standalone must be removable");

        // Protected contains the hard-link sibling (bbb).
        assert_eq!(g.protected.len(), 1, "one protected: the hard-link sibling");
        assert_eq!(
            g.protected[0].path, bbb,
            "hard-link sibling must be protected"
        );

        // reclaimable = size of ccc only (anchor and its sibling are not removable).
        let reclaimable: u64 = g.removable.iter().map(|r| r.size).sum();
        let ccc_size = fs::metadata(&ccc).unwrap().len();
        assert_eq!(
            reclaimable, ccc_size,
            "reclaimable must equal the removable standalone size"
        );
    }

    #[test]
    fn two_standalones_plus_anchor_keeper_is_standalone_min() {
        // ADR-0008 anchor rule scenario 2: hard-link set + two standalone copies.
        //
        // `h.jpg` ↔ `h2.jpg` (same inode, anchor rep = `h.jpg` which is min path);
        // `c1.jpg` and `c2.jpg` are distinct inodes, byte-identical to the rest.
        // lex_order: keeper = min of {c1, c2} = c1; c2 is removable; h is protected.
        let dir = tempfile::tempdir().unwrap();
        let h = write(&dir, "h.jpg", b"CONTENT");
        let h2 = dir.path().join("h2.jpg");
        fs::hard_link(&h, &h2).expect("hard link creatable");
        let c1 = write(&dir, "c1.jpg", b"CONTENT");
        let c2 = write(&dir, "c2.jpg", b"CONTENT");

        let out = find_duplicates(vec![cf(&h), cf(&h2), cf(&c1), cf(&c2)], lex_order);

        assert_eq!(out.groups.len(), 1, "one group");
        let g = &out.groups[0];

        // Keeper = min = c1 (lex order, shorter than anchor h).
        assert_eq!(g.keeper.path, c1, "keeper must be c1 (min by lex_order)");

        // Removable = the other standalone c2.
        assert_eq!(g.removable.len(), 1, "one removable");
        assert_eq!(g.removable[0].path, c2, "c2 must be removable");

        // Protected = the anchor (h) and its hard-link sibling (h2).
        assert_eq!(g.protected.len(), 2, "anchor and hard-link sibling");
        assert!(
            g.protected.iter().any(|r| r.path == h),
            "h must be protected"
        );
        assert!(
            g.protected.iter().any(|r| r.path == h2),
            "h2 sibling must be protected"
        );

        // reclaimable = c2.size only (h and h2 are anchors, excluded).
        let reclaimable: u64 = g.removable.iter().map(|r| r.size).sum();
        let c2_size = fs::metadata(&c2).unwrap().len();
        assert_eq!(reclaimable, c2_size, "reclaimable = removable size only");

        // Anchor must never be in removable.
        assert!(
            g.removable.iter().all(|r| r.path != h && r.path != h2),
            "anchors must never be in removable"
        );
    }

    #[test]
    fn all_anchor_group_emits_no_duplicate_group() {
        // ADR-0008 anchor rule edge case 1: every distinct inode is multiply-linked
        // in-scope → every representative is an anchor → no DuplicateGroup emitted.
        //
        // Inode I1: a.jpg ↔ a2.jpg; Inode I2: b.jpg ↔ b2.jpg; both byte-identical.
        // Both representatives are anchors → no group, 0 reclaimable.
        let dir = tempfile::tempdir().unwrap();
        let a = write(&dir, "a.jpg", b"SAME");
        let a2 = dir.path().join("a2.jpg");
        fs::hard_link(&a, &a2).expect("hard link creatable");
        let b = write(&dir, "b.jpg", b"SAME");
        let b2 = dir.path().join("b2.jpg");
        fs::hard_link(&b, &b2).expect("hard link creatable");

        let out = find_duplicates(vec![cf(&a), cf(&a2), cf(&b), cf(&b2)], lex_order);

        // No DuplicateGroup: all reps are anchors.
        assert!(
            out.groups.is_empty(),
            "all-anchor group must emit no DuplicateGroup; got {:?}",
            out.groups
        );

        // Hard-link notes must be present for the two inode sets.
        assert_eq!(
            out.hard_link_notes.len(),
            2,
            "two hard-link notes expected (one per inode); got {:?}",
            out.hard_link_notes
        );

        // No reclaimable bytes.
        let total_reclaimable: u64 = out
            .groups
            .iter()
            .flat_map(|g| g.removable.iter())
            .map(|r| r.size)
            .sum();
        assert_eq!(
            total_reclaimable, 0,
            "all-anchor group must have 0 reclaimable"
        );
    }

    #[test]
    fn lone_hard_link_set_is_surfaced_as_already_deduplicated() {
        // TASK-0316: one inode with ≥2 scanned links and nothing else matching is
        // still an all-anchor case under ADR-0008 edge case 1 — a single inode
        // trivially satisfies "every distinct inode is multiply-linked in-scope".
        // It was previously dropped before reaching that branch, so the user saw
        // "No duplicate groups found." for a tree the guide documents as
        // already-deduplicated.
        let dir = tempfile::tempdir().unwrap();
        let a = write(&dir, "beach.jpg", b"LONESET");
        let a2 = dir.path().join("beach-hardlink.jpg");
        fs::hard_link(&a, &a2).expect("hard link creatable");

        let out = find_duplicates(vec![cf(&a), cf(&a2)], lex_order);

        assert!(
            out.groups.is_empty(),
            "a lone hard-link set has nothing separate to remove; got {:?}",
            out.groups
        );
        assert_eq!(
            out.hard_link_notes.len(),
            1,
            "the lone set must surface exactly one hard-link note; got {:?}",
            out.hard_link_notes
        );
        let note = &out.hard_link_notes[0];
        assert_eq!(
            note.representative, a,
            "representative is the shortest path (beach.jpg beats beach-hardlink.jpg)"
        );
        assert_eq!(note.siblings, vec![a2], "the other link is the sibling");
    }

    #[test]
    fn lone_file_without_hard_links_reports_nothing() {
        // Companion guard for the fix above: dropping the reps < 2 early return
        // must not start reporting an ordinary single file. Only a set with a
        // real sibling is already-deduplicated.
        let dir = tempfile::tempdir().unwrap();
        let a = write(&dir, "solo.jpg", b"SOLO");

        let out = find_duplicates(vec![cf(&a)], lex_order);

        assert!(out.groups.is_empty(), "a single file forms no group");
        assert!(
            out.hard_link_notes.is_empty(),
            "a file with no hard links is not an already-deduplicated set; got {:?}",
            out.hard_link_notes
        );
    }

    #[test]
    fn non_anchor_single_link_is_ordinary_removal_candidate() {
        // Regression guard: a representative with NO surviving sibling (the only
        // scanned link to its inode) is NOT an anchor and must remain an ordinary
        // removal candidate. The anchor rule must not bar single-link reps.
        //
        // a.jpg and b.jpg are distinct inodes (no hard links). Both are byte-identical.
        // Neither is an anchor. lex_order → keeper = a, removable = b.
        let dir = tempfile::tempdir().unwrap();
        let a = write(&dir, "a.jpg", b"ORDINARY");
        let b = write(&dir, "b.jpg", b"ORDINARY");

        let out = find_duplicates(vec![cf(&a), cf(&b)], lex_order);

        assert_eq!(out.groups.len(), 1, "ordinary pair must still group");
        let g = &out.groups[0];
        assert_eq!(g.keeper.path, a, "a must be keeper (lex min)");
        assert_eq!(g.removable.len(), 1, "b must be removable");
        assert_eq!(g.removable[0].path, b);
        assert!(
            g.protected.is_empty(),
            "no anchors when neither rep has siblings; got {:?}",
            g.protected
        );
    }

    #[test]
    fn anchor_group_determinism_across_input_orders_and_thread_widths() {
        // NFR-3 / ADR-0008 anchor rule: an anchor-containing group must produce
        // byte-identical output across shuffled input order and thread-pool widths.
        use rayon::ThreadPoolBuilder;

        let dir = tempfile::tempdir().unwrap();
        // hard-link set: h ↔ h2 (anchor); two standalones: c1, c2.
        let h = write(&dir, "h.jpg", b"ANCHOR_CONTENT");
        let h2 = dir.path().join("h2.jpg");
        fs::hard_link(&h, &h2).expect("hard link creatable");
        let c1 = write(&dir, "c1.jpg", b"ANCHOR_CONTENT");
        let c2 = write(&dir, "c2.jpg", b"ANCHOR_CONTENT");

        let files_order1 = vec![cf(&h), cf(&h2), cf(&c1), cf(&c2)];
        let files_order2 = vec![cf(&c2), cf(&c1), cf(&h2), cf(&h)];

        // Same order, 1 vs 4 threads.
        let out_single = ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap()
            .install(|| find_duplicates(files_order1.clone(), lex_order));
        let out_multi = ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap()
            .install(|| find_duplicates(files_order1.clone(), lex_order));

        assert_eq!(
            out_single, out_multi,
            "anchor group must be identical at 1 vs 4 threads"
        );

        // Reversed input order.
        let out_reversed = find_duplicates(files_order2, lex_order);
        assert_eq!(
            out_single, out_reversed,
            "anchor group must be identical across input orders"
        );
    }

    #[test]
    #[cfg(unix)]
    fn anchor_wins_keep_shortest_removes_standalones() {
        // Scenario: beach.jpg ↔ beach-hardlink.jpg (anchor) + beach-copy.jpg (standalone)
        // Files are byte-identical. With --keep shortest, beach.jpg (shortest path) is an anchor.
        // Expected: keeper = beach.jpg (anchor), removable = [beach-copy.jpg], protected = [beach-hardlink.jpg]

        let dir = tempfile::tempdir().unwrap();

        // beach.jpg (anchor, shortest path)
        let beach = write(&dir, "beach.jpg", b"identical content");

        // beach-hardlink.jpg (same inode as beach.jpg, hard-link)
        let hardlink = dir.path().join("beach-hardlink.jpg");
        #[cfg(unix)]
        fs::hard_link(&beach, &hardlink).unwrap();
        #[cfg(windows)]
        {
            // Windows doesn't reliably support hard_link in std, use copy for this test
            fs::copy(&beach, &hardlink).unwrap();
        }

        // beach-copy.jpg (separate inode, standalone, longer path)
        let copy = write(&dir, "beach-copy.jpg", b"identical content");

        // Get ConfirmedFile entries for all three files
        let files = vec![cf(&beach), cf(&hardlink), cf(&copy)];

        // Run dedup with --keep shortest
        let shortest_order = crate::keeper::build_comparator(crate::cli::KeepRule::Shortest);
        let outcome = find_duplicates(files, shortest_order);

        // Assertions
        assert_eq!(outcome.groups.len(), 1, "Should have 1 duplicate group");
        let group = &outcome.groups[0];

        // Keeper should be beach.jpg (shortest, anchor)
        assert_eq!(
            group.keeper.path.file_name().unwrap(),
            "beach.jpg",
            "Keeper must be beach.jpg (shortest path, anchor)"
        );

        // Removable should be [beach-copy.jpg]
        assert_eq!(
            group.removable.len(),
            1,
            "Should have 1 removable file (beach-copy.jpg)"
        );
        assert_eq!(
            group.removable[0].path.file_name().unwrap(),
            "beach-copy.jpg",
            "Removable must be beach-copy.jpg"
        );

        // Protected should be [beach-hardlink.jpg]
        assert_eq!(
            group.protected.len(),
            1,
            "Should have 1 protected anchor (beach-hardlink.jpg)"
        );
        assert_eq!(
            group.protected[0].path.file_name().unwrap(),
            "beach-hardlink.jpg",
            "Protected must be beach-hardlink.jpg (anchor)"
        );

        // Reclaimable should equal beach-copy.jpg's size only (anchor size excluded)
        let reclaimable: u64 = group.removable.iter().map(|r| r.size).sum();
        let copy_size = fs::metadata(&copy).unwrap().len();
        assert_eq!(
            reclaimable, copy_size,
            "Reclaimable must equal beach-copy.jpg size (anchor excluded)"
        );
    }

    #[test]
    #[cfg(unix)]
    fn standalone_wins_keep_shortest_when_shorter_than_anchors() {
        // Scenario: short.jpg (standalone, shortest) + longer.jpg ↔ longer-link.jpg (anchor, longer path)
        // All byte-identical. With --keep shortest, short.jpg (shortest) is a standalone.
        // Expected: keeper = short.jpg (standalone), removable = [], protected = [longer.jpg, longer-link.jpg]

        let dir = tempfile::tempdir().unwrap();

        // short.jpg (standalone, shortest)
        let short = write(&dir, "short.jpg", b"identical content");

        // longer.jpg and longer-link.jpg (hard-linked, longer path)
        let longer = write(&dir, "longer.jpg", b"identical content");
        let longer_link = dir.path().join("longer-link.jpg");
        #[cfg(unix)]
        fs::hard_link(&longer, &longer_link).unwrap();
        #[cfg(windows)]
        fs::copy(&longer, &longer_link).unwrap();

        // Get ConfirmedFile entries for all three files
        let files = vec![cf(&short), cf(&longer), cf(&longer_link)];

        // Run dedup with --keep shortest
        let shortest_order = crate::keeper::build_comparator(crate::cli::KeepRule::Shortest);
        let outcome = find_duplicates(files, shortest_order);

        // Assertions
        assert_eq!(outcome.groups.len(), 1, "Should have 1 duplicate group");
        let group = &outcome.groups[0];

        // Keeper should be short.jpg (shortest, standalone)
        assert_eq!(
            group.keeper.path.file_name().unwrap(),
            "short.jpg",
            "Keeper must be short.jpg (shortest path, standalone)"
        );

        // Removable should be empty (short.jpg is the only standalone and it's the keeper)
        assert_eq!(
            group.removable.len(),
            0,
            "Should have 0 removable files (short.jpg is keeper, anchor is protected)"
        );

        // Protected should be [longer.jpg, longer-link.jpg]
        assert_eq!(
            group.protected.len(),
            2,
            "Should have 2 protected files (anchor and its hard-link sibling)"
        );

        // Check that both longer files are protected
        let protected_names: std::collections::HashSet<_> = group
            .protected
            .iter()
            .filter_map(|r| r.path.file_name().and_then(|n| n.to_str()))
            .collect();
        assert!(
            protected_names.contains("longer.jpg"),
            "longer.jpg must be protected"
        );
        assert!(
            protected_names.contains("longer-link.jpg"),
            "longer-link.jpg must be protected"
        );

        // Reclaimable should be 0
        let reclaimable: u64 = group.removable.iter().map(|r| r.size).sum();
        assert_eq!(
            reclaimable, 0,
            "No standalones to remove, so reclaimable is 0"
        );
    }

    // ── FR-19 / TASK-0315: progress sink plumbing ──────────────────────────────

    /// A `ProgressSink` that records what it was told, attributing each
    /// `advance` call to whichever phase was active at the time (mirroring
    /// how the real `Progress<W>` interprets the same call sequence) — used
    /// only to prove the engine notifies the sink correctly and with the
    /// right per-phase accounting. Rendering itself is unit-tested in
    /// `progress.rs`.
    struct RecordingSink {
        total: std::sync::atomic::AtomicU64,
        /// Number of times `set_total` was called — the engine must call it
        /// exactly once, itself (finding 11), never relying on a caller to
        /// have pre-set it.
        set_total_calls: std::sync::atomic::AtomicU64,
        /// The denominator passed to the most recent `retotal` call (the
        /// verify phase's own workload — see `dedup_core_with_sink`).
        verify_total: std::sync::atomic::AtomicU64,
        /// Number of times `retotal` was called — must be zero when the
        /// verify phase is skipped (finding 2).
        retotal_calls: std::sync::atomic::AtomicU64,
        hashing_advanced: std::sync::atomic::AtomicU64,
        /// Number of *separate* `advance` calls made while `Hashing` was the
        /// active phase — used to distinguish incremental progress from one
        /// bulk jump (the defect this module's tests guard against).
        hashing_advance_calls: std::sync::atomic::AtomicU64,
        verifying_advanced: std::sync::atomic::AtomicU64,
        phase: std::sync::Mutex<crate::progress::Phase>,
        phases: std::sync::Mutex<Vec<crate::progress::Phase>>,
    }

    impl Default for RecordingSink {
        fn default() -> Self {
            Self {
                total: std::sync::atomic::AtomicU64::new(0),
                set_total_calls: std::sync::atomic::AtomicU64::new(0),
                verify_total: std::sync::atomic::AtomicU64::new(0),
                retotal_calls: std::sync::atomic::AtomicU64::new(0),
                hashing_advanced: std::sync::atomic::AtomicU64::new(0),
                hashing_advance_calls: std::sync::atomic::AtomicU64::new(0),
                verifying_advanced: std::sync::atomic::AtomicU64::new(0),
                phase: std::sync::Mutex::new(crate::progress::Phase::Scanning),
                phases: std::sync::Mutex::new(Vec::new()),
            }
        }
    }

    impl crate::progress::ProgressSink for RecordingSink {
        fn set_total(&self, total: u64) {
            self.total.store(total, std::sync::atomic::Ordering::SeqCst);
            self.set_total_calls
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
        fn set_phase(&self, phase: crate::progress::Phase) {
            *self.phase.lock().unwrap() = phase;
            self.phases.lock().unwrap().push(phase);
        }
        fn advance(&self, n: u64) {
            match *self.phase.lock().unwrap() {
                crate::progress::Phase::Hashing => {
                    self.hashing_advanced
                        .fetch_add(n, std::sync::atomic::Ordering::SeqCst);
                    self.hashing_advance_calls
                        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
                crate::progress::Phase::Verifying => {
                    self.verifying_advanced
                        .fetch_add(n, std::sync::atomic::Ordering::SeqCst);
                }
                crate::progress::Phase::Scanning | crate::progress::Phase::Done => {}
            }
        }
        fn retotal(&self, total: u64) {
            self.verify_total
                .store(total, std::sync::atomic::Ordering::SeqCst);
            self.retotal_calls
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
        fn finish(&self) {}
    }

    #[test]
    fn find_duplicates_with_progress_advances_telescope_per_phase() {
        // FR-19 per-phase counting invariant: hashing-phase advances sum to
        // the full confirmed-file count (y), across a mix of
        // size-singletons, hash-unique files, and a genuine duplicate pair.
        // Verifying-phase advances sum to exactly the files that entered
        // byte-verification — its own, smaller denominator (fixed via
        // `retotal`), never `y`.
        let dir = tempfile::tempdir().unwrap();
        let solo = write(&dir, "solo.jpg", b"unique-size-payload"); // size-singleton
        let u1 = write(&dir, "u1.jpg", b"AAAAAAAAAA"); // same size as u2, different content
        let u2 = write(&dir, "u2.jpg", b"BBBBBBBBBB"); // hash-unique within its cohort
        let d1 = write(&dir, "d1.jpg", b"DUPLICATE!"); // true duplicate pair
        let d2 = write(&dir, "d2.jpg", b"DUPLICATE!");

        let files = vec![cf(&solo), cf(&u1), cf(&u2), cf(&d1), cf(&d2)];
        let total = files.len() as u64;

        let sink = RecordingSink::default();
        let shortest_order = crate::keeper::build_comparator(crate::cli::KeepRule::Shortest);
        let outcome = find_duplicates_with_progress(files, shortest_order, &sink);

        assert_eq!(
            outcome.groups.len(),
            1,
            "exactly one genuine duplicate group expected"
        );
        assert_eq!(
            sink.hashing_advanced
                .load(std::sync::atomic::Ordering::SeqCst),
            total,
            "hashing-phase advances must sum to the full confirmed-file count (y)"
        );
        assert_eq!(
            sink.verifying_advanced
                .load(std::sync::atomic::Ordering::SeqCst),
            2,
            "verifying-phase advances must sum to exactly the files that \
             entered byte-verification (here: the d1/d2 duplicate pair)"
        );
        assert_eq!(
            sink.verify_total.load(std::sync::atomic::Ordering::SeqCst),
            2,
            "retotal must fix the verify phase's own denominator, not y"
        );
    }

    #[test]
    fn find_duplicates_with_progress_sets_the_run_total_itself() {
        // Architect finding 11: the engine, not the caller, is the single
        // source of truth for the run-wide denominator — it must call
        // `set_total` itself, exactly once, with `files.len()`, so it can
        // never desync from a value some caller computed independently.
        let dir = tempfile::tempdir().unwrap();
        let a = write(&dir, "a.jpg", b"same-content");
        let b = write(&dir, "b.jpg", b"same-content");
        let c = write(&dir, "c.jpg", b"solo-size-content!!");
        let files = vec![cf(&a), cf(&b), cf(&c)];
        let expected_total = files.len() as u64;

        let sink = RecordingSink::default();
        let _ = find_duplicates_with_progress(files, lex_order, &sink);

        assert_eq!(
            sink.set_total_calls
                .load(std::sync::atomic::Ordering::SeqCst),
            1,
            "the engine must call set_total exactly once"
        );
        assert_eq!(
            sink.total.load(std::sync::atomic::Ordering::SeqCst),
            expected_total,
            "the engine must set the total to files.len()"
        );
    }

    #[test]
    fn find_duplicates_with_progress_hashing_advances_incrementally() {
        // Regression (coordinator-reported defect): hashing must animate as
        // files complete, not sit at 0% for the entire hashing pass and then
        // jump once in bulk at the end. Over a tree of unique files (no
        // duplicates, so nothing is deferred to the verify pass), the sink
        // must observe more than one `advance` call while `Hashing` is
        // active.
        let dir = tempfile::tempdir().unwrap();
        let mut files = Vec::new();
        for i in 0..8 {
            let name = format!("f{i}.jpg");
            let p = write(&dir, &name, format!("unique-content-{i}").as_bytes());
            files.push(cf(&p));
        }

        let sink = RecordingSink::default();
        let _ = find_duplicates_with_progress(files, lex_order, &sink);

        let calls = sink
            .hashing_advance_calls
            .load(std::sync::atomic::Ordering::SeqCst);
        assert!(
            calls > 1,
            "expected more than one advance() call during hashing \
             (incremental progress), got {calls}"
        );
    }

    #[test]
    fn find_duplicates_with_progress_sets_hashing_then_verifying() {
        // The engine owns the Hashing → Verifying phase transitions (main.rs
        // owns Scanning, before candidates are even known); exactly one of
        // each, in order, regardless of how many size cohorts / hash groups
        // exist (bounded phase toggling, FR-19).
        let dir = tempfile::tempdir().unwrap();
        let a = write(&dir, "a.jpg", b"same-content");
        let b = write(&dir, "b.jpg", b"same-content");
        let c = write(&dir, "c.jpg", b"different-content!!");
        let d = write(&dir, "d.jpg", b"different-content!!");

        let files = vec![cf(&a), cf(&b), cf(&c), cf(&d)];
        let sink = RecordingSink::default();
        let lex = lex_order;
        let _ = find_duplicates_with_progress(files, lex, &sink);

        assert_eq!(
            *sink.phases.lock().unwrap(),
            vec![
                crate::progress::Phase::Hashing,
                crate::progress::Phase::Verifying
            ],
            "expected exactly one Hashing→Verifying transition"
        );
    }

    #[test]
    fn find_duplicates_with_progress_skips_verifying_when_workload_is_zero() {
        // Architect finding 2: when nothing needs byte-verification (every
        // file is either a size-singleton or hash-unique within its
        // cohort — no true duplicates), the engine must skip the Verifying
        // phase entirely: no `retotal` call, no `set_phase(Verifying)`.
        // Showing a degenerate "0 of 0 (100%)" would otherwise read as a
        // second, pointless completion state.
        let dir = tempfile::tempdir().unwrap();
        let solo = write(&dir, "solo.jpg", b"unique-size-payload");
        let u1 = write(&dir, "u1.jpg", b"AAAAAAAAAA");
        let u2 = write(&dir, "u2.jpg", b"BBBBBBBBBB");
        let files = vec![cf(&solo), cf(&u1), cf(&u2)];

        let sink = RecordingSink::default();
        let outcome = find_duplicates_with_progress(files, lex_order, &sink);

        assert!(
            outcome.groups.is_empty(),
            "fixture must contain no genuine duplicates"
        );
        assert_eq!(
            *sink.phases.lock().unwrap(),
            vec![crate::progress::Phase::Hashing],
            "Verifying must never be entered when its workload is zero"
        );
        assert_eq!(
            sink.retotal_calls.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "retotal must never be called when the verify workload is zero"
        );
    }

    #[test]
    fn find_duplicates_with_progress_skips_hashing_when_candidate_set_is_empty() {
        // Architect finding N1: the same "skip a zero-workload phase
        // entirely, don't show a degenerate 0 of 0" rule finding 2
        // established for Verifying applies to Hashing too. An empty
        // candidate set has nothing to hash, so the engine must never call
        // `set_phase(Hashing)` — the run-wide total is still set (to 0), so
        // `ProgressSink::finish` can still render its terminal `done: 0 of 0
        // (100%)` line, but no phase is ever entered by the engine itself.
        let sink = RecordingSink::default();
        let outcome = find_duplicates_with_progress(Vec::new(), lex_order, &sink);

        assert!(outcome.groups.is_empty());
        assert!(
            sink.phases.lock().unwrap().is_empty(),
            "no phase should ever be entered for an empty candidate set, got: {:?}",
            sink.phases.lock().unwrap()
        );
        assert_eq!(
            sink.set_total_calls
                .load(std::sync::atomic::Ordering::SeqCst),
            1,
            "set_total must still be called (with 0) so finish() knows the \
             run-wide total"
        );
        assert_eq!(sink.total.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn find_duplicates_with_progress_output_is_identical_with_real_sink_and_no_progress() {
        // AC-24: progress is output-only. A prior version of this test
        // compared `find_duplicates` (routed through `NoProgress`) against
        // `find_duplicates_with_progress` with a bespoke, equally-inert mock
        // sink — both paths converge on the same internal function with two
        // sink implementations that never influence control flow, so it
        // could not have caught a bug where the *real* renderer's locking
        // or timing perturbed results (architect finding 6). This version
        // instead runs the identical input through the sole production
        // entry point twice: once with a real `Progress<Vec<u8>>` renderer
        // (actual mutex locking, actual repaint decisions) and once with
        // `NoProgress`, and asserts the outcomes are equal.
        let dir = tempfile::tempdir().unwrap();
        let a = write(&dir, "a.jpg", b"same-content");
        let b = write(&dir, "b.jpg", b"same-content");
        let c = write(&dir, "c.jpg", b"solo-size-content!!");
        let files = vec![cf(&a), cf(&b), cf(&c)];

        let real_sink = crate::progress::Progress::new(Vec::new(), false);
        let with_real_sink = find_duplicates_with_progress(
            files.clone(),
            crate::keeper::build_comparator(crate::cli::KeepRule::Shortest),
            &real_sink,
        );

        let with_no_progress = find_duplicates_with_progress(
            files,
            crate::keeper::build_comparator(crate::cli::KeepRule::Shortest),
            &crate::progress::NoProgress,
        );

        assert_eq!(
            with_real_sink, with_no_progress,
            "progress must not change the dedup outcome, even with a real, \
             locking renderer in the loop"
        );
    }
}
