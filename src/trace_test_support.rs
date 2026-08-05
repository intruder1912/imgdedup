//! Test-only tracing capture helper (TASK-0338, ADR-0017).
//!
//! Each pipeline-stage module (`enumerate.rs`, `sniff.rs`, `engine.rs`,
//! `apply.rs`) opens its own span and needs its own unit tests to assert
//! *which* span fired and with what field values. [`capture`] runs a closure
//! and returns everything it logged (mirroring `--timings`'s real
//! configuration: `FmtSpan::CLOSE`, `INFO`, no target, no ANSI — see
//! `main.rs::run`) as a `String`.
//!
//! ## Why a single global subscriber, not one scoped per test
//!
//! The obvious approach — `tracing::subscriber::with_default` around each
//! test's closure — is *documented* as test-oriented, but is unsound under
//! `cargo test`'s default multi-threaded runner for a subtler reason than
//! thread-safety: `tracing-core`'s per-**callsite** `Interest` cache is
//! process-global, not per-subscriber. A span macro invocation site (e.g. the
//! literal `tracing::info_span!("hash", ...)` in `engine.rs`) registers its
//! `Interest` the *first time it is ever reached in the process*, by
//! querying whichever dispatcher is current *on whichever thread reaches it
//! first* — and that decision is then cached and reused globally, regardless
//! of which subscriber is active on a *different* thread later. With many
//! other tests concurrently exercising the same instrumented pipeline code
//! with **no** subscriber installed on their own thread (the ordinary,
//! uninstrumented case — most tests never call [`capture`] at all), one of
//! them can race a `with_default`-scoped test to registering a callsite,
//! caching `Interest::never()` for it — and a scoped subscriber that starts
//! *after* that has no general way to force re-registration for a callsite
//! it didn't itself first reach. Empirically: a `dedup_core_with_sink` span
//! nested three levels deep would intermittently produce **no** close line
//! at all under `cargo test`'s default parallelism, but always did under
//! `--test-threads=1` — the signature of exactly this race, not a logic bug
//! in the spans themselves.
//!
//! Installing **one** subscriber globally, once, for the whole test binary's
//! lifetime (via [`std::sync::Once`]) removes the race at its root: there is
//! never a window where some other thread's ambient dispatcher is "no
//! subscriber", so every callsite's first registration — no matter which
//! thread reaches it first — sees the same subscriber and gets cached
//! `Interest::always()` (our subscriber accepts every `INFO`-level span,
//! matching `--timings`'s real config) from then on, for the rest of the
//! process. Per-test isolation is recovered by keying the (shared, global)
//! output buffer by [`std::thread::current().id()`], since `cargo test`
//! never interleaves two test functions on the same OS thread — combined
//! with clearing any stale content left by a *previous* test that happened
//! to reuse the same worker thread, right before running the closure under
//! test.
//!
//! `#[cfg(test)]`-only: never compiled into a release binary.

#![cfg(test)]

use std::collections::HashMap;
use std::sync::{Mutex, Once, OnceLock};
use std::thread::ThreadId;

/// Per-thread output buffers, written to by the one global subscriber
/// installed for the whole test binary (see the module doc).
fn buffers() -> &'static Mutex<HashMap<ThreadId, Vec<u8>>> {
    static BUFFERS: OnceLock<Mutex<HashMap<ThreadId, Vec<u8>>>> = OnceLock::new();
    BUFFERS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// A `Write` sink that appends into the calling thread's own entry in
/// [`buffers`], so concurrently-running tests on different threads never mix
/// output.
#[derive(Clone, Default)]
struct PerThreadWriter;

impl std::io::Write for PerThreadWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        buffers()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(std::thread::current().id())
            .or_default()
            .extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for PerThreadWriter {
    type Writer = PerThreadWriter;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

static INIT: Once = Once::new();

/// Install the one global subscriber for this test binary, exactly once.
fn ensure_installed() {
    INIT.call_once(|| {
        let subscriber = tracing_subscriber::fmt()
            .with_writer(PerThreadWriter)
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
            .with_max_level(tracing::Level::INFO)
            .with_target(false)
            .with_ansi(false)
            .finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("the test-support tracing subscriber must install exactly once");
    });
}

/// Run `f` and return everything it logged to this thread's buffer, as a
/// `String`. See the module doc for why this is a global-once subscriber
/// keyed by thread ID, not a `with_default` scope per call.
pub(crate) fn capture<F: FnOnce()>(f: F) -> String {
    ensure_installed();
    let id = std::thread::current().id();
    // Discard anything left over from a previous test that happened to reuse
    // this same worker thread and never drained its own output.
    buffers()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&id);

    f();

    let bytes = buffers()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&id)
        .unwrap_or_default();
    String::from_utf8_lossy(&bytes).into_owned()
}
