# Report paging and the closing status — design (TASK-0340, FR-22)

- **Date:** 2026-11-13
- **Status:** awaiting owner approval (all owner questions answered)
- **Size:** a project — one feature slice with its own ADR and a PRD-0001
  amendment
- **Replaces:** the tentative FR-22 text and AC-33..AC-37 in PRD-0001's
  Amendment 2026-08-06, which were never folded into the body

## Intent

After a long run the owner comes back to the terminal and wants two things:

1. **To see the status of the run** — and for it to stay in the terminal,
   above the prompt, after they are done looking.
2. **To be able to scroll up to the first output**, however long the report
   is.

The original complaint (a 69k-file run, 2026-08) was that the report overran
the terminal's scrollback, so its beginning was gone. `--output` (FR-20) has
since made sure results are not *lost*, but only for a user who thought of it
before starting. Paging needs no foresight.

The tentative FR-22 was written around a pager's default behavior: "can be
read from its first line". The owner's description is the opposite view: land
at the end, on the status, and scroll up from there.

## Goals

- On an interactive terminal, a report longer than the screen is shown in a
  pager, **opened at its end**.
- The last thing in that view, and what remains in the normal terminal after
  the pager closes, is a short **closing status**.
- A report that fits on the screen prints exactly as it does today, with no
  keypress needed.

## Non-goals

- Paging anything other than the human-readable report on an interactive
  terminal. Never under `--json`, never when stdout is redirected or piped.
- Changing the report. Its bytes — what a redirect captures and what
  `--output` records — stay exactly as they are.
- A built-in pager. An external program does the paging.
- Solving Windows. See "Platforms".

## Anti-goals

- A run that looks finished but lost its status: the status must not depend
  on what the pager does to the screen when it exits.
- A pager that swallows the tool's exit code, or turns "I pressed `q`" into an
  error.

## What the user sees

### The closing status

A short block built from lines the report already contains, with the counts
and none of the lists.

After a dry run:

```text
Summary: 3120 duplicate groups, 6412 files, 48213377112 bytes reclaimable.
12 warnings; see the full report.
```

After `--apply`:

```text
Summary: 3120 duplicate groups, 6412 files, 48213377112 bytes reclaimable.
Trashed 3288 files. Skipped 4 (left in place).
12 warnings; see the full report.
```

- The `Summary:` line is the report's own line, unchanged.
- The second line appears only on an `--apply` run; `Trashed` becomes
  `Deleted` with `--delete`; the `Skipped` part appears only when something
  was skipped.
- The warnings line appears only when there are warnings. Its wording is new
  and is the product-owner's to settle.
- Scan roots that could not be read are already reported on stderr and by the
  exit code; whether the status repeats their count is left to the
  product-owner.

The owner approved this block as what "status" means (2026-11-13).

Why it is needed and "open at the end" is not enough: on an `--apply` run the
report is ordered warnings, groups, `Summary:`, then every removed path, then
every skipped one. After a large apply its end is thousands of paths, with the
summary far above and the warnings at the very top.

### The flow

1. The run proceeds as today. Progress is on the terminal (FR-21) and
   finishes before anything below happens.
2. If paging applies (next section), the pager opens showing the full report
   followed by the closing status, positioned at the end.
3. The user scrolls up as far as the first line, and quits.
4. The closing status is printed into the normal terminal. The prompt
   returns.

When paging does not apply, the report prints exactly as today and none of
steps 2–4 happen; see "The status without a pager".

### When paging applies

All of:

- stdout **and stdin** are interactive terminals — someone has to be able to
  press a key;
- `TERM` is not `dumb` (editor shell buffers and the like);
- the output is the human-readable report (not `--json`);
- neither `--no-pager` nor `--timings` is given;
- `$PAGER` is not set to the empty string or to `cat` — the convention `git`
  uses for "no paging", which also gives scripts and CI an environment-level
  way to turn it off;
- a pager can be started;
- the report does not fit on the screen.

The stdin, `TERM` and `PAGER=cat` conditions are this document's calls. They
do not cover every place where a terminal exists but nobody is there (a CI
runner or `docker run -it` that allocates a pty): there the answer is
`--no-pager` or `PAGER=cat`, and the guide must say so.

On by default (owner decision, 2026-11-13: approach A).

A consequence the owner accepted with that choice: the run does not return to
the prompt until the pager is closed, so `imgdedup photos && next-command`
typed at a terminal waits for a keypress. `--no-pager` is the way out, and so
is anything that makes stdout not a terminal.

### The status without a pager

**Owner decision (2026-11-13): the closing status appears only after a
pager.** When no pager ran — the report fits on the screen, `--no-pager`,
Windows without `$PAGER`, a pager that could not be started — the output is
exactly what it is today, with no closing status.

The first draft of this document printed the status on every interactive run.
The reviewer pointed out that every `--apply` at a terminal would then change,
and a five-file apply would show `Summary:` and `Trashed 5 files` twice within
ten lines. The owner chose the narrower rule. Its cost, accepted: a long
`--apply` with `--no-pager`, or on Windows, still ends on the list of removed
paths.

One case follows the pager, not this rule: after a pager that **failed** (see
"Exit codes") the report is printed directly and the closing status follows
it, because a pager was started and the user is owed the status.

The status never appears when stdout is redirected or piped, never under
`--json`, and is never part of the `--output` record. stdout bytes when stdout
is not a terminal are unchanged (NFR-3).

### Exit codes

- The tool's exit code is its own; the pager's exit status is ignored.
- Quitting the pager before reaching any particular point is normal. A report
  handed to the pager counts as delivered: PRD-0001's rule that an
  undelivered report exits `1` (FR-15, AC-46, AC-47) does not fire, and no
  error is printed. The product-owner already recorded that this must be
  reconciled.
- If the pager cannot be started, the report is printed directly, as today.
- **A pager that starts and then fails** (this document's call). Started
  through a shell, a misspelled `$PAGER` still "starts"; the only evidence is
  a failure exit status. Rule: when the pager exits with a failure status, the
  report is printed directly after it, then the closing status. The user may
  in rare cases see the report twice (a pager that showed it and still exited
  non-zero); that is better than not seeing it at all. Whether a one-line
  hint naming the pager is shown is the product-owner's. The exit code is
  still the tool's own: the report reached the terminal, so it counts as
  delivered.

## Platforms

- **macOS, Linux:** `$PAGER` if set, otherwise `less`.
- **Windows:** no default pager. With `$PAGER` set it is used; otherwise the
  report prints directly, as today. This is the gap the
  product-owner recommended accepting rather than blocking the slice
  (PRD-0001, open question of 2026-08-06). Taken here as accepted — the owner
  should say so if not.

## Constraints

- No new crates unless the architect finds there is no sound way without one
  (the terminal's height is not available from `std`).
- TTY-ness is probed once in `main.rs` and injected (ADR-0018 §8).
- Nothing may be written to the terminal while the pager owns it.
- A panic-free run: the pager's pipe closing early is the normal case, not an
  error (see TASK-0342's closed-stream work).
- The pager is named by user-controlled environment. How it is started is a
  security decision that must be stated in the ADR, not inherited by analogy
  to `git`.
- TDD; `make validate`; architect code review; product-owner acceptance.

## Approaches considered

- **A — a pager opened at the end, then the status printed into the
  terminal.** Meets both of the owner's points for a report of any length,
  because scrolling happens inside the pager and not in the terminal's
  limited scrollback. Costs: the run waits for a keypress; `$PAGER` and
  Windows need design work. **Chosen by the owner.**
- **B — no pager; every run keeps a copy of its record in a fixed per-user
  file.** Nothing waits, no pager or Windows problem, and it covers a killed
  run without foresight. But "scroll up to the first output" only reaches as
  far as the terminal's scrollback — the original complaint — and every run
  would write a file listing the user's paths without being asked.
- **C — a pager only on request (`--pager`).** No surprise at the prompt, but
  it needs the same foresight as `--output` or `| less`, which is what failed
  in the field. Dropped.

## Left to the architect (ADR)

These decide how, not what, and each has a wrong answer that matters:

1. **Starting `$PAGER`.** Through a shell (needed for the common
   `PAGER="less -R"`) or split and exec'd directly; the trust model and any
   injection surface, stated explicitly.
2. **"Opened at the end" and "only when it does not fit".** Both are easy for
   `less` (`+G`, `-F`) and not expressible for an arbitrary `$PAGER`. Decide
   what is promised for `less` and what "whatever your pager does" means for
   the rest — and whether the tool should find out the terminal's height
   itself (and how) instead of relying on a pager flag.
3. **The screen after the pager exits.** Some pagers restore the previous
   screen, some leave their last view. The closing status is printed
   afterward either way; decide whether the tool asks `less` not to clear
   (`-X`) and what the terminal then looks like.
4. **Ordering.** Progress and apply-time status lines share stdout with the
   report on a terminal. They must be complete before the pager starts.
5. **The pipe.** Early quit, `EPIPE`, waiting for the pager to exit before
   printing the status, and what happens on Ctrl-C while the pager is up.
6. **`--output`.** The record's completion document is written regardless of
   what the pager does, and before or after it — decide which, given that a
   user may sit in the pager for a long time.

## Left to the product-owner (PRD)

- Replace the tentative FR-22 and AC-33..AC-37 with requirements for this
  design. AC-33's "the bytes displayed are identical to those written when
  redirected" no longer holds as written: the pager shows the report plus the
  closing status.
- The closing status: exact wording, the warnings line, unreadable roots.
- Reconcile FR-15 / AC-46 / AC-47 with "handed to the pager counts as
  delivered".
- Guide and README.

## Decisions made here, for review

This document's calls, not the owner's:

1. (Withdrawn — the owner decided the closing status appears only after a
   pager.)
2. The Windows gap is accepted.
3. `--no-pager` is the opt-out flag; no imgdedup-specific environment
   variable is added, because `PAGER=cat` (or empty) already turns paging off.
4. `--timings` disables paging, as the tentative FR-22 said.
5. The status is never written to the `--output` record.
6. Paging also requires an interactive stdin and a `TERM` other than `dumb`.
7. A pager that exits with a failure status is followed by the report printed
   directly.
