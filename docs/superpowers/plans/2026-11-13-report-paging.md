# Report Paging and Closing Status Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** On an interactive terminal, a human-readable report longer than the screen opens in a pager positioned at its end, and a short closing status stays in the terminal after the pager exits.

**Architecture:** The paging decision is a pure function of facts probed once in `main.rs`, like `streams::route`. A new module `src/pager.rs` owns starting the pager, feeding it, and waiting for it. The closing status is rendered by `report.rs` from the same data as the report. Nothing in the report's bytes, the `--output` record or redirected stdout changes.

**Tech Stack:** Rust, `std::process`; no new crates. ADR-0019 makes `libc` (Unix) and `windows-sys` (Windows) direct dependencies for the terminal size and `SIGINT`; both are already in `Cargo.lock`.

**Spec:** [`docs/superpowers/specs/2026-11-13-report-paging-design.md`](../specs/2026-11-13-report-paging-design.md) — owner-approved 2026-11-13.

## Global Constraints

- Branch `feat/report-paging`, one PR: `feat(output): page long reports and leave a closing status (TASK-0340)`.
- **Design before code:** Tasks 1 and 2 (ADR-0019, PRD amendment) must be merged into this branch before Task 3 starts. Where a later task says "per ADR-0019", the ADR's answer replaces this plan's placeholder.
- Paging applies only when all hold: stdout and stdin are terminals; `TERM` is not `dumb`; not `--json`; neither `--no-pager` nor `--timings`; `$PAGER` is not empty and not `cat`; a pager can be started; the report does not fit on the screen.
- Pager: `$PAGER` if set, else `less`. Windows has no default: without `$PAGER` the report prints as today.
- The closing status appears **only after a pager** (owner decision) — including after a pager that started and then failed, where the report is first printed directly.
- When no pager runs, stdout is byte-for-byte what it is today. Redirected or piped stdout, `--json` and the `--output` record never contain the closing status.
- The exit code is the tool's own; the pager's status never replaces it. A report handed to the pager counts as delivered (FR-15 / AC-46 / AC-47 do not fire), and an early quit prints no error.
- Nothing is written to the terminal while the pager owns it; progress and apply-time status lines are complete before it starts.
- TTY facts are probed once in `main.rs` and injected. No panicking prints (`streams::diag` / `write_stdout`).
- `--help` cites no internal identifiers. TDD. `make validate` and `cargo deny check` green before pushing; check `make validate`'s own exit status, not a pipe's.

## Review Focus

1. **`PAGER` misspelled** (`PAGER=lesss`) — the report must still reach the terminal, followed by the closing status; exit code unchanged. Task 5.
2. **User quits the pager on the first screen of a 100k-line report** — no error line, no panic, exit code unchanged, closing status printed. Task 5.
3. **A terminal with nobody at it** (`TERM=dumb`, stdin not a terminal) — no pager, no hang. Task 3.
4. **`PAGER="less -R"`** (arguments in the variable) — works, per ADR-0019's rule for starting the pager. Task 5.
5. **Ctrl-C while the pager is open** — the terminal is left usable and the `--output` record already has its completion document. Task 5 / ADR-0019.

## File Structure

| File | Status | Responsibility |
|---|---|---|
| `docs/adr/0019-report-paging.md` | create | how the pager is started, positioned, and waited for; security; screen handling |
| `docs/prd/PRD-0001-…md` | modify | FR-22 and AC-33..AC-37 replaced by the approved design |
| `src/streams.rs` | modify | `PagingFacts`, pure `should_page()` |
| `src/report.rs` | modify | `render_closing_status()` |
| `src/pager.rs` | create | spawn, feed, wait; outcome type |
| `src/cli.rs` | modify | `--no-pager` |
| `src/main.rs` | modify | probe facts, call the pager, print the status |
| `tests/cli_integration.rs` | modify | end-to-end with a fake `$PAGER` |
| guide, README, planning | modify | Task 6 |

## Batches and usage budget

Calibrated on PRs #107–#110: a full agent pass costs about 10% of a 5h window; an architect review of new product code should be budgeted with a fix round and a confirmation pass.

| Batch | Tasks | Agent passes | Est. 5h window |
|---|---|---|---|
| A — design | 1–2 | architect (ADR), product-owner (PRD) | 25–35% |
| B — code | 3–5 | none (inline TDD) | 15–20% |
| C — docs, review, PR | 6 | product-owner (docs + acceptance), architect review + confirmation | 35–45% |

Run each batch in a **fresh session**; A and B fit one window together, C needs its own. Weekly: about 8–10% in total. Cut line: stop after any batch with the branch committed and `status.md` updated.

---

### Task 1: ADR-0019 (architect)

- [x] **Step 1:** Dispatch the `architect` agent with the spec. It must decide, with reasons, each item under the spec's "Left to the architect": (1) starting `$PAGER` — through a shell or split and exec'd, and the security statement; (2) how "opened at the end" and "only when it does not fit" are achieved for `less` and what is promised for other pagers, including whether the tool learns the terminal height itself and how; (3) the screen after exit (`-X` or not); (4) ordering against progress and apply-time lines; (5) the pipe: early quit, waiting, Ctrl-C; (6) when the `--output` completion document is written relative to the pager.
- [x] **Step 2:** The ADR must end with the exact interface for `src/pager.rs` and the `less` arguments or environment the tool sets. Update Tasks 3–5 of this plan where it differs. *Done: [ADR-0019](../../adr/0019-report-paging.md) (`proposed`); Tasks 3–5 below follow it. One question for the owner is open in the ADR (unset or empty `TERM` on Unix counts as `dumb`); Task 3 implements the recommendation unless the owner rules otherwise.*
- [x] **Step 3:** `make validate`; commit `docs(adr): ADR-0019 report paging`.

### Task 2: PRD-0001 amendment (product-owner)

- [x] **Step 1:** Dispatch the `product-owner` agent with the spec and ADR-0019: replace the tentative FR-22 and AC-33..AC-37 with requirements for the approved design, fold them into the body, and settle the closing status's exact wording (the warnings line; whether unreadable roots are counted). Reconcile FR-15 / AC-46 / AC-47 with "handed to the pager counts as delivered". Add `--no-pager` to §8. *Done: PRD-0001 Amendment 2026-11-14 — FR-22 in §6, AC-33..AC-37 and AC-48..AC-52 in §9, FR-15 reconciled; the closing-status wording (AC-48) and a failed-pager warning line (FR-22, AC-36) are settled; unreadable roots are not counted. The unset/empty `TERM` clause is marked pending the owner.*
- [x] **Step 2:** `make validate`; commit `docs(prd): FR-22 report paging and closing status`.

### Task 3: The paging decision

**Files:** `src/streams.rs`, `src/pager.rs` (create, `PagerCommand` only), `src/cli.rs`, `src/record.rs`, `src/main.rs`.

**Interfaces — produces (ADR-0019, "The interface"):**

```rust
// src/pager.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PagerCommand {
    /// `$PAGER` unset (never on Windows): `less`, found on `PATH`.
    Less,
    /// `$PAGER`'s value, run by the platform's command interpreter.
    Shell(OsString),
}

// src/streams.rs
pub struct PagingFacts<'a> {
    pub json: bool,
    pub no_pager: bool,
    pub timings: bool,
    pub stdout_tty: bool,
    pub stdin_tty: bool,
    pub term: Option<&'a OsStr>,      // $TERM
    pub pager_env: Option<&'a OsStr>, // $PAGER
    pub windows: bool,
}

/// `None`: do not page. Every condition but "does not fit" (that is
/// `pager::fits`, Task 5).
pub fn should_page(facts: &PagingFacts<'_>) -> Option<pager::PagerCommand>;
```

- [ ] **Step 1: Failing unit tests** in `src/streams.rs`, one per constraint: each of `json`, `no_pager`, `timings`, `!stdout_tty`, `!stdin_tty`, `term == Some("dumb")`, `pager_env == Some("")`, `pager_env == Some("cat")` gives `None`; off Windows, `term == None` and `term == Some("")` give `None` too (ADR-0019 §8; on Windows they do not disable paging); unset `PAGER` gives `Some(PagerCommand::Less)` off Windows and `None` on Windows; `PAGER="less -R"` gives `Some(Shell("less -R"))`; `PAGER=" cat"` and `PAGER="/bin/cat"` give `Some(Shell(..))` (only the exact value `cat` disables); on Unix a non-UTF-8 `PAGER` is passed through byte-for-byte. Plus a table test over all boolean combinations asserting `None` whenever any disabling fact is set.
- [ ] **Step 2:** Run — fails to compile.
- [ ] **Step 3:** Implement `should_page` and the `PagerCommand` type; add `--no-pager` to `Cli` (help text: `Do not show a long report through a pager. By default, on an interactive terminal, a report longer than the screen opens in a pager positioned at its end.`); add `no_pager` to `record::Invocation` and to both record headers (human `no-pager=`, NDJSON `invocation.no_pager`) — the header names every flag (ADR-0018 implementation note 10); probe the facts in `main.rs` next to the existing `route` call — `stdout_tty` is probed once and shared with `route`; `TERM`, `PAGER` and `LESS` are read with `std::env::var_os`. "Fits on the screen" is not part of this function — it is `pager::fits` (Task 5), and the terminal size is probed only when `should_page` returns `Some`.
- [ ] **Step 4:** `cargo test`; extend `help_lists_all_flags` with `--no-pager`; extend the record-header tests with `no-pager=`.
- [ ] **Step 5:** Commit `feat(output): the paging decision (FR-22)`.

### Task 4: The closing status

**Files:** `src/report.rs`.

**Interfaces — produces:** `pub fn render_closing_status(...) -> String`, taking the same arguments as `report::render`; the returned text ends with a newline.

- [ ] **Step 1: Failing unit tests**, with the wording fixed by Task 2 (PRD-0001 FR-22, AC-48). The returned text always begins with one empty line (`"\n"`), which separates it from the report in the pager and from the restored screen after it:
  - dry run, no warnings → `"\n"` then exactly the report's own `Summary:` line;
  - a report with no `Summary:` line → `"\n"` then `No duplicate groups found.`;
  - dry run with 12 warnings → that plus `12 warnings, listed at the top of the report.` (the count is the `Warnings:` section's entries);
  - `--apply` → second line `Trashed 3 files.`; nothing removed → `Trashed 0 files.`; with skips `Trashed 3 files. Skipped 1 (left in place).`; `--delete` → `Deleted`;
  - the first line is byte-identical to the summary line inside `render`'s output for the same input, for every form of it (with and without already-deduplicated hard-link sets; no parallel wording);
  - singular forms (`1 warning, listed at the top of the report.`, `Trashed 1 file.`).
- [ ] **Step 2:** Run — fails.
- [ ] **Step 3:** Implement by reusing the function that writes the report's `Summary:` line; extract it if it is inline.
- [ ] **Step 4:** `cargo test`; the golden `--json` test and every report test still pass unchanged.
- [ ] **Step 5:** Commit `feat(report): closing status (FR-22)`.

### Task 5: The pager

**Files:** `src/pager.rs` (+ private `sys` submodule), `src/main.rs`, `Cargo.toml`, `tests/cli_integration.rs`.

**Interfaces — produces (ADR-0019, "The interface"; full doc comments there):**

```rust
pub struct ScreenSize { pub rows: u16, pub cols: u16 }
pub fn screen_size() -> Option<ScreenSize>;                // the probe (sys)
pub fn fits(text: &str, screen: ScreenSize) -> bool;       // pure
pub fn less_env(user: Option<&OsStr>) -> OsString;         // pure
pub fn command(pager: &PagerCommand, user_less: Option<&OsStr>) -> Command; // pure

pub enum Paged {
    /// Exited successfully, or was interrupted by the user (`SIGINT`, 130).
    Shown,
    /// Started, then any other exit status or signal.
    Failed,
    /// Could not be started at all.
    NotStarted,
}
pub fn classify(status: ExitStatus) -> Paged;              // pure
pub fn run(command: Command, parts: &[&str]) -> Paged;     // spawn, write, wait

pub enum Delivery<'a> {
    Direct,
    Paged { pager: &'a PagerCommand, closing_status: &'a str },
}
pub fn deliver(
    delivery: Delivery<'_>,
    report: &str,
    page: &mut dyn FnMut(&PagerCommand, &[&str]) -> Paged,
    write: &mut dyn FnMut(&str) -> io::Result<()>,
) -> io::Result<()>;                                       // Err = not delivered
```

Fixed by ADR-0019: `PagerCommand::Less` → `Command::new("less")`, no arguments; `Shell(v)` → `/bin/sh -c <v>` (Unix) or `cmd.exe /D /S /C "<v>"` via `raw_arg` (Windows); every pager gets `LESS` = `+G$-+E -+e -+F` when the user's `LESS` is unset or empty, else `+G$<user's LESS>$-+E -+e -+F`; stdin piped, stdout/stderr inherited; no writer thread; `SIGINT` ignored after spawn until `wait` returns, previous disposition restored by a guard.

- [ ] **Step 1: Failing unit tests** in `src/pager.rs`:
  - pure: `fits` (exactly `rows - 1` rows fits, one more does not; a line of `cols + 1` ASCII characters takes two rows; a non-ASCII character counts two columns; empty text fits; the count stops early on a huge text); `less_env` (unset, empty, `-R`, `-Pfoo bar` → the exact strings above); `command` (program, arguments and `LESS` via `get_program` / `get_args` / `get_envs`, for `Less` and `Shell`, per platform); `classify` via `ExitStatusExt::from_raw` (0 → `Shown`; `SIGINT` → `Shown`; 130 → `Shown`; 1, 3, 127, `SIGTERM` → `Failed`).
  - real processes (`#[cfg(unix)]`, sharing one test-local mutex because they change a process-wide signal disposition): `cat > <file>` receives exactly the parts in order → `Shown`; `true` (reads nothing) → `Shown`, no panic on the closed pipe; `exit 3` → `Failed`; `lesss` through the shell → `Failed`; `kill -INT $$` and `exit 130` → `Shown`; `Command::new("/nonexistent/pager")` → `NotStarted`; 5 MB to `head -n 1 > /dev/null` → returns promptly, `Shown`; the `SIGINT` disposition after `run` equals the one before.
  - Windows (`#[cfg(windows)]`): `exit /b 3` → `Failed`; `exit /b 0` → `Shown`.
- [ ] **Step 2: Failing sequencing and integration tests.** No hidden test hook (ADR-0019 §9). `deliver` unit tests with the pager and writer injected: `Direct` → report only; `Shown` → stdout gets only the closing status; `Failed` → report, then status; `NotStarted` → report only, no status, no warning; after `Failed` the warning line `warning: pager failed: <command>; the report follows` (PRD-0001 FR-22, AC-36) goes to the injected stderr writer **before** the report, with `<command>` escaped (`$PAGER`'s value, or `less`), and after `Shown`, `NotStarted` and `Direct` nothing is written to it; a failing warning write changes nothing; a failing writer on the report → `Err` (on `Shown`, a failing status write is still `Ok`). Exit code: `Shown` counts as delivered, so the code equals the unpaged run's — including `--exit-code` with duplicates (`3`). Integration (`tests/cli_integration.rs`, stdout is a pipe): with `PAGER` set to a command that would create a marker file, the marker is never created and stdout is byte-identical to a run without `PAGER`; the same with `--no-pager`.
- [ ] **Step 3:** Implement per ADR-0019. `Cargo.toml`: `libc = "0.2"` under `[target.'cfg(unix)'.dependencies]`, `windows-sys = { version = "0.61", features = ["Win32_Foundation", "Win32_System_Console"] }` under `[target.'cfg(windows)'.dependencies]` (both already in `Cargo.lock`; `cargo deny check` must stay clean). `unsafe` only in `pager::sys` (`#[allow(unsafe_code)]`, a `// SAFETY:` comment per block); the crate root gains `#![deny(unsafe_code)]`. `main.rs` follows the ADR's call sequence: the record's completion document is written **before** delivery on every path; then `should_page` → `screen_size` → `fits` → `render_closing_status` (only when paging) → `deliver`. The text handed to the pager is the report followed by the closing status. `sink.finish()` and all apply-time lines happen before the pager starts. The failed-pager warning line Task 2 settled is written through `deliver`'s injected `warn` closure (`streams::diag` in `main.rs`; the command escaped like every other user-controlled text); it is not written to the `--output` record.
- [ ] **Step 4:** `cargo test`, `make scale`; manual check on a real terminal (ADR-0019 §9): default `less` with a long report (opens at the end; `q`; the status below the restored screen); a report shorter than the screen (no pager, no status, no keypress); `PAGER=cat`; `PAGER=lesss`; `PAGER="less -R"`; `LESS=-FRX`; Ctrl-C, then `q`; the `--output` record has its completion document while the pager is still open — record the evidence in the commit message.
- [ ] **Step 5:** Commit `feat(output): page long reports on an interactive terminal (FR-22)`.

### Task 6: Docs, review, PR

- [ ] **Step 1: Product-owner** — guide (`output-and-exit-codes.md`: a "Long reports" section, `--no-pager`, `PAGER=cat` for scripts and CI, the Windows gap), README, acceptance against the built binary.
- [ ] **Step 2: Planning** — `status.md` (session entry with estimate vs. actual usage; also record that PR #110 merged and its 22% usage actual), `backlog.md` TASK-0340 → `done (this PR)`.
- [ ] **Step 3:** `make validate && cargo deny check`.
- [ ] **Step 4: Architect** — heavyweight code review of `git diff main...HEAD`; security read of the pager start; fix round; confirmation pass.
- [ ] **Step 5:** Push and open the PR (owner go-ahead); wait for CI; the owner merges.

---

## Self-review

| Spec requirement | Task |
|---|---|
| When paging applies — every condition | 3 |
| Pager choice; Windows gap | 3 |
| Closing status content | 4 (wording: 2) |
| Opened at the end; only when it does not fit; screen after exit | 1, 5 |
| Status only after a pager; after a failed pager the report is printed first | 5 |
| Exit codes; delivered; early quit | 5 (rule: 2) |
| Never in `--json`, redirect, or the record | 3, 5 |
| Starting `$PAGER` safely | 1, 5, 6 Step 4 |
| PRD / guide | 2, 6 |

Task 5's interface and tests were provisional until ADR-0019 existed; they now follow it (Task 1 Step 2). Still open: the owner's ruling on unset or empty `TERM` (ADR-0019, "Questions for the owner").
