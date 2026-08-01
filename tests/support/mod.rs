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

// Each integration test file that does `mod support;` compiles this module
// tree as part of its own separate test binary (Rust integration tests are
// each their own crate). A given test binary typically only exercises a
// subset of these helpers — e.g. `cli_integration.rs` uses `images`/`tree`
// but not the `oracle` harness — so `dead_code` would otherwise fire
// per-binary even though every item here is used by *some* consumer
// (`golden_set.rs` uses the full surface). `#[allow(dead_code)]` on each
// submodule keeps that a non-issue without disabling the lint crate-wide.
#[allow(dead_code)]
pub mod images;
#[allow(dead_code)]
pub mod oracle;
#[allow(dead_code)]
pub mod tree;
