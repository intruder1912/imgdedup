//! Opening the `--output` file without destroying anything unasked
//! (TASK-0342; amends ADR-0018 §6).
//!
//! A deduplication tool must not destroy a file the user did not ask it to
//! destroy, and `--output` is a path typed by hand next to a tree of
//! irreplaceable photos. So the target is classified before it is opened:
//!
//! | The target…                  | What happens                                   |
//! |------------------------------|------------------------------------------------|
//! | does not exist               | created                                        |
//! | is an empty file             | written; nothing to keep                       |
//! | is an earlier run record     | moved to `<file>.bak`, then written            |
//! | is any other file            | refused, unless `overwrite`; then as above     |
//! | is a directory / unopenable  | refused                                        |
//!
//! One generation of backup is kept. An existing `<file>.bak` is replaced
//! when it is empty or itself a run record, and blocks the run otherwise —
//! with or without `overwrite`: that flag is permission to replace the output
//! file, not some other file that happens to sit at the backup name.
//!
//! Everything here runs before anything is scanned, so a refusal costs the
//! user nothing but a re-run.

use crate::safe_text::escape_control_chars;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// The first bytes of a run record: the human format and the NDJSON format
/// (`record.rs`). A file that starts with either was written by this tool.
const RECORD_MARKERS: [&[u8]; 2] = [b"# imgdedup ", b"{\"type\":\"header\""];

/// The open record file, and where the previous content went, if any.
#[derive(Debug)]
pub struct Opened {
    pub file: File,
    /// Where the previous content was kept, if there was any. Not shown to
    /// the user today (the guide documents the `.bak` rule); kept so the
    /// tests can pin it and a notice can be added without a new API.
    #[cfg_attr(not(test), allow(dead_code))]
    pub backed_up_to: Option<PathBuf>,
}

/// Why the `--output` file was not opened. Nothing on disk has been changed
/// when one of these is returned, except that a failed backup *copy* (the
/// symlink case) may leave a partial `<file>.bak` (`BackupFailed`).
#[derive(Debug)]
pub enum Refusal {
    /// The target exists, is not empty and is not a record; no flag given.
    NotARecord { path: PathBuf, backup: PathBuf },
    /// The `.bak` name is taken by something that is not a record.
    BackupBlocked { path: PathBuf, backup: PathBuf },
    /// The previous content could not be moved or copied to the backup, so
    /// the target was not replaced (e.g. a writable record in a directory the
    /// user cannot write to).
    BackupFailed {
        path: PathBuf,
        backup: PathBuf,
        error: io::Error,
    },
    /// The target could not be opened.
    Io { path: PathBuf, error: io::Error },
}

impl Refusal {
    /// The complete `error: …` line (no trailing newline), with paths and
    /// the OS reason escaped.
    pub fn message(&self) -> String {
        let show = |p: &Path| escape_control_chars(&p.display().to_string()).into_owned();
        match self {
            Refusal::NotARecord { path, backup } => format!(
                "error: output file {} already exists and is not an imgdedup run record; \
                 choose another path, or pass --overwrite-output to replace it (it will be \
                 kept as {})",
                show(path),
                show(backup)
            ),
            Refusal::BackupBlocked { path, backup } => format!(
                "error: cannot back up {}: {} already exists and is not an imgdedup run \
                 record; move {} away or choose another output path",
                show(path),
                show(backup),
                show(backup)
            ),
            Refusal::BackupFailed {
                path,
                backup,
                error,
            } => format!(
                "error: cannot back up {} to {}: {}",
                show(path),
                show(backup),
                escape_control_chars(&error.to_string())
            ),
            Refusal::Io { path, error } => format!(
                "error: cannot open output file {}: {}",
                show(path),
                escape_control_chars(&error.to_string())
            ),
        }
    }
}

/// `run.txt` → `run.txt.bak`: the full name plus `.bak`, never a replaced
/// extension, so two outputs that differ only in extension cannot share a
/// backup.
pub fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".bak");
    PathBuf::from(name)
}

/// What an existing regular file holds, as far as replacing it goes.
#[derive(Debug, PartialEq, Eq)]
enum Content {
    Empty,
    Record,
    Foreign,
}

/// Classify by the first bytes, read through `path` (following a symlink).
/// A file that cannot be read is `Foreign`: when in doubt, do not replace.
fn classify(path: &Path) -> Content {
    let longest = RECORD_MARKERS.iter().map(|m| m.len()).max().unwrap_or(0);
    let mut head = Vec::with_capacity(longest);
    let read = File::open(path).and_then(|f| f.take(longest as u64).read_to_end(&mut head));
    match read {
        Err(_) => Content::Foreign,
        Ok(_) if head.is_empty() => Content::Empty,
        Ok(_) if RECORD_MARKERS.iter().any(|m| head.starts_with(m)) => Content::Record,
        Ok(_) => Content::Foreign,
    }
}

/// Open `path` for the run record, backing up whatever it replaces.
///
/// `overwrite` is `--overwrite-output`: permission to replace a file that is
/// not a run record.
pub fn open(path: &Path, overwrite: bool) -> Result<Opened, Refusal> {
    let io_refusal = |error: io::Error| Refusal::Io {
        path: path.to_owned(),
        error,
    };

    // A missing file is created with create-new semantics, so a file that
    // appears between this check and the open is classified rather than
    // clobbered. One retry is enough: a second race is reported.
    for _ in 0..2 {
        match fs::symlink_metadata(path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                match OpenOptions::new().write(true).create_new(true).open(path) {
                    Ok(file) => {
                        return Ok(Opened {
                            file,
                            backed_up_to: None,
                        });
                    }
                    Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(e) => return Err(io_refusal(e)),
                }
            }
            Err(e) => return Err(io_refusal(e)),
            Ok(meta) => return open_existing(path, &meta, overwrite),
        }
    }
    Err(io_refusal(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "the file kept appearing and disappearing",
    )))
}

fn open_existing(path: &Path, meta: &fs::Metadata, overwrite: bool) -> Result<Opened, Refusal> {
    let io_refusal = |error: io::Error| Refusal::Io {
        path: path.to_owned(),
        error,
    };
    let is_symlink = meta.file_type().is_symlink();

    // What the path leads to must be a regular file. A directory, a dangling
    // symlink, a device: let the OS say why it cannot be the record.
    let target_is_file = fs::metadata(path).map(|m| m.is_file());
    if !matches!(target_is_file, Ok(true)) {
        return match OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
        {
            // Writable but not a regular file (a FIFO, `/dev/full`, …), or a
            // dangling symlink whose target is created by this open: nothing
            // is replaced, so there is nothing to protect or back up.
            Ok(file) => Ok(Opened {
                file,
                backed_up_to: None,
            }),
            Err(e) => Err(io_refusal(e)),
        };
    }

    let truncate = || {
        OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(path)
            .map_err(io_refusal)
    };

    let content = classify(path);
    if content == Content::Empty {
        return Ok(Opened {
            file: truncate()?,
            backed_up_to: None,
        });
    }

    let backup = backup_path(path);
    if content == Content::Foreign && !overwrite {
        return Err(Refusal::NotARecord {
            path: path.to_owned(),
            backup,
        });
    }

    // One generation: an existing backup is replaced only when replacing it
    // loses nothing that is not ours.
    let backup_exists = match fs::symlink_metadata(&backup) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => false,
        Ok(m) if m.is_file() && classify(&backup) != Content::Foreign => true,
        _ => {
            return Err(Refusal::BackupBlocked {
                path: path.to_owned(),
                backup,
            });
        }
    };

    let backup_failed = |error: io::Error| Refusal::BackupFailed {
        path: path.to_owned(),
        backup: backup.clone(),
        error,
    };
    let file = if is_symlink {
        // Keep the link where it is, pointing where it did: copy the content
        // out, then rewrite the file the link leads to.
        fs::copy(path, &backup).map_err(backup_failed)?;
        truncate()?
    } else {
        // Windows refuses to rename onto an existing file.
        if backup_exists && cfg!(windows) {
            fs::remove_file(&backup).map_err(backup_failed)?;
        }
        fs::rename(path, &backup).map_err(backup_failed)?;
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(io_refusal)?
    };
    Ok(Opened {
        file,
        backed_up_to: Some(backup),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    const HUMAN: &[u8] = b"# imgdedup 0.1.0 run record\n# started: x\n";
    const NDJSON: &[u8] = b"{\"type\":\"header\",\"elapsed_ms\":0}\n";
    const PHOTO: &[u8] = b"\x89PNG\r\n\x1a\nnot really, but not ours either";

    fn dir() -> TempDir {
        TempDir::new().unwrap()
    }

    /// Open, write `new` through the returned handle, and return the result.
    fn open_and_write(path: &Path, overwrite: bool, new: &[u8]) -> Result<Opened, Refusal> {
        let mut opened = open(path, overwrite)?;
        opened.file.write_all(new).unwrap();
        Ok(opened)
    }

    #[test]
    fn backup_path_appends_bak_to_the_full_name() {
        assert_eq!(
            backup_path(Path::new("a/run.txt")),
            Path::new("a/run.txt.bak")
        );
        assert_eq!(backup_path(Path::new("run")), Path::new("run.bak"));
    }

    #[test]
    fn a_missing_file_is_created_without_backup() {
        let d = dir();
        let p = d.path().join("run.txt");
        let opened = open_and_write(&p, false, b"new").unwrap();
        assert_eq!(opened.backed_up_to, None);
        assert_eq!(fs::read(&p).unwrap(), b"new");
        assert!(!backup_path(&p).exists());
    }

    #[test]
    fn an_empty_file_is_written_without_backup() {
        let d = dir();
        let p = d.path().join("run.txt");
        fs::write(&p, b"").unwrap();
        let opened = open_and_write(&p, false, b"new").unwrap();
        assert_eq!(opened.backed_up_to, None);
        assert_eq!(fs::read(&p).unwrap(), b"new");
        assert!(!backup_path(&p).exists());
    }

    #[test]
    fn a_human_record_is_moved_to_bak() {
        let d = dir();
        let p = d.path().join("run.txt");
        fs::write(&p, HUMAN).unwrap();
        let opened = open_and_write(&p, false, b"new").unwrap();
        assert_eq!(opened.backed_up_to, Some(backup_path(&p)));
        assert_eq!(fs::read(&p).unwrap(), b"new");
        assert_eq!(fs::read(backup_path(&p)).unwrap(), HUMAN);
    }

    #[test]
    fn an_ndjson_record_is_moved_to_bak() {
        let d = dir();
        let p = d.path().join("run.ndjson");
        fs::write(&p, NDJSON).unwrap();
        let opened = open_and_write(&p, false, b"new").unwrap();
        assert_eq!(opened.backed_up_to, Some(backup_path(&p)));
        assert_eq!(fs::read(backup_path(&p)).unwrap(), NDJSON);
    }

    #[test]
    fn a_foreign_file_is_refused_and_untouched() {
        let d = dir();
        let p = d.path().join("img.png");
        fs::write(&p, PHOTO).unwrap();
        let refusal = open(&p, false).unwrap_err();
        assert!(matches!(refusal, Refusal::NotARecord { .. }), "{refusal:?}");
        assert_eq!(fs::read(&p).unwrap(), PHOTO);
        assert!(!backup_path(&p).exists());
    }

    #[test]
    fn a_foreign_file_with_overwrite_is_moved_to_bak() {
        let d = dir();
        let p = d.path().join("img.png");
        fs::write(&p, PHOTO).unwrap();
        let opened = open_and_write(&p, true, b"new").unwrap();
        assert_eq!(opened.backed_up_to, Some(backup_path(&p)));
        assert_eq!(fs::read(&p).unwrap(), b"new");
        assert_eq!(fs::read(backup_path(&p)).unwrap(), PHOTO);
    }

    #[test]
    fn an_existing_record_bak_is_replaced() {
        let d = dir();
        let p = d.path().join("run.txt");
        fs::write(backup_path(&p), b"# imgdedup generation 1\n").unwrap();
        fs::write(&p, b"# imgdedup generation 2\n").unwrap();
        open_and_write(&p, false, b"# imgdedup generation 3\n").unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"# imgdedup generation 3\n");
        assert_eq!(
            fs::read(backup_path(&p)).unwrap(),
            b"# imgdedup generation 2\n"
        );
    }

    #[test]
    fn an_empty_bak_is_replaced() {
        let d = dir();
        let p = d.path().join("run.txt");
        fs::write(backup_path(&p), b"").unwrap();
        fs::write(&p, HUMAN).unwrap();
        open_and_write(&p, false, b"new").unwrap();
        assert_eq!(fs::read(backup_path(&p)).unwrap(), HUMAN);
    }

    #[test]
    fn a_foreign_bak_refuses_even_with_overwrite() {
        let d = dir();
        let p = d.path().join("run.txt");
        fs::write(backup_path(&p), PHOTO).unwrap();
        for (content, overwrite) in [(HUMAN, false), (HUMAN, true), (PHOTO, true)] {
            fs::write(&p, content).unwrap();
            let refusal = open(&p, overwrite).unwrap_err();
            assert!(
                matches!(refusal, Refusal::BackupBlocked { .. }),
                "{refusal:?}"
            );
            assert_eq!(fs::read(&p).unwrap(), content, "target untouched");
            assert_eq!(fs::read(backup_path(&p)).unwrap(), PHOTO, "bak untouched");
        }
    }

    #[test]
    fn a_directory_at_the_backup_name_refuses() {
        let d = dir();
        let p = d.path().join("run.txt");
        fs::create_dir(backup_path(&p)).unwrap();
        fs::write(&p, HUMAN).unwrap();
        let refusal = open(&p, false).unwrap_err();
        assert!(
            matches!(refusal, Refusal::BackupBlocked { .. }),
            "{refusal:?}"
        );
        assert_eq!(fs::read(&p).unwrap(), HUMAN);
        assert!(backup_path(&p).is_dir());
    }

    #[test]
    fn a_file_shorter_than_the_marker_is_not_a_record() {
        // A record cut short inside its first line (a kill at the worst
        // moment) is not recognizable; the user is asked, nothing is lost.
        let d = dir();
        let p = d.path().join("run.txt");
        fs::write(&p, b"# img").unwrap();
        let refusal = open(&p, false).unwrap_err();
        assert!(matches!(refusal, Refusal::NotARecord { .. }), "{refusal:?}");
        assert_eq!(fs::read(&p).unwrap(), b"# img");
    }

    #[test]
    fn a_directory_target_is_an_io_refusal() {
        let d = dir();
        let refusal = open(d.path(), true).unwrap_err();
        assert!(matches!(refusal, Refusal::Io { .. }), "{refusal:?}");
        assert!(
            refusal
                .message()
                .starts_with("error: cannot open output file "),
            "{}",
            refusal.message()
        );
    }

    #[test]
    fn a_missing_parent_is_an_io_refusal() {
        let d = dir();
        let refusal = open(&d.path().join("nope/run.txt"), false).unwrap_err();
        assert!(matches!(refusal, Refusal::Io { .. }), "{refusal:?}");
    }

    #[test]
    fn messages_escape_control_characters() {
        let path = PathBuf::from("run\n# forged\u{1b}[2J.txt");
        let backup = backup_path(&path);
        for refusal in [
            Refusal::NotARecord {
                path: path.clone(),
                backup: backup.clone(),
            },
            Refusal::BackupBlocked {
                path: path.clone(),
                backup,
            },
            Refusal::BackupFailed {
                path: path.clone(),
                backup: backup_path(&path),
                error: io::Error::other("bad\u{1b}[2J"),
            },
            Refusal::Io {
                path,
                error: io::Error::other("bad\u{1b}[2J"),
            },
        ] {
            let message = refusal.message();
            assert_eq!(message.lines().count(), 1, "{message:?}");
            assert!(!message.contains('\u{1b}'), "{message:?}");
            assert!(message.starts_with("error: "), "{message:?}");
        }
    }

    #[test]
    fn the_refusal_message_names_the_flag_and_the_backup() {
        let refusal = Refusal::NotARecord {
            path: PathBuf::from("photos/img.jpg"),
            backup: PathBuf::from("photos/img.jpg.bak"),
        };
        assert_eq!(
            refusal.message(),
            "error: output file photos/img.jpg already exists and is not an imgdedup run \
             record; choose another path, or pass --overwrite-output to replace it (it \
             will be kept as photos/img.jpg.bak)"
        );
    }

    #[test]
    fn the_blocked_backup_message_says_what_to_do() {
        let refusal = Refusal::BackupBlocked {
            path: PathBuf::from("run.txt"),
            backup: PathBuf::from("run.txt.bak"),
        };
        assert_eq!(
            refusal.message(),
            "error: cannot back up run.txt: run.txt.bak already exists and is not an \
             imgdedup run record; move run.txt.bak away or choose another output path"
        );
    }

    /// A record the user can open, in a directory they cannot write to: the
    /// backup cannot be made, so the record is not replaced — and the message
    /// says it was the backup that failed, not the open.
    #[cfg(unix)]
    #[test]
    fn a_backup_that_cannot_be_made_refuses_and_says_so() {
        use std::os::unix::fs::PermissionsExt;
        let d = dir();
        let locked = d.path().join("logs");
        fs::create_dir(&locked).unwrap();
        let p = locked.join("run.txt");
        fs::write(&p, HUMAN).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();

        let refusal = open(&p, false).unwrap_err();

        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(
            matches!(refusal, Refusal::BackupFailed { .. }),
            "{refusal:?}"
        );
        assert!(
            refusal
                .message()
                .starts_with("error: cannot back up run.txt to run.txt.bak: ")
                || refusal.message().contains("/run.txt to "),
            "{}",
            refusal.message()
        );
        assert_eq!(fs::read(&p).unwrap(), HUMAN, "the record is intact");
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_record_is_backed_up_by_copy_and_the_link_survives() {
        let d = dir();
        let real = d.path().join("real.txt");
        let link = d.path().join("run.txt");
        fs::write(&real, HUMAN).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let opened = open_and_write(&link, false, b"new").unwrap();

        assert_eq!(opened.backed_up_to, Some(backup_path(&link)));
        assert_eq!(fs::read_link(&link).unwrap(), real, "the link survives");
        assert_eq!(fs::read(&real).unwrap(), b"new", "written through the link");
        let bak = backup_path(&link);
        assert!(fs::symlink_metadata(&bak).unwrap().is_file());
        assert_eq!(fs::read(&bak).unwrap(), HUMAN);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_to_a_foreign_file_is_refused_and_untouched() {
        let d = dir();
        let real = d.path().join("img.png");
        let link = d.path().join("run.txt");
        fs::write(&real, PHOTO).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let refusal = open(&link, false).unwrap_err();
        assert!(matches!(refusal, Refusal::NotARecord { .. }), "{refusal:?}");
        assert_eq!(fs::read(&real).unwrap(), PHOTO);
    }
}
