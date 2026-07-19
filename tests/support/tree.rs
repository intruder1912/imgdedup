//! Golden-tree materialization (TASK-0313).
//!
//! Turns declarative file/hard-link specs into an on-disk tree under a scan
//! root. Every mutable input the pipeline observes — relative path, content
//! bytes, mtime, filesystem identity — is controlled by the spec, so a
//! scenario's expected `--json` manifest can be derived by hand and asserted
//! byte-exactly (mtimes included).

use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

/// One regular file in a golden tree.
pub struct FileSpec {
    /// Path relative to the scan root, always `/`-separated.
    pub rel_path: String,
    /// Exact file content (use `images::image_bytes` for valid images).
    pub bytes: Vec<u8>,
    /// Modification time as non-negative seconds since the Unix epoch;
    /// surfaces verbatim in the JSON `mtime` fields.
    pub mtime: i64,
}

/// One hard link in a golden tree. Created after all files exist; shares the
/// target's inode and therefore its mtime.
pub struct LinkSpec {
    /// Existing file (a `FileSpec::rel_path`), relative to the scan root.
    pub existing: String,
    /// New link path to create, relative to the scan root.
    pub new: String,
}

/// Materialize `files` and `links` under `root`. Parent directories are
/// created as needed; mtimes are applied last so no later write disturbs them.
pub fn materialize(root: &Path, files: &[FileSpec], links: &[LinkSpec]) {
    for f in files {
        let path = root.join(&f.rel_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent dirs must be creatable");
        }
        fs::write(&path, &f.bytes).expect("golden file must be writable");
    }
    for l in links {
        let new = root.join(&l.new);
        if let Some(parent) = new.parent() {
            fs::create_dir_all(parent).expect("parent dirs must be creatable");
        }
        fs::hard_link(root.join(&l.existing), &new).expect("hard link must be creatable");
    }
    for f in files {
        let path = root.join(&f.rel_path);
        let file = fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .expect("golden file must reopen for mtime");
        file.set_modified(epoch_secs(f.mtime))
            .expect("mtime must be settable");
    }
}

/// Convert non-negative epoch seconds to a [`SystemTime`].
fn epoch_secs(secs: i64) -> SystemTime {
    let secs = u64::try_from(secs).expect("golden mtimes are non-negative");
    SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
}
