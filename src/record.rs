//! The durable run record (`--output <file>`, FR-20, ADR-0018).
//!
//! One type, [`FileRecord`], is the single writer to the record file. Stages
//! hand it [`RecordEvent`]s at the moment things happen; it renders each one
//! as a complete, newline-terminated unit and writes it with one unbuffered
//! `write_all`, so everything recorded survives the process dying.

// Temporary: nothing calls this module until `main.rs` is wired to it in the
// `--output` step of the same unit of work.
#![allow(dead_code)]

use crate::apply::ActionMode;
use crate::engine::{DuplicateGroup, HardLinkNote};
use crate::progress::Phase;
use crate::safe_text::escape_control_chars;
use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Version of the record's *event vocabulary*. Independent of the result
/// document's `schema_version`. The vocabulary is unstable for now: consumers
/// must ignore unknown `type` values and unknown fields (ADR-0018 §4c).
pub const RECORD_VERSION: u32 = 1;

/// The invocation, as written into the record header.
#[derive(Debug, Clone, Serialize)]
pub struct Invocation {
    pub roots: Vec<String>,
    pub keep: &'static str,
    pub apply: bool,
    pub delete: bool,
    pub json: bool,
    pub no_recurse: bool,
    pub hidden: bool,
    pub follow_symlinks: bool,
    pub ext: Option<String>,
    pub exit_code: bool,
    pub no_progress: bool,
    pub timings: bool,
    pub output: String,
}

/// One thing that happened during a run, handed to a [`RunRecorder`] at the
/// moment it happened.
pub enum RecordEvent<'a> {
    Header(&'a Invocation),
    /// A coarse progress milestone. `total`/`percent` are `None` while the
    /// denominator is unknown.
    Milestone {
        phase: Phase,
        current: u64,
        total: Option<u64>,
        percent: Option<u64>,
        run_total: u64,
    },
    /// A duplicate group whose membership is final (ADR-0018 §1). `group`'s
    /// members and `notes` are already in canonical order. Never indexed.
    Group {
        group: &'a DuplicateGroup,
        notes: &'a [HardLinkNote],
    },
    AlreadyDeduplicated(&'a HardLinkNote),
    Warning {
        stage: &'static str,
        path: &'a Path,
        reason: &'a str,
    },
    /// A scan root that could not be read (run-level, exit 1).
    RootError {
        path: &'a Path,
        reason: &'a str,
    },
    /// A run-level failure not tied to one path.
    RunError {
        reason: &'a str,
    },
    /// An apply-time removal (`skipped: None`) or skip (`Some(reason)`).
    Action {
        mode: ActionMode,
        path: &'a Path,
        skipped: Option<&'a str>,
    },
    /// The completion document. `rendered` is the exact string printed to
    /// stdout; `json` is the same document as a value, present under `--json`.
    Result {
        rendered: &'a str,
        json: Option<&'a crate::json::JsonReport>,
    },
}

/// Observer the pipeline notifies as results and diagnostics are produced.
/// Orthogonal to `ProgressSink`: `--no-progress` never silences it.
pub trait RunRecorder: Sync + Send {
    fn record(&self, event: RecordEvent<'_>);

    /// `false` for [`NoRecord`], so a caller can skip work that only exists
    /// to build an event.
    fn is_active(&self) -> bool {
        true
    }

    /// `true` once a write has failed; the run then exits 1 (ADR-0018 §7).
    fn is_degraded(&self) -> bool {
        false
    }
}

/// The recorder used when `--output` is absent. Does nothing.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoRecord;

impl RunRecorder for NoRecord {
    fn record(&self, _event: RecordEvent<'_>) {}

    fn is_active(&self) -> bool {
        false
    }
}

/// The record file's format: human-readable by default, NDJSON under `--json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Human,
    Ndjson,
}

/// The single writer to the record file.
///
/// Generic over its writer so the failure path is testable without a full
/// disk. In production `W` is a bare `std::fs::File` — **never** a
/// `BufWriter`: `main()` ends in `process::exit`, which runs no destructors,
/// so a user-space buffer would lose the tail of every run.
pub struct FileRecord<W: Write + Send> {
    writer: Mutex<W>,
    format: Format,
    start: Instant,
    /// The `--output` path, for the failure message.
    target: String,
    degraded: AtomicBool,
    /// Called once, with the complete message, on the first failed write.
    on_failure: Box<dyn Fn(&str) + Send + Sync>,
}

impl<W: Write + Send> FileRecord<W> {
    pub fn new(
        writer: W,
        format: Format,
        target: String,
        on_failure: Box<dyn Fn(&str) + Send + Sync>,
    ) -> Self {
        Self {
            writer: Mutex::new(writer),
            format,
            start: Instant::now(),
            target,
            degraded: AtomicBool::new(false),
            on_failure,
        }
    }
}

impl<W: Write + Send> RunRecorder for FileRecord<W> {
    fn record(&self, event: RecordEvent<'_>) {
        // Checked before the mutex: once the file is unwritable every later
        // record costs one relaxed load, even on the rayon hot path.
        if self.degraded.load(Ordering::Relaxed) {
            return;
        }
        let result = {
            let mut writer = self.writer.lock().unwrap_or_else(|e| e.into_inner());
            // Re-checked under the lock: a thread that was waiting here when
            // another one failed must not attempt a second write.
            if self.degraded.load(Ordering::Relaxed) {
                return;
            }
            // Stamped and rendered under the lock so records are written in
            // the order their timestamps claim.
            let elapsed_ms = self.start.elapsed().as_millis() as u64;
            let wall_secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            let text = match self.format {
                Format::Human => render_human(&event, elapsed_ms, wall_secs),
                Format::Ndjson => render_ndjson(&event, elapsed_ms, wall_secs),
            };
            let result = writer.write_all(text.as_bytes());
            if result.is_err() {
                self.degraded.store(true, Ordering::Relaxed);
            }
            result
        };
        // The file is left exactly as it is — partial, with no completion
        // document — and the run carries on (ADR-0018 §7). Only one thread
        // can observe `Err` (the flag is set under the lock above), so this
        // runs exactly once; the message is emitted outside the lock.
        if let Err(e) = result {
            (self.on_failure)(&format!(
                "error: could not write to {}: {}",
                esc(&self.target),
                esc(&e.to_string())
            ));
        }
    }

    fn is_degraded(&self) -> bool {
        self.degraded.load(Ordering::Relaxed)
    }
}

// ── Human-readable rendering (ADR-0018 §4a) ──────────────────────────────

/// Continuation-line indent. A record starts at column 0 with `[` or `#`;
/// everything else is indented, which makes truncation visible.
const INDENT: &str = "              ";

fn stamp(elapsed_ms: u64) -> String {
    format!("[{:>8.1}s] ", elapsed_ms as f64 / 1000.0)
}

fn esc(text: &str) -> String {
    escape_control_chars(text).into_owned()
}

fn esc_path(path: &Path) -> String {
    esc(&path.display().to_string())
}

/// `hard-linked: a, b` — representative plus siblings, byte-wise sorted, the
/// same row `report.rs` prints.
fn hard_link_row(note: &HardLinkNote) -> String {
    let mut set: Vec<&PathBuf> = std::iter::once(&note.representative)
        .chain(note.siblings.iter())
        .collect();
    set.sort();
    let joined = set
        .iter()
        .map(|p| esc_path(p))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{INDENT}hard-linked: {joined}\n")
}

/// Render one event as a complete record. Pure: time is passed in.
fn render_human(event: &RecordEvent<'_>, elapsed_ms: u64, wall_secs: u64) -> String {
    let at = stamp(elapsed_ms);
    match event {
        RecordEvent::Header(inv) => {
            let roots = inv
                .roots
                .iter()
                .map(|r| esc(r))
                .collect::<Vec<_>>()
                .join(", ");
            let ext = inv
                .ext
                .as_deref()
                .map_or_else(|| "<default>".to_owned(), esc);
            format!(
                "# imgdedup {version} run record\n\
                 # started:    {started}\n\
                 # invocation: roots=[{roots}] keep={keep} apply={apply} delete={delete} hidden={hidden}\n\
                 #             recurse={recurse} follow-symlinks={follow} ext={ext} exit-code={exit_code}\n",
                version = env!("CARGO_PKG_VERSION"),
                started = rfc3339_utc(wall_secs),
                keep = inv.keep,
                apply = inv.apply,
                delete = inv.delete,
                hidden = inv.hidden,
                recurse = !inv.no_recurse,
                follow = inv.follow_symlinks,
                exit_code = inv.exit_code,
            )
        }
        RecordEvent::Milestone {
            phase,
            current,
            total,
            percent,
            run_total,
        } => {
            let label = phase.label();
            match (total, percent) {
                (Some(total), Some(pct)) if *phase == Phase::Verifying => {
                    format!("{at}{label}: {current} of {total} ({pct}%) ({run_total} candidates)\n")
                }
                (Some(total), Some(pct)) => {
                    format!("{at}{label}: {current} of {total} ({pct}%)\n")
                }
                _ => format!("{at}{label}...\n"),
            }
        }
        RecordEvent::Group { group, notes } => {
            let reclaimable: u64 = group.removable.iter().map(|r| r.size).sum();
            let mut out = format!("{at}group ({reclaimable} bytes reclaimable):\n");
            out.push_str(&format!(
                "{INDENT}KEEP    {}\n",
                esc_path(&group.keeper.path)
            ));
            for r in &group.removable {
                out.push_str(&format!("{INDENT}REMOVE  {}\n", esc_path(&r.path)));
            }
            for p in &group.protected {
                out.push_str(&format!("{INDENT}PROTECT {}\n", esc_path(&p.path)));
            }
            for note in *notes {
                out.push_str(&hard_link_row(note));
            }
            out
        }
        RecordEvent::AlreadyDeduplicated(note) => format!(
            "{at}already deduplicated (hard-linked): {}\n{}",
            esc_path(&note.representative),
            hard_link_row(note)
        ),
        RecordEvent::Warning {
            stage,
            path,
            reason,
        } => {
            format!(
                "{at}warning: {stage}: {}: {}\n",
                esc_path(path),
                esc(reason)
            )
        }
        RecordEvent::RootError { path, reason } => {
            format!("{at}error: {}: {}\n", esc_path(path), esc(reason))
        }
        RecordEvent::RunError { reason } => format!("{at}error: {}\n", esc(reason)),
        RecordEvent::Action {
            mode,
            path,
            skipped: None,
        } => {
            format!("{at}{}: {}\n", mode.past_tense(), esc_path(path))
        }
        RecordEvent::Action {
            path,
            skipped: Some(reason),
            ..
        } => {
            format!("{at}warning: {}: {}\n", esc_path(path), esc(reason))
        }
        // The banner is FR-20's legibility mechanism: its absence means the
        // run did not finish. The bytes after it are stdout's, verbatim
        // (AC-26) — deliberately not escaped or re-wrapped.
        RecordEvent::Result { rendered, .. } => format!(
            "\n===== RESULT \u{2014} run completed {} after {:.1}s =====\n\n{rendered}",
            rfc3339_utc(wall_secs),
            elapsed_ms as f64 / 1000.0,
        ),
    }
}

// ── NDJSON rendering (ADR-0018 §4b) ──────────────────────────────────────

/// One line of the NDJSON record: `type` first, `elapsed_ms` second, then the
/// event's own fields.
#[derive(Serialize)]
struct Line<'a, T: Serialize> {
    #[serde(rename = "type")]
    kind: &'a str,
    elapsed_ms: u64,
    #[serde(flatten)]
    payload: T,
}

fn ndjson_line<T: Serialize>(kind: &str, elapsed_ms: u64, payload: T) -> String {
    // Compact, so it is one line; `serde_json` escapes every control
    // character, so a path can neither break the line nor reach a terminal.
    let mut line = serde_json::to_string(&Line {
        kind,
        elapsed_ms,
        payload,
    })
    .expect("JSON serialization of plain structs cannot fail");
    line.push('\n');
    line
}

#[derive(Serialize)]
struct HeaderPayload<'a> {
    record_version: u32,
    tool: &'static str,
    tool_version: &'static str,
    started_at: String,
    invocation: &'a Invocation,
}

#[derive(Serialize)]
struct MilestonePayload {
    phase: &'static str,
    current: u64,
    total: Option<u64>,
    percent: Option<u64>,
    run_total: u64,
}

#[derive(Serialize)]
struct RootErrorPayload {
    scope: &'static str,
    #[serde(flatten)]
    error: crate::json::JsonRootError,
}

#[derive(Serialize)]
struct RunErrorPayload<'a> {
    scope: &'static str,
    reason: &'a str,
}

#[derive(Serialize)]
struct ActionPayload<'a> {
    action: &'static str,
    mode: &'static str,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'a str>,
}

#[derive(Serialize)]
struct ResultPayload<'a> {
    record_version: u32,
    result: &'a crate::json::JsonReport,
}

/// Under `--json` `main.rs` always supplies the document as a value. This
/// fallback only exists so a caller that does not still gets a parseable line.
#[derive(Serialize)]
struct ResultTextPayload<'a> {
    record_version: u32,
    result_text: &'a str,
}

/// Render one event as a single NDJSON line. Pure: time is passed in.
fn render_ndjson(event: &RecordEvent<'_>, elapsed_ms: u64, wall_secs: u64) -> String {
    use crate::json;
    match event {
        RecordEvent::Header(invocation) => ndjson_line(
            "header",
            elapsed_ms,
            HeaderPayload {
                record_version: RECORD_VERSION,
                tool: "imgdedup",
                tool_version: env!("CARGO_PKG_VERSION"),
                started_at: rfc3339_utc(wall_secs),
                invocation,
            },
        ),
        RecordEvent::Milestone {
            phase,
            current,
            total,
            percent,
            run_total,
        } => ndjson_line(
            "milestone",
            elapsed_ms,
            MilestonePayload {
                phase: phase.label(),
                current: *current,
                total: *total,
                percent: *percent,
                run_total: *run_total,
            },
        ),
        RecordEvent::Group { group, notes } => {
            ndjson_line("group", elapsed_ms, json::group_payload(group, notes))
        }
        RecordEvent::AlreadyDeduplicated(note) => ndjson_line(
            "already_deduplicated",
            elapsed_ms,
            json::hard_link_set_payload(note),
        ),
        RecordEvent::Warning {
            stage,
            path,
            reason,
        } => ndjson_line(
            "warning",
            elapsed_ms,
            json::warning_payload(stage, path, reason),
        ),
        RecordEvent::RootError { path, reason } => ndjson_line(
            "error",
            elapsed_ms,
            RootErrorPayload {
                scope: "root",
                error: json::root_error_payload(path, reason),
            },
        ),
        RecordEvent::RunError { reason } => ndjson_line(
            "error",
            elapsed_ms,
            RunErrorPayload {
                scope: "run",
                reason,
            },
        ),
        RecordEvent::Action {
            mode,
            path,
            skipped,
        } => ndjson_line(
            "action",
            elapsed_ms,
            ActionPayload {
                action: if skipped.is_some() {
                    "skipped"
                } else {
                    "removed"
                },
                mode: mode.as_str(),
                path: path.to_string_lossy().into_owned(),
                reason: *skipped,
            },
        ),
        RecordEvent::Result {
            json: Some(document),
            ..
        } => ndjson_line(
            "result",
            elapsed_ms,
            ResultPayload {
                record_version: RECORD_VERSION,
                result: document,
            },
        ),
        RecordEvent::Result {
            rendered,
            json: None,
        } => ndjson_line(
            "result",
            elapsed_ms,
            ResultTextPayload {
                record_version: RECORD_VERSION,
                result_text: rendered,
            },
        ),
    }
}

// ── Timestamps ───────────────────────────────────────────────────────────

/// Format seconds since the Unix epoch as RFC 3339 UTC. Done by hand to
/// avoid a date-time dependency for one line of output; the day-to-date
/// conversion is the standard days-from-civil inverse (proleptic Gregorian).
pub(crate) fn rfc3339_utc(secs_since_epoch: u64) -> String {
    let days = secs_since_epoch / 86_400;
    let rem = secs_since_epoch % 86_400;
    let (hour, minute, second) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z % 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

// ── Test support ─────────────────────────────────────────────────────────

/// A recorder that keeps each event's human rendering (stamped at 0), for
/// other modules' tests to assert on what a stage recorded.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct Capture(Mutex<Vec<String>>);

#[cfg(test)]
impl Capture {
    pub(crate) fn lines(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

#[cfg(test)]
impl RunRecorder for Capture {
    fn record(&self, event: RecordEvent<'_>) {
        self.0.lock().unwrap().push(render_human(&event, 0, 0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{DuplicateGroup, FileRec, HardLinkNote};
    use std::path::PathBuf;
    use std::sync::Arc;

    fn rec(path: &str, size: u64) -> FileRec {
        FileRec::new_for_test(PathBuf::from(path), size, Some(1_750_000_000))
    }

    fn invocation() -> Invocation {
        Invocation {
            roots: vec!["/photos".to_owned()],
            keep: "shortest",
            apply: false,
            delete: false,
            json: false,
            no_recurse: false,
            hidden: false,
            follow_symlinks: false,
            ext: None,
            exit_code: false,
            no_progress: false,
            timings: false,
            output: "run.txt".to_owned(),
        }
    }

    // ── timestamps ───────────────────────────────────────────────────────

    #[test]
    fn rfc3339_utc_formats_known_instants() {
        assert_eq!(rfc3339_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339_utc(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(rfc3339_utc(1_786_015_353), "2026-08-06T11:22:33Z");
    }

    // ── human format (ADR-0018 §4a) ──────────────────────────────────────

    #[test]
    fn human_header_identifies_tool_start_and_invocation() {
        let text = render_human(&RecordEvent::Header(&invocation()), 0, 1_786_015_353);
        let expected = format!(
            "# imgdedup {} run record\n\
             # started:    2026-08-06T11:22:33Z\n\
             # invocation: roots=[/photos] keep=shortest apply=false delete=false hidden=false\n\
             #             recurse=true follow-symlinks=false ext=<default> exit-code=false\n",
            env!("CARGO_PKG_VERSION")
        );
        assert_eq!(text, expected);
    }

    #[test]
    fn human_milestones_are_stamped_and_follow_the_progress_vocabulary() {
        let m = |phase, current, total, percent, run_total| RecordEvent::Milestone {
            phase,
            current,
            total,
            percent,
            run_total,
        };
        assert_eq!(
            render_human(&m(Phase::Scanning, 0, None, None, 0), 0, 0),
            "[     0.0s] scanning...\n"
        );
        assert_eq!(
            render_human(
                &m(Phase::Hashing, 1200, Some(28450), Some(4), 28450),
                12_040,
                0
            ),
            "[    12.0s] hashing: 1200 of 28450 (4%)\n"
        );
        assert_eq!(
            render_human(
                &m(Phase::Verifying, 12, Some(40), Some(30), 28450),
                131_400,
                0
            ),
            "[   131.4s] verifying: 12 of 40 (30%) (28450 candidates)\n"
        );
        assert_eq!(
            render_human(
                &m(Phase::Done, 28450, Some(28450), Some(100), 28450),
                141_300,
                0
            ),
            "[   141.3s] done: 28450 of 28450 (100%)\n"
        );
    }

    #[test]
    fn human_group_block_has_no_index_and_indents_its_rows() {
        let group = DuplicateGroup {
            keeper: rec("/photos/a.jpg", 4096),
            removable: vec![rec("/photos/b.jpg", 4096), rec("/photos/c.jpg", 4096)],
            protected: vec![rec("/photos/d.jpg", 4096)],
        };
        let notes = vec![HardLinkNote {
            representative: PathBuf::from("/photos/d.jpg"),
            siblings: vec![PathBuf::from("/photos/e.jpg")],
        }];
        let text = render_human(
            &RecordEvent::Group {
                group: &group,
                notes: &notes,
            },
            131_500,
            0,
        );
        assert_eq!(
            text,
            "[   131.5s] group (8192 bytes reclaimable):\n\
             \x20             KEEP    /photos/a.jpg\n\
             \x20             REMOVE  /photos/b.jpg\n\
             \x20             REMOVE  /photos/c.jpg\n\
             \x20             PROTECT /photos/d.jpg\n\
             \x20             hard-linked: /photos/d.jpg, /photos/e.jpg\n"
        );
        assert!(
            !text.contains("Group "),
            "a record group never carries an index"
        );
    }

    #[test]
    fn human_diagnostics_and_actions() {
        let p = PathBuf::from("/photos/x.jpg");
        let note = HardLinkNote {
            representative: PathBuf::from("/photos/f.jpg"),
            siblings: vec![PathBuf::from("/photos/g.jpg")],
        };
        let cases: Vec<(RecordEvent<'_>, &str)> = vec![
            (
                RecordEvent::AlreadyDeduplicated(&note),
                "[     1.0s] already deduplicated (hard-linked): /photos/f.jpg\n\
                 \x20             hard-linked: /photos/f.jpg, /photos/g.jpg\n",
            ),
            (
                RecordEvent::Warning {
                    stage: "sniff",
                    path: &p,
                    reason: "file is empty",
                },
                "[     1.0s] warning: sniff: /photos/x.jpg: file is empty\n",
            ),
            (
                RecordEvent::RootError {
                    path: &p,
                    reason: "No such file or directory",
                },
                "[     1.0s] error: /photos/x.jpg: No such file or directory\n",
            ),
            (
                RecordEvent::RunError {
                    reason: "not applying",
                },
                "[     1.0s] error: not applying\n",
            ),
            (
                RecordEvent::Action {
                    mode: ActionMode::Trash,
                    path: &p,
                    skipped: None,
                },
                "[     1.0s] trashed: /photos/x.jpg\n",
            ),
            (
                RecordEvent::Action {
                    mode: ActionMode::Delete,
                    path: &p,
                    skipped: Some("no longer exists"),
                },
                "[     1.0s] warning: /photos/x.jpg: no longer exists\n",
            ),
        ];
        for (event, expected) in cases {
            assert_eq!(render_human(&event, 1_000, 0), expected);
        }
    }

    #[test]
    fn human_result_is_a_banner_then_the_exact_report_bytes() {
        let report = "Group 1/1 (2 files, 10 bytes reclaimable):\n  KEEP  /a\n";
        let text = render_human(
            &RecordEvent::Result {
                rendered: report,
                json: None,
            },
            141_300,
            1_786_015_353,
        );
        assert_eq!(
            text,
            format!(
                "\n===== RESULT \u{2014} run completed 2026-08-06T11:22:33Z after 141.3s =====\n\n{report}"
            )
        );
        assert!(
            text.ends_with(report),
            "AC-26: the tail is the report, verbatim"
        );
    }

    /// Review Focus 1: a hostile filename must not start a forged record.
    #[test]
    fn human_record_escapes_control_characters_in_paths() {
        let evil = PathBuf::from("/p/a\n[     9.9s] trashed: /etc/passwd\u{1b}[2J.jpg");
        let group = DuplicateGroup {
            keeper: rec("/p/keep.jpg", 1),
            removable: vec![FileRec::new_for_test(evil.clone(), 1, None)],
            protected: vec![],
        };
        let mut inv = invocation();
        inv.roots = vec![evil.display().to_string()];
        let rendered = [
            render_human(&RecordEvent::Header(&inv), 0, 0),
            render_human(
                &RecordEvent::Group {
                    group: &group,
                    notes: &[],
                },
                0,
                0,
            ),
            render_human(
                &RecordEvent::Warning {
                    stage: "engine",
                    path: &evil,
                    reason: "x\ny",
                },
                0,
                0,
            ),
            render_human(
                &RecordEvent::RootError {
                    path: &evil,
                    reason: "r",
                },
                0,
                0,
            ),
            render_human(
                &RecordEvent::Action {
                    mode: ActionMode::Trash,
                    path: &evil,
                    skipped: None,
                },
                0,
                0,
            ),
        ];
        for text in rendered {
            assert!(!text.contains('\u{1b}'), "raw ESC leaked: {text:?}");
            assert!(
                !text.lines().any(|l| l.starts_with("[     9.9s]")),
                "forged record line: {text:?}"
            );
        }
    }

    // ── FileRecord ───────────────────────────────────────────────────────

    #[test]
    fn no_record_is_inactive_and_never_degraded() {
        let r = NoRecord;
        r.record(RecordEvent::RunError { reason: "ignored" });
        assert!(!r.is_active());
        assert!(!r.is_degraded());
    }

    /// Review Focus 5: nothing sits in a user-space buffer. The file is read
    /// back while the recorder is still alive and never flushed or dropped —
    /// what a `tail -f`, or a reader after SIGKILL, would see (AC-25).
    #[test]
    fn records_are_visible_without_drop_or_flush() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.txt");
        let file = std::fs::File::create(&path).unwrap();
        let recorder = FileRecord::new(
            file,
            Format::Human,
            path.display().to_string(),
            Box::new(|_| panic!("no write should fail")),
        );

        recorder.record(RecordEvent::Header(&invocation()));
        recorder.record(RecordEvent::RunError { reason: "first" });

        let seen = std::fs::read_to_string(&path).unwrap();
        assert!(seen.starts_with("# imgdedup "));
        assert!(seen.ends_with("error: first\n"));
        assert!(!seen.contains("===== RESULT"), "no completion document yet");
        std::mem::forget(recorder); // as `process::exit` would: no destructor
        assert_eq!(std::fs::read_to_string(&path).unwrap(), seen);
    }

    #[test]
    fn each_record_is_one_complete_newline_terminated_write() {
        #[derive(Clone, Default)]
        struct Writes(Arc<Mutex<Vec<Vec<u8>>>>);
        impl Write for Writes {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().push(buf.to_vec());
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let writes = Writes::default();
        let recorder = FileRecord::new(
            writes.clone(),
            Format::Human,
            "run.txt".to_owned(),
            Box::new(|_| {}),
        );
        recorder.record(RecordEvent::Header(&invocation()));
        recorder.record(RecordEvent::RunError { reason: "x" });

        let writes = writes.0.lock().unwrap();
        assert_eq!(writes.len(), 2, "one write per record");
        assert!(writes.iter().all(|w| w.ends_with(b"\n")));
    }

    // ── Write failure (ADR-0018 §7, AC-28a) ──────────────────────────────

    /// Fails every write, counting attempts.
    #[derive(Clone, Default)]
    struct Broken(Arc<std::sync::atomic::AtomicUsize>);

    impl Write for Broken {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(std::io::Error::other("No space left on device"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn broken_recorder(target: &str) -> (FileRecord<Broken>, Broken, Arc<Mutex<Vec<String>>>) {
        let writer = Broken::default();
        let messages: Arc<Mutex<Vec<String>>> = Arc::default();
        let sink = Arc::clone(&messages);
        let recorder = FileRecord::new(
            writer.clone(),
            Format::Human,
            target.to_owned(),
            Box::new(move |msg| sink.lock().unwrap().push(msg.to_owned())),
        );
        (recorder, writer, messages)
    }

    #[test]
    fn a_failed_write_is_reported_once_and_degrades_the_recorder() {
        let (recorder, writer, messages) = broken_recorder("/mnt/nas/run.txt");
        assert!(!recorder.is_degraded());

        for _ in 0..5 {
            recorder.record(RecordEvent::RunError { reason: "x" });
        }

        assert!(recorder.is_degraded());
        assert_eq!(
            *messages.lock().unwrap(),
            vec!["error: could not write to /mnt/nas/run.txt: No space left on device".to_owned()]
        );
        assert_eq!(
            writer.0.load(Ordering::SeqCst),
            1,
            "once degraded, no further write is attempted"
        );
    }

    #[test]
    fn the_failure_message_escapes_the_target_path() {
        let (recorder, _writer, messages) = broken_recorder("run\u{1b}[2J.txt");
        recorder.record(RecordEvent::RunError { reason: "x" });
        let messages = messages.lock().unwrap();
        assert!(!messages[0].contains('\u{1b}'), "{:?}", messages[0]);
    }

    /// Review Focus 4: rayon workers failing at the same instant.
    #[test]
    fn concurrent_write_failures_report_exactly_once() {
        let (recorder, _writer, messages) = broken_recorder("run.txt");
        std::thread::scope(|scope| {
            for _ in 0..16 {
                scope.spawn(|| {
                    for _ in 0..200 {
                        recorder.record(RecordEvent::RunError { reason: "x" });
                    }
                });
            }
        });
        assert_eq!(messages.lock().unwrap().len(), 1);
        assert!(recorder.is_degraded());
    }

    // ── NDJSON format (ADR-0018 §4b) ─────────────────────────────────────

    fn parse(line: &str) -> serde_json::Value {
        assert!(
            line.ends_with('\n') && !line.trim_end().contains('\n'),
            "one line: {line:?}"
        );
        serde_json::from_str(line).unwrap()
    }

    #[test]
    fn ndjson_lines_start_with_type_then_elapsed_ms() {
        let line = render_ndjson(&RecordEvent::RunError { reason: "x" }, 1500, 0);
        assert!(
            line.starts_with(r#"{"type":"error","elapsed_ms":1500,"#),
            "{line}"
        );
    }

    #[test]
    fn ndjson_header_carries_version_tool_start_and_structured_invocation() {
        let v = parse(&render_ndjson(
            &RecordEvent::Header(&invocation()),
            0,
            1_786_015_353,
        ));
        assert_eq!(v["type"], "header");
        assert_eq!(v["record_version"], 1);
        assert_eq!(v["tool"], "imgdedup");
        assert_eq!(v["tool_version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(v["started_at"], "2026-08-06T11:22:33Z");
        assert_eq!(v["invocation"]["roots"][0], "/photos");
        assert_eq!(v["invocation"]["keep"], "shortest");
        assert_eq!(v["invocation"]["ext"], serde_json::Value::Null);
        assert_eq!(v["invocation"]["output"], "run.txt");
    }

    #[test]
    fn ndjson_milestone_uses_null_for_an_unknown_denominator() {
        let v = parse(&render_ndjson(
            &RecordEvent::Milestone {
                phase: Phase::Scanning,
                current: 0,
                total: None,
                percent: None,
                run_total: 0,
            },
            0,
            0,
        ));
        assert_eq!(v["type"], "milestone");
        assert_eq!(v["phase"], "scanning");
        assert!(v["total"].is_null() && v["percent"].is_null());

        let v = parse(&render_ndjson(
            &RecordEvent::Milestone {
                phase: Phase::Hashing,
                current: 1200,
                total: Some(28450),
                percent: Some(4),
                run_total: 28450,
            },
            12_040,
            0,
        ));
        assert_eq!(
            (v["current"].as_u64(), v["total"].as_u64()),
            (Some(1200), Some(28450))
        );
        assert_eq!(v["percent"], 4);
        assert_eq!(v["run_total"], 28450);
    }

    #[test]
    fn ndjson_group_has_the_json_group_shape() {
        let group = DuplicateGroup {
            keeper: rec("/photos/a.jpg", 4096),
            removable: vec![rec("/photos/b.jpg", 4096)],
            protected: vec![],
        };
        let v = parse(&render_ndjson(
            &RecordEvent::Group {
                group: &group,
                notes: &[],
            },
            9,
            0,
        ));
        assert_eq!(v["type"], "group");
        assert_eq!(v["keeper"]["path"], "/photos/a.jpg");
        assert_eq!(v["keeper"]["size"], 4096);
        assert_eq!(v["removable"][0]["path"], "/photos/b.jpg");
        assert_eq!(v["reclaimable_bytes"], 4096);
        assert!(v["protected"].as_array().unwrap().is_empty());
        assert!(v["hard_link_sets"].as_array().unwrap().is_empty());
    }

    #[test]
    fn ndjson_diagnostics_and_actions() {
        let p = PathBuf::from("/photos/x.jpg");
        let note = HardLinkNote {
            representative: PathBuf::from("/photos/f.jpg"),
            siblings: vec![PathBuf::from("/photos/g.jpg")],
        };

        let v = parse(&render_ndjson(
            &RecordEvent::AlreadyDeduplicated(&note),
            0,
            0,
        ));
        assert_eq!(v["type"], "already_deduplicated");
        assert_eq!(v["representative"], "/photos/f.jpg");
        assert_eq!(v["siblings"][0], "/photos/g.jpg");

        let v = parse(&render_ndjson(
            &RecordEvent::Warning {
                stage: "sniff",
                path: &p,
                reason: "file is empty",
            },
            0,
            0,
        ));
        assert_eq!(
            (v["type"].as_str(), v["stage"].as_str()),
            (Some("warning"), Some("sniff"))
        );
        assert_eq!(v["path"], "/photos/x.jpg");
        assert_eq!(v["reason"], "file is empty");

        let v = parse(&render_ndjson(
            &RecordEvent::RootError {
                path: &p,
                reason: "gone",
            },
            0,
            0,
        ));
        assert_eq!(
            (v["type"].as_str(), v["scope"].as_str()),
            (Some("error"), Some("root"))
        );
        assert_eq!(v["path"], "/photos/x.jpg");

        let v = parse(&render_ndjson(
            &RecordEvent::RunError {
                reason: "not applying",
            },
            0,
            0,
        ));
        assert_eq!(
            (v["type"].as_str(), v["scope"].as_str()),
            (Some("error"), Some("run"))
        );
        assert!(v.get("path").is_none());

        let v = parse(&render_ndjson(
            &RecordEvent::Action {
                mode: ActionMode::Trash,
                path: &p,
                skipped: None,
            },
            0,
            0,
        ));
        assert_eq!(v["type"], "action");
        assert_eq!(
            (v["action"].as_str(), v["mode"].as_str()),
            (Some("removed"), Some("trash"))
        );
        assert!(v.get("reason").is_none());

        let v = parse(&render_ndjson(
            &RecordEvent::Action {
                mode: ActionMode::Delete,
                path: &p,
                skipped: Some("no longer exists"),
            },
            0,
            0,
        ));
        assert_eq!(
            (v["action"].as_str(), v["mode"].as_str()),
            (Some("skipped"), Some("delete"))
        );
        assert_eq!(v["reason"], "no longer exists");
    }

    #[test]
    fn ndjson_never_emits_a_raw_control_byte_or_a_second_line() {
        let evil = PathBuf::from("/p/a\n{\"type\":\"result\"}\u{1b}[2J.jpg");
        let line = render_ndjson(
            &RecordEvent::Warning {
                stage: "engine",
                path: &evil,
                reason: "x\ny",
            },
            0,
            0,
        );
        assert_eq!(
            line.matches('\n').count(),
            1,
            "exactly the terminator: {line:?}"
        );
        assert!(!line.contains('\u{1b}'));
        assert_eq!(parse(&line)["path"], evil.display().to_string());
    }

    #[test]
    fn ndjson_result_wraps_the_document_compactly() {
        let outcome = crate::engine::DedupOutcome::default();
        let doc = crate::json::document(
            &outcome,
            &[],
            &[],
            &[],
            ActionMode::DryRun,
            &crate::apply::ApplyOutcome::default(),
        );
        let pretty = doc.to_pretty();
        let v = parse(&render_ndjson(
            &RecordEvent::Result {
                rendered: &pretty,
                json: Some(&doc),
            },
            141_300,
            0,
        ));
        assert_eq!(v["type"], "result");
        assert_eq!(v["record_version"], 1);
        // AC-27: the same document as stdout's, though not the same bytes.
        let from_stdout: serde_json::Value = serde_json::from_str(&pretty).unwrap();
        assert_eq!(v["result"], from_stdout);
        assert_eq!(v["result"]["schema_version"], 1);
    }
}
