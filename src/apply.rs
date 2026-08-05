//! Apply stage — remove non-keepers, safely (FR-10, FR-16, NFR-1).
//!
//! This is the first stage that mutates the filesystem, so it is built around
//! one rule: **a removal only ever happens for a file that has just been
//! re-proved removable**. The scan-time byte verify (TASK-0304) is not enough —
//! the tree can change between the scan and the apply — so every candidate is
//! re-validated immediately before it is removed. That check is a TOCTOU
//! defense, not a repeat of the scan (ADR-0008).
//!
//! The removal mechanism is injected via [`Remover`] so that TASK-0310 can add
//! permanent deletion by swapping the implementation rather than duplicating
//! this logic, and so the safety rules can be unit-tested without touching a
//! real trash can.
//!
//! **Removal is by path, and that has two consequences.** It is the only
//! interface the OS trash offers, so a single unavoidable gap remains between
//! the last check and the removal itself (see `revalidate`). It also means the
//! trash path is the looser of the two: `fs::remove_file` refuses a directory,
//! while the OS trash would happily accept one if a directory were swapped in
//! during that gap.
//!
//! **Re-validation re-reads the keeper once per candidate.** A group of ten
//! duplicates of a 50 MB file therefore reads about a gigabyte at apply time.
//! That is a deliberate trade: byte-exactness on the irreversible path is worth
//! more than the I/O, and hashing instead would reintroduce the
//! trust-the-hash-alone problem FR-4 exists to avoid.
//!
//! **A failure never escalates.** If a removal fails, the file is reported and
//! skipped and the run continues; the tool never falls back from trashing to
//! permanent deletion (ADR-0009). Permanent removal happens only when the user
//! explicitly asks for it.

use std::fmt;
use std::fs;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use crate::engine::{DuplicateGroup, files_equal_handles};

/// How a file is removed. Injected so the apply logic is shared between
/// `--apply` (trash) and `--apply --delete` (permanent), and mockable in tests.
pub trait Remover {
    /// Remove `path`. Returning `Err` must leave the file in place.
    fn remove(&self, path: &Path) -> io::Result<()>;
}

/// Moves files to the OS trash / recycle bin (ADR-0009, FR-10).
///
/// Holds a configured [`trash::TrashContext`] rather than calling the crate's
/// free function, because the macOS default is not usable for a CLI — see
/// [`TrashRemover::new`].
pub struct TrashRemover {
    ctx: trash::TrashContext,
}

impl TrashRemover {
    pub fn new() -> Self {
        #[allow(unused_mut)]
        let mut ctx = trash::TrashContext::default();

        // macOS: the crate defaults to `DeleteMethod::Finder`, which runs
        // `osascript` to ask Finder to do the delete. That requires macOS
        // Automation permission, and when the prompt cannot be answered — any
        // non-interactive shell, an SSH session, CI — the call **blocks
        // indefinitely** rather than failing. A CLI that hangs forever on
        // `--apply` is worse than one that reports an error, and ADR-0009's
        // fail-safe design assumes a trash failure *returns*.
        //
        // `NsFileManager` calls `trashItemAtURL` directly, needs no extra
        // permission, and is faster. The documented trade-off is that "Put
        // Back" may not appear in Finder's context menu on some systems (a
        // macOS bug, not a crate one); the file is still in the Trash and
        // still recoverable by hand, which is what FR-10 requires.
        #[cfg(target_os = "macos")]
        {
            use trash::macos::{DeleteMethod, TrashContextExtMacos};
            ctx.set_delete_method(DeleteMethod::NsFileManager);
        }

        Self { ctx }
    }
}

impl Default for TrashRemover {
    fn default() -> Self {
        Self::new()
    }
}

impl Remover for TrashRemover {
    fn remove(&self, path: &Path) -> io::Result<()> {
        // The trash crate's error type carries platform detail we surface
        // verbatim in the skip reason; there is deliberately no fallback to
        // std::fs::remove_file here (NFR-1, ADR-0009).
        self.ctx
            .delete(path)
            .map_err(|e| io::Error::other(e.to_string()))
    }
}

/// Permanently removes files, with no trash indirection (FR-11, ADR-0009).
///
/// Reached only via `--apply --delete`; the CLI layer rejects `--delete` on its
/// own before any filesystem call is made. Nothing else in the crate may
/// construct this — a trash failure must never be retried as a deletion
/// (NFR-1), which is why the escalation lives nowhere in the executor.
pub struct DeleteRemover;

impl Remover for DeleteRemover {
    fn remove(&self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }
}

/// Why a candidate was not removed. Every variant means the file is still on
/// disk, untouched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    /// The candidate no longer exists (already removed, or removed by someone
    /// else since the scan). Not an error — this is what makes a re-run a
    /// no-op (FR-12).
    Vanished,
    /// The candidate is no longer a regular file — most importantly, it may
    /// have been swapped for a symlink since the scan (ADR-0008).
    NotARegularFile,
    /// The candidate is a symlink. Reachable when the scan ran with
    /// `--follow-symlinks`, which enumerates the link path itself: the tool
    /// never unlinks a symlink standing in for a scanned file, because the user
    /// asked about the target, not the link.
    IsASymlink,
    /// The file being kept is itself a symlink (reachable under
    /// `--follow-symlinks`). Its target is a real file somewhere, but this
    /// candidate was matched against a link, so nothing is removed for it.
    KeeperIsASymlink,
    /// The engine handed the keeper's own path back as a removal candidate.
    /// Impossible by construction, so this means an invariant broke — recorded
    /// rather than silently skipped so it is visible in the output.
    KeeperInRemovableSet,
    /// The keeper this candidate was verified against is gone or is no longer a
    /// regular file. Removing the candidate could destroy the last copy.
    KeeperUnusable,
    /// The candidate is no longer byte-for-byte identical to its keeper.
    ContentChanged,
    /// The candidate and the keeper are now the same physical file (a hard link
    /// created since the scan). Removing it would delete one of the keeper's
    /// own names (FR-17).
    SameFileAsKeeper,
    /// Re-validation itself could not be completed (an I/O error while
    /// comparing). Fail closed: no removal.
    Unverifiable(String),
    /// Re-validation passed but the removal failed. The file is still there.
    RemovalFailed(String),
}

impl fmt::Display for SkipReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkipReason::Vanished => write!(f, "no longer exists"),
            SkipReason::NotARegularFile => write!(f, "no longer a regular file"),
            SkipReason::IsASymlink => write!(
                f,
                "is a symbolic link; symlinks are never removed, only their targets are compared"
            ),
            SkipReason::KeeperIsASymlink => write!(
                f,
                "the file being kept is a symbolic link; symlinks are never removed"
            ),
            SkipReason::KeeperInRemovableSet => write!(
                f,
                "internal invariant violated: the file to keep was also listed for removal"
            ),
            SkipReason::KeeperUnusable => write!(f, "the file to keep is missing or unusable"),
            SkipReason::ContentChanged => write!(f, "content changed since the scan"),
            SkipReason::SameFileAsKeeper => {
                write!(f, "now hard-linked to the file being kept")
            }
            SkipReason::Unverifiable(e) => write!(f, "could not be re-verified: {e}"),
            SkipReason::RemovalFailed(e) => write!(f, "removal failed: {e}"),
        }
    }
}

/// A candidate that was left in place, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkipRecord {
    pub path: PathBuf,
    pub reason: SkipReason,
}

/// What the run did to the files — the `mode` the output reports (FR-14).
///
/// Kept separate from the CLI flags so the formatters describe what *happened*
/// rather than re-deriving it from what was asked for: a run that refuses to
/// act must not report itself as having acted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionMode {
    /// Nothing was touched (the default).
    DryRun,
    /// Non-keepers were moved to the OS trash (`--apply`).
    Trash,
    /// Non-keepers were permanently removed (`--apply --delete`).
    Delete,
}

impl ActionMode {
    /// The stable string used in the JSON `mode` field.
    pub fn as_str(self) -> &'static str {
        match self {
            ActionMode::DryRun => "dry-run",
            ActionMode::Trash => "trash",
            ActionMode::Delete => "delete",
        }
    }

    /// Past-tense verb for the human report, e.g. "trashed".
    pub fn past_tense(self) -> &'static str {
        match self {
            // Unreachable: the report suppresses the whole section in a dry
            // run, and a dry run never produces removals to describe.
            ActionMode::DryRun => "would have removed",
            ActionMode::Trash => "trashed",
            ActionMode::Delete => "deleted",
        }
    }
}

/// What the apply stage did. Every candidate lands in exactly one of the two
/// lists, so `removed.len() + skipped.len()` is the number of files considered.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ApplyOutcome {
    pub removed: Vec<PathBuf>,
    pub skipped: Vec<SkipRecord>,
}

/// Re-validate a candidate immediately before removing it (FR-16, AC-17).
///
/// Returns `Ok(())` only when removing `candidate` is provably safe: it still
/// exists as a regular file, the keeper still exists as a regular file, the two
/// are still byte-for-byte identical, and they are not the same physical file.
/// Anything else — including an I/O error during the check — is a skip.
fn revalidate(keeper: &Path, candidate: &Path) -> Result<(), SkipReason> {
    // Every check below is driven from handles this function opened, not from
    // repeated path lookups. A path can be re-pointed between two lookups, so
    // checking metadata by path and then comparing bytes by path would prove
    // two things about possibly-different files. Opening once and asking the
    // open file its metadata and identity keeps all four checks consistent.
    //
    // A residual window remains and cannot be closed: between the last check
    // and the `remove` call — which is by path, because that is the only
    // interface the OS trash offers — the path can still be re-pointed. What
    // this narrows it to is that single gap, rather than spanning a whole
    // byte-for-byte comparison of a possibly multi-gigabyte file.

    // symlink_metadata, not metadata: a symlink swapped in since the scan must
    // be detected, not followed (ADR-0008). This is also the only check that
    // must be by path — an open handle has already resolved the link.
    let cand_link_meta = match fs::symlink_metadata(candidate) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(SkipReason::Vanished),
        Err(e) => return Err(SkipReason::Unverifiable(e.to_string())),
    };
    if cand_link_meta.is_symlink() {
        // Reachable under --follow-symlinks, where enumeration yields the link
        // path itself. Removing it would unlink the link, not the duplicate the
        // user was shown.
        return Err(SkipReason::IsASymlink);
    }
    if !cand_link_meta.is_file() {
        return Err(SkipReason::NotARegularFile);
    }

    let keeper_link_meta = match fs::symlink_metadata(keeper) {
        Ok(m) => m,
        Err(_) => return Err(SkipReason::KeeperUnusable),
    };
    if keeper_link_meta.is_symlink() {
        return Err(SkipReason::KeeperIsASymlink);
    }
    if !keeper_link_meta.is_file() {
        return Err(SkipReason::KeeperUnusable);
    }

    let mut cand_file = match File::open(candidate) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(SkipReason::Vanished),
        Err(e) => return Err(SkipReason::Unverifiable(e.to_string())),
    };
    let mut keeper_file = match File::open(keeper) {
        Ok(f) => f,
        Err(_) => return Err(SkipReason::KeeperUnusable),
    };

    // Identity from the open handles: a hard link created between scan and
    // apply would pass the byte comparison while meaning the candidate *is* the
    // keeper. The engine never proposes hard-linked siblings for removal
    // (FR-17), and that promise has to hold at apply time too — this is also
    // what makes overlapping or repeated scan roots safe.
    match (
        same_file::Handle::from_file(keeper_file.try_clone().map_err(unverifiable)?),
        same_file::Handle::from_file(cand_file.try_clone().map_err(unverifiable)?),
    ) {
        (Ok(k), Ok(c)) if k == c => return Err(SkipReason::SameFileAsKeeper),
        (Err(e), _) | (_, Err(e)) => return Err(SkipReason::Unverifiable(e.to_string())),
        _ => {}
    }

    match files_equal_handles(&mut keeper_file, &mut cand_file) {
        Ok(true) => Ok(()),
        Ok(false) => Err(SkipReason::ContentChanged),
        Err(e) => Err(SkipReason::Unverifiable(e.to_string())),
    }
}

fn unverifiable(e: io::Error) -> SkipReason {
    SkipReason::Unverifiable(e.to_string())
}

/// What the executor did to one candidate, reported the moment it happens.
///
/// Streamed rather than only returned because a process killed partway through
/// an irreversible run would otherwise leave the user with no record of what
/// was destroyed — the final report never gets printed.
pub enum Event<'a> {
    Removed(&'a Path),
    Skipped(&'a Path, &'a SkipReason),
}

/// Remove the non-keepers of every group, re-validating each one first.
///
/// The keeper is never passed to the remover, so a group can never be emptied
/// (NFR-1). Groups are walked in the canonical order the engine produced, so an
/// apply visits files in the same order the dry run reported them.
///
/// `mode` is only used to record the `apply` tracing span's `mode` field
/// (ADR-0017, TASK-0338) — it never changes behavior here; the actual
/// mechanism is entirely determined by `remover`. Callers always pass the
/// same `mode`/`remover` pair they already compute together (never
/// [`ActionMode::DryRun`], which never reaches this function at all).
pub fn apply(
    groups: &[DuplicateGroup],
    mode: ActionMode,
    remover: &dyn Remover,
    on_event: &mut dyn FnMut(Event<'_>),
) -> ApplyOutcome {
    // ADR-0017 (TASK-0338): this stage's own span. `mode` and `groups` are
    // known up front; `removed`/`skipped` are only known once every group
    // has been walked, so they are `Empty` and recorded just before
    // returning. `mode` is a fixed, small enum string (never a path), and
    // `mode.as_str()` is recorded via ordinary string `Value`, not `%`
    // (Display) formatting of anything user-controlled (ADR-0017 §6).
    let apply_span = tracing::info_span!(
        "apply",
        mode = mode.as_str(),
        groups = groups.len(),
        removed = tracing::field::Empty,
        skipped = tracing::field::Empty,
    );
    let _apply_guard = apply_span.enter();

    let mut outcome = ApplyOutcome::default();

    for group in groups {
        for candidate in &group.removable {
            // Defense in depth: the engine guarantees a keeper is never in
            // `removable`, but this is the last code before an irreversible
            // action, so it does not take that on trust.
            if candidate.path == group.keeper.path {
                // Impossible by construction. Recorded rather than skipped
                // silently — and deliberately *not* a panic: aborting here
                // would leave a half-applied tree with no report at all, which
                // is the worst outcome on an irreversible path.
                on_event(Event::Skipped(
                    &candidate.path,
                    &SkipReason::KeeperInRemovableSet,
                ));
                outcome.skipped.push(SkipRecord {
                    path: candidate.path.clone(),
                    reason: SkipReason::KeeperInRemovableSet,
                });
                continue;
            }

            if let Err(reason) = revalidate(&group.keeper.path, &candidate.path) {
                on_event(Event::Skipped(&candidate.path, &reason));
                outcome.skipped.push(SkipRecord {
                    path: candidate.path.clone(),
                    reason,
                });
                continue;
            }

            match remover.remove(&candidate.path) {
                Ok(()) => {
                    on_event(Event::Removed(&candidate.path));
                    outcome.removed.push(candidate.path.clone());
                }
                Err(e) => {
                    let reason = SkipReason::RemovalFailed(e.to_string());
                    on_event(Event::Skipped(&candidate.path, &reason));
                    outcome.skipped.push(SkipRecord {
                        path: candidate.path.clone(),
                        reason,
                    });
                }
            }
        }
    }

    apply_span.record("removed", outcome.removed.len());
    apply_span.record("skipped", outcome.skipped.len());

    outcome
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::FileRec;
    use std::cell::RefCell;
    use std::fs::File;
    use std::io::Write;

    /// Records what it was asked to remove instead of touching the filesystem,
    /// so the safety rules can be tested without a real trash can. Paths in
    /// `fail` are refused, leaving the file in place — the trash-unavailable
    /// case ADR-0009 requires us to survive.
    struct MockRemover {
        calls: RefCell<Vec<PathBuf>>,
        fail: Vec<PathBuf>,
    }

    impl MockRemover {
        fn new() -> Self {
            Self {
                calls: RefCell::new(Vec::new()),
                fail: Vec::new(),
            }
        }

        fn failing_on(paths: Vec<PathBuf>) -> Self {
            Self {
                calls: RefCell::new(Vec::new()),
                fail: paths,
            }
        }

        fn calls(&self) -> Vec<PathBuf> {
            self.calls.borrow().clone()
        }
    }

    impl Remover for MockRemover {
        fn remove(&self, path: &Path) -> io::Result<()> {
            self.calls.borrow_mut().push(path.to_path_buf());
            if self.fail.iter().any(|p| p == path) {
                return Err(io::Error::other("mock trash unavailable"));
            }
            fs::remove_file(path)
        }
    }

    fn write(dir: &tempfile::TempDir, name: &str, bytes: &[u8]) -> PathBuf {
        let p = dir.path().join(name);
        let mut f = File::create(&p).expect("create");
        f.write_all(bytes).expect("write");
        p
    }

    fn rec(path: &Path) -> FileRec {
        let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        FileRec::new_for_test(path.to_path_buf(), size, None)
    }

    fn group(keeper: &Path, removable: &[&Path]) -> DuplicateGroup {
        DuplicateGroup {
            keeper: rec(keeper),
            removable: removable.iter().map(|p| rec(p)).collect(),
            protected: Vec::new(),
        }
    }

    #[test]
    fn removes_non_keepers_and_never_touches_the_keeper() {
        // AC-3 / NFR-1: the core promise. The keeper must survive and must
        // never even be offered to the remover.
        let dir = tempfile::tempdir().unwrap();
        let keeper = write(&dir, "keep.jpg", b"DUPE");
        let dup1 = write(&dir, "dup1.jpg", b"DUPE");
        let dup2 = write(&dir, "dup2.jpg", b"DUPE");

        let mock = MockRemover::new();
        let out = apply(
            &[group(&keeper, &[&dup1, &dup2])],
            ActionMode::Trash,
            &mock,
            &mut |_| {},
        );

        assert_eq!(out.removed, vec![dup1.clone(), dup2.clone()]);
        assert!(
            out.skipped.is_empty(),
            "nothing to skip; got {:?}",
            out.skipped
        );
        assert!(keeper.exists(), "the keeper must never be removed");
        assert!(
            !mock.calls().contains(&keeper),
            "the keeper must never be passed to the remover"
        );
    }

    #[test]
    fn skips_a_candidate_that_vanished_since_the_scan() {
        // FR-12 idempotence rests on this: a second apply finds the files gone
        // and does nothing, rather than erroring.
        let dir = tempfile::tempdir().unwrap();
        let keeper = write(&dir, "keep.jpg", b"DUPE");
        let dup = write(&dir, "dup.jpg", b"DUPE");
        let g = group(&keeper, &[&dup]);
        fs::remove_file(&dup).unwrap();

        let mock = MockRemover::new();
        let out = apply(&[g], ActionMode::Trash, &mock, &mut |_| {});

        assert!(out.removed.is_empty());
        assert_eq!(out.skipped.len(), 1);
        assert_eq!(out.skipped[0].reason, SkipReason::Vanished);
        assert!(mock.calls().is_empty(), "a vanished file is never removed");
    }

    #[test]
    fn skips_a_candidate_whose_content_changed_since_the_scan() {
        // AC-17: the whole point of re-validating. The scan proved these equal;
        // by apply time they are not, so the removal must not happen.
        let dir = tempfile::tempdir().unwrap();
        let keeper = write(&dir, "keep.jpg", b"DUPE");
        let dup = write(&dir, "dup.jpg", b"DUPE");
        let g = group(&keeper, &[&dup]);
        write(&dir, "dup.jpg", b"CHANGED!");

        let mock = MockRemover::new();
        let out = apply(&[g], ActionMode::Trash, &mock, &mut |_| {});

        assert!(out.removed.is_empty());
        assert_eq!(out.skipped[0].reason, SkipReason::ContentChanged);
        assert!(dup.exists(), "a changed file must be left alone");
        assert!(mock.calls().is_empty());
    }

    #[test]
    fn skips_when_the_keeper_disappeared_before_the_apply() {
        // Removing the candidate here could destroy the last copy of the
        // content — exactly the outcome NFR-1 forbids.
        let dir = tempfile::tempdir().unwrap();
        let keeper = write(&dir, "keep.jpg", b"DUPE");
        let dup = write(&dir, "dup.jpg", b"DUPE");
        let g = group(&keeper, &[&dup]);
        fs::remove_file(&keeper).unwrap();

        let mock = MockRemover::new();
        let out = apply(&[g], ActionMode::Trash, &mock, &mut |_| {});

        assert_eq!(out.skipped[0].reason, SkipReason::KeeperUnusable);
        assert!(dup.exists(), "the last copy must survive");
        assert!(mock.calls().is_empty());
    }

    #[test]
    fn a_removal_failure_is_reported_and_never_escalates() {
        // ADR-0009: no desktop trash on a headless box is a per-file skip, not
        // a reason to delete permanently. The file must still be there, and the
        // rest of the run must continue.
        let dir = tempfile::tempdir().unwrap();
        let keeper = write(&dir, "keep.jpg", b"DUPE");
        let stubborn = write(&dir, "stubborn.jpg", b"DUPE");
        let ordinary = write(&dir, "ordinary.jpg", b"DUPE");

        let mock = MockRemover::failing_on(vec![stubborn.clone()]);
        let out = apply(
            &[group(&keeper, &[&stubborn, &ordinary])],
            ActionMode::Trash,
            &mock,
            &mut |_| {},
        );

        assert_eq!(out.removed, vec![ordinary.clone()], "the run continues");
        assert_eq!(out.skipped.len(), 1);
        assert!(matches!(
            out.skipped[0].reason,
            SkipReason::RemovalFailed(_)
        ));
        assert!(stubborn.exists(), "a failed removal must leave the file");
        assert_eq!(
            mock.calls().iter().filter(|p| **p == stubborn).count(),
            1,
            "no retry, and above all no second mechanism"
        );
    }

    #[test]
    fn a_second_apply_over_the_same_groups_does_nothing() {
        // AC-5 / FR-12 idempotence, end to end through the executor.
        let dir = tempfile::tempdir().unwrap();
        let keeper = write(&dir, "keep.jpg", b"DUPE");
        let dup = write(&dir, "dup.jpg", b"DUPE");
        let groups = vec![group(&keeper, &[&dup])];

        let first = apply(&groups, ActionMode::Trash, &MockRemover::new(), &mut |_| {});
        assert_eq!(first.removed.len(), 1);

        let mock = MockRemover::new();
        let second = apply(&groups, ActionMode::Trash, &mock, &mut |_| {});

        assert!(second.removed.is_empty(), "nothing left to remove");
        assert_eq!(second.skipped[0].reason, SkipReason::Vanished);
        assert!(mock.calls().is_empty());
        assert!(keeper.exists());
    }

    #[test]
    fn a_group_with_no_removable_files_does_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let keeper = write(&dir, "keep.jpg", b"ALONE");

        let mock = MockRemover::new();
        let out = apply(
            &[group(&keeper, &[])],
            ActionMode::Trash,
            &mock,
            &mut |_| {},
        );

        assert_eq!(out, ApplyOutcome::default());
        assert!(mock.calls().is_empty());
        assert!(keeper.exists());
    }

    #[test]
    fn every_candidate_is_accounted_for_exactly_once() {
        // The reporting layer relies on this: removed + skipped covers every
        // file the run considered, with no double counting.
        let dir = tempfile::tempdir().unwrap();
        let keeper = write(&dir, "keep.jpg", b"DUPE");
        let ok = write(&dir, "ok.jpg", b"DUPE");
        let changed = write(&dir, "changed.jpg", b"DUPE");
        let gone = write(&dir, "gone.jpg", b"DUPE");
        let g = group(&keeper, &[&ok, &changed, &gone]);
        write(&dir, "changed.jpg", b"NOT THE SAME");
        fs::remove_file(&gone).unwrap();

        let out = apply(&[g], ActionMode::Trash, &MockRemover::new(), &mut |_| {});

        assert_eq!(out.removed.len() + out.skipped.len(), 3);
        assert_eq!(out.removed, vec![ok]);
        assert_eq!(out.skipped.len(), 2);
    }

    #[test]
    fn delete_remover_removes_the_file_permanently() {
        // AC-7 / FR-11: the file is gone from the filesystem, not relocated.
        let dir = tempfile::tempdir().unwrap();
        let victim = write(&dir, "victim.jpg", b"GONE");

        DeleteRemover.remove(&victim).expect("delete succeeds");

        assert!(!victim.exists(), "the file must be permanently gone");
        assert_eq!(
            ActionMode::Delete.past_tense(),
            "deleted",
            "the verb the report uses for this mechanism"
        );
    }

    #[test]
    fn the_delete_path_re_validates_exactly_like_the_trash_path() {
        // AC-7 + FR-16: the irreversible path gets the same TOCTOU defense, by
        // construction — it is the same executor with a different Remover.
        // A file changed since the scan must survive even under --delete.
        let dir = tempfile::tempdir().unwrap();
        let keeper = write(&dir, "keep.jpg", b"DUPE");
        let changed = write(&dir, "changed.jpg", b"DUPE");
        let ok = write(&dir, "ok.jpg", b"DUPE");
        let g = group(&keeper, &[&changed, &ok]);
        write(&dir, "changed.jpg", b"TAMPERED");

        let out = apply(&[g], ActionMode::Delete, &DeleteRemover, &mut |_| {});

        assert_eq!(out.removed, vec![ok.clone()], "the valid duplicate goes");
        assert!(!ok.exists(), "and it is really gone");
        assert_eq!(out.skipped[0].reason, SkipReason::ContentChanged);
        assert!(
            changed.exists(),
            "a file changed since the scan must never be deleted"
        );
        assert!(keeper.exists(), "the keeper survives (NFR-1)");
    }

    #[cfg(unix)]
    #[test]
    fn skips_a_candidate_swapped_for_a_symlink() {
        // ADR-0008 calls this out by name: following a symlink planted between
        // scan and apply would trash something that was never scanned.
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let keeper = write(&dir, "keep.jpg", b"DUPE");
        let dup = write(&dir, "dup.jpg", b"DUPE");
        let g = group(&keeper, &[&dup]);

        fs::remove_file(&dup).unwrap();
        symlink(&keeper, &dup).expect("symlink creatable");

        let mock = MockRemover::new();
        let out = apply(&[g], ActionMode::Trash, &mock, &mut |_| {});

        assert_eq!(
            out.skipped[0].reason,
            SkipReason::IsASymlink,
            "the reason must say what is actually true: it is a symlink"
        );
        assert!(mock.calls().is_empty());
        assert!(keeper.exists(), "the symlink target must be untouched");
    }

    #[test]
    fn a_keeper_listed_as_removable_is_recorded_not_silently_dropped() {
        // The engine cannot produce this, so it means an invariant broke. It
        // must be visible in the output rather than skipped silently, and it
        // must not panic: aborting mid-loop would leave a half-applied tree
        // with no report at all.
        let dir = tempfile::tempdir().unwrap();
        let keeper = write(&dir, "keep.jpg", b"DUPE");
        let dup = write(&dir, "dup.jpg", b"DUPE");
        let mut g = group(&keeper, &[&dup]);
        g.removable.push(rec(&keeper));

        let mock = MockRemover::new();
        let out = apply(&[g], ActionMode::Trash, &mock, &mut |_| {});

        assert_eq!(out.removed, vec![dup], "the genuine duplicate still goes");
        assert_eq!(
            out.skipped.len(),
            1,
            "the violation is accounted for, not dropped"
        );
        assert_eq!(out.skipped[0].reason, SkipReason::KeeperInRemovableSet);
        assert!(keeper.exists(), "and the keeper is still never removed");
        assert!(
            !mock.calls().contains(&keeper),
            "the keeper is never offered to the remover"
        );
    }

    #[test]
    fn events_are_emitted_as_each_candidate_is_decided() {
        // The stream is what a killed run leaves behind, so it must carry every
        // decision — not just the ones that succeeded.
        let dir = tempfile::tempdir().unwrap();
        let keeper = write(&dir, "keep.jpg", b"DUPE");
        let ok = write(&dir, "ok.jpg", b"DUPE");
        let gone = write(&dir, "gone.jpg", b"DUPE");
        let g = group(&keeper, &[&gone, &ok]);
        fs::remove_file(&gone).unwrap();

        let mut seen: Vec<String> = Vec::new();
        let out = apply(
            &[g],
            ActionMode::Trash,
            &MockRemover::new(),
            &mut |event| match event {
                Event::Removed(p) => seen.push(format!("removed {}", p.display())),
                Event::Skipped(p, r) => seen.push(format!("skipped {} ({r})", p.display())),
            },
        );

        assert_eq!(seen.len(), 2, "one event per candidate; got {seen:?}");
        assert!(seen[0].starts_with("skipped"), "in iteration order");
        assert!(seen[1].starts_with("removed"));
        assert_eq!(out.removed.len() + out.skipped.len(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn skips_a_candidate_hard_linked_to_the_keeper_since_the_scan() {
        // FR-17 says hard-linked siblings are never removal candidates. The
        // engine enforces that at scan time; if the link appears afterwards,
        // the byte compare would happily pass, so identity is checked too.
        let dir = tempfile::tempdir().unwrap();
        let keeper = write(&dir, "keep.jpg", b"DUPE");
        let dup = write(&dir, "dup.jpg", b"DUPE");
        let g = group(&keeper, &[&dup]);

        fs::remove_file(&dup).unwrap();
        fs::hard_link(&keeper, &dup).expect("hard link creatable");

        let mock = MockRemover::new();
        let out = apply(&[g], ActionMode::Trash, &mock, &mut |_| {});

        assert_eq!(out.skipped[0].reason, SkipReason::SameFileAsKeeper);
        assert!(mock.calls().is_empty());
        assert!(dup.exists() && keeper.exists());
    }

    // ── ADR-0017 / TASK-0338: `apply` span ────────────────────────────────

    #[test]
    fn apply_emits_an_apply_span_with_expected_fields() {
        use crate::trace_test_support::capture;

        let dir = tempfile::tempdir().unwrap();
        let keeper = write(&dir, "keep.jpg", b"DUPE");
        let dup1 = write(&dir, "dup1.jpg", b"DUPE");
        let dup2 = write(&dir, "dup2.jpg", b"DUPE");

        let mock = MockRemover::new();
        let out = capture(|| {
            let _ = apply(
                &[group(&keeper, &[&dup1, &dup2])],
                ActionMode::Trash,
                &mock,
                &mut |_| {},
            );
        });

        assert!(
            out.contains("apply"),
            "expected an `apply` span; got:\n{out}"
        );
        assert!(out.contains("mode=\"trash\""), "got:\n{out}");
        assert!(out.contains("groups=1"), "got:\n{out}");
        assert!(out.contains("removed=2"), "got:\n{out}");
        assert!(out.contains("skipped=0"), "got:\n{out}");
    }
}
