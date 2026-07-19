//! Shared test-support code for integration tests (TASK-0313).
//!
//! This directory is not a test crate itself: Cargo only builds it when a
//! top-level integration test declares `mod support;`. It hosts the
//! deterministic golden-set tooling:
//!
//! - [`images`] — tiny valid image bytes per supported format, byte-exactly
//!   reproducible from `(format, seed, len)`.
//! - [`tree`] — materialize file/hard-link specs with explicit mtimes into a
//!   scan root.
//! - [`oracle`] — run the real `imgdedup --json` binary and diff its output
//!   against a hand-derived expected manifest.

pub mod images;
pub mod oracle;
pub mod tree;
