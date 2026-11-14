# ADR-0019: Report paging — starting, positioning and waiting for the pager

- **Status:** proposed (becomes `accepted` in the FR-22 slice's PR, the way
  ADR-0018 went from `proposed` to `accepted` in #107)
- **Date:** 2026-11-14
- **Deciders:** architect, human owner
- **Related:** [PRD-0001](../prd/PRD-0001-exact-image-deduplication.md) — the
  tentative FR-22 and AC-33..AC-37 (Amendment 2026-08-06), FR-15, AC-46,
  AC-47; the owner-approved
  [design](../superpowers/specs/2026-11-13-report-paging-design.md) and its
  [implementation plan](../superpowers/plans/2026-11-13-report-paging.md);
  [ADR-0018](0018-durable-run-record-and-output-stream-routing.md) (§3
  durability, §8 stream routing, "Writers that cannot panic");
  [ADR-0017](0017-performance-instrumentation-tracing-spans.md) (`--timings`,
  `process::exit` runs no destructors); [ADR-0005](0005-mandatory-tdd-and-security-gate.md)
  (security gate); TASK-0340.

## Context and problem statement

The owner approved a design (2026-11-13) for FR-22: on an interactive
terminal a report longer than the screen opens in a pager **positioned at its
end**; the last thing in the pager, and the thing left in the terminal after
it closes, is a short **closing status**. The owner's decisions are fixed and
not reopened here:

- the pager is on by default on an interactive terminal (approach A);
- it opens at the **end** of the report;
- the closing status is the last thing in the pager and is printed into the
  normal terminal after the pager exits;
- the status appears **only after a pager** — never after a report that was
  printed directly — except after a pager that started and failed, where the
  report is printed directly and then the status;
- Windows has no default pager;
- `--no-pager`; `PAGER` empty or `cat` disables paging; `--timings` disables
  paging.

The design hands six "how" questions to the architect (its "Left to the
architect" section): how `$PAGER` is started and what the injection surface
is; how "opened at the end" and "only when it does not fit" are achieved;
the screen after the pager exits; ordering against progress and apply-time
lines; the pipe (early quit, waiting, Ctrl-C); and when the `--output`
completion document is written. The plan adds two more: how "could not be
started" and "started and failed" are told apart, and how the feature is
tested without a pty in the suite.

**What the code does today** (read for this ADR): `main.rs` finishes progress
(`sink.finish()`), prints run-level errors to stderr, runs `--apply` (whose
`trashed:`/`deleted:` lines go to stdout on a terminal, ADR-0018 §9 #6, and
whose skips go to stderr), renders the report into one `String`, writes it
with `streams::write_stdout`, and **then** writes the record's completion
document. The Rust runtime leaves `SIGPIPE` ignored in the process, so a
closed pipe is an `EPIPE` error, not a death; `write_stdout` and
`streams::diag` already turn that into a value (ADR-0018, "Writers that
cannot panic"). The crate has no `unsafe` code and installs no signal
handlers.

**Evidence.** Behaviour in question was measured rather than recalled, on
macOS (Darwin 27.2) with `/usr/bin/less` **668** (`less --version`:
"less 668 (POSIX regular expressions)"), `/bin/sh` = bash 3.2.57, `dash`,
rustc 1.97.1, using a throwaway Rust harness that spawns a pager the way this
ADR specifies and a Python `pty` driver (24×80, `TERM=xterm-256color`) that
types keys and records the raw terminal bytes. Each claim below is tagged
**[measured]** or **[not measured]**; the
[evidence table](#evidence) lists every experiment. Nothing was measured on
Linux or Windows.

## Decision drivers

- **The owner's flow must hold for the common setups**, including
  `PAGER=less` and a user `LESS` such as `-R` or `-FRX`, which are the
  defaults of popular shell setups (the development machine for this ADR has
  `PAGER=less`, `LESS=-R`).
- **The status rule must hold**: a status printed after a report that did not
  go through a pager is exactly the duplication the owner rejected.
- **"I pressed `q`" is not an error**, and the tool's exit code is its own
  (design, "Exit codes").
- **Nothing may be written to the terminal while the pager owns it.**
- **No panic, no hang** — a pipe closed early is the normal case.
- **Security**: the pager is named by user-controlled environment; scanned
  file names are untrusted input and must never reach a command line.
- **Testable without a pty**: decisions are pure functions over injected
  facts (ADR-0018 §8's pattern); impurity is confined to the boundary.
- **No new crate unless there is no sound way without one** (design,
  "Constraints").

## Considered options

1. **Starting `$PAGER`:** S1 through the platform shell (`/bin/sh -c`;
   `cmd.exe` on Windows); S2 split on whitespace and exec directly; S3 exec
   the value as one program name.
2. **Positioning (`+G`) and "do not quit by yourself":** P1 `less` arguments
   on the command line (default pager only); P2 the `LESS` environment
   variable, set only when unset (git's rule); P3 the `LESS` environment
   variable, always, wrapped around the user's value; P4 nothing — leave it
   to the pager.
3. **"Only when it does not fit":** F1 rely on `less -F`; F2 the tool
   measures the terminal (`ioctl(TIOCGWINSZ)` / the console API) through
   `libc` / `windows-sys`; F3 the `terminal_size` crate; F4 `stty size` in a
   subprocess; F5 the `LINES` variable.
4. **Screen after exit:** X1 ask `less` not to use the alternate screen
   (`-X`); X2 leave the screen to `less`'s default and the user's `LESS`.
5. **Ctrl-C while the pager is up:** C1 do nothing; C2 ignore `SIGINT` from
   spawn to exit, via `libc`; C3 `signal-hook`; C4 hand-written `extern "C"`
   declarations.
6. **Feeding the pipe:** W1 a writer thread; W2 write on the calling thread,
   then wait.
7. **Completion document vs. pager:** R1 after the pager (today's order);
   R2 before any delivery.

## Decision outcome

### 1. Starting the pager — S1, the platform shell for `$PAGER`; `less` directly otherwise

| `$PAGER` | Unix (macOS, Linux) | Windows |
|---|---|---|
| unset | `less`, exec'd directly, found on `PATH` | no pager (owner decision) |
| set, not empty, not exactly `cat` | `/bin/sh -c <value>` | `cmd.exe /D /S /C "<value>"` |
| empty, or exactly `cat` | no pager | no pager |

- **Why a shell.** `PAGER="less -R"` is the common form, and `PAGER` values
  in the wild also carry quoted paths (`"/opt/my tools/less" -R`) and
  occasionally pipelines. Splitting on whitespace (S2) gets quoting wrong in
  a way that cannot be fixed without reimplementing a shell parser; exec'ing
  the value as one name (S3) breaks the common form. Running the value
  through the platform's command interpreter means it behaves exactly as if
  the user had typed it, which is the only behaviour a user can predict.
- **The trust model, stated explicitly.** `PAGER` (and `LESS`, below) are the
  **user's own environment**, set by the same person, with the same
  privileges, as the shell that starts `imgdedup`. Anyone who can set them
  can already run any command as that user, so executing them grants nothing
  new. `imgdedup` is not setuid and crosses no privilege boundary. Under
  `sudo`, whatever environment `sudo` passes through is the administrator's
  choice; a pager run as root with shell escapes (`less`'s `!`) gives nothing
  the `sudo` user does not already have.
- **The injection surface, stated explicitly.** It is closed by construction:
  - The value of `PAGER` is passed as **one** argument (`Command::arg`) to
    `sh -c` — `imgdedup` never builds a shell string, never concatenates
    anything onto the value, and passes no further arguments (no `"$@"`).
  - **Nothing derived from scanned file names or report content ever reaches
    a command line or the environment.** The report reaches the pager only
    through its stdin pipe, as data. The only environment variable the tool
    sets is `LESS`, built from a fixed string and the user's own `LESS`.
  - The report itself is already terminal-safe: every path and reason in it
    passed through `safe_text::escape_control_chars`, so even a pager in raw
    mode (`less -r`) receives no control bytes from file names. The closing
    status carries counts only; any future path in it must be escaped the
    same way.
  - The child inherits no descriptor beyond stdin/stdout/stderr: `std` opens
    every file (including the `--output` record) close-on-exec.
- **`/bin/sh` by absolute path** rather than `sh` from `PATH`: every
  supported Unix has it, and it removes one lookup from the question.
  No `--` is passed after `-c`: a value beginning with `-` makes the shell
  exit `2` [measured, bash and dash], which lands safely in the
  "started and failed" path below.
- **The default `less` is found on `PATH`** (`Command::new("less")`) — the
  same lookup the user's own `less` command gets; `PATH` is the user's
  environment like `PAGER`. Not found → the spawn fails with `NotFound`
  [measured] → "could not be started" → the report prints as today.
- **Windows with `$PAGER` set** uses `cmd.exe /D /S /C "<value>"`, passing
  the quoted value through `CommandExt::raw_arg` so `cmd` sees it exactly as
  typed (`/S` strips the outer quotes and keeps the rest literally; `/D`
  skips the user's `AutoRun` commands, the same switch set Node.js uses for
  its `shell` option). `cmd.exe` resolves through Rust's Windows search,
  which does not search the current directory. The trust model is the same
  as on Unix. **[not measured]** — no Windows machine was available; the
  unit tests in plan Task 5 run on the `windows-latest` CI leg.

### 2. Opened at the end, and only when it does not fit

**The tool measures the terminal itself (F2) and starts the pager only when
the report does not fit; it never relies on `less -F` for that decision.**

Why F1 (`less -F`) is unsound here: with `-F`, `less` exits at once on a
short report — 7 ms, exit status `0`, after printing it to the normal screen
[measured; with `+G` it also pads the screen with `~` rows above the text].
The tool cannot tell that from a user who paged and pressed `q`, so it would
print the closing status after a short report: a five-file apply would show
`Summary:` and `Trashed 5 files` twice — exactly what the owner's rule
forbids. A timing heuristic ("exited within 50 ms") was rejected as unsound.

**"Fits"** is decided by a pure function over the report and the screen
size:

- `rows` and `cols` come from `ioctl(STDOUT_FILENO, TIOCGWINSZ)` on Unix
  [measured on the pty] and from `GetConsoleScreenBufferInfo` on the
  stdout handle on Windows (`srWindow`) [not measured].
- **Unknown size → no pager.** A failed probe or a reported size of `0` rows
  or `0` columns (typical of a pty nobody sized — a CI runner, `docker run -t`)
  means the report prints as today. This doubles as one more guard against
  "a terminal with nobody at it".
- A report **fits** when its displayed rows are at most `rows - 1` (one row
  stays free for the prompt that follows). Each `\n`-terminated line takes
  `max(1, ceil(width / cols))` rows, where `width` counts **1 for an ASCII
  character and 2 for any other** — conservative for wide (CJK) characters
  and an overestimate for accented letters and combining marks. The report
  contains no tabs or other control characters (they are escaped), so no
  other cases arise. The count stops as soon as it exceeds the budget, so a
  multi-megabyte report costs at most one screen's worth of lines.
- Erring towards "does not fit" opens a pager for a report a few rows shorter
  than the screen (one `q`); erring the other way would print a report that
  scrolls slightly. Both are cosmetic at the boundary; overestimating keeps
  the original complaint (lost beginning) from recurring. A Unicode width
  table (`unicode-width`) would make the boundary exact; not worth a crate.
- `less` with a locale that is not UTF-8 shows each non-ASCII byte as `<C3>`
  (four columns), which this count underestimates. That is the user's
  locale and equally true of `… | less`; accepted.

**Positioning (P3).** For **every** pager it starts, the tool sets one
environment variable on the child (only the child): `LESS`, built as
described in [the final section](#exact-less-environment). `+G` opens at the
end; `-+E -+e -+F` reset the three options that make `less` quit **without
a keypress** (at end of file, at the second end of file, and when the text
fits on one screen).

- **Why the environment and not arguments (P1).** Command-line arguments
  work only for the default pager. `PAGER=less` and `PAGER="less -R"` — the
  most common settings — would then open at the **start**, and the owner's
  flow would fail on exactly the machines it was designed on. `LESS` reaches
  every `less` the tool starts, including one started by a wrapper such as
  `bat`.
- **Why not "only when unset" (P2, git's rule).** With the user's `LESS=-R`
  set, P2 adds nothing and the report opens at the start [measured: without
  `+G`, `less` opens on the first screen]. git can afford the rule because its flags are a
  preference; for this design `+G` is the owner's requirement.
- **Why the `$` separators.** Naively prepending breaks real values: with
  `LESS="+G -R"` the initial command swallows the space and `-R` and `less` opens on an
  error prompt; with `LESS="-Pfoo bar +G"` the prompt string swallows `+G`
  [both measured]. `less` ends an option string at `$` (its manual, `LESS`
  section), and `+G$<user>$-+E -+e -+F` worked for every value tried: unset,
  empty, `-R`, `-FRX`, `-E`, `-e`, `-F`, `-Pfoo bar` — the pager stayed open
  until `q` in every case, for both a long and a short text [measured]. The
  user's own options are parsed between the two halves and still apply
  (colours, prompts, keys, `-X`); options given in `$PAGER` itself
  (`PAGER="less -F"`) are command-line options and win over `LESS`
  [measured] — the user's explicit command is respected.
- **What is promised.** For `less` (the default, or any `$PAGER` that runs
  `less`): opened at the end, never quits on its own, the user's `LESS`
  settings otherwise applied. For any other `$PAGER`: **whatever that pager
  does** — the tool starts it only when the report does not fit, feeds it the
  report and the closing status, and prints the status after it exits; where
  it opens and what it leaves on the screen are the pager's. `LESS` is
  harmless to programs that do not read it.
- **Residual, accepted.** A broken user `LESS` (for example `-j` with no
  number) makes `less` print an error and exit `0` [measured]; the tool then
  sees "shown". That is equally broken with `… | less`. Very old `less`
  versions were not tested; the `$` terminator is documented behaviour of
  long standing, but **[not measured]** below 668. BusyBox `less` (Alpine)
  is not expected to read `LESS` at all and would open at the start
  **[not measured]**.

### 3. The screen after exit — X2, no `-X`

The tool does **not** pass `-X` and does not override an `X` in the user's
`LESS`.

- **Without `-X` (the default).** `less` switches to the terminal's
  alternate screen on start and back on exit (`ESC[?1049h` / `ESC[?1049l`)
  [measured]. After `q` the user sees the normal screen as it was before the
  pager — the command line, the progress completion line, any `trashed:` /
  `deleted:` lines — and the tool then prints the closing status below it.
  The report is not left in the scrollback; that is the pager's purpose, and
  the closing status is the thing that stays. This is the owner's flow.
- **With `-X` (asked for by the user's `LESS`, e.g. the popular `-FRX`).**
  No alternate screen [measured]: the pager's last page — the end of the
  report and the closing status — stays on the normal screen, and the tool
  prints the status again directly below it, so the status appears twice in
  adjacent lines. That is the user's configured pager behaviour, and the
  status printed afterwards is still correct and last; overriding the user's
  `X` (`-+X`) was rejected because keeping the last page on screen is
  precisely what `X` is set for. The guide should say this in one sentence.
- Either way the anti-goal holds: the status is printed by the tool, after
  the pager has exited, so it never depends on what the pager does to the
  screen.

### 4. Ordering — everything else is finished before the pager starts

Order in `main.rs` (unchanged up to rendering; see the
[call sequence](#call-sequence-in-mainrs)):

1. progress `sink.finish()` (already before everything below, AC-20);
2. run-level root errors to stderr;
3. `--apply` with its `trashed:`/`deleted:` lines (stdout) and skip warnings
   (stderr) — all written synchronously from the apply loop;
4. render the report;
5. **write the record's completion document** (§6);
6. decide paging and, if paging, start the pager.

While the pager runs, the main thread is blocked inside `pager::run` writing
to the pipe or waiting, and no other thread exists that writes to a terminal
(the progress renderer has finished; rayon's workers are idle; there is no
writer thread, §5). The record writes nothing after step 5, so a
record-write failure — the only mid-run stderr writer (ADR-0018 §7) — cannot
fire while the pager is up. `--timings`, whose stage lines are stderr
writers, disables paging. **Invariant for code review:** between the spawn
and the return of `wait` nothing in the process writes to stdout or stderr.

The shell's own message for a misspelled `PAGER` (`sh: lesss: command not
found`) is written by the shell to the terminal before any pager owns it
[measured]; it is the one line the user sees ahead of the directly printed
report in that case.

### 5. The pipe — written on the calling thread; `SIGINT` ignored while waiting

- **Stdio.** The child's stdin is a pipe; its stdout and stderr are
  inherited (the terminal). The child gets no access to the tool's stdin;
  `less` reads keys from `/dev/tty`.
- **No writer thread (W2).** The report and the status are written with
  `write_all` on the calling thread, the pipe is then closed (EOF), and the
  tool waits. Only stdin is piped, so there is no second pipe that could
  fill and deadlock: a pager that is slow to read just makes `write_all`
  block until it reads or exits. On an early quit the pager's exit closes
  the pipe and `write_all` returns `BrokenPipe` at once — on the first
  screen of a 5 MB, 100 000-line report, promptly after `q` [measured]. A
  thread (W1) was measured to behave identically and adds a spawn that can
  fail, a join, and a panic surface for no gain.
- **Write errors are not errors.** `BrokenPipe` is the normal early quit.
  Any write error stops writing; the pager's exit status alone decides the
  outcome. Nothing is printed for it.
- **`SIGPIPE`.** The Rust runtime ignores `SIGPIPE` in the tool, so the
  closed pipe is an `EPIPE` error, never a death [measured: the disposition
  is `SIG_IGN`]; the spawned pager gets the default disposition back from
  `std` [measured: `yes | head` inside the child dies of `SIGPIPE`]. Nothing
  to change.
- **Waiting.** The status is printed only after `wait` returns. With `+G`,
  `less` reads the whole input before showing the end; 5 MB took under half
  a second [measured].
- **Ctrl-C (C2).** The terminal sends `SIGINT` to its whole foreground
  process group: the tool and the pager. Measured under an interactive bash
  with job control: when the tool does **not** ignore it, the tool dies
  (shell sees exit `130`), `less` restores the screen and exits, the
  terminal is usable, but the closing status is never printed and the
  tool's own exit code is replaced [measured]. When the tool ignores
  `SIGINT`, `less` 668 just beeps and stays [measured — `less` handles
  `SIGINT` itself and does not exit on it]; the user presses `q`, the status
  is printed and the tool exits with its own code [measured]. Therefore:
  - the tool sets `SIGINT` to `SIG_IGN` **after** a successful spawn —
    so the pager does not inherit the ignored disposition (ignored signals
    survive `exec`; handlers do not) — and restores the **previous**
    disposition when `wait` returns, through a guard value. Restoring the
    previous value rather than the default matters: a tool started in the
    background by a non-interactive shell inherits `SIGINT` already ignored;
  - the microsecond window between spawn and ignore is accepted: a Ctrl-C
    landing there behaves like C1 (tool dies, terminal usable);
  - `SIGQUIT` (Ctrl-\\) and `SIGTSTP` (Ctrl-Z) are left at their defaults:
    the first is an explicit hard abort of both; the second stops the job,
    and `less` restores the terminal on suspend and `fg` resumes both
    [not measured];
  - a terminal that is closed sends `SIGHUP`: tool and pager die; the record
    is already complete (§6).
- **A pager killed by a signal** other than `SIGINT` (`SIGTERM`, `SIGKILL`,
  a crash) counts as "started and failed" (§7): the report is printed
  directly, then the status. A pager that ends by `SIGINT` — killed directly,
  or exit status `130` from a shell that relays it [both measured as the
  statuses `wait` returns] — counts as **shown**: the user interrupted it on
  purpose, and printing the whole report after a deliberate Ctrl-C would be
  hostile. A pager that ignores keys and never exits can still be left with
  Ctrl-\\ or Ctrl-Z.
- **Windows Ctrl-C [not measured]:** the console delivers `CTRL_C_EVENT` to
  every process attached to it and the tool installs no handler, so Ctrl-C
  ends the tool (and whatever the pager does with it is the pager's). The
  record is already complete. Accepted with the Windows gap; ignoring it would
  need `SetConsoleCtrlHandler` timed after the spawn — a follow-up only if a
  Windows user reports it.

### 6. `--output` — R2, the completion document is written before any delivery

`main.rs` moves `recorder.record(RecordEvent::Result { … })` from after the
stdout write to **before** delivery — on every path, paged or not. A user may
sit in the pager for hours; a Ctrl-C landing in the spawn window, a closed
terminal or a killed session then cannot cost the record its completion
(AC-25's "the absence of the completion document means the run did not
finish" stays honest). The bytes recorded are the same `rendered` string as
before (AC-26/AC-27 unchanged). Two visible consequences, both acceptable:
a run killed while a large report is being written to a slow terminal or
pipe now still has its completion document (better than today); and a
record-write failure's stderr message now appears before the report instead
of after it on a terminal.

### 7. The outcome type — `Paged { Shown, Failed, NotStarted }` confirmed

Through a shell, "the pager could not be started" and "the pager started and
failed" cannot be told apart reliably — `sh -c lesss` exits `127`
[measured, bash and dash], a non-executable file `126` [measured], and a
real pager may use any code. The plan's three-way type is right, with these
precise meanings:

| Outcome | When | After it |
|---|---|---|
| `Shown` | exit status `0`; or ended by `SIGINT`; or exit status `130` | the closing status only |
| `Failed` | spawned, then any other exit status or signal; or `wait` itself failed | the report, then the closing status |
| `NotStarted` | the spawn itself failed (no `less` on `PATH`; no `/bin/sh`; no `cmd.exe`) | the report only — exactly today's output |

So `PAGER=lesss` is `Failed` (report, then status — plan Review Focus 1), and
a missing default `less` is `NotStarted` (today's output, no status: no
pager was started). On Windows, `0xC000013A` (`STATUS_CONTROL_C_EXIT`) maps
like `SIGINT` and `cmd`'s `9009` (command not found) is `Failed`
[not measured].

**Delivery (FR-15).** `Shown` counts as delivered (the design's rule; the
PRD reconciliation of FR-15/AC-46/AC-47 is the product-owner's). After
`Failed` or `NotStarted` the directly written report decides delivery exactly
as today (`streams::write_stdout`; `undelivered_report_message`). The closing
status is written like a status line: a failure to write it changes nothing.

**Left to the product-owner:** whether a one-line hint names the failed pager
(and its exit status), and its wording. If one is added it is a genuine
failure under ADR-0018 §9 and goes to stderr through `streams::diag`, before
the report, with the pager command escaped by `safe_text::escape_control_chars`.
Also the product-owner's: the closing status's exact wording, the warnings
line, whether unreadable roots are counted, and any separator between the
report and the status inside the pager — this ADR only fixes that the text
handed to the pager is the report followed by `report::render_closing_status`'s
output.

### 8. `TERM` — unset or empty counts as `dumb` on Unix (pending the owner)

The design's condition is "`TERM` is not `dumb`". With `TERM` unset or empty
`less` behaves exactly as with `dumb`: it prints `WARNING: terminal is not
fully functional` and `Press RETURN to continue`; a `q` then makes it exit
`0` [measured, all three values]. The tool would read that as `Shown` and
print only the closing status — the report would never be seen. This ADR
therefore decides that on Unix an unset or empty `TERM` disables paging like
`dumb`; on Windows `TERM` is normally unset and is not required. Because
this narrows a condition the design states, it is put to the owner below.
Unknown `TERM` values without a terminfo entry may produce the same warning;
that cannot be detected without terminfo and is accepted.

### 9. Testability — no hidden test hook

**Ruling on plan Task 5 Step 2: no hidden test hook in the shipped binary**
(no environment variable or hidden flag that forces "stdout is a terminal").
A hook would be untested product surface reachable by any user or script,
and it would bypass the very conditions — an interactive stdin and stdout —
that guarantee someone is there to press `q`. Instead:

- **Pure, unit-tested:** `streams::should_page` (every condition, table
  test); `pager::fits` (boundary at `rows - 1`, wrapping, wide characters,
  empty text, zero-sized screen); `pager::less_env` (unset, empty, set);
  `pager::command` (inspected with `get_program` / `get_args` / `get_envs`);
  `pager::classify` (built with `ExitStatusExt::from_raw`); `pager::deliver`
  (the sequencing, with the pager and the writer injected: `Shown` → status
  only; `Failed` → report then status; `NotStarted` → report only; a failing
  writer → `Err`).
- **Real processes, unit-tested** (`#[cfg(unix)]`): `pager::run` with small
  shell commands as pagers — `cat > <file>` receives exactly the text;
  `true` (reads nothing) is `Shown` without panicking; `exit 3` is `Failed`;
  `lesss` through the shell is `Failed`; `kill -INT $$` and `exit 130` are
  `Shown`; a program path that does not exist is `NotStarted`; 5 MB to
  `head -n 1 > /dev/null` returns promptly as `Shown`; the `SIGINT`
  disposition after `run` equals the one before. These tests change a
  process-wide signal disposition, so they share one test-local mutex.
  On Windows: `exit /b 3` is `Failed`, `exit /b 0` is `Shown`.
- **End-to-end through the binary** (`tests/cli_integration.rs`, stdout is a
  pipe under `assert_cmd`): with `PAGER` set to a command that would create a
  marker file, the marker is never created and stdout is byte-identical to a
  run without `PAGER` — the "never pages when stdout is not a terminal"
  guarantee, end to end; likewise with `--no-pager`; `--help` lists
  `--no-pager`; the record header names `no-pager=`.
- **Manual, on a real terminal** (plan Task 5 Step 4, evidence in the commit
  message): default `less` long report (opens at the end; `q`; status below
  the restored screen); a report shorter than the screen (no pager, no
  status, no keypress); `PAGER=cat`; `PAGER=lesss`; `PAGER="less -R"`;
  `LESS=-FRX`; Ctrl-C then `q`; `--output` record complete while the pager is
  still open.

### Questions for the owner

1. **`TERM` unset or empty on Unix disables paging** (§8). The design says
   only `dumb`; as written, a terminal without `TERM` would show `less`'s
   "Press RETURN" warning and then only the closing status, never the
   report. **Recommendation:** treat unset and empty like `dumb` on Unix.
   Implementation proceeds on the recommendation unless the owner rules
   otherwise.

Nothing else in the design needed changing. Two consequences are recorded for
the owner's awareness rather than decision: with `-X` in the user's `LESS` the
status appears twice in adjacent lines (§3); and a size the terminal cannot
report means no pager (§2).

## Consequences

- **Positive.** The owner's flow works for the default pager and for the
  common `PAGER=less` / `LESS=-R` / `LESS=-FRX` setups. The status rule
  holds because the tool, not the pager, decides whether a pager runs. The
  start of `$PAGER` has a stated trust model and a closed injection surface.
  Every decision but the process boundary is a pure, unit-tested function.
  The record's completion no longer depends on stdout delivery at all.
- **Negative / trade-offs.** The crate gains its first `unsafe` code — two
  small blocks on Unix (`ioctl`, `signal`) and one on Windows
  (`GetConsoleScreenBufferInfo`), confined to `pager::sys`. `libc` becomes a
  direct dependency (compiled newly on macOS). The tool rewrites `LESS` for
  its child, which a `less` expert may not expect; the guide must say so.
  Windows Ctrl-C ends the tool. The fit check overestimates rows for
  non-ASCII text.
- **Follow-ups.** Plan Tasks 3–5 updated to this ADR. The product-owner's
  items (§7, and the FR-15 reconciliation). Guide: `PAGER=cat` /
  `--no-pager` for CI and `docker run -t`; what the tool does to `LESS`;
  `-X` shows the status twice; the Windows gap including Ctrl-C.

## The interface

### `src/pager.rs`

```rust
use std::ffi::{OsStr, OsString};
use std::io;
use std::process::{Command, ExitStatus};

/// Which pager to start. Decided by `streams::should_page`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PagerCommand {
    /// `$PAGER` unset (never on Windows): `less`, found on `PATH`.
    Less,
    /// `$PAGER`'s value, run by the platform's command interpreter.
    Shell(OsString),
}

/// The terminal's visible size in character cells. Both fields non-zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenSize {
    pub rows: u16,
    pub cols: u16,
}

/// The size of the terminal on stdout; `None` when it cannot be learned or
/// is reported as zero. The only probe; called by `main` at the boundary.
pub fn screen_size() -> Option<ScreenSize>;

/// Whether `text` shows in full with one row left for the prompt. Pure.
pub fn fits(text: &str, screen: ScreenSize) -> bool;

/// The `LESS` value for the pager's environment, from the user's own. Pure.
pub fn less_env(user: Option<&OsStr>) -> OsString;

/// The process to start, configured but not started. Pure.
pub fn command(pager: &PagerCommand, user_less: Option<&OsStr>) -> Command;

/// How a pager ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paged {
    /// Exited successfully, or was interrupted by the user (`SIGINT`, 130).
    Shown,
    /// Started, then any other exit status or signal.
    Failed,
    /// Could not be started at all.
    NotStarted,
}

/// Read a pager's exit status. Pure; never returns `NotStarted`.
pub fn classify(status: ExitStatus) -> Paged;

/// Start `command` with a piped stdin, write `parts` in order, close the
/// pipe, wait. `SIGINT` is ignored from spawn until `wait` returns (Unix).
/// Never panics; write errors (an early quit) are not reported.
pub fn run(command: Command, parts: &[&str]) -> Paged;

/// How the report is to be delivered.
pub enum Delivery<'a> {
    /// Print the report, as today.
    Direct,
    /// Page the report followed by `closing_status`.
    Paged { pager: &'a PagerCommand, closing_status: &'a str },
}

/// Deliver the report and, after a pager, the closing status. `Err` means
/// the report was not delivered (FR-15); after `Shown` it always was.
pub fn deliver(
    delivery: Delivery<'_>,
    report: &str,
    page: &mut dyn FnMut(&PagerCommand, &[&str]) -> Paged,
    write: &mut dyn FnMut(&str) -> io::Result<()>,
) -> io::Result<()>;
```

`deliver`'s arms: `Direct` → `write(report)`. `Paged` →
`page(pager, &[report, closing_status])`, then `Shown` → `let _ =
write(closing_status); Ok(())`; `Failed` → `write(report)?` (plus the
product-owner's optional stderr hint before it), then `let _ =
write(closing_status); Ok(())`; `NotStarted` → `write(report)`.

`pager::sys` (private, `#[allow(unsafe_code)]`, every block with a
`// SAFETY:` comment) holds the platform code: `screen_size` (Unix:
`libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut winsize)` on a
zeroed `libc::winsize`; Windows: `GetStdHandle(STD_OUTPUT_HANDLE)` +
`GetConsoleScreenBufferInfo`, `rows = Bottom - Top + 1`, `cols = Right -
Left + 1`) and an `IgnoreInterrupt` guard (Unix: `libc::signal(SIGINT,
SIG_IGN)` returns the previous disposition, `Drop` restores it; Windows: a
no-op). The crate root gains `#![deny(unsafe_code)]`, so any further `unsafe`
is a visible, reviewed decision.

### `src/streams.rs` — `PagingFacts` / `should_page` (replaces plan Task 3's placeholder)

```rust
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

/// `None`: do not page. Every condition but "does not fit", which needs the
/// rendered report and the screen size and is `pager::fits`'s.
pub fn should_page(facts: &PagingFacts<'_>) -> Option<pager::PagerCommand>;
```

Changes from the plan: the environment values are `OsStr` (a `PAGER` that is
not UTF-8 is passed through untouched, never lossily converted); the result
is `PagerCommand` rather than a `String`; `term` disables paging when it is
`dumb`, and on Unix also when unset or empty (§8, pending the owner);
`PAGER` disables it when empty or exactly `cat` (byte-for-byte, git's rule —
`/bin/cat`, or `cat` with a leading space, are run as pagers). **Terminal height is not a
`PagingFacts` fact**: it is probed only when `should_page` returns `Some`, so
no run that cannot page pays for the probe.

### Call sequence in `main.rs`

1. Probe once, next to `streams::route`: `stdout_tty` (shared with `route`),
   `stdin_tty`, `TERM`, `PAGER`, `LESS` (`std::env::var_os`).
2. Unchanged: `sink.finish()`; root errors; `--apply`; render `rendered`.
3. `recorder.record(RecordEvent::Result { … })` — moved here, before any
   delivery (§6).
4. `let pager = streams::should_page(&facts).filter(|_| pager::screen_size()
   .is_some_and(|screen| !pager::fits(&rendered, screen)));`
5. If `Some`: `closing_status = report::render_closing_status(…)` and
   `Delivery::Paged { … }`; otherwise `Delivery::Direct`.
6. `pager::deliver(delivery, &rendered, &mut |p, parts| pager::run(
   pager::command(p, less.as_deref()), parts), &mut streams::write_stdout)`;
   `Err(e)` → `streams::undelivered_report_message(&e)` → `streams::diag`,
   and `delivered = false`, exactly as today.
7. Exit code: unchanged composition (`!delivered` is a run-level failure).

### Exact `LESS` environment

The tool passes **no arguments** to `less` or to any `$PAGER`. It sets exactly
one environment variable, on the pager's process only, whenever it starts a
pager (default `less` or `$PAGER`, on every platform):

| The user's `LESS` | The pager's `LESS` |
|---|---|
| unset or empty | `+G$-+E -+e -+F` |
| set to `<v>` | `+G$<v>$-+E -+e -+F` |

`+G`: start at the end. `$`: ends the `+G` string, and ends any unterminated
string option at the end of `<v>`. `-+E -+e -+F`: reset "quit at end of
file", "quit at the second end of file" and "quit if one screen", so `less`
leaves only on a keypress. Everything else in `<v>` applies as the user set
it. `-X`, `-R`, `-S`, `-K`, `LESSSECURE`, `LESSCHARSET` and the locale are
not set: the report has no colour or control bytes to pass through, the
screen behaviour is the user's (§3), and `LESSSECURE` would also disable
`less`'s `s` (save to file) — useful on exactly this report. Measured on
less 668 for every row above (see [Evidence](#evidence)).

### Dependency statement

- **Unix:** `libc = "0.2"` under `[target.'cfg(unix)'.dependencies]` — for
  `ioctl(TIOCGWINSZ)` and `signal(SIGINT)`. Already in `Cargo.lock`
  (0.2.186, via `trash` on Linux and `cpufeatures` on some targets), so no
  new lock entry; compiled newly for macOS builds only.
- **Windows:** `windows-sys = { version = "0.61", features =
  ["Win32_Foundation", "Win32_System_Console"] }` under
  `[target.'cfg(windows)'.dependencies]` — for the console size. Already in
  `Cargo.lock` (0.61.2, via `clap` → `anstream` → `anstyle-wincon`, which
  enables exactly these two features), so no new crate and no new compiled
  code.
- **Licenses / `cargo deny`:** both are `MIT OR Apache-2.0`, on the
  allow-list; both are already in `cargo deny`'s graph (`cargo deny list`
  shows `libc@0.2.186` and `windows-sys@0.61.2`); no new version, so the
  `multiple-versions` ban is unaffected. **No new crate.**
- **Rejected:** `terminal_size` (a new crate, promotes `rustix` from a dev-
  to a normal dependency, and still leaves `SIGINT` needing `libc`);
  `signal-hook` (two new crates for one disposition); hand-written `extern
  "C"` declarations (the same `unsafe` without the per-platform constants
  and struct layouts `libc` maintains); `stty size` in a subprocess (no
  Windows, a process per run, output parsing, and `SIGINT` still unsolved);
  `LINES` (not exported by shells — absent from the environment of the
  login shell on the development machine [measured]).

## Pros and cons of the options

### S1 — the platform shell (chosen)

- Good: the value means what the user typed; quoting, arguments and paths
  with spaces work; one argument, no string building.
- Bad: "not found" is an exit status (`127`), not a spawn error; a shell
  process per paged run.

### S2 — split on whitespace, exec

- Good: no shell; a missing program is a spawn error.
- Bad: wrong for quoted values; a home-grown parser is a new bug surface.

### S3 — exec the value as one program

- Good: simplest.
- Bad: breaks the most common form, `less -R`.

### P1 — `less` arguments

- Good: works on any `less` version; ours always win.
- Bad: only for the default pager; `PAGER=less` would open at the start.

### P2 — `LESS` only when unset (git)

- Good: never touches a user's setting.
- Bad: with any user `LESS` (e.g. `-R`) the report opens at the start.

### P3 — `LESS` wrapped around the user's value (chosen)

- Good: one mechanism for every `less`; the user's settings still apply;
  measured robust against the values that break naive prepending.
- Bad: relies on `less`'s `$` terminator; rewrites a variable the user set.

### P4 — leave positioning to the pager

- Good: no coupling to `less`.
- Bad: fails the owner's "opened at the end".

### F1 — `less -F`

- Good: no measurement, no `unsafe`.
- Bad: the tool cannot tell a short report from a paged one, so the status
  would follow a short report — forbidden by the owner's rule.

### F2 — measure through `libc` / `windows-sys` (chosen)

- Good: exact rows and columns on both platforms; no new crate; the same
  dependency also solves `SIGINT`.
- Bad: the crate's first `unsafe`, small and confined.

### F3 — `terminal_size`

- Good: a safe API.
- Bad: a new crate; `SIGINT` still needs `libc`.

### F4 — `stty size`

- Good: std only.
- Bad: no Windows; a subprocess and parsing per run; `SIGINT` unsolved.

### F5 — `LINES`

- Good: trivial.
- Bad: not exported by shells; wrong after a resize.

### X1 — `-X`

- Good: the report's last page stays in the scrollback.
- Bad: the status then appears twice on every paged run; `+G`'s paint lands
  in the scrollback.

### X2 — `less`'s default and the user's `LESS` (chosen)

- Good: the owner's flow — the screen restored, the status below it.
- Bad: with a user `-X` the status appears twice.

### C1 — do nothing on Ctrl-C

- Good: no `unsafe`.
- Bad: the tool dies on a reflexive Ctrl-C; no status; exit code `130`
  instead of the tool's own [measured].

### C2 — ignore `SIGINT` from spawn to exit via `libc` (chosen)

- Good: `less` handles Ctrl-C itself; the tool keeps its status and its exit
  code [measured].
- Bad: one `unsafe` call pair; a tiny window before the ignore takes effect.

### C3 — `signal-hook`

- Good: safe API.
- Bad: two new crates for one disposition change.

### C4 — hand-written `extern "C"`

- Good: no crate.
- Bad: the same `unsafe` with platform constants maintained by hand.

### W1 — writer thread

- Good: the conventional shape for a pipe to a child.
- Bad: no benefit here (only stdin is piped); a spawn that can fail and a
  join.

### W2 — write on the calling thread (chosen)

- Good: simplest; deadlock-free with one pipe; measured prompt on early quit.
- Bad: none found.

### R1 — completion document after delivery (today)

- Good: no change.
- Bad: a user sitting in the pager holds the record's completion hostage.

### R2 — completion document before delivery (chosen)

- Good: the record is complete before anything can wait on a human.
- Bad: a record-write failure's message now precedes the report.

## Evidence

All on macOS (Darwin 27.2), less 668, `/bin/sh` = bash 3.2.57, rustc 1.97.1,
through a Rust harness spawning the pager as specified above and a Python
`pty` driver at 24×80 with `TERM=xterm-256color`. Throwaway; not in the
repository.

| # | Experiment | Result |
|---|---|---|
| 1 | `less +G`, 200 lines, `q` | opens at the end; last line before `(END)` is the text's last line; alternate screen entered and left; parent's later output on the restored screen |
| 2 | `less +G -X` | no alternate screen; last page stays; parent's output follows it |
| 3 | `less +G -F`, 10 lines | exits in 7 ms, status `0`, text on the normal screen, `~` rows above it |
| 4 | command-line `+G` with `LESS=-R`, `-FRX`; `-+X -+F` | `+G` applies; user's `X` applies unless `-+X` |
| 5 | `LESS="+G -R"`; `LESS="-Pfoo bar +G"` | first: error prompt (initial command `G -R`); second: `+G` swallowed into the prompt |
| 6 | `LESS="+G$<v>$-+E -+e -+F"` for `<v>` ∈ unset, empty, `-R`, `-FRX`, `-E`, `-e`, `-F`, `-Pfoo bar`; long and short text | opens at the end and stays open until `q` in every case; `X` in `<v>` still applies |
| 7 | `PAGER="less -F"` via `sh -c` with our `LESS` | `-F` on the command line wins over `LESS` |
| 8 | user `LESS=-E` with `+G` | exits in 8 ms; the report flashes on the alternate screen |
| 9 | `TERM` unset / empty / `dumb` | "terminal is not fully functional", "Press RETURN"; `q` → exit `0` |
| 10 | 100 000 lines (5 MB), `q` on the first screen, inline and threaded writer | `write_all` → `BrokenPipe` promptly; no signal death |
| 11 | parent `SIGPIPE` disposition; child's | parent `SIG_IGN`; child default (`yes \| head` → 141) |
| 12 | `sh -c lesss` (bash, dash); non-executable; `-x`; `exec lesss` | `127`; `126`; `2`; `127` |
| 13 | direct spawn of `less` with `PATH=/nonexistent` | `io::ErrorKind::NotFound` |
| 14 | Ctrl-C under interactive bash, parent not ignoring `SIGINT` | parent dies (`130`), `less` restores the screen and exits, terminal usable, no status |
| 15 | same, parent ignoring `SIGINT` after spawn | `less` beeps and stays; `q` → status printed, exit `0` |
| 16 | `seq 200 \| less +G`, Ctrl-C at `(END)` | beep and repaint; `less` does not exit |
| 17 | `wait` statuses: `kill -INT $$`; `exit 130`; `SIGTERM` | signal 2; code 130; signal 15 |
| 18 | `ioctl(1, TIOCGWINSZ)` on the pty | rows 24, cols 80 |
| 19 | `LESS="+G$-j"` (a broken user value) | error message, exit `0` |
| 20 | `env` in the login shell | no `LINES` or `COLUMNS` |
| 21 | `cargo deny list`; registry manifests | `libc@0.2.186`, `windows-sys@0.61.2` in the graph; both `MIT OR Apache-2.0`; `anstyle-wincon` enables `Win32_System_Console` and `Win32_Foundation` |

**Not measured:** any Linux `less` (presence, version, `$` handling below
668); BusyBox `less`; anything on Windows (`cmd.exe /D /S /C`, exit `9009`,
the console size API, Ctrl-C); `SIGTSTP` with the pager up. Presence of
`less`: it is `/usr/bin/less` on macOS [measured]; on Linux it is in the
standard installs of the major distributions but missing from many minimal
container images [not measured] — there the spawn fails and the report
prints as today.

## Links

- [Report paging design](../superpowers/specs/2026-11-13-report-paging-design.md)
  — the owner's decisions this ADR implements.
- [Implementation plan](../superpowers/plans/2026-11-13-report-paging.md) —
  Tasks 3–5 updated to this ADR.
- [ADR-0018](0018-durable-run-record-and-output-stream-routing.md) — stream
  routing (§8), the record's durability (§3, §7), "Writers that cannot
  panic".
- [ADR-0017](0017-performance-instrumentation-tracing-spans.md) —
  `--timings` and `process::exit`.
- `less`(1), section "LESS" (the `$` terminator) and option `-+`.
