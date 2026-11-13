# `--output` Overwrite Safety Net Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `--output` never empties a file that is not an imgdedup run record unless `--overwrite-output` is passed, always keeps one `.bak` generation, and a closed stderr or stdout can no longer panic a run.

**Architecture:** A new module `src/output_file.rs` owns "classify the target, back up if needed, open": `main.rs` calls one function and gets an open `File` or a refusal to print. All diagnostics in `main.rs` go through non-panicking write helpers in `src/streams.rs`. Nothing in `record.rs` changes.

**Tech Stack:** Rust (std only — no new crates), `clap`, `assert_cmd` + `tempfile` in tests.

**Spec:** [`docs/superpowers/specs/2026-11-13-output-overwrite-safety-net-design.md`](../specs/2026-11-13-output-overwrite-safety-net-design.md) — approved 2026-11-13; the owner ruled exit code `1` for an undeliverable report.

## Global Constraints

- One branch, one PR: `feat/output-overwrite-safety-net`, title `feat(output): never overwrite a foreign file with the run record (TASK-0342)`.
- TDD: failing test first. `make validate` and `cargo deny check` green before pushing. No new crates.
- Every `--output` refusal happens before scanning, exits `2`, writes nothing to stdout, and leaves every existing file byte-for-byte intact.
- A file is "an earlier record" when its first bytes are `# imgdedup` + space, or `{"type":"header"`. An unreadable file is "any other file".
- Backup name: the full file name plus `.bak` (`run.txt` → `run.txt.bak`). One generation.
- A `.bak` that exists, is not empty and is not a record refuses the run **with or without** `--overwrite-output`.
- Backup is a rename; when the `--output` path is a symlink it is a copy and the link stays.
- `--overwrite-output` without `--output` is a usage error, exit `2`.
- FR-20's other guarantees stand: stdout bytes identical with and without `--output`; no `BufWriter`; the mid-run message `error: could not write to <path>: <reason>` is unchanged.
- Paths in messages pass through `safe_text::escape_control_chars`.
- `--help` text cites no internal identifiers (`help_text_cites_no_internal_identifiers`).
- A failed write to stderr is ignored. A failed write of the final report exits `1`, after the completion document has been recorded.

## Review Focus

1. **`--output` names one of the scanned images** — the image must be intact after the refused run. Task 2, `refusing_leaves_an_existing_image_intact`.
2. **`--output` is a symlink to an earlier record** — the link must survive and still point at the same target; the old content lands in `<link>.bak`. Task 1, `a_symlinked_record_is_backed_up_by_copy_and_the_link_survives`.
3. **A record file cut short by `kill -9` inside its first line** (shorter than the marker) — treated as "any other file" unless empty; the user gets the refusal message, not a silent loss. Task 1, `a_file_shorter_than_the_marker_is_not_a_record`.
4. **`imgdedup photos | head -1`** — no panic, no backtrace on stderr, exit `1`. Task 4, `a_closed_stdout_does_not_panic_and_exits_1`.
5. **The backup target is a directory** (`run.txt.bak/`) — refused with the backup message, nothing moved. Task 1, `a_directory_at_the_backup_name_refuses`.

## File Structure

| File | Status | Responsibility |
|---|---|---|
| `src/output_file.rs` | create | classify the `--output` target, back up, open; refusal type and messages |
| `src/streams.rs` | modify | `diag()` and `write_stdout()` — writes that cannot panic |
| `src/cli.rs` | modify | `--overwrite-output`; validation |
| `src/main.rs` | modify | call `output_file::open`; route all prints through the helpers; exit `1` on an undelivered report |
| `tests/output_record.rs` | modify | end-to-end rows of the behavior table |
| `tests/cli_integration.rs` | modify | usage error; closed-pipe tests |
| PRD-0001, ADR-0018, guide, README, planning | modify | Task 5 |

## Batches and usage budget

Inline implementation, as for PR #107/#108. Estimates from this unit's two predecessors: inline TDD about 10–12% of a 5h window for Tasks 1–4; Task 5 has two agent dispatches (product-owner, architect) at about 10% each. Weekly: about 3–4% in total.

| Batch | Tasks | Est. 5h window |
|---|---|---|
| A — code | 1–4 | 10–12% |
| B — docs, review, PR | 5 | 20–25% |

Cut line: if the window has less than about 30% left after Batch A, stop there with the branch committed and run Batch B in a fresh window.

---

### Task 1: `output_file` — classify, back up, open

**Files:**

- Create: `src/output_file.rs`; add `mod output_file;` to `src/main.rs`.

**Interfaces:**

- Produces:

```rust
pub struct Opened {
    pub file: std::fs::File,
    /// Where the previous content was kept, if there was any.
    pub backed_up_to: Option<std::path::PathBuf>,
}

pub enum Refusal {
    /// The target exists, is not empty and is not a record; no flag given.
    NotARecord { path: PathBuf, backup: PathBuf },
    /// The `.bak` name is taken by something that is not a record.
    BackupBlocked { path: PathBuf, backup: PathBuf },
    /// The target or its backup could not be opened, moved or copied.
    Io { path: PathBuf, error: std::io::Error },
}

impl Refusal {
    /// The complete `error: …` line, paths and OS reason escaped.
    pub fn message(&self) -> String;
}

pub fn backup_path(path: &Path) -> PathBuf;          // run.txt -> run.txt.bak
pub fn open(path: &Path, overwrite: bool) -> Result<Opened, Refusal>;
```

- Exact messages:

```text
error: output file <path> already exists and is not an imgdedup run record; pass --overwrite-output to replace it (it will be kept as <backup>)
error: cannot back up <path>: <backup> already exists and is not an imgdedup run record
error: cannot open output file <path>: <reason>
```

- [x] **Step 1: Write the failing unit tests** (in `src/output_file.rs`, `tempfile` dirs). One test per line; each asserts the returned variant, the content of the target and of `.bak` afterwards, and `backed_up_to`:
  - `a_missing_file_is_created_without_backup`
  - `an_empty_file_is_written_without_backup`
  - `a_human_record_is_moved_to_bak` (content `# imgdedup 0.1.0 run record\n…`)
  - `an_ndjson_record_is_moved_to_bak` (content `{"type":"header",…}\n`)
  - `a_foreign_file_is_refused_and_untouched` → `NotARecord`, bytes unchanged, no `.bak`
  - `a_foreign_file_with_overwrite_is_moved_to_bak`
  - `an_existing_record_bak_is_replaced` (two generations in, newest two out)
  - `an_empty_bak_is_replaced`
  - `a_foreign_bak_refuses_even_with_overwrite` → `BackupBlocked`; target and `.bak` both unchanged
  - `a_directory_at_the_backup_name_refuses` → `BackupBlocked`
  - `a_file_shorter_than_the_marker_is_not_a_record` (content `# img`) → `NotARecord`
  - `a_directory_target_is_an_io_refusal`
  - `messages_escape_control_characters` (path containing `\u{1b}[2J` and `\n`: one line, no raw ESC)
  - `#[cfg(unix)] a_symlinked_record_is_backed_up_by_copy_and_the_link_survives` — target of the link is truncated and rewritten, `read_link` unchanged, `<link>.bak` is a regular file holding the old content
  - `backup_path_appends_bak_to_the_full_name` (`a/run.txt` → `a/run.txt.bak`, `run` → `run.bak`)

- [x] **Step 2: Run** `cargo test output_file` — expected: does not compile / fails.

- [x] **Step 3: Implement.** Order inside `open`, which the tests do not fully determine:
  1. `symlink_metadata(path)`; not found → `OpenOptions::new().write(true).create_new(true)`; if that reports `AlreadyExists`, start again from step 1 (once).
  2. A directory, or any other metadata error → `Io`.
  3. Classify by reading at most 32 bytes through the path (following a symlink): empty, record, or foreign. A read error → foreign.
  4. Empty → open with truncate, no backup.
  5. Foreign and `!overwrite` → `NotARecord`.
  6. Check `backup_path`: missing, empty file, or record → usable; anything else (foreign file, directory, unreadable) → `BackupBlocked`.
  7. Back up: `rename(path, backup)` — or, when `path` is a symlink, `copy(path, backup)`. On Windows `rename` over an existing file can fail: remove a usable existing backup first.
  8. Open the target: `create_new` after a rename, truncate after a copy.

- [x] **Step 4: Run** `cargo test output_file` — expected: all pass. `cargo clippy --all-targets -- -D warnings` clean.

- [x] **Step 5: Commit** — `feat(output): classify, back up and open the --output target (TASK-0342)`.

### Task 2: `--overwrite-output` and the wiring in `main.rs`

**Files:**

- Modify: `src/cli.rs`, `src/main.rs`, `tests/output_record.rs`, `tests/cli_integration.rs`.

**Interfaces:**

- Consumes: `output_file::open`, `Refusal::message`.
- Produces: `Cli::overwrite_output: bool` (`--overwrite-output`).

- [x] **Step 1: Write the failing tests.**
  - `src/cli.rs` unit: `overwrite_output_requires_output` — `validate()` returns `Err("--overwrite-output requires --output")`.
  - `tests/cli_integration.rs`: `overwrite_output_without_output_is_a_usage_error` (exit `2`, empty stdout); extend `help_lists_all_flags` with `--overwrite-output`.
  - `tests/output_record.rs`, each with the three-pair `fixture`:
    - `rerunning_with_the_same_output_keeps_the_previous_record_as_bak` — two runs, no flag, both exit `0`; `.bak` equals the first run's record byte for byte.
    - `a_json_rerun_backs_up_a_human_record` — formats may differ.
    - `refusing_leaves_an_existing_image_intact` — `--output` names `dir0/a.png` inside the scanned tree: exit `2`, stdout empty, stderr starts with `error: output file` and contains `--overwrite-output`, the image's bytes are unchanged, no `.bak` exists, stderr has no `scanning`.
    - `overwrite_output_replaces_a_foreign_file_and_keeps_it_as_bak` — exit `0`; `.bak` holds the original bytes; the record ends with stdout.
    - `a_foreign_bak_refuses_the_run` — exit `2`, stderr starts with `error: cannot back up`.
  - Update `unwritable_output_path_fails_before_scanning` only if its assertions no longer hold (the directory and missing-parent cases keep `error: cannot open output file`).

- [x] **Step 2: Run** the new tests — expected: fail.

- [x] **Step 3: Implement.**
  - `src/cli.rs`: the flag, placed after `output`. Help text (no internal identifiers):

    ```text
    Replace an existing FILE given to --output even if it is not an imgdedup
    run record. Without this flag, --output replaces only an earlier run
    record or an empty file and refuses anything else. Whatever is replaced is
    kept as FILE.bak. Requires --output.
    ```

    Extend the `--output` help: replace the sentence `An existing FILE is overwritten.` with `An earlier run record at FILE is kept as FILE.bak and replaced; any other existing file is refused unless --overwrite-output is given.`
  - `Cli::validate`: the new rule after the `--delete` rule.
  - `src/main.rs`: replace the `File::create` block with `output_file::open(path, args.overwrite_output)`; on `Err(r)` print `r.message()` and return `exit_code::USAGE`. `backed_up_to` is not printed (the product-owner decides in Task 5 whether a notice is wanted).
  - `record::Invocation` is unchanged: the flag does not shape the result.

- [x] **Step 4: Run** `cargo test` — expected: all green, including the unchanged AC-26/AC-28 tests.

- [x] **Step 5: Commit** — `feat(output): refuse to overwrite a foreign file without --overwrite-output (TASK-0342)`.

### Task 3: A closed stderr cannot abort a run

**Files:**

- Modify: `src/streams.rs`, `src/main.rs`, `tests/cli_integration.rs`.

**Interfaces:**

- Produces: `pub fn diag(text: &str)` in `streams` — one `write_all` of `text` to stderr, result ignored. `text` carries its own trailing newline.

- [x] **Step 1: Write the failing test** `#[cfg(unix)] a_closed_stderr_does_not_abort_an_apply` in `tests/cli_integration.rs`: a fixture with duplicates where at least one removal is skipped (reuse the suite's existing skip-producing setup) plus an unreadable-root argument so several diagnostics fire; spawn with `std::process::Command`, `stderr(Stdio::piped())`, drop the child's stderr handle before `wait`; run with `--apply --delete --json --no-progress --output <file>`. Assert: exit code is the same as the identical run with stderr open; the stdout JSON parses and its `actions.removed` matches that run; the record's last line has `"type":"result"`.

- [x] **Step 2: Run it** — expected: fails with exit `101`.

- [x] **Step 3: Implement** `streams::diag` and replace all six `eprintln!` / `eprint!` sites in `src/main.rs` with it (`diag(&format!("…\n"))`; the record-failure closure passes `interrupting_line(..)` straight through). After this step `grep -n 'eprint' src/main.rs` shows only comments.

- [x] **Step 4: Run** `cargo test` — expected: green.

- [x] **Step 5: Commit** — `fix: a closed stderr no longer aborts a run`.

### Task 4: A closed stdout cannot panic; an undelivered report exits 1

**Files:**

- Modify: `src/streams.rs`, `src/main.rs`, `tests/cli_integration.rs`.

**Interfaces:**

- Produces: `pub fn write_stdout(text: &str) -> bool` in `streams` — `write_all` + `flush` on stdout; `true` when both succeeded.

- [x] **Step 1: Write the failing tests** in `tests/cli_integration.rs` (`#[cfg(unix)]`):
  - `a_closed_stdout_does_not_panic_and_exits_1` — spawn with `stdout(Stdio::piped())`, drop the handle before `wait`, `--no-progress`: exit code `1`; stderr does not contain `panicked`.
  - `a_closed_stdout_still_completes_the_record` — same with `--output`: the record contains `===== RESULT`.
  - `a_closed_stdout_exits_1_even_with_exit_code_flag` — `--exit-code` on a tree with duplicates: `1`, not `3`.

- [x] **Step 2: Run them** — expected: exit `101`.

- [x] **Step 3: Implement.**
  - `print!("{rendered}")` → `let delivered = streams::write_stdout(&rendered);`
  - The apply-time `println!` status line → `streams::write_stdout(..)`, result ignored.
  - The record's `Result` event is still recorded after the write attempt, unconditionally.
  - `exit_code::compute(root_errors || recorder.is_degraded() || !delivered, …)`.
  - No message is printed for the undelivered report: the usual cause is a reader that stopped reading on purpose.

- [x] **Step 4: Run** `cargo test` and `make scale` — expected: green.

- [x] **Step 5: Commit** — `fix: a closed stdout no longer panics; an undelivered report exits 1`.

### Task 5: Requirements, docs, review, PR

**Files:**

- Modify: `docs/prd/PRD-0001-exact-image-deduplication.md`, `docs/adr/0018-durable-run-record-and-output-stream-routing.md`, `README.md`, `docs/guide/output-and-exit-codes.md`, `docs/guide/getting-started.md`, `docs/planning/status.md`, `docs/planning/backlog.md`.

- [x] **Step 1: Product-owner (`product-owner` agent).** One dispatch, with the spec and real output from the built binary:
  - PRD-0001: a dated amendment to FR-20 replacing "an existing file is overwritten" with the spec's behavior tables; new acceptance criteria for each table row, for `--overwrite-output` without `--output`, and for the closed-stderr / closed-stdout behavior (FR-15: exit `1` for an undelivered report).
  - Rule on the three message wordings and on whether a `kept previous record as <file>.bak` notice should be shown (and where, under FR-21's routing). Any wording change comes back as a code change before review.
  - Guide: `output-and-exit-codes.md` "Practical notes" and the exit-code table (`2` for a refused output file; `1` for an undelivered report), README's "Saving results to a file", `getting-started.md`.

- [x] **Step 2: ADR-0018.** A dated amendment superseding §6 ("truncate") with the classify / back-up / refuse rule and why the earlier "same risk as `>`" judgment was overturned by the owner; an implementation note for the non-panicking writers.

- [x] **Step 3: Planning.** `status.md`: session entry with estimate vs. actual usage, and the line recording that PR #108 merged. `backlog.md`: TASK-0342 → `done (this PR)`.

- [x] **Step 4: Validate.** `make validate && cargo deny check` — expected: `✓ local validation passed`, deny clean.

- [x] **Step 5: Architect review (`architect` agent)** on `git diff main...HEAD`: heavyweight code review plus a ruling on the ADR-0018 amendment. Security read: the check-then-open window, symlinks at the target and at the `.bak` name, rename across the target's own directory only, and Windows rename semantics. Iterate until converged.

- [x] **Step 6: Push and open the PR** (owner go-ahead), wait for `checks`, `ci-gate`, `security`; the owner merges.

---

## Self-review

| Spec requirement | Task |
|---|---|
| Behavior table, all rows; `.bak` table, all rows | 1 (unit), 2 (end to end) |
| Record markers; unreadable file is foreign | 1 |
| Move not copy; symlink copies | 1 |
| `--overwrite-output`, usage error without `--output` | 2 |
| Messages, escaped | 1 |
| Create-new for a missing file | 1 Step 3.1 |
| Backup not recorded as a run event; notice left to the product-owner | 2, 5 |
| Closed stderr | 3 |
| Closed stdout, exit `1`, record still completed | 4 |
| PRD / ADR / guide / plugin check (the plugin does not pass `--output`: no change) | 5 |
| `status.md` PR #108 line | 5 Step 3 |
