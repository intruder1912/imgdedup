//! Output-stream routing (FR-21, ADR-0018 §8).
//!
//! One pure function decides which stream carries progress and apply-time
//! status. TTY-ness is probed exactly once, in `main.rs`, and passed in.

use std::io::{self, Write};

/// One of the process's two output streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

/// A stream together with whether *that stream* is an interactive terminal.
/// Binding the two is what makes it impossible to pick the in-place renderer
/// from one stream's TTY-ness while writing to the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    pub stream: Stream,
    pub tty: bool,
}

/// Where routine output goes for this run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Routing {
    /// Progress target; `None` when `--no-progress` or `--timings` suppress it.
    pub progress: Option<Target>,
    /// Apply-time `trashed:`/`deleted:` lines; `Some` only in the default
    /// mode with stdout an interactive terminal. Never stderr.
    pub status: Option<Target>,
}

/// Decide the routing from the five facts it depends on (ADR-0018 §8).
pub fn route(
    json: bool,
    no_progress: bool,
    timings: bool,
    stdout_tty: bool,
    stderr_tty: bool,
) -> Routing {
    let interactive_stdout = !json && stdout_tty;
    let stdout = Target {
        stream: Stream::Stdout,
        tty: true,
    };
    let progress = if no_progress || timings {
        None
    } else if interactive_stdout {
        Some(stdout)
    } else {
        Some(Target {
            stream: Stream::Stderr,
            tty: stderr_tty,
        })
    };
    Routing {
        progress,
        status: interactive_stdout.then_some(stdout),
    }
}

/// The writer `Progress` renders to, chosen at runtime. An enum rather than
/// `Box<dyn Write>`: no allocation, no dynamic dispatch.
pub enum ProgressStream {
    Out(io::Stdout),
    Err(io::Stderr),
}

/// The text of a mid-run stderr message that may arrive while a live
/// progress line is on screen (today: the record-write failure, ADR-0018 §7).
///
/// While a line is live the message starts with a line break, so it lands on
/// a clean line instead of after the progress text; once progress has
/// finished the cursor is already on a fresh line and the break would only
/// leave a stray blank one. The result is written with **one** `write`, so a
/// concurrent repaint cannot land between the break and the message.
pub fn interrupting_line(message: &str, line_is_live: bool) -> String {
    if line_is_live {
        format!("\n{message}\n")
    } else {
        format!("{message}\n")
    }
}

/// Write a diagnostic to stderr with one `write`, ignoring failure.
///
/// `eprintln!` panics when stderr cannot be written — a pipe whose reader has
/// gone away is enough. A diagnostic nobody can read is not worth aborting a
/// run for, and must never abort an `--apply` partway through. `text` carries
/// its own line ending.
pub fn diag(text: &str) {
    let _ = io::stderr().write_all(text.as_bytes());
}

/// Write `text` to stdout and flush it, without panicking. Returns whether
/// it was delivered: `false` means the reader is gone (or the disk behind a
/// redirect is full) and the caller decides what that is worth.
#[must_use]
pub fn write_stdout(text: &str) -> bool {
    let mut out = io::stdout().lock();
    out.write_all(text.as_bytes()).is_ok() && out.flush().is_ok()
}

impl ProgressStream {
    pub fn open(stream: Stream) -> Self {
        match stream {
            Stream::Stdout => ProgressStream::Out(io::stdout()),
            Stream::Stderr => ProgressStream::Err(io::stderr()),
        }
    }
}

impl Write for ProgressStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            ProgressStream::Out(s) => s.write(buf),
            ProgressStream::Err(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            ProgressStream::Out(s) => s.flush(),
            ProgressStream::Err(s) => s.flush(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interrupting_line_breaks_first_only_while_a_line_is_live() {
        assert_eq!(interrupting_line("error: x", true), "\nerror: x\n");
        assert_eq!(interrupting_line("error: x", false), "error: x\n");
    }

    const OUT_TTY: Target = Target {
        stream: Stream::Stdout,
        tty: true,
    };

    fn err(tty: bool) -> Target {
        Target {
            stream: Stream::Stderr,
            tty,
        }
    }

    #[test]
    fn default_mode_on_an_interactive_stdout_routes_everything_to_stdout() {
        // AC-29, both stderr states (the second row is "stderr redirected").
        for stderr_tty in [true, false] {
            let r = route(false, false, false, true, stderr_tty);
            assert_eq!(r.progress, Some(OUT_TTY));
            assert_eq!(r.status, Some(OUT_TTY));
        }
    }

    #[test]
    fn default_mode_with_stdout_redirected_falls_back_to_stderrs_own_tty_ness() {
        // AC-30: `imgdedup ./photos > report.txt` keeps its live line.
        let r = route(false, false, false, false, true);
        assert_eq!(r.progress, Some(err(true)));
        assert_eq!(r.status, None);

        let r = route(false, false, false, false, false);
        assert_eq!(r.progress, Some(err(false)));
        assert_eq!(r.status, None);
    }

    #[test]
    fn json_never_routes_anything_to_stdout() {
        // AC-31, terminal or not.
        for stdout_tty in [true, false] {
            for stderr_tty in [true, false] {
                let r = route(true, false, false, stdout_tty, stderr_tty);
                assert_eq!(r.progress, Some(err(stderr_tty)));
                assert_eq!(r.status, None);
            }
        }
    }

    #[test]
    fn no_progress_and_timings_suppress_progress_but_not_status() {
        // AC-23 / ADR-0017: status still follows the stdout-TTY rule.
        for (no_progress, timings) in [(true, false), (false, true), (true, true)] {
            let r = route(false, no_progress, timings, true, true);
            assert_eq!(r.progress, None);
            assert_eq!(r.status, Some(OUT_TTY));
        }
    }

    #[test]
    fn invariants_hold_for_all_32_combinations() {
        for bits in 0u8..32 {
            let json = bits & 1 != 0;
            let no_progress = bits & 2 != 0;
            let timings = bits & 4 != 0;
            let stdout_tty = bits & 8 != 0;
            let stderr_tty = bits & 16 != 0;
            let r = route(json, no_progress, timings, stdout_tty, stderr_tty);
            let case = format!("{bits:05b}");

            // A target's `tty` always describes the stream it travels with,
            // so in-place control codes can never reach a redirected file.
            for t in [r.progress, r.status].into_iter().flatten() {
                let real = match t.stream {
                    Stream::Stdout => stdout_tty,
                    Stream::Stderr => stderr_tty,
                };
                assert_eq!(t.tty, real, "tty must match its stream ({case})");
            }
            // Nothing but the document reaches a stdout that is not a
            // terminal, or any stdout under --json.
            if json || !stdout_tty {
                assert!(
                    r.progress.is_none_or(|t| t.stream == Stream::Stderr),
                    "{case}"
                );
                assert_eq!(r.status, None, "{case}");
            }
            // Routine status never falls back to stderr (ADR-0018 §9 #6).
            assert!(
                r.status.is_none_or(|t| t.stream == Stream::Stdout),
                "{case}"
            );
            if no_progress || timings {
                assert_eq!(r.progress, None, "{case}");
            }
        }
    }
}
