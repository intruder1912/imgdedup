//! Test-only tracing capture helper (TASK-0338, ADR-0017).
//!
//! Each pipeline-stage module (`enumerate.rs`, `sniff.rs`, `engine.rs`,
//! `apply.rs`) opens its own span and needs its own unit tests to assert
//! *which* span fired and with what field values — without installing a
//! process-global subscriber (which would leak across the whole test binary
//! and cannot be installed twice). [`capture`] runs a closure under a
//! **scoped** subscriber (`tracing::subscriber::with_default`, thread-local,
//! never global) configured identically to `--timings`'s real one
//! (`FmtSpan::CLOSE`, `INFO`, no target, no ANSI — see `main.rs::run`), and
//! returns everything it wrote as a `String`.
//!
//! `#[cfg(test)]`-only: never compiled into a release binary, and this
//! module's own existence adds nothing to the shipped dependency footprint
//! beyond what `tracing-subscriber` already contributes.

#![cfg(test)]

use std::sync::{Arc, Mutex};

/// A `Write` + `Clone` in-memory sink, shared behind an `Arc<Mutex<_>>` so
/// the subscriber's own clone (taken once per `make_writer` call) still
/// writes into the same buffer the test reads back afterward.
#[derive(Clone, Default)]
struct CaptureBuf(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for CaptureBuf {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CaptureBuf {
    type Writer = CaptureBuf;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// Run `f` under a scoped `fmt` subscriber mirroring `--timings`'s real
/// configuration, and return everything it wrote to its (fake) "stderr" as a
/// `String`.
///
/// Scoped rather than global (`with_default`, not `try_init`/`set_global_default`):
/// this may be called from many tests running concurrently in the same test
/// binary, and a global subscriber can only ever be installed once per
/// process.
pub(crate) fn capture<F: FnOnce()>(f: F) -> String {
    let buf = CaptureBuf::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(buf.clone())
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .with_ansi(false)
        .finish();
    tracing::subscriber::with_default(subscriber, f);
    let bytes = buf.0.lock().unwrap().clone();
    String::from_utf8_lossy(&bytes).into_owned()
}
