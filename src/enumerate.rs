//! File enumeration and scope filtering — the first pipeline stage.
//!
//! This module implements the `enumerate candidates` step from the ADR-0008
//! deduplication pipeline:
//!
//! ```text
//! enumerate candidates (FR-1/FR-2/FR-6)
//!   → magic-byte confirm each is an image (TASK-0303)
//!   → …
//! ```
//!
//! It walks the user-supplied input paths and returns a [`CandidateSet`]
//! containing every file that passes the scope rules, plus a list of
//! [`EnumerateWarning`]s for entries that were skipped due to I/O errors.
//!
//! ## Scope rules applied here (in order)
//!
//! 1. **Recursion** (FR-1): descend into subdirectories by default;
//!    `--no-recurse` limits traversal to the immediate children of each input
//!    path (depth 1).
//! 2. **Hidden files/dirs** (FR-6): entries whose name starts with `.` are
//!    excluded by default; `--hidden` opts in.
//! 3. **Symlinks** (FR-6): symbolic links are not followed by default;
//!    `--follow-symlinks` opts in. Even with the flag, only the target content
//!    is visited — we never follow a chain that loops back.
//! 4. **Extension allow-list** (FR-2): only files whose extension matches the
//!    allow-list are included. The default list is
//!    `jpg`, `jpeg`, `png`, `gif`, `webp`, `bmp`, `tif`, `tiff`.
//!    `--ext` replaces the entire list. Matching is **case-insensitive**.
//!    Leading dots on provided extensions are tolerated (`.jpg` → `jpg`).
//! 5. **I/O errors** (NFR-6): any entry that cannot be read (e.g.
//!    permission-denied) is collected as an [`EnumerateWarning`] and skipped;
//!    enumeration of the remaining entries continues.
//!
//! ## What is NOT done here
//!
//! - No file content reads (magic-byte sniffing is TASK-0303).
//! - No path canonicalization for keeper tie-breaks (TASK-0305/TASK-0306).
//! - No deduplication or grouping logic.
//!
//! ## Progress (TASK-0317)
//!
//! [`enumerate_with_progress`] additionally depends on
//! [`crate::progress::ProgressSink`], notifying it once per accepted
//! candidate so `main.rs` can animate the indeterminate Scanning phase
//! instead of leaving it frozen for the whole traversal. This is the only
//! dependency this module has outside `walkdir`/`std`.

#[cfg(test)]
use crate::progress::NoProgress;
use crate::progress::ProgressSink;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

// ── Default extension allow-list (FR-2) ──────────────────────────────────────

/// The default image-extension allow-list (FR-2, ADR-0008).
///
/// Extensions are stored lower-case; matching against candidates is performed
/// after lower-casing the candidate extension, so the comparison is always
/// case-insensitive regardless of filesystem.
const DEFAULT_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "gif", "webp", "bmp", "tif", "tiff"];

// ── Public types ──────────────────────────────────────────────────────────────

/// A single candidate file entry produced by [`enumerate`].
///
/// Contains the path and the file-system metadata that later pipeline stages
/// may need (size for grouping, mtime for the keeper rule). No file content is
/// read at this stage.
#[derive(Debug, Clone)]
pub struct Candidate {
    /// Absolute (or as-given) path to the file.
    pub path: PathBuf,
    /// Byte size of the file, from directory-walk metadata. Threaded through
    /// `sniff::filter_candidates` into `ConfirmedFile` and consumed by the
    /// engine's size-grouping stage (TASK-0304).
    pub size: u64,
    /// Modification time of the file in whole Unix seconds (seconds since the
    /// UNIX epoch), from directory-walk metadata. `None` when the platform
    /// does not expose mtime or the value cannot be obtained without error.
    /// No extra stat is performed: this reuses the metadata already fetched for
    /// `size`. Used by the keeper rule (TASK-0305).
    pub mtime: Option<i64>,
}

impl Candidate {
    fn new(path: PathBuf, size: u64, mtime: Option<i64>) -> Self {
        Self { path, size, mtime }
    }
}

/// A warning produced during enumeration when an entry is skipped.
///
/// Warnings are collected and returned alongside the candidate set so that
/// the report unit (TASK-0307) can surface them without coupling I/O
/// rendering to this module.
#[derive(Debug, Clone)]
pub struct EnumerateWarning {
    /// The path of the entry that could not be processed.
    pub path: PathBuf,
    /// Human-readable reason the entry was skipped.
    pub reason: String,
}

impl EnumerateWarning {
    fn new(path: PathBuf, reason: impl Into<String>) -> Self {
        Self {
            path,
            reason: reason.into(),
        }
    }
}

/// The output of [`enumerate`]: a set of candidate files and any warnings
/// generated during traversal.
///
/// Errors are split into two collections so that downstream stages (TASK-0311)
/// can distinguish a **run-level error** (an unreadable or nonexistent scan
/// root, ADR-0008 §exit-code → exit `1`) from a **per-entry skip** (a
/// descendant that was inaccessible but did not abort enumeration, NFR-6 → no
/// exit-code change).
#[derive(Debug, Default)]
pub struct CandidateSet {
    /// Files that passed all scope filters and are ready for the next pipeline
    /// stage (magic-byte confirmation in TASK-0303).
    // TODO(TASK-0303): consumed by the magic-byte sniff stage.
    pub candidates: Vec<Candidate>,
    /// Non-fatal **per-entry / descendant** errors collected during traversal
    /// (NFR-6). These do not abort enumeration and do not by themselves change
    /// the exit code from `0`; they are reported by TASK-0307.
    // TODO(TASK-0307): surfaced by the report unit.
    pub warnings: Vec<EnumerateWarning>,
    /// **Run-level** errors: failures on a scan *root* itself (depth 0) —
    /// e.g. the path does not exist or is a directory that cannot be opened.
    ///
    /// A non-empty `root_errors` signals to TASK-0311 that the run should exit
    /// with the run-level error code (`1`, ADR-0008). The distinction from
    /// `warnings` is intentional: a root that cannot be scanned at all is a
    /// fundamentally different situation from a descendant entry that was
    /// skipped.
    // TODO(TASK-0311): mapped to exit code 1 (run-level error) by the exit-code stage.
    pub root_errors: Vec<EnumerateWarning>,
}

// ── Scope options ─────────────────────────────────────────────────────────────

/// Scope parameters extracted from the parsed CLI (`Cli` in `cli.rs`).
///
/// Decoupled from the `Cli` struct so the enumeration module can be tested
/// without constructing a full `Cli`.
#[derive(Debug, Clone)]
pub struct ScopeOptions {
    /// When `true`, only the immediate children of each input path are visited
    /// (corresponds to `--no-recurse`, FR-1).
    pub no_recurse: bool,
    /// When `true`, hidden files and directories (names starting with `.`) are
    /// included (corresponds to `--hidden`, FR-6).
    pub include_hidden: bool,
    /// When `true`, symbolic links are followed during traversal (corresponds
    /// to `--follow-symlinks`, FR-6).
    pub follow_symlinks: bool,
    /// Extension allow-list to use. Each entry is already normalized to
    /// lower-case without a leading dot. When `None` the [`DEFAULT_EXTENSIONS`]
    /// list is used.
    pub extensions: Option<Vec<String>>,
}

impl ScopeOptions {
    /// Construct a `ScopeOptions` with all defaults matching the CLI defaults:
    /// recurse, no hidden, no symlink-follow, default extension list.
    pub fn default_scope() -> Self {
        Self {
            no_recurse: false,
            include_hidden: false,
            follow_symlinks: false,
            extensions: None,
        }
    }

    /// Parse a raw `--ext` string value into a normalized extension list.
    ///
    /// Accepts comma- or whitespace-separated tokens; strips leading dots;
    /// lower-cases every token. Returns an empty `Vec` if the input is blank.
    ///
    /// The input is a `&str` (already valid UTF-8 from the CLI argument parser),
    /// so no non-UTF-8 extension tokens can appear here. Non-UTF-8 extensions
    /// encountered during traversal are handled by [`extension_matches`], where
    /// lossy conversion ensures they are treated as non-matching (excluded).
    ///
    /// # Example (non-doc, see unit tests)
    ///
    /// `".JPG, PNG , gif"` → `["jpg", "png", "gif"]`
    pub fn parse_ext_list(raw: &str) -> Vec<String> {
        raw.split(|c: char| c == ',' || c.is_whitespace())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.trim_start_matches('.').to_lowercase())
            .collect()
    }

    /// Return `true` if the given file extension (already lower-cased, no dot)
    /// is in the effective allow-list.
    fn extension_allowed(&self, ext_lower: &str) -> bool {
        match &self.extensions {
            Some(list) => list.iter().any(|e| e == ext_lower),
            None => DEFAULT_EXTENSIONS.contains(&ext_lower),
        }
    }
}

// ── Main entry point ──────────────────────────────────────────────────────────

/// Walk `paths` applying `opts` scope rules and return the candidate set.
///
/// Each path in `paths` is traversed independently. Directories are walked
/// depth-first; the order of candidates within the returned set is not
/// guaranteed to be stable across runs or platforms (sorting for determinism
/// is the responsibility of TASK-0305/TASK-0306).
///
/// # Arguments
///
/// * `paths` — the user-supplied scan roots (from `Cli::paths`).
/// * `opts` — scope rules derived from the parsed CLI flags.
///
/// # Returns
///
/// A [`CandidateSet`] containing all files that passed all scope filters, plus
/// any per-entry [`EnumerateWarning`]s for entries that were skipped.
///
/// This function never returns an `Err`; per-entry I/O errors are collected as
/// warnings (NFR-6). A caller may treat the result as always-`Ok` — the
/// returned `CandidateSet` may simply contain warnings describing what was
/// skipped.
///
/// Progress-free variant of [`enumerate_with_progress`], kept
/// `#[cfg(test)]`-only (mirrors `engine.rs`'s `find_duplicates` /
/// `find_duplicates_with_progress` split, architect finding 6 there):
/// `enumerate_with_progress` is this module's **sole production entry
/// point** — the compiler proves it (no production code path can reach this
/// function at all), rather than relying on a lint suppression. `pub(crate)`
/// (rather than fully private) for parity with that same precedent, even
/// though today only this module's own tests call it.
#[cfg(test)]
pub(crate) fn enumerate(paths: &[PathBuf], opts: &ScopeOptions) -> CandidateSet {
    enumerate_with_progress(paths, opts, &NoProgress)
}

/// Like [`enumerate`], additionally notifying `sink` as each candidate file
/// is accepted (TASK-0317, FR-19): the total candidate count is not known
/// until enumeration finishes, so `main.rs` shows a live, indeterminate
/// count ("scanning... N files found") rather than a frozen "scanning..."
/// for the whole enumeration sub-phase. `sink.advance(1)` is called exactly
/// once per candidate that passes every scope rule — not once per directory
/// entry walkdir visits — so the count only ever grows by files that will
/// actually reach the next pipeline stage.
pub fn enumerate_with_progress(
    paths: &[PathBuf],
    opts: &ScopeOptions,
    sink: &dyn ProgressSink,
) -> CandidateSet {
    let mut result = CandidateSet::default();

    for root in &dedupe_roots(paths, opts) {
        enumerate_root(root, opts, &mut result, sink);
    }

    result
}

/// Remove input scan roots that are exact duplicates of, or nested inside,
/// another given root (TASK-0319).
///
/// Without this, `imgdedup ./d ./d` or overlapping roots like
/// `./d ./d/sub` walk the same physical file more than once. Each occurrence
/// becomes its own [`Candidate`] with the same content, so the engine's
/// identity collapse (`same-file::Handle`, ADR-0008) correctly recognizes
/// them as the same physical file — and reports it as a hard-link set
/// containing only that one file, silently absorbing what should be zero
/// duplicate files into a false "already deduplicated" note (fail-safe, but
/// misleading: a user could conclude a tree is clean when it was never
/// checked against anything).
///
/// **A nested root is only redundant if the ancestor's walk actually reaches
/// it.** Two scope rules can make that false, and treating a nested root as
/// redundant anyway would silently drop files the user explicitly asked to
/// scan — the same class of harm this fix exists to prevent:
///
/// - `--no-recurse` caps every root's walk at its own immediate children
///   (depth 1), so an ancestor root never reaches a nested root's contents
///   at all. Under `--no-recurse`, only *exact*-duplicate roots are
///   collapsed; nesting is never treated as coverage.
/// - Hidden entries are skipped unless `--hidden` is given — **except** a
///   hidden path passed *explicitly* as a scan root, which is always
///   included (`docs/guide/scanning-and-scope.md`). Collapsing an
///   explicitly-given hidden root into its ancestor would silently
///   un-opt-in it, since the ancestor's own walk *would* then skip it as a
///   hidden descendant. So a nested root that is hidden relative to its
///   ancestor is kept, unless `--hidden` is also set.
///
/// Only the **root list** is canonicalized — normally a handful of paths —
/// never every candidate file, so this adds no per-file traversal cost.
/// Single-root invocations (the common case) return `paths` unchanged
/// without canonicalizing anything, since one root cannot overlap itself.
/// For multi-root invocations, canonical paths are **sorted** rather than
/// compared all-pairs: `Path`'s `Ord` is lexicographic over components, the
/// same property that makes "a path's descendants occupy a contiguous range
/// immediately after it in sorted order, with nothing unrelated in between"
/// hold — the standard prefix-dedup technique, letting one linear pass over
/// the sorted list replace an O(n²) comparison (the latter measured at a 25×
/// slowdown on 20,000 roots in review). The sort is stable, so exact
/// duplicates keep their original relative order, which is what makes "keep
/// the first occurrence" fall out for free.
///
/// A root whose canonical form cannot be determined (e.g. it does not
/// exist) is passed through unchanged rather than dropped, so the existing
/// root-level error path (`root_errors`, ADR-0008 exit-code semantics) is
/// unaffected — this function only ever removes roots it can *prove* are
/// redundant.
fn dedupe_roots(paths: &[PathBuf], opts: &ScopeOptions) -> Vec<PathBuf> {
    if paths.len() <= 1 {
        return paths.to_vec();
    }

    let canonical: Vec<Option<PathBuf>> = paths
        .iter()
        .map(|p| std::fs::canonicalize(p).ok())
        .collect();

    // Only successfully-canonicalized roots participate; `None` entries are
    // always kept (handled by the final filter below) and never compared.
    let mut resolved: Vec<usize> = (0..paths.len())
        .filter(|&i| canonical[i].is_some())
        .collect();
    resolved.sort_by(|&a, &b| canonical[a].cmp(&canonical[b]));

    let mut keep = vec![true; paths.len()];
    let mut last_kept: Option<usize> = None;
    for i in resolved {
        let ci = canonical[i].as_ref().unwrap();
        let redundant = match last_kept {
            None => false,
            Some(k) => {
                let ck = canonical[k].as_ref().unwrap();
                if ci == ck {
                    // Exact duplicate: unconditionally redundant — there is
                    // no scope rule under which walking the same path twice
                    // discovers anything a first walk would not.
                    true
                } else if ci.starts_with(ck) {
                    // A proper descendant of the last-kept root is only
                    // redundant if that root's own walk would actually reach
                    // it (see the two exceptions in the doc comment above).
                    !opts.no_recurse && (opts.include_hidden || !is_hidden_relative_to(ck, ci))
                } else {
                    false
                }
            }
        };

        if redundant {
            keep[i] = false;
        } else {
            last_kept = Some(i);
        }
    }

    paths
        .iter()
        .enumerate()
        .filter(|&(i, _)| keep[i])
        .map(|(_, p)| p.clone())
        .collect()
}

/// Walk a single root path and accumulate candidates/warnings into `out`,
/// notifying `sink` once per accepted candidate (TASK-0317).
fn enumerate_root(
    root: &Path,
    opts: &ScopeOptions,
    out: &mut CandidateSet,
    sink: &dyn ProgressSink,
) {
    // Determine max depth: depth 1 = immediate children only (no-recurse).
    // walkdir depth 0 is the root itself; depth 1 includes direct children.
    let max_depth = if opts.no_recurse { 1usize } else { usize::MAX };

    let walker = WalkDir::new(root)
        .max_depth(max_depth)
        .follow_links(opts.follow_symlinks)
        .into_iter();

    for entry_result in walker {
        match entry_result {
            Err(e) => {
                // Collect the path from the error if available; otherwise use
                // the root as the best available proxy.
                let path = e
                    .path()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| root.to_path_buf());
                // Depth-0 errors mean the root itself is inaccessible — that
                // is a run-level error (ADR-0008 exit-code semantics), not a
                // per-entry skip. Route accordingly so TASK-0311 can map
                // non-empty `root_errors` to exit code 1.
                if e.depth() == 0 {
                    out.root_errors
                        .push(EnumerateWarning::new(path, e.to_string()));
                } else {
                    out.warnings
                        .push(EnumerateWarning::new(path, e.to_string()));
                }
            }
            Ok(entry) => {
                let path = entry.path().to_path_buf();

                // Skip hidden entries unless opted in (FR-6).
                // We check only path components *below* the scan root, so
                // that a user-supplied root whose own name starts with `.`
                // (or whose ancestors do, as is common for temp directories)
                // is not itself treated as hidden.
                if !opts.include_hidden && is_hidden_relative_to(root, &path) {
                    continue;
                }

                // We only care about regular files (not dirs, symlinks to
                // dirs that we're visiting, etc.).
                let file_type = entry.file_type();
                if !file_type.is_file() {
                    // Symlink entries (when follow_links=false) appear as
                    // symlinks; we skip them here since we're looking for
                    // regular files only.
                    continue;
                }

                // Extension filter (FR-2).
                if !extension_matches(&path, opts) {
                    continue;
                }

                // Collect the size and mtime from the entry's metadata.
                // Both are extracted from the same metadata call so no extra
                // stat is performed (TASK-0305 requirement: one stat, no I/O
                // in the keeper comparator).
                let meta = match entry.metadata() {
                    Ok(m) => m,
                    Err(e) => {
                        out.warnings.push(EnumerateWarning::new(
                            path.clone(),
                            format!("could not read metadata: {e}"),
                        ));
                        continue;
                    }
                };
                let size = meta.len();
                // mtime: whole-second Unix timestamp. None when the platform
                // does not provide it (e.g. WebAssembly) or the value is out
                // of range. We discard sub-second precision (ADR-0008 §keeper).
                let mtime = meta.modified().ok().and_then(|t| {
                    t.duration_since(std::time::UNIX_EPOCH)
                        .ok()
                        .map(|d| d.as_secs() as i64)
                });

                out.candidates.push(Candidate::new(path, size, mtime));
                sink.advance(1);
            }
        }
    }
}

/// Return `true` if any path component of `path` that is *below* `root`
/// starts with `.` (i.e. is hidden).
///
/// The root itself and its ancestors are not checked — a user who passes a
/// scan root starting with `.` is explicitly asking to scan it. Only
/// subdirectory and file names encountered during the walk are subject to the
/// hidden-entry filter.
///
/// Classification is over the **in-scan traversal path** (the path walkdir
/// produces during the walk), not over any resolved symlink target — so a
/// symlink whose *name* starts with `.` is hidden, regardless of where its
/// target lives.
///
/// Examples (given root `/home/user/photos`):
/// - `/home/user/photos/img.jpg` → `false` (nothing below root starts with `.`)
/// - `/home/user/photos/.hidden.jpg` → `true` (`.hidden.jpg` is below root)
/// - `/home/user/photos/.hiddendir/img.jpg` → `true` (`.hiddendir` is below root)
fn is_hidden_relative_to(root: &Path, path: &Path) -> bool {
    // Strip the root prefix to obtain only the relative portion.
    // If strip_prefix fails (which should not happen given walkdir always
    // produces descendants of root), fall back to a full-path check so we
    // are conservative (no false negatives).
    let relative = match path.strip_prefix(root) {
        Ok(rel) => rel,
        Err(_) => path,
    };
    relative.components().any(|c| {
        use std::path::Component;
        match c {
            Component::Normal(name) => name.to_str().map(|s| s.starts_with('.')).unwrap_or(false),
            _ => false,
        }
    })
}

/// Return `true` if `path`'s extension (case-insensitive) is in the effective
/// allow-list.
///
/// A non-UTF-8 extension is treated as non-matching (lossy conversion →
/// excluded). This is the safe direction: we never accidentally include a
/// file with an undecodable extension.
fn extension_matches(path: &Path, opts: &ScopeOptions) -> bool {
    match path.extension() {
        None => false,
        Some(ext) => {
            let ext_lower = ext.to_string_lossy().to_lowercase();
            opts.extension_allowed(&ext_lower)
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use tempfile::TempDir;

    /// Minimal spy `ProgressSink` (TASK-0317): counts the total advanced,
    /// nothing else — enumeration only ever calls `advance`, never
    /// `set_total`/`retotal`/`set_phase`/`finish`, so those are no-ops here.
    #[derive(Default)]
    struct CountingSink {
        advanced: AtomicU64,
    }

    impl crate::progress::ProgressSink for CountingSink {
        fn set_total(&self, _total: u64) {}
        fn set_phase(&self, _phase: crate::progress::Phase) {}
        fn advance(&self, n: u64) {
            self.advanced.fetch_add(n, Ordering::SeqCst);
        }
        fn retotal(&self, _total: u64) {}
        fn finish(&self) {}
    }

    // ── Fixtures ──────────────────────────────────────────────────────────────

    /// Create a file at `dir/rel_path` with empty content, creating parent
    /// directories as needed.
    fn touch(dir: &TempDir, rel_path: &str) -> PathBuf {
        let p = dir.path().join(rel_path);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&p, b"").unwrap();
        p
    }

    /// Collect just the filenames (not full paths) from a CandidateSet for
    /// easy comparison in assertions.
    fn candidate_names(set: &CandidateSet) -> Vec<String> {
        let mut names: Vec<String> = set
            .candidates
            .iter()
            .map(|c| c.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// Collect relative paths (from `root`) of all candidates, sorted.
    /// Used to assert full traversal shape, not just filenames.
    fn candidate_rel_paths(set: &CandidateSet, root: &std::path::Path) -> Vec<String> {
        let mut paths: Vec<String> = set
            .candidates
            .iter()
            .map(|c| {
                c.path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        paths.sort();
        paths
    }

    // ── AC-15 (FR-1): recursion / no-recurse ─────────────────────────────────

    #[test]
    fn recurse_by_default_finds_nested_files() {
        // AC-15: with default options, files in subdirectories are enumerated.
        // Finding 7: assert the full relative path to lock traversal shape.
        let dir = tempfile::tempdir().unwrap();
        touch(&dir, "top.jpg");
        touch(&dir, "sub/nested.jpg");
        touch(&dir, "sub/deep/verydeep.jpg");

        let opts = ScopeOptions::default_scope();
        let result = enumerate(&[dir.path().to_path_buf()], &opts);

        assert!(
            result.warnings.is_empty(),
            "expected no warnings, got: {:?}",
            result
                .warnings
                .iter()
                .map(|w| &w.reason)
                .collect::<Vec<_>>()
        );
        // Assert full relative paths to lock the traversal shape (Finding 7).
        let rel_paths = candidate_rel_paths(&result, dir.path());
        assert_eq!(
            rel_paths,
            // The relative string uses the native separator (`/` on Unix,
            // `\` on Windows). Build the expected paths with `Path::join`,
            // which inserts the native separator, so the assertion is portable.
            {
                let mut expected = vec![
                    std::path::Path::new("sub")
                        .join("nested.jpg")
                        .to_string_lossy()
                        .into_owned(),
                    std::path::Path::new("sub")
                        .join("deep")
                        .join("verydeep.jpg")
                        .to_string_lossy()
                        .into_owned(),
                    "top.jpg".to_owned(),
                ];
                expected.sort();
                expected
            },
            "expected all three nested files with full relative paths; got: {rel_paths:?}"
        );
    }

    #[test]
    fn no_recurse_finds_only_top_level() {
        // AC-15: --no-recurse limits to the top level of each path.
        let dir = tempfile::tempdir().unwrap();
        touch(&dir, "top.jpg");
        touch(&dir, "sub/nested.jpg");

        let opts = ScopeOptions {
            no_recurse: true,
            ..ScopeOptions::default_scope()
        };
        let result = enumerate(&[dir.path().to_path_buf()], &opts);

        let names = candidate_names(&result);
        assert_eq!(
            names,
            vec!["top.jpg"],
            "no-recurse should only return top-level files; got: {names:?}"
        );
    }

    #[test]
    fn no_recurse_does_not_descend_into_any_subdir() {
        // AC-15: with --no-recurse, files in multiple subdirectory levels are
        // excluded, regardless of nesting depth.
        let dir = tempfile::tempdir().unwrap();
        touch(&dir, "a.png");
        touch(&dir, "level1/b.png");
        touch(&dir, "level1/level2/c.png");

        let opts = ScopeOptions {
            no_recurse: true,
            ..ScopeOptions::default_scope()
        };
        let result = enumerate(&[dir.path().to_path_buf()], &opts);

        let names = candidate_names(&result);
        assert_eq!(names, vec!["a.png"]);
    }

    // ── FR-2: extension allow-list ────────────────────────────────────────────

    #[test]
    fn default_ext_list_accepts_all_allowed_formats() {
        // FR-2: the default allow-list covers jpg/jpeg/png/gif/webp/bmp/tif/tiff.
        let dir = tempfile::tempdir().unwrap();
        touch(&dir, "a.jpg");
        touch(&dir, "b.jpeg");
        touch(&dir, "c.png");
        touch(&dir, "d.gif");
        touch(&dir, "e.webp");
        touch(&dir, "f.bmp");
        touch(&dir, "g.tif");
        touch(&dir, "h.tiff");

        let opts = ScopeOptions::default_scope();
        let result = enumerate(&[dir.path().to_path_buf()], &opts);

        let names = candidate_names(&result);
        // candidate_names already sorts alphabetically.
        assert_eq!(
            names,
            vec![
                "a.jpg", "b.jpeg", "c.png", "d.gif", "e.webp", "f.bmp", "g.tif", "h.tiff"
            ]
        );
    }

    #[test]
    fn default_ext_list_excludes_non_image_files() {
        // FR-2: files without an allowed extension are excluded.
        let dir = tempfile::tempdir().unwrap();
        touch(&dir, "photo.jpg");
        touch(&dir, "document.txt");
        touch(&dir, "video.mp4");
        touch(&dir, "script.sh");
        touch(&dir, "noext");

        let opts = ScopeOptions::default_scope();
        let result = enumerate(&[dir.path().to_path_buf()], &opts);

        let names = candidate_names(&result);
        assert_eq!(names, vec!["photo.jpg"]);
    }

    #[test]
    fn ext_matching_is_case_insensitive() {
        // FR-2: .JPG, .Jpeg, .PNG etc. must match the allow-list.
        let dir = tempfile::tempdir().unwrap();
        touch(&dir, "upper.JPG");
        touch(&dir, "mixed.Jpeg");
        touch(&dir, "capital.PNG");
        touch(&dir, "allcaps.TIFF");

        let opts = ScopeOptions::default_scope();
        let result = enumerate(&[dir.path().to_path_buf()], &opts);

        let names = candidate_names(&result);
        assert_eq!(
            names,
            vec!["allcaps.TIFF", "capital.PNG", "mixed.Jpeg", "upper.JPG"]
        );
    }

    #[test]
    fn custom_ext_overrides_default_list() {
        // FR-2: --ext replaces the default allow-list entirely.
        let dir = tempfile::tempdir().unwrap();
        touch(&dir, "a.jpg"); // in default list; NOT in custom
        touch(&dir, "b.png"); // in default list; NOT in custom
        touch(&dir, "c.raw"); // NOT in default; IS in custom
        touch(&dir, "d.cr2"); // NOT in default; IS in custom

        let opts = ScopeOptions {
            extensions: Some(ScopeOptions::parse_ext_list("raw,cr2")),
            ..ScopeOptions::default_scope()
        };
        let result = enumerate(&[dir.path().to_path_buf()], &opts);

        let names = candidate_names(&result);
        assert_eq!(names, vec!["c.raw", "d.cr2"]);
    }

    #[test]
    fn parse_ext_list_strips_leading_dots() {
        // FR-2: parse_ext_list tolerates ".jpg" → "jpg".
        let exts = ScopeOptions::parse_ext_list(".jpg,.PNG, .gif");
        assert_eq!(exts, vec!["jpg", "png", "gif"]);
    }

    #[test]
    fn parse_ext_list_handles_comma_and_space_separators() {
        // FR-2: comma- or space-separated tokens are both accepted.
        let by_comma = ScopeOptions::parse_ext_list("jpg,png,gif");
        let by_space = ScopeOptions::parse_ext_list("jpg png gif");
        let mixed = ScopeOptions::parse_ext_list("jpg, png gif");
        assert_eq!(by_comma, vec!["jpg", "png", "gif"]);
        assert_eq!(by_space, vec!["jpg", "png", "gif"]);
        assert_eq!(mixed, vec!["jpg", "png", "gif"]);
    }

    #[test]
    fn parse_ext_list_lowercases_all_tokens() {
        let exts = ScopeOptions::parse_ext_list("JPG PNG GIF");
        assert_eq!(exts, vec!["jpg", "png", "gif"]);
    }

    // ── AC-14 (FR-6): hidden files ────────────────────────────────────────────

    #[test]
    fn hidden_files_excluded_by_default() {
        // AC-14: files starting with '.' are excluded by default.
        let dir = tempfile::tempdir().unwrap();
        touch(&dir, "visible.jpg");
        touch(&dir, ".hidden.jpg");

        let opts = ScopeOptions::default_scope();
        let result = enumerate(&[dir.path().to_path_buf()], &opts);

        let names = candidate_names(&result);
        assert_eq!(names, vec!["visible.jpg"]);
    }

    #[test]
    fn hidden_dirs_excluded_by_default() {
        // AC-14: files inside hidden directories are also excluded by default.
        let dir = tempfile::tempdir().unwrap();
        touch(&dir, "visible.jpg");
        touch(&dir, ".hiddendir/inside.jpg");

        let opts = ScopeOptions::default_scope();
        let result = enumerate(&[dir.path().to_path_buf()], &opts);

        let names = candidate_names(&result);
        assert_eq!(names, vec!["visible.jpg"]);
    }

    #[test]
    fn hidden_flag_includes_hidden_files() {
        // AC-14: --hidden opts in hidden files.
        let dir = tempfile::tempdir().unwrap();
        touch(&dir, "visible.jpg");
        touch(&dir, ".hidden.jpg");

        let opts = ScopeOptions {
            include_hidden: true,
            ..ScopeOptions::default_scope()
        };
        let result = enumerate(&[dir.path().to_path_buf()], &opts);

        let names = candidate_names(&result);
        assert_eq!(names, vec![".hidden.jpg", "visible.jpg"]);
    }

    #[test]
    fn hidden_flag_includes_files_in_hidden_dirs() {
        // AC-14: --hidden opts in files inside hidden directories too.
        let dir = tempfile::tempdir().unwrap();
        touch(&dir, "visible.jpg");
        touch(&dir, ".hiddendir/inside.jpg");

        let opts = ScopeOptions {
            include_hidden: true,
            ..ScopeOptions::default_scope()
        };
        let result = enumerate(&[dir.path().to_path_buf()], &opts);

        let names = candidate_names(&result);
        // candidate_names extracts file_name(), so "inside.jpg" and "visible.jpg".
        assert_eq!(names, vec!["inside.jpg", "visible.jpg"]);
    }

    // ── AC-14 (FR-6): symlinks ────────────────────────────────────────────────

    #[cfg(unix)]
    #[test]
    fn symlinks_not_followed_by_default() {
        // AC-14: symlinks are not followed without --follow-symlinks.
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        // Real file in a separate temp dir.
        let target_dir = tempfile::tempdir().unwrap();
        let target_file = target_dir.path().join("real.jpg");
        fs::write(&target_file, b"").unwrap();

        // Symlink inside our scan dir pointing to the real file.
        let link_path = dir.path().join("link.jpg");
        symlink(&target_file, &link_path).unwrap();

        // Also create a real file in the scan dir so we confirm regular files work.
        touch(&dir, "real_in_dir.jpg");

        let opts = ScopeOptions::default_scope();
        let result = enumerate(&[dir.path().to_path_buf()], &opts);

        let names = candidate_names(&result);
        assert_eq!(
            names,
            vec!["real_in_dir.jpg"],
            "symlinks should not be followed by default; got: {names:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn follow_symlinks_flag_includes_symlink_targets() {
        // AC-14: --follow-symlinks makes the symlink target accessible.
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let target_dir = tempfile::tempdir().unwrap();
        let target_file = target_dir.path().join("real.jpg");
        fs::write(&target_file, b"").unwrap();

        let link_path = dir.path().join("link.jpg");
        symlink(&target_file, &link_path).unwrap();

        touch(&dir, "real_in_dir.jpg");

        let opts = ScopeOptions {
            follow_symlinks: true,
            ..ScopeOptions::default_scope()
        };
        let result = enumerate(&[dir.path().to_path_buf()], &opts);

        let names = candidate_names(&result);
        assert!(
            names.contains(&"link.jpg".to_string()),
            "follow-symlinks should include symlink target; got: {names:?}"
        );
        assert!(
            names.contains(&"real_in_dir.jpg".to_string()),
            "regular files should still be included; got: {names:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_to_directory_not_followed_by_default() {
        // AC-14: a symlink to a directory is not followed by default.
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let target_dir = tempfile::tempdir().unwrap();
        let target_file = target_dir.path().join("inside.jpg");
        fs::write(&target_file, b"").unwrap();

        // Symlink to the directory itself.
        let dir_link = dir.path().join("linked_dir");
        symlink(target_dir.path(), &dir_link).unwrap();
        touch(&dir, "top.jpg");

        let opts = ScopeOptions::default_scope();
        let result = enumerate(&[dir.path().to_path_buf()], &opts);

        let names = candidate_names(&result);
        assert_eq!(
            names,
            vec!["top.jpg"],
            "symlink to directory should not be followed by default; got: {names:?}"
        );
    }

    // ── AC-10 (NFR-6): I/O error skip-and-report ─────────────────────────────

    // Note: we cannot easily create an unreadable file in a cross-platform /
    // CI-safe way without platform-specific helpers. On Unix we use chmod; on
    // Windows this test is skipped because we cannot reliably simulate a
    // permission-denied file in a temp directory without admin rights.

    #[cfg(unix)]
    #[test]
    fn unreadable_descendant_produces_warning_not_root_error() {
        // AC-10 (NFR-6): a permission-denied *descendant* is skipped with a
        // warning in `warnings`, NOT in `root_errors`. The root itself is fine.
        // This test locks that the two collections are kept distinct (Finding 1).
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        touch(&dir, "good1.jpg");
        let bad_dir = dir.path().join("bad_subdir");
        fs::create_dir(&bad_dir).unwrap();
        // Put a file in the unreadable subdirectory.
        let bad_file = bad_dir.join("bad.jpg");
        fs::write(&bad_file, b"").unwrap();
        // Make the subdirectory unreadable/non-traversable.
        fs::set_permissions(&bad_dir, fs::Permissions::from_mode(0o000)).unwrap();
        touch(&dir, "good2.jpg");

        let opts = ScopeOptions::default_scope();
        let result = enumerate(&[dir.path().to_path_buf()], &opts);

        // Restore permissions so tempdir cleanup can succeed.
        fs::set_permissions(&bad_dir, fs::Permissions::from_mode(0o755)).unwrap();

        // The good files must be in the candidate set.
        let names = candidate_names(&result);
        assert!(
            names.contains(&"good1.jpg".to_string()),
            "good1.jpg should be in candidates; got: {names:?}"
        );
        assert!(
            names.contains(&"good2.jpg".to_string()),
            "good2.jpg should be in candidates; got: {names:?}"
        );

        // The permission error must have produced at least one warning …
        assert!(
            !result.warnings.is_empty(),
            "expected at least one warning for the unreadable descendant"
        );
        // … but it must NOT appear as a root error (the scan root is readable).
        assert!(
            result.root_errors.is_empty(),
            "descendant error must not populate root_errors; got: {:?}",
            result.root_errors
        );
    }

    // Keep the original AC-10 test name as an alias so any CI reference to it
    // continues to compile (the new name above is the canonical form).
    #[cfg(unix)]
    #[test]
    fn unreadable_entry_produces_warning_and_rest_completes() {
        // AC-10 (NFR-6): a permission-denied entry is skipped with a warning;
        // enumeration of the rest of the directory completes.
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        touch(&dir, "good1.jpg");
        let bad_dir = dir.path().join("bad_subdir");
        fs::create_dir(&bad_dir).unwrap();
        let bad_file = bad_dir.join("bad.jpg");
        fs::write(&bad_file, b"").unwrap();
        fs::set_permissions(&bad_dir, fs::Permissions::from_mode(0o000)).unwrap();
        touch(&dir, "good2.jpg");

        let opts = ScopeOptions::default_scope();
        let result = enumerate(&[dir.path().to_path_buf()], &opts);

        fs::set_permissions(&bad_dir, fs::Permissions::from_mode(0o755)).unwrap();

        let names = candidate_names(&result);
        assert!(names.contains(&"good1.jpg".to_string()));
        assert!(names.contains(&"good2.jpg".to_string()));
        assert!(!result.warnings.is_empty());
    }

    // ── ADR-0008 root-error distinction (Finding 1 / Finding 6) ──────────────

    /// Nonexistent root: portable test (no cfg(unix) gate) that drives the
    /// `root_errors` collection path (Finding 6 — Windows CI coverage).
    #[test]
    fn nonexistent_root_produces_root_error_not_warning() {
        // A path that does not exist is a run-level error (ADR-0008 exit §1),
        // not a per-entry skip. The error must land in `root_errors`, the
        // candidate set must be empty, and `warnings` must be empty.
        let nonexistent = std::path::PathBuf::from("/does/not/exist/imgdedup_test_root.jpg");

        let opts = ScopeOptions::default_scope();
        let result = enumerate(std::slice::from_ref(&nonexistent), &opts);

        assert!(
            result.candidates.is_empty(),
            "nonexistent root should yield no candidates; got: {:?}",
            result.candidates
        );
        assert!(
            result.warnings.is_empty(),
            "nonexistent root error must not appear in per-entry warnings; got: {:?}",
            result.warnings
        );
        assert_eq!(
            result.root_errors.len(),
            1,
            "expected exactly one root_error; got: {:?}",
            result.root_errors
        );
        assert_eq!(
            result.root_errors[0].path, nonexistent,
            "root_error path must be the bad root itself"
        );
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_root_directory_produces_root_error() {
        // An unreadable root directory (mode 0o000) is a run-level error.
        // The error must land in `root_errors`, not `warnings`.
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let bad_root = dir.path().join("bad_root");
        fs::create_dir(&bad_root).unwrap();
        // Place a file inside so the test would find it if permissions allowed.
        fs::write(bad_root.join("photo.jpg"), b"").unwrap();
        fs::set_permissions(&bad_root, fs::Permissions::from_mode(0o000)).unwrap();

        let opts = ScopeOptions::default_scope();
        let result = enumerate(std::slice::from_ref(&bad_root), &opts);

        // Restore so tempdir cleanup can succeed.
        fs::set_permissions(&bad_root, fs::Permissions::from_mode(0o755)).unwrap();

        assert!(
            result.candidates.is_empty(),
            "unreadable root should yield no candidates; got: {:?}",
            result.candidates
        );
        assert!(
            result.warnings.is_empty(),
            "root error must not appear in per-entry warnings; got: {:?}",
            result.warnings
        );
        assert_eq!(
            result.root_errors.len(),
            1,
            "expected exactly one root_error; got: {:?}",
            result.root_errors
        );
        assert_eq!(
            result.root_errors[0].path, bad_root,
            "root_error path must be the bad root itself"
        );
    }

    // ── File-as-root (Finding 2) ──────────────────────────────────────────────

    #[test]
    fn single_image_file_as_root_is_enumerated() {
        // Cli::paths accepts files, not just dirs. A single image file passed
        // directly as a root must appear in the candidate set (Finding 2).
        let dir = tempfile::tempdir().unwrap();
        let file = touch(&dir, "photo.jpg");

        let opts = ScopeOptions::default_scope();
        let result = enumerate(std::slice::from_ref(&file), &opts);

        assert_eq!(
            result.candidates.len(),
            1,
            "a single file root should yield exactly one candidate; got: {:?}",
            result.candidates
        );
        assert_eq!(result.candidates[0].path, file);
        assert!(result.warnings.is_empty());
        assert!(result.root_errors.is_empty());
    }

    #[test]
    fn single_image_file_as_root_with_no_recurse_is_enumerated() {
        // Even with --no-recurse, a file root must still be enumerated
        // (Finding 2 — file-as-root variant with --no-recurse).
        let dir = tempfile::tempdir().unwrap();
        let file = touch(&dir, "photo.jpg");

        let opts = ScopeOptions {
            no_recurse: true,
            ..ScopeOptions::default_scope()
        };
        let result = enumerate(std::slice::from_ref(&file), &opts);

        assert_eq!(
            result.candidates.len(),
            1,
            "--no-recurse must not exclude a file passed directly as a root; got: {:?}",
            result.candidates
        );
        assert_eq!(result.candidates[0].path, file);
    }

    #[test]
    fn hidden_file_passed_explicitly_as_root_is_included() {
        // A hidden file (name starts with '.') passed explicitly as a root is
        // included regardless of the --hidden flag (opt-in-by-explicit-root
        // semantics, Finding 2). The hidden filter applies only to *discovered*
        // descendants, not to user-supplied roots.
        let dir = tempfile::tempdir().unwrap();
        let hidden_file = touch(&dir, ".photo.jpg");

        // Default scope: hidden filter is active.
        let opts = ScopeOptions::default_scope();
        let result = enumerate(std::slice::from_ref(&hidden_file), &opts);

        assert_eq!(
            result.candidates.len(),
            1,
            "a hidden file passed explicitly as root must be included; got: {:?}",
            result.candidates
        );
        assert_eq!(result.candidates[0].path, hidden_file);
    }

    // ── Multiple roots ────────────────────────────────────────────────────────

    #[test]
    fn multiple_roots_are_each_walked() {
        // enumerate() accepts multiple paths and walks them all.
        let dir_a = tempfile::tempdir().unwrap();
        let dir_b = tempfile::tempdir().unwrap();
        touch(&dir_a, "from_a.jpg");
        touch(&dir_b, "from_b.png");

        let opts = ScopeOptions::default_scope();
        let result = enumerate(
            &[dir_a.path().to_path_buf(), dir_b.path().to_path_buf()],
            &opts,
        );

        let names = candidate_names(&result);
        assert_eq!(names, vec!["from_a.jpg", "from_b.png"]);
    }

    // ── TASK-0319: overlapping/duplicate roots are not double-counted ───────

    #[test]
    fn same_root_given_twice_is_not_double_counted() {
        // TASK-0319: `imgdedup ./d ./d` must not walk the tree twice — a
        // single physical file must yield exactly one candidate, not two
        // (which downstream would look like the file is hard-linked to
        // itself and mask real duplicates elsewhere).
        let dir = tempfile::tempdir().unwrap();
        touch(&dir, "a.jpg");

        let opts = ScopeOptions::default_scope();
        let root = dir.path().to_path_buf();
        let result = enumerate(&[root.clone(), root], &opts);

        assert_eq!(
            result.candidates.len(),
            1,
            "the same root given twice must not double the candidate count"
        );
    }

    #[test]
    fn nested_root_is_not_double_counted_regardless_of_argument_order() {
        // TASK-0319: overlapping roots like `./d ./d/sub` must not walk
        // `./d/sub`'s contents twice (once via `./d`, once via the explicit
        // `./d/sub` root). Checked both argument orders since the fix must
        // not depend on which root is listed first.
        let dir = tempfile::tempdir().unwrap();
        touch(&dir, "top.jpg");
        touch(&dir, "sub/nested.jpg");
        let sub = dir.path().join("sub");

        let opts = ScopeOptions::default_scope();

        let ancestor_first = enumerate(&[dir.path().to_path_buf(), sub.clone()], &opts);
        assert_eq!(
            candidate_names(&ancestor_first),
            vec!["nested.jpg", "top.jpg"],
            "ancestor listed first: nested.jpg must appear exactly once"
        );

        let descendant_first = enumerate(&[sub, dir.path().to_path_buf()], &opts);
        assert_eq!(
            candidate_names(&descendant_first),
            vec!["nested.jpg", "top.jpg"],
            "descendant listed first: nested.jpg must still appear exactly once"
        );
    }

    #[test]
    fn distinct_non_overlapping_roots_are_unaffected() {
        // The fix must not merge or drop genuinely distinct roots — this is
        // the same shape as `multiple_roots_are_each_walked` but pinned here
        // too as a regression guard alongside the new dedup behavior.
        let dir_a = tempfile::tempdir().unwrap();
        let dir_b = tempfile::tempdir().unwrap();
        touch(&dir_a, "from_a.jpg");
        touch(&dir_b, "from_b.png");

        let opts = ScopeOptions::default_scope();
        let result = enumerate(
            &[dir_a.path().to_path_buf(), dir_b.path().to_path_buf()],
            &opts,
        );

        assert_eq!(candidate_names(&result), vec!["from_a.jpg", "from_b.png"]);
    }

    #[test]
    fn no_recurse_nested_root_is_not_dropped_as_redundant() {
        // TASK-0319 review finding: with --no-recurse, an ancestor root's
        // walk is capped at its own immediate children (depth 1), so it
        // never actually reaches a nested root's contents. Treating the
        // nested root as "covered" and dropping it would silently exclude
        // files the user explicitly asked to scan by naming that root.
        let dir = tempfile::tempdir().unwrap();
        touch(&dir, "top.jpg");
        touch(&dir, "sub/a.jpg");
        touch(&dir, "sub/b.jpg");
        let sub = dir.path().join("sub");

        let opts = ScopeOptions {
            no_recurse: true,
            ..ScopeOptions::default_scope()
        };
        let result = enumerate(&[dir.path().to_path_buf(), sub], &opts);

        assert_eq!(
            candidate_names(&result),
            vec!["a.jpg", "b.jpg", "top.jpg"],
            "with --no-recurse, the nested root's own files must still be \
             found — its ancestor's capped walk never reaches them"
        );
    }

    #[test]
    fn hidden_nested_root_named_explicitly_is_not_dropped_as_redundant() {
        // TASK-0319 review finding: a hidden path passed explicitly as a
        // scan root is always included (opt-in-by-explicit-root,
        // docs/guide/scanning-and-scope.md), even without --hidden.
        // Collapsing such a root into its ancestor (because the ancestor's
        // walk "covers" it) would silently un-opt-in it, since the
        // ancestor's own walk skips hidden entries by default.
        let dir = tempfile::tempdir().unwrap();
        touch(&dir, "visible.jpg");
        touch(&dir, ".hidden.jpg");

        let opts = ScopeOptions::default_scope(); // include_hidden: false
        let result = enumerate(
            &[dir.path().to_path_buf(), dir.path().join(".hidden.jpg")],
            &opts,
        );

        assert_eq!(
            candidate_names(&result),
            vec![".hidden.jpg", "visible.jpg"],
            "an explicitly-named hidden root must survive dedup even though \
             it is nested inside the other given root"
        );
    }

    #[test]
    fn hidden_nested_directory_root_named_explicitly_is_not_dropped() {
        // Same as above, but the explicitly-named nested root is a hidden
        // *directory*, not a hidden file — the whole subtree under it must
        // still be reachable.
        let dir = tempfile::tempdir().unwrap();
        touch(&dir, "visible.jpg");
        touch(&dir, ".cache/x.jpg");
        let hidden_dir = dir.path().join(".cache");

        let opts = ScopeOptions::default_scope();
        let result = enumerate(&[dir.path().to_path_buf(), hidden_dir], &opts);

        assert_eq!(
            candidate_names(&result),
            vec!["visible.jpg", "x.jpg"],
            "an explicitly-named hidden directory root must survive dedup, \
             and its contents must still be enumerated"
        );
    }

    #[test]
    fn duplicate_nonexistent_root_still_reports_a_root_error() {
        // A root that cannot be canonicalized (doesn't exist) must not be
        // silently dropped by the dedup pass — the existing root-level
        // error path (ADR-0008 exit-code semantics) must still fire.
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("does-not-exist");

        let opts = ScopeOptions::default_scope();
        let result = enumerate(&[bad.clone(), bad], &opts);

        assert!(
            !result.root_errors.is_empty(),
            "a nonexistent root must still be reported as a root error"
        );
    }

    // ── Candidate metadata ────────────────────────────────────────────────────

    #[test]
    fn candidate_carries_correct_file_size() {
        // The Candidate.size field reflects the actual file size.
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sized.jpg");
        fs::write(&p, b"hello world").unwrap(); // 11 bytes

        let opts = ScopeOptions::default_scope();
        let result = enumerate(&[dir.path().to_path_buf()], &opts);

        assert_eq!(result.candidates.len(), 1);
        assert_eq!(result.candidates[0].size, 11, "size should be 11 bytes");
    }

    #[test]
    fn file_with_no_extension_is_excluded() {
        // A file with no extension is not in the allow-list and is skipped.
        let dir = tempfile::tempdir().unwrap();
        touch(&dir, "noext");
        touch(&dir, "has.jpg");

        let opts = ScopeOptions::default_scope();
        let result = enumerate(&[dir.path().to_path_buf()], &opts);

        let names = candidate_names(&result);
        assert_eq!(names, vec!["has.jpg"]);
    }

    // ── mtime threading ───────────────────────────────────────────────────────

    #[test]
    fn candidate_carries_mtime_from_metadata() {
        // mtime is threaded from the single metadata call into Candidate.mtime
        // (no extra stat). The value must be Some on a real, freshly-created
        // file on all supported platforms; None is only acceptable when the
        // platform does not expose mtime (e.g. Wasm).
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("with_mtime.jpg");
        fs::write(&p, b"data").unwrap();

        let opts = ScopeOptions::default_scope();
        let result = enumerate(&[dir.path().to_path_buf()], &opts);

        assert_eq!(result.candidates.len(), 1);
        // On all mainstream OS targets (macOS, Linux, Windows) mtime is
        // available, so Some is expected. The exact value is not asserted
        // because it depends on when the test runs.
        assert!(
            result.candidates[0].mtime.is_some(),
            "Candidate.mtime should be Some on a real freshly-created file; got None"
        );
    }

    #[test]
    fn candidate_mtime_is_whole_seconds() {
        // mtime is stored at whole-second resolution (ADR-0008 §keeper rule).
        // Write a file, record the time-before/after, check mtime is in that
        // range (i.e. is a real Unix timestamp, not sub-second noise).
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("seconds.jpg");
        let before = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        fs::write(&p, b"data").unwrap();
        let after = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let opts = ScopeOptions::default_scope();
        let result = enumerate(&[dir.path().to_path_buf()], &opts);

        assert_eq!(result.candidates.len(), 1);
        let mtime = result.candidates[0]
            .mtime
            .expect("mtime should be Some for a real file");
        assert!(
            mtime >= before && mtime <= after + 1,
            "mtime {mtime} should be within the write window [{before}, {after}]"
        );
    }

    // ── TASK-0317: progress advances during enumeration ────────────────────

    #[test]
    fn enumerate_with_progress_advances_once_per_candidate_found() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir, "a.jpg");
        touch(&dir, "b.jpg");
        touch(&dir, "c.jpg");
        touch(&dir, "not-an-image.txt"); // excluded by the extension filter

        let opts = ScopeOptions::default_scope();
        let sink = CountingSink::default();
        let result = enumerate_with_progress(&[dir.path().to_path_buf()], &opts, &sink);

        assert_eq!(result.candidates.len(), 3, "3 images pass the scope rules");
        assert_eq!(
            sink.advanced.load(Ordering::SeqCst),
            3,
            "advance() must be called exactly once per accepted candidate, \
             not per directory entry visited (the excluded .txt file must \
             not advance the counter)"
        );
    }

    #[test]
    fn plain_enumerate_still_works_without_a_progress_sink() {
        // enumerate() must stay usable by every existing call site/test
        // unchanged — it is enumerate_with_progress with a NoProgress sink,
        // not a breaking signature change.
        let dir = tempfile::tempdir().unwrap();
        touch(&dir, "a.jpg");
        let opts = ScopeOptions::default_scope();

        let result = enumerate(&[dir.path().to_path_buf()], &opts);

        assert_eq!(result.candidates.len(), 1);
    }
}
