# Report Paging and Closing Status Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** On an interactive terminal, a human-readable report longer than the screen opens in a pager positioned at its end, and a short closing status stays in the terminal after the pager exits.

**Architecture:** The paging decision is a pure function of facts probed once in `main.rs`, like `streams::route`. A new module `src/pager.rs` owns starting the pager, feeding it, and waiting for it. The closing status is rendered by `report.rs` from the same data as the report. Nothing in the report's bytes, the `--output` record or redirected stdout changes.

**Tech Stack:** Rust, `std::process`; no new crates unless ADR-0019 rules one in (terminal height).

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

- [ ] **Step 1:** Dispatch the `architect` agent with the spec. It must decide, with reasons, each item under the spec's "Left to the architect": (1) starting `$PAGER` — through a shell or split and exec'd, and the security statement; (2) how "opened at the end" and "only when it does not fit" are achieved for `less` and what is promised for other pagers, including whether the tool learns the terminal height itself and how; (3) the screen after exit (`-X` or not); (4) ordering against progress and apply-time lines; (5) the pipe: early quit, waiting, Ctrl-C; (6) when the `--output` completion document is written relative to the pager.
- [ ] **Step 2:** The ADR must end with the exact interface for `src/pager.rs` and the `less` arguments or environment the tool sets. Update Tasks 3–5 of this plan where it differs.
- [ ] **Step 3:** `make validate`; commit `docs(adr): ADR-0019 report paging`.

### Task 2: PRD-0001 amendment (product-owner)

- [ ] **Step 1:** Dispatch the `product-owner` agent with the spec and ADR-0019: replace the tentative FR-22 and AC-33..AC-37 with requirements for the approved design, fold them into the body, and settle the closing status's exact wording (the warnings line; whether unreadable roots are counted). Reconcile FR-15 / AC-46 / AC-47 with "handed to the pager counts as delivered". Add `--no-pager` to §8.
- [ ] **Step 2:** `make validate`; commit `docs(prd): FR-22 report paging and closing status`.

### Task 3: The paging decision

**Files:** `src/streams.rs`, `src/cli.rs`, `src/main.rs`.

**Interfaces — produces:**

```rust
pub struct PagingFacts<'a> {
    pub json: bool,
    pub no_pager: bool,
    pub timings: bool,
    pub stdout_tty: bool,
    pub stdin_tty: bool,
    pub term: Option<&'a str>,      // $TERM
    pub pager_env: Option<&'a str>, // $PAGER
    pub windows: bool,
}

/// `None`: do not page. `Some(cmd)`: the pager command to start.
pub fn should_page(facts: &PagingFacts<'_>) -> Option<String>;
```

- [ ] **Step 1: Failing unit tests** in `src/streams.rs`, one per constraint: each of `json`, `no_pager`, `timings`, `!stdout_tty`, `!stdin_tty`, `term == Some("dumb")`, `pager_env == Some("")`, `pager_env == Some("cat")` gives `None`; unset `PAGER` gives `Some("less")` off Windows and `None` on Windows; `PAGER="less -R"` gives `Some("less -R")`. Plus a table test over all boolean combinations asserting `None` whenever any disabling fact is set.
- [ ] **Step 2:** Run — fails to compile.
- [ ] **Step 3:** Implement `should_page`; add `--no-pager` to `Cli` (help text: `Do not show a long report through a pager. By default, on an interactive terminal, a report longer than the screen opens in a pager positioned at its end.`); probe the facts in `main.rs` next to the existing `route` call. "Fits on the screen" is not part of this function — it is decided per ADR-0019.
- [ ] **Step 4:** `cargo test`; extend `help_lists_all_flags` with `--no-pager`.
- [ ] **Step 5:** Commit `feat(output): the paging decision (FR-22)`.

### Task 4: The closing status

**Files:** `src/report.rs`.

**Interfaces — produces:** `pub fn render_closing_status(...) -> String`, taking the same arguments as `report::render`; the returned text ends with a newline.

- [ ] **Step 1: Failing unit tests**, with the wording fixed by Task 2 (the spec's sketch until then):
  - dry run, no warnings → exactly the report's own `Summary:` line;
  - dry run with 12 warnings → that line plus `12 warnings; see the full report.`;
  - `--apply` → second line `Trashed 3 files.`; with skips `Trashed 3 files. Skipped 1 (left in place).`; `--delete` → `Deleted`;
  - the first line is byte-identical to the `Summary:` line inside `render`'s output for the same input (no parallel wording);
  - singular forms (`1 warning`, `1 file`).
- [ ] **Step 2:** Run — fails.
- [ ] **Step 3:** Implement by reusing the function that writes the report's `Summary:` line; extract it if it is inline.
- [ ] **Step 4:** `cargo test`; the golden `--json` test and every report test still pass unchanged.
- [ ] **Step 5:** Commit `feat(report): closing status (FR-22)`.

### Task 5: The pager

**Files:** `src/pager.rs` (create), `src/main.rs`, `tests/cli_integration.rs`.

**Interfaces — produces (confirm against ADR-0019):**

```rust
pub enum Paged {
    /// The pager ran and exited successfully (including an early quit).
    Shown,
    /// The pager started and exited with a failure status.
    Failed,
    /// The pager could not be started at all.
    NotStarted,
}

pub fn page(command: &str, text: &str) -> Paged;
```

- [ ] **Step 1: Failing unit tests** in `src/pager.rs` (`#[cfg(unix)]`), using small shell commands as the pager: `cat > <file>` receives exactly `text` → `Shown`; `true` (reads nothing, exits 0) → `Shown`, no panic on the closed pipe; `exit 3` → `Failed`; a command that does not exist → `Failed` or `NotStarted` per ADR-0019; 5 MB of text to a pager that reads one line and exits → returns promptly, `Shown`.
- [ ] **Step 2: Failing integration tests.** A pty is not available in the suite, so `main`'s wiring takes its facts from a small testable function; end-to-end tests drive the binary through a hidden test hook only if ADR-0019 allows one. Otherwise the wiring is covered by: unit tests of the function that sequences "page → on `Failed`/`NotStarted` print the report → print the status → exit code", with the pager and the writers injected. Required cases: `Shown` → stdout gets only the closing status; `Failed` → report, then status; `NotStarted` → report only, no status; exit code equals the unpaged run's in all three; `--exit-code` with duplicates stays `3`.
- [ ] **Step 3:** Implement `pager::page` per ADR-0019 and the sequencing in `main.rs`. The text handed to the pager is the report followed by the closing status. `sink.finish()` and all apply-time lines happen before `page`.
- [ ] **Step 4:** `cargo test`, `make scale`; manual check on a real terminal with `less`, with `PAGER=cat`, with `PAGER=lesss`, and with a report shorter than the screen — record the evidence in the commit message.
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

Open by design: Task 5's interface and tests are provisional until ADR-0019 exists; Task 1 Step 2 requires the plan to be updated then.
