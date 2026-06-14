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
    /// Byte size of the file, from directory-walk metadata.
    pub size: u64,
}

impl Candidate {
    fn new(path: PathBuf, size: u64) -> Self {
        Self { path, size }
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
#[derive(Debug, Default)]
pub struct CandidateSet {
    /// Files that passed all scope filters and are ready for the next pipeline
    /// stage (magic-byte confirmation in TASK-0303).
    pub candidates: Vec<Candidate>,
    /// Non-fatal per-entry errors collected during traversal (NFR-6).
    /// These do not abort enumeration; they are reported by TASK-0307.
    pub warnings: Vec<EnumerateWarning>,
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
pub fn enumerate(paths: &[PathBuf], opts: &ScopeOptions) -> CandidateSet {
    let mut result = CandidateSet::default();

    for root in paths {
        enumerate_root(root, opts, &mut result);
    }

    result
}

/// Walk a single root path and accumulate candidates/warnings into `out`.
fn enumerate_root(root: &Path, opts: &ScopeOptions, out: &mut CandidateSet) {
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
                out.warnings
                    .push(EnumerateWarning::new(path, e.to_string()));
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

                // Collect the size from the entry's metadata.
                let size = match entry.metadata() {
                    Ok(m) => m.len(),
                    Err(e) => {
                        out.warnings.push(EnumerateWarning::new(
                            path.clone(),
                            format!("could not read metadata: {e}"),
                        ));
                        continue;
                    }
                };

                out.candidates.push(Candidate::new(path, size));
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
    use tempfile::TempDir;

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

    // ── AC-15 (FR-1): recursion / no-recurse ─────────────────────────────────

    #[test]
    fn recurse_by_default_finds_nested_files() {
        // AC-15: with default options, files in subdirectories are enumerated.
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
        let names = candidate_names(&result);
        assert_eq!(
            names,
            vec!["nested.jpg", "top.jpg", "verydeep.jpg"],
            "expected all three nested files; got: {names:?}"
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
            vec!["a.jpg", "b.jpeg", "c.png", "d.gif", "e.webp", "f.bmp", "g.tif", "h.tiff"]
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
    fn unreadable_entry_produces_warning_and_rest_completes() {
        // AC-10 (NFR-6): a permission-denied entry is skipped with a warning;
        // enumeration of the rest of the directory completes.
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

        // The permission error must have produced at least one warning.
        assert!(
            !result.warnings.is_empty(),
            "expected at least one warning for the unreadable entry"
        );
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
}
