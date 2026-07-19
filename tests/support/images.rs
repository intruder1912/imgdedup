//! Deterministic tiny-image generator (TASK-0313).
//!
//! Produces the smallest byte sequences that the real pipeline accepts as
//! images for every magic-byte signature `src/sniff.rs` supports. The output
//! is a **pure function of `(format, seed, len)`** — no randomness, no clock,
//! no platform dependence — so golden trees are byte-identical on every run
//! and every OS (NFR-3).
//!
//! - Equal `(format, seed, len)` → byte-identical files (exact duplicates).
//! - Different `seed` → different payload bytes at equal length (the
//!   "near-miss": same format and size, different content).
//! - `len` sets the total file size exactly (size pre-grouping control).
//!
//! The TIFF prefixes pin bytes 8..12 to zero so the seed-derived payload can
//! never place `CR` at offset 8, which `infer` treats as a Canon CR2 and
//! rejects (see the caveat in `src/sniff.rs`).

/// Every magic-byte signature the sniffer supports (one variant per
/// signature, so both GIF and both TIFF flavors are distinct).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageFormat {
    Jpeg,
    Png,
    Gif87a,
    Gif89a,
    Webp,
    Bmp,
    TiffLe,
    TiffBe,
}

/// All supported signatures, for exhaustive per-format scenarios.
pub const ALL_FORMATS: [ImageFormat; 8] = [
    ImageFormat::Jpeg,
    ImageFormat::Png,
    ImageFormat::Gif87a,
    ImageFormat::Gif89a,
    ImageFormat::Webp,
    ImageFormat::Bmp,
    ImageFormat::TiffLe,
    ImageFormat::TiffBe,
];

/// Smallest `len` accepted by [`image_bytes`]: the longest prefix (12 bytes
/// for WebP/TIFF) plus room for seed-distinguishable payload.
pub const MIN_LEN: usize = 16;

impl ImageFormat {
    /// A filename extension on the default allow-list for this format.
    pub fn extension(self) -> &'static str {
        match self {
            ImageFormat::Jpeg => "jpg",
            ImageFormat::Png => "png",
            ImageFormat::Gif87a | ImageFormat::Gif89a => "gif",
            ImageFormat::Webp => "webp",
            ImageFormat::Bmp => "bmp",
            ImageFormat::TiffLe | ImageFormat::TiffBe => "tif",
        }
    }

    /// The fixed header bytes for this format. `total_len` feeds the RIFF and
    /// BMP length fields so the container structure stays self-consistent.
    fn prefix(self, total_len: usize) -> Vec<u8> {
        match self {
            ImageFormat::Jpeg => vec![0xFF, 0xD8, 0xFF, 0xE0],
            ImageFormat::Png => vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
            ImageFormat::Gif87a => b"GIF87a".to_vec(),
            ImageFormat::Gif89a => b"GIF89a".to_vec(),
            ImageFormat::Webp => {
                let mut p = b"RIFF".to_vec();
                p.extend(((total_len - 8) as u32).to_le_bytes());
                p.extend(b"WEBP");
                p
            }
            ImageFormat::Bmp => {
                let mut p = b"BM".to_vec();
                p.extend((total_len as u32).to_le_bytes());
                p.extend([0, 0, 0, 0]); // reserved fields
                p
            }
            // IFD offset 8, then zeroed bytes 8..12 (the CR2-exclusion zone).
            ImageFormat::TiffLe => vec![0x49, 0x49, 0x2A, 0x00, 8, 0, 0, 0, 0, 0, 0, 0],
            ImageFormat::TiffBe => vec![0x4D, 0x4D, 0x00, 0x2A, 0, 0, 0, 8, 0, 0, 0, 0],
        }
    }
}

/// Produce exactly `len` bytes: the format's fixed header followed by a
/// deterministic seed-derived payload. Panics if `len < MIN_LEN`.
pub fn image_bytes(format: ImageFormat, seed: u8, len: usize) -> Vec<u8> {
    assert!(len >= MIN_LEN, "len {len} below MIN_LEN {MIN_LEN}");
    let mut bytes = format.prefix(len);
    let payload_start = bytes.len();
    bytes.extend((payload_start..len).map(|i| seed.wrapping_add(i as u8)));
    bytes
}
