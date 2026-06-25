//! Magic-byte image confirmation — the second pipeline stage (ADR-0008, FR-3,
//! NFR-2).
//!
//! This module provides a **pure predicate** over a file's content: it reads
//! only a small, bounded header prefix (at most [`HEADER_LEN`] bytes) and uses
//! the [`infer`] crate to determine whether that header matches a known image
//! format. Files that pass are confirmed images; files that fail are collected
//! as [`SniffWarning`] records and excluded from all downstream stages.
//!
//! ## Pipeline position (ADR-0008)
//!
//! ```text
//! enumerate candidates (TASK-0302)
//!   → magic-byte confirm each is an image (this module, TASK-0303)
//!   → group by exact byte size (TASK-0304)
//!   → …
//! ```
//!
//! ## Confirmation semantics
//!
//! Confirmation accepts **any image format that `infer` recognizes from the
//! header bytes** — this is deliberately broader than the default extension
//! allow-list. For example, a HEIC file misnamed `.jpg` would pass confirmation
//! because `infer` identifies it as an image by its magic bytes, while a text
//! file named `.jpg` is rejected. This broader acceptance is safe because:
//!
//! - Dedup identity is determined by content hash and byte-for-byte verification
//!   (ADR-0008), not by the format label — so the specific image format is
//!   irrelevant to grouping.
//! - The byte-verify stage (TASK-0304) is the sole authority for authorizing
//!   removal; only byte-identical files are ever removed.
//! - NFR-2 (no collateral damage to non-images) still holds because only genuine
//!   image files pass the magic-byte check, and only byte-identical copies are
//!   ever proposed for removal.
//!
//! ## Confirmed formats
//!
//! The formats below cover the default extension allow-list (from `enumerate.rs`).
//! `infer` may additionally recognize other image formats (e.g. HEIC, AVIF) that
//! are not in the default extension allow-list; such files pass confirmation if
//! they arrive via `--ext`.
//!
//! | Format | Magic bytes |
//! |--------|-------------|
//! | JPEG   | `FF D8 FF` |
//! | PNG    | `89 50 4E 47 0D 0A 1A 0A` |
//! | GIF87a | `47 49 46 38 37 61` (`GIF87a`) |
//! | GIF89a | `47 49 46 38 39 61` (`GIF89a`) |
//! | WebP   | `52 49 46 46 ?? ?? ?? ?? 57 45 42 50` (`RIFF....WEBP`) |
//! | BMP    | `42 4D` |
//! | TIFF (little-endian) | `49 49 2A 00` |
//! | TIFF (big-endian)    | `4D 4D 00 2A` |
//!
//! **TIFF caveat:** `infer`'s TIFF matcher inspects bytes 8–9 to exclude Canon
//! CR2 files (which share the TIFF magic bytes but have `CR` at offset 8). As a
//! result, unusual-but-valid TIFFs that happen to carry `CR` at bytes 8–9 will
//! not be confirmed. This is fail-closed and NFR-2-safe: a skipped file is simply
//! never acted on.
//!
//! ## Custom extensions and unrecognizable formats
//!
//! When `--ext` includes an extension that `infer` cannot confirm (e.g.
//! `.raw`, `.cr2`), the file will **never** pass `confirm_image` regardless of
//! its actual content. The file is silently dropped before grouping — it appears
//! as a [`SniffWarning`] with reason `"content not recognized as a supported
//! image format"`. This is the safe direction (NFR-2): it is better to skip an
//! unrecognized file than to act on it.
//!
//! TASK-0307 will surface these warnings in the human-readable report.

use crate::enumerate::Candidate;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

// ── Constants ─────────────────────────────────────────────────────────────────

/// Number of bytes read from the start of each file for magic-byte detection.
///
/// The longest signature we need to detect is the WebP container, which
/// requires 12 bytes (`RIFF` + 4-byte size + `WEBP`). 64 bytes gives comfortable
/// headroom for all current and plausible future formats while keeping the read
/// tiny relative to any realistic image file. The full file is **never** read by
/// this module.
pub const HEADER_LEN: usize = 64;

// ── Public types ──────────────────────────────────────────────────────────────

/// A record describing a candidate that failed magic-byte confirmation.
///
/// Mirrors the shape of [`enumerate::EnumerateWarning`] but is defined here so
/// that `sniff` stays decoupled from `enumerate`'s internals. Surfaced by the
/// report stage (TASK-0307).
#[derive(Debug, Clone)]
pub struct SniffWarning {
    /// The path of the file that failed confirmation.
    pub path: PathBuf,
    /// Human-readable reason the file was rejected.
    pub reason: String,
}

impl SniffWarning {
    fn new(path: PathBuf, reason: impl Into<String>) -> Self {
        Self {
            path,
            reason: reason.into(),
        }
    }
}

/// A candidate that passed magic-byte confirmation, carried forward to the
/// dedup engine (TASK-0304).
///
/// Holds the `size` and `mtime` that [`enumerate`] already stat'd so the
/// engine's first stage (size pre-grouping) and the keeper rule (TASK-0305)
/// do not have to re-stat the file — avoiding a silent double-stat and a small
/// TOCTOU widening. Decoupled from [`enumerate::Candidate`] so the
/// confirmed-set carrier is an explicit stage boundary rather than a reused
/// traversal type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedFile {
    /// Path to the confirmed image file.
    pub path: PathBuf,
    /// Byte size of the file, threaded through from enumeration.
    pub size: u64,
    /// Modification time in whole Unix seconds, threaded through from
    /// enumeration. `None` when unavailable (see [`Candidate::mtime`]).
    pub mtime: Option<i64>,
}

impl ConfirmedFile {
    fn new(path: PathBuf, size: u64, mtime: Option<i64>) -> Self {
        Self { path, size, mtime }
    }
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Confirm that the file at `path` is a genuine image by its magic-byte
/// signature.
///
/// Reads at most [`HEADER_LEN`] bytes from the start of the file and queries
/// the [`infer`] crate. Returns `true` if and only if `infer` identifies the
/// header as one of the supported image formats (JPEG, PNG, GIF, WebP, BMP,
/// TIFF). Returns `false` for any I/O error, an unrecognized format, or a
/// non-image MIME type (e.g. a text file named `.jpg`).
///
/// This function **never reads the whole file** — the bounded read is a hard
/// invariant (NFR-4, ADR-0008 "Magic-byte coverage"). A file with a valid
/// header followed by arbitrary junk will still confirm as `true`; the byte-
/// for-byte verification in TASK-0304 is the authority for content equality.
///
/// # When `infer` cannot confirm
///
/// If the file's format is not in `infer`'s built-in signature table (for
/// example a camera raw format added via `--ext`), this function returns
/// `false`. The caller should collect the file as a [`SniffWarning`]; see the
/// module-level note on custom extensions.
///
/// The pipeline integrates through [`filter_candidates`] (which carries `size`
/// through); this single-path predicate is retained as public API and exercised
/// by the unit tests, hence the `dead_code` allowance for the non-test build.
#[allow(dead_code)]
pub fn confirm_image(path: &Path) -> bool {
    match read_header(path) {
        Ok(buf) => is_image_header(&buf),
        Err(_) => false,
    }
}

/// Filter a list of candidate paths through magic-byte confirmation.
///
/// Returns a tuple `(confirmed, warnings)`:
/// - `confirmed`: paths whose header identified them as a supported image format.
/// - `warnings`: [`SniffWarning`] records for every path that failed confirmation,
///   whether due to an I/O error or a non-matching header.
///
/// Files are tested in the order given; confirmed files are returned in the same
/// relative order. Warnings preserve the order of the rejected files.
///
/// This function is the integration point for TASK-0304: hand it the
/// `Vec<Candidate>` from enumeration and consume the `confirmed` set downstream.
/// The `size` that enumeration already stat'd is threaded through into each
/// [`ConfirmedFile`] so the engine's size-grouping stage need not re-stat.
///
/// The header is read **once per file**; both the confirm decision and the
/// rejection reason are derived from that single read.
pub fn filter_candidates(candidates: Vec<Candidate>) -> (Vec<ConfirmedFile>, Vec<SniffWarning>) {
    let mut confirmed = Vec::new();
    let mut warnings = Vec::new();

    for candidate in candidates {
        let Candidate { path, size, mtime } = candidate;
        match read_header(&path) {
            Err(e) => {
                warnings.push(SniffWarning::new(
                    path,
                    format!("could not read file header: {e}"),
                ));
            }
            Ok(buf) => {
                if is_image_header(&buf) {
                    confirmed.push(ConfirmedFile::new(path, size, mtime));
                } else {
                    let reason = if buf.is_empty() {
                        "file is empty".to_owned()
                    } else {
                        "content not recognized as a supported image format".to_owned()
                    };
                    warnings.push(SniffWarning::new(path, reason));
                }
            }
        }
    }

    (confirmed, warnings)
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Read at most [`HEADER_LEN`] bytes from the start of `path`.
///
/// Returns fewer bytes if the file is shorter than `HEADER_LEN` — the `infer`
/// crate handles partial headers gracefully for all supported formats (the
/// minimum signature it needs is 2 bytes for BMP).
fn read_header(path: &Path) -> std::io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let mut buf = vec![0u8; HEADER_LEN];
    let n = file.read(&mut buf)?;
    buf.truncate(n);
    Ok(buf)
}

/// Return `true` if `buf` is recognized by `infer` as any image type.
///
/// Delegates directly to [`infer::is_image`], which returns `true` for any
/// format whose magic-byte signature `infer` has a built-in matcher for and
/// whose MIME type is `"image/*"`. No additional allow-list filtering is applied
/// here; see the module-level doc on confirmation semantics for the rationale.
fn is_image_header(buf: &[u8]) -> bool {
    infer::is_image(buf)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // ── Fixtures ──────────────────────────────────────────────────────────────

    /// Write `bytes` to a file named `name` inside `dir` and return its path.
    fn write_fixture(dir: &TempDir, name: &str, bytes: &[u8]) -> PathBuf {
        let path = dir.path().join(name);
        fs::write(&path, bytes).unwrap();
        path
    }

    /// Wrap a path in a [`Candidate`] carrying its real on-disk size (0 if the
    /// file does not exist) and mtime, mirroring what `enumerate` produces.
    fn candidate(path: PathBuf) -> Candidate {
        let meta = fs::metadata(&path).ok();
        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
        let mtime = meta.as_ref().and_then(|m| {
            m.modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
        });
        Candidate { path, size, mtime }
    }

    // ── Format-detection tests (one per format) ───────────────────────────────

    #[test]
    fn jpeg_header_is_confirmed() {
        // FR-3, NFR-2: JPEG files start with FF D8 FF.
        // Use a minimal valid JPEG SOI (start-of-image) marker.
        let dir = tempfile::tempdir().unwrap();
        let path = write_fixture(
            &dir,
            "photo.jpg",
            &[
                0xFF, 0xD8, 0xFF, 0xE0, // SOI + APP0 marker
                0x00, 0x10, // APP0 length
                b'J', b'F', b'I', b'F', 0x00, // "JFIF\0"
                0x01, 0x01, // version
                0x00, // aspect ratio units
                0x00, 0x01, 0x00, 0x01, // Xdensity, Ydensity
                0x00, 0x00, // thumbnail size
            ],
        );
        assert!(
            confirm_image(&path),
            "JPEG file with valid FF D8 FF header must be confirmed"
        );
    }

    #[test]
    fn png_header_is_confirmed() {
        // FR-3, NFR-2: PNG files start with 89 50 4E 47 0D 0A 1A 0A.
        let dir = tempfile::tempdir().unwrap();
        let path = write_fixture(
            &dir,
            "image.png",
            &[
                0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG magic
                // Minimal IHDR chunk (required after the PNG signature)
                0x00, 0x00, 0x00, 0x0D, // chunk length = 13
                b'I', b'H', b'D', b'R', // chunk type
                0x00, 0x00, 0x00, 0x01, // width = 1
                0x00, 0x00, 0x00, 0x01, // height = 1
                0x08, 0x02, // bit depth = 8, colour type = RGB
                0x00, 0x00, 0x00, // compression, filter, interlace
            ],
        );
        assert!(
            confirm_image(&path),
            "PNG file with valid 89 50 4E 47 0D 0A 1A 0A header must be confirmed"
        );
    }

    #[test]
    fn gif87a_header_is_confirmed() {
        // FR-3, NFR-2: GIF87a files start with "GIF87a".
        let dir = tempfile::tempdir().unwrap();
        // Minimal GIF87a: header + logical screen descriptor
        let mut bytes = b"GIF87a".to_vec();
        bytes.extend_from_slice(&[
            0x01, 0x00, // logical screen width = 1
            0x01, 0x00, // logical screen height = 1
            0x00, // packed field
            0x00, // background color index
            0x00, // pixel aspect ratio
            0x3B, // trailer
        ]);
        let path = write_fixture(&dir, "anim.gif", &bytes);
        assert!(
            confirm_image(&path),
            "GIF87a file must be confirmed by its 'GIF87a' header"
        );
    }

    #[test]
    fn gif89a_header_is_confirmed() {
        // FR-3, NFR-2: GIF89a files start with "GIF89a".
        let dir = tempfile::tempdir().unwrap();
        let mut bytes = b"GIF89a".to_vec();
        bytes.extend_from_slice(&[
            0x01, 0x00, // logical screen width = 1
            0x01, 0x00, // logical screen height = 1
            0x00, // packed field
            0x00, // background color index
            0x00, // pixel aspect ratio
            0x3B, // trailer
        ]);
        let path = write_fixture(&dir, "anim89.gif", &bytes);
        assert!(
            confirm_image(&path),
            "GIF89a file must be confirmed by its 'GIF89a' header"
        );
    }

    #[test]
    fn webp_header_is_confirmed() {
        // FR-3, NFR-2: WebP files start with RIFF....WEBP.
        let dir = tempfile::tempdir().unwrap();
        // Minimal WebP RIFF container: "RIFF" + 4-byte little-endian file size
        // (not counting "RIFF" + size field itself) + "WEBP"
        let mut bytes = b"RIFF".to_vec();
        bytes.extend_from_slice(&[0x24, 0x00, 0x00, 0x00]); // file size (dummy)
        bytes.extend_from_slice(b"WEBP"); // format marker
                                          // Minimal VP8L chunk
        bytes.extend_from_slice(b"VP8L");
        bytes.extend_from_slice(&[0x0A, 0x00, 0x00, 0x00]); // chunk size
        bytes.extend_from_slice(&[0x2F, 0x00, 0x00, 0x00, 0x00, 0x88, 0x88, 0x08, 0x08, 0x00]);
        let path = write_fixture(&dir, "photo.webp", &bytes);
        assert!(
            confirm_image(&path),
            "WebP file with RIFF....WEBP header must be confirmed"
        );
    }

    #[test]
    fn bmp_header_is_confirmed() {
        // FR-3, NFR-2: BMP files start with 42 4D ("BM").
        let dir = tempfile::tempdir().unwrap();
        // Minimal BMP: file header (14 bytes) + DIB header (40 bytes BITMAPINFOHEADER)
        let mut bytes = vec![
            0x42, 0x4D, // "BM" magic
            0x3E, 0x00, 0x00, 0x00, // file size = 62 bytes
            0x00, 0x00, // reserved1
            0x00, 0x00, // reserved2
            0x36, 0x00, 0x00, 0x00, // pixel data offset = 54
        ];
        // BITMAPINFOHEADER (40 bytes)
        bytes.extend_from_slice(&[
            0x28, 0x00, 0x00, 0x00, // header size = 40
            0x01, 0x00, 0x00, 0x00, // width = 1
            0x01, 0x00, 0x00, 0x00, // height = 1
            0x01, 0x00, // color planes = 1
            0x18, 0x00, // bits per pixel = 24
            0x00, 0x00, 0x00, 0x00, // compression = BI_RGB
            0x03, 0x00, 0x00, 0x00, // image size
            0x13, 0x0B, 0x00, 0x00, // X pixels per meter
            0x13, 0x0B, 0x00, 0x00, // Y pixels per meter
            0x00, 0x00, 0x00, 0x00, // colors in table
            0x00, 0x00, 0x00, 0x00, // important color count
        ]);
        // Pixel data (1x1 pixel, 24-bit RGB, padded to 4-byte boundary)
        bytes.extend_from_slice(&[0xFF, 0x00, 0x00, 0x00]); // blue pixel + padding
        let path = write_fixture(&dir, "image.bmp", &bytes);
        assert!(
            confirm_image(&path),
            "BMP file with 42 4D ('BM') header must be confirmed"
        );
    }

    #[test]
    fn tiff_little_endian_header_is_confirmed() {
        // FR-3, NFR-2: TIFF little-endian files start with 49 49 2A 00 ("II*\0").
        // The `infer` crate requires buf.len() > 9 and checks buf[8]/buf[9] are
        // not the CR2 discriminator bytes (0x43/'C', 0x52/'R'). We pad bytes
        // 8-9 with non-CR2 values (0x00, 0x00) so they satisfy the TIFF check.
        let dir = tempfile::tempdir().unwrap();
        let bytes: &[u8] = &[
            0x49, 0x49, // "II" — little-endian byte order
            0x2A, 0x00, // magic number 42, little-endian
            0x08, 0x00, 0x00, 0x00, // offset to first IFD = 8
            0x00, 0x00, // bytes 8-9: not 'C','R' (CR2 discriminator)
            0x00, 0x00, // padding to satisfy len > 9
        ];
        let path = write_fixture(&dir, "scan.tif", bytes);
        assert!(
            confirm_image(&path),
            "TIFF little-endian file (49 49 2A 00) must be confirmed"
        );
    }

    #[test]
    fn tiff_big_endian_header_is_confirmed() {
        // FR-3, NFR-2: TIFF big-endian files start with 4D 4D 00 2A ("MM\0*").
        // The `infer` crate requires buf.len() > 9 and checks buf[8]/buf[9] are
        // not the CR2 discriminator bytes (0x43/'C', 0x52/'R').
        let dir = tempfile::tempdir().unwrap();
        let bytes: &[u8] = &[
            0x4D, 0x4D, // "MM" — big-endian byte order
            0x00, 0x2A, // magic number 42, big-endian
            0x00, 0x00, 0x00, 0x08, // offset to first IFD = 8, big-endian
            0x00, 0x00, // bytes 8-9: not 'C','R' (CR2 discriminator)
            0x00, 0x00, // padding to satisfy len > 9
        ];
        let path = write_fixture(&dir, "scan.tiff", bytes);
        assert!(
            confirm_image(&path),
            "TIFF big-endian file (4D 4D 00 2A) must be confirmed"
        );
    }

    // ── AC-4: mislabeled non-image is rejected ────────────────────────────────

    #[test]
    fn text_file_named_photo_jpg_is_rejected() {
        // AC-4 (FR-3, NFR-2): a file whose extension claims it is a JPEG but
        // whose content is plain text must be rejected by confirm_image.
        // This is the core safety guarantee: a mislabeled file is never grouped
        // or acted on.
        let dir = tempfile::tempdir().unwrap();
        let path = write_fixture(
            &dir,
            "photo.jpg",
            b"This is just a plain text file, not a real JPEG.",
        );
        assert!(
            !confirm_image(&path),
            "a text file named photo.jpg must be rejected; only real JPEG headers pass"
        );
    }

    // ── Bounded-read invariant ────────────────────────────────────────────────

    #[test]
    fn valid_header_followed_by_junk_still_confirms() {
        // The bounded-read invariant: we read only the header prefix, so a file
        // with a valid magic-byte header followed by garbage bytes still confirms.
        // If this test fails, we may be doing a full-file parse rather than a
        // header check.
        let dir = tempfile::tempdir().unwrap();
        let mut bytes = vec![
            0xFF, 0xD8, 0xFF, 0xE0, // JPEG SOI + APP0 marker
            0x00, 0x10, b'J', b'F', b'I', b'F', 0x00, 0x01, 0x01, 0x00, 0x00, 0x01, 0x00, 0x01,
            0x00, 0x00,
        ];
        // Append 10 000 bytes of junk that would break a full JPEG parser.
        bytes.extend(std::iter::repeat_n(0xAB_u8, 10_000));
        let path = write_fixture(&dir, "junk_tail.jpg", &bytes);
        assert!(
            confirm_image(&path),
            "a file with a valid JPEG header and junk tail must still confirm \
             (only the header prefix is read)"
        );
    }

    // ── filter_candidates ─────────────────────────────────────────────────────

    #[test]
    fn filter_candidates_separates_confirmed_and_rejected() {
        // filter_candidates returns confirmed paths and SniffWarning records
        // for rejected paths. Confirm the split is correct.
        let dir = tempfile::tempdir().unwrap();

        let jpeg_path = write_fixture(
            &dir,
            "real.jpg",
            &[
                0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0x00, 0x01, 0x01, 0x00,
                0x00, 0x01, 0x00, 0x01, 0x00, 0x00,
            ],
        );
        let text_path = write_fixture(&dir, "fake.jpg", b"not an image at all");

        let (confirmed, warnings) = filter_candidates(vec![
            candidate(jpeg_path.clone()),
            candidate(text_path.clone()),
        ]);

        assert_eq!(confirmed.len(), 1, "exactly one file should confirm");
        assert_eq!(
            confirmed[0].path, jpeg_path,
            "the confirmed file should be the JPEG"
        );

        assert_eq!(warnings.len(), 1, "exactly one file should be warned");
        assert_eq!(
            warnings[0].path, text_path,
            "the warned file should be the text file"
        );
        assert!(
            !warnings[0].reason.is_empty(),
            "warning reason must not be empty"
        );
    }

    #[test]
    fn filter_candidates_rejected_reason_describes_format_mismatch() {
        // The SniffWarning reason for a file whose header doesn't match should
        // be meaningful, not just "error".
        let dir = tempfile::tempdir().unwrap();
        let path = write_fixture(&dir, "lies.png", b"I am not a PNG");

        let (_confirmed, warnings) = filter_candidates(vec![candidate(path)]);
        assert_eq!(warnings.len(), 1);
        assert!(
            warnings[0]
                .reason
                .contains("not recognized as a supported image format"),
            "reason should say format was not recognized; got: {:?}",
            warnings[0].reason
        );
    }

    #[test]
    fn filter_candidates_preserves_confirmed_order() {
        // Confirmed paths are returned in the same order they were supplied.
        let dir = tempfile::tempdir().unwrap();

        let jpeg_header: &[u8] = &[
            0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0x00, 0x01, 0x01, 0x00,
            0x00, 0x01, 0x00, 0x01, 0x00, 0x00,
        ];
        let p1 = write_fixture(&dir, "first.jpg", jpeg_header);
        let p2 = write_fixture(&dir, "second.jpg", jpeg_header);
        let p3 = write_fixture(&dir, "third.jpg", jpeg_header);

        let (confirmed, warnings) = filter_candidates(vec![
            candidate(p1.clone()),
            candidate(p2.clone()),
            candidate(p3.clone()),
        ]);

        assert!(warnings.is_empty(), "all three should confirm");
        let confirmed_paths: Vec<PathBuf> = confirmed.into_iter().map(|c| c.path).collect();
        assert_eq!(confirmed_paths, vec![p1, p2, p3], "order must be preserved");
    }

    #[test]
    fn filter_candidates_preserves_size_from_enumeration() {
        // The carrier thread (TASK-0304): the size enumerate already stat'd must
        // be threaded through into ConfirmedFile so the engine's size-grouping
        // stage does not re-stat. A confirmed file's size must equal its input
        // Candidate's size, not 0 or the header length.
        let dir = tempfile::tempdir().unwrap();
        let jpeg_header: &[u8] = &[
            0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0x00, 0x01, 0x01, 0x00,
            0x00, 0x01, 0x00, 0x01, 0x00, 0x00,
        ];
        // Make the file larger than the bounded header read so a regression that
        // returned the header length instead of the file size would be caught.
        let mut bytes = jpeg_header.to_vec();
        bytes.extend(std::iter::repeat_n(0xAB_u8, 5_000));
        let path = write_fixture(&dir, "sized.jpg", &bytes);
        let expected_size = bytes.len() as u64;

        let (confirmed, warnings) = filter_candidates(vec![Candidate {
            path: path.clone(),
            size: expected_size,
            mtime: None,
        }]);

        assert!(warnings.is_empty(), "the JPEG should confirm cleanly");
        assert_eq!(confirmed.len(), 1);
        assert_eq!(confirmed[0].path, path);
        assert_eq!(
            confirmed[0].size, expected_size,
            "ConfirmedFile.size must carry the enumerated size through unchanged"
        );
    }

    #[test]
    fn nonexistent_file_is_rejected_with_io_error_reason() {
        // A file that doesn't exist at all should fail gracefully with an I/O
        // error reason, not panic.
        let path = PathBuf::from("/does/not/exist/ghost.jpg");
        let (_confirmed, warnings) = filter_candidates(vec![candidate(path.clone())]);

        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].path, path);
        assert!(
            warnings[0].reason.contains("could not read file header"),
            "I/O error reason expected; got: {:?}",
            warnings[0].reason
        );
    }

    #[test]
    fn filter_candidates_threads_mtime_into_confirmed_file() {
        // The mtime carrier: the mtime enumerate already fetched must be
        // threaded through into ConfirmedFile unchanged.
        let dir = tempfile::tempdir().unwrap();
        let jpeg_header: &[u8] = &[
            0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0x00, 0x01, 0x01, 0x00,
            0x00, 0x01, 0x00, 0x01, 0x00, 0x00,
        ];
        let path = write_fixture(&dir, "mtime_thread.jpg", jpeg_header);
        let sentinel_mtime: Option<i64> = Some(1_234_567_890);

        let (confirmed, warnings) = filter_candidates(vec![Candidate {
            path: path.clone(),
            size: jpeg_header.len() as u64,
            mtime: sentinel_mtime,
        }]);

        assert!(warnings.is_empty(), "the JPEG should confirm cleanly");
        assert_eq!(confirmed.len(), 1);
        assert_eq!(
            confirmed[0].mtime, sentinel_mtime,
            "ConfirmedFile.mtime must carry the enumerated mtime through unchanged"
        );
    }
}
