# `--output` overwrite safety net — design (TASK-0342)

- **Date:** 2026-11-13
- **Status:** awaiting owner approval
- **Size:** a small change — one feature unit, one PR (plus two riders, see
  "Also on this branch")
- **Amends:** PRD-0001 FR-20 and ADR-0018 §6 ("Pre-existing `--output`
  target — truncate")

## Intent

A deduplication tool must not destroy any file without being asked. Today
`--output <file>` opens its file the way `>` does: an existing file is emptied
before scanning starts, even on a dry run. `imgdedup --output
photos/img_0412.jpg photos` destroys that photo. ADR-0018 §6 accepted this as
"the same risk as `>`" and left it to documentation; the owner has ruled that
is not good enough for this tool.

## Goals

- `--output` never empties or replaces a file that is not one of the tool's
  own run records, unless the user passes an explicit flag.
- Even then, and for the tool's own records too, the previous content is kept
  as a backup — one generation.
- Re-running the same command with the same `--output` path keeps working
  with no extra flag.

## Non-goals

- Protecting against anything other than the `--output` path (scanned files
  are governed by `--apply` / `--delete`, unchanged).
- Keeping more than one earlier generation.
- Changing the record's content, formats, durability or the stdout contract.

## Anti-goals

- A flag people learn to pass every time. If the everyday re-run needed it,
  it would be there on the day the path is mistyped.
- Backups piling up next to every record.

## Behavior

All of this happens at startup, with the other `--output` checks, before
anything is scanned. Every refusal exits `2` and writes no report.

| The `--output` path… | What happens |
|---|---|
| does not exist | Created, as today. |
| is an empty file | Written, as today. Nothing to keep, so no backup. |
| is an earlier imgdedup record | Moved to `<file>.bak`, then the new record is written. No flag needed. |
| is any other file | Refused. With `--overwrite-output`: moved to `<file>.bak`, then the record is written. |
| is a folder, or cannot be opened | Refused, as today. |

An existing `<file>.bak`:

| `<file>.bak`… | What happens |
|---|---|
| does not exist, or is empty | The backup is written there. |
| is an earlier imgdedup record | Replaced by the new backup. |
| is any other file | Refused — with or without `--overwrite-output`. |

The last row is this document's call, not something the owner stated: the
flag gives permission to replace the output file, and a foreign file that
happens to sit at the `.bak` name was never mentioned by the user at all.

**What counts as an earlier record.** A file whose first bytes are
`# imgdedup` followed by a space (the human format) or `{"type":"header"` (NDJSON). The format of
the old record does not have to match the format of the new run.

**Messages** (wording for the product-owner to settle; paths escaped as
everywhere else):

```text
error: output file photos/img_0412.jpg already exists and is not an imgdedup run record; pass --overwrite-output to replace it (it will be kept as photos/img_0412.jpg.bak)
error: cannot back up run.txt: run.txt.bak already exists and is not an imgdedup run record
```

## Approaches considered

- **A — never overwrite without a flag.** One-sentence rule, but the normal
  re-run fails until the flag is added, and a flag passed by habit protects
  nothing.
- **B — overwrite only the tool's own records; refuse everything else
  without a flag.** Keeps the re-run loop frictionless and the flag rare.
  **Chosen by the owner**, with the addition that whatever is replaced is
  first kept as `.bak`.
- **C — always move the old file aside, no refusal.** Nothing is lost, but a
  mistyped photo path still leaves the photo renamed with a text file under
  its name. Dropped as a standalone approach; its backup idea was folded
  into B.

For an existing `.bak`, the owner chose **one generation** over numbered
backups, which never replace anything but pile up under a nightly job.

## Decisions made here, for review

These are this document's calls, not the owner's:

1. **Move, not copy.** The old file is renamed to `.bak`: instant and atomic,
   where copying a large file takes time and can fail halfway. The owner said
   "copy"; the result on disk is the same. Exception: when the `--output`
   path is a symlink the content is copied instead, so the link stays in
   place and keeps pointing where it did.
2. **Flag name `--overwrite-output`.** Not `--force`, which in a tool with
   `--delete` could be read as forcing removals.
3. **The backup name is `<file>.bak`** — the full name plus `.bak`
   (`run.txt.bak`), not a replaced extension.
4. **A new file is created with "create new" semantics**, so a file that
   appears between the check and the open is not clobbered.
5. **The backup is not recorded as a run event.** It happens before the
   record exists. Whether to say `kept previous record as run.txt.bak` on
   the terminal is left to the product-owner; if shown, it follows FR-21's
   routing and never appears on a redirected stdout.

## Constraints

- FR-20's other guarantees stand: fail before scanning with exit `2`; stdout
  bytes identical with and without `--output`; no `BufWriter`, one write per
  record.
- `--overwrite-output` without `--output` is a usage error (exit `2`), like
  `--delete` without `--apply`.
- No new crates.
- TDD; `make validate`; one architect review on the branch diff.

## What the change touches

- `src/cli.rs` — the new flag and its validation.
- `src/main.rs` — the open step becomes: classify the target, back up if
  needed, then open. Best extracted into a small testable function (in
  `src/record.rs` or a new module) that takes the path and the flag and
  returns the opened file or a refusal.
- `tests/output_record.rs` — one test per table row above, plus the symlink
  case and the "existing photo is byte-for-byte intact after a refused run"
  case.
- PRD-0001 FR-20 and AC-28 (product-owner): the overwrite rule and new
  acceptance criteria.
- ADR-0018 §6 (architect): superseded by an amendment; implementation note.
- Guide: `output-and-exit-codes.md` "Practical notes" (currently "an existing
  file is overwritten"), README, `--help` text for `--output`.
- `claude-plugin/` — check whether the skill tells the agent to pass
  `--output`; if so it must know about the refusal.

## Also on this branch (owner request)

The owner asked for two unrelated items to ride the same branch and PR.

### A closed stderr must not abort a run

`eprintln!` panics when stderr cannot be written — for example when stderr is
a pipe whose reader has gone away. `src/main.rs` has six such sites; two can
fire in the middle of `--apply` (the per-file `warning:` skip line, and the
record-write failure message). A panic there aborts the apply loop partway
through, exits `101`, and leaves the `--output` record without its result.
The architect flagged this in the PR #107 review; it predates that work.

- **Fix.** All six sites write through one small helper that ignores a
  failed write to stderr. If nobody can read the diagnostic there is nothing
  better to do with it, and the run, its record and its exit code must not
  depend on it. `src/progress.rs` already writes this way.
- **Test.** An integration test runs `--apply --delete` with a skip-producing
  fixture and stderr connected to a closed pipe, and asserts the removals all
  happened, the exit code is the normal one, and the record is complete.
- **Stdout has the same defect and is included (this document's call).** The
  apply-time `trashed:` / `deleted:` line and the final report are written
  with `println!` / `print!`, so `imgdedup photos | head` panics once `head`
  exits. The status line gets the same ignore-on-failure treatment. A failed
  write of the final report must not panic either: the run still writes the
  completion document to the `--output` record, prints nothing further, and
  exits `1` — the report the user asked for was not delivered. The exit code
  is the one open point here; the alternative is `0`, treating a reader that
  stopped listening as the reader's choice.

### `status.md`

Session 35's entry gains the line recording that PR #108 merged.

## Left to the builder

- Where the classify-and-open function lives and its exact signature.
- How many bytes to read for the record check, and the handling of a file
  that cannot be read (treat as "any other file").
- Windows specifics of rename-over-existing for the `.bak` replacement.
