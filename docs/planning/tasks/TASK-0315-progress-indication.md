# TASK-0315 — Progress indication (live, stderr, TTY-aware)

- **Phase:** 3 — Implementation (TDD)
- **Role:** implementer
- **Status:** done (batch 4b / session 1, 2026-08-01; see
  [`../reviews/progress-indication.md`](../reviews/progress-indication.md))
- **Depends on:** TASK-0308
- **Parallel:** no (must coexist with the `--json` stdout-purity contract)

## Goal

While a scan runs, show a live progress indicator on **standard error** so the
user sees the tool is working and how far along it is: the current phase, `x of y`
files done, and a percentage. Enabled by default; auto-adapts to whether stderr is
a TTY; `--no-progress` turns it off. Progress is output-only and never touches
stdout in any mode (including `--json`).

## Context & links

- **PRD-0001 FR-19** (Amendment 2026-07-19) — progress indication requirement and
  the three owner decisions; acceptance criteria **AC-20..AC-24**.
- **Stdout-purity contract (slice S2)** — `docs/guide/output-and-exit-codes.md`:
  stdout is exactly the report / one `--json` document; all diagnostics go to
  stderr. Progress must go to **stderr only** and must never interleave into
  stdout, in any mode.
- **NFR-3 (determinism)** governs stdout content only; progress on stderr is
  explicitly excluded from determinism/parity guarantees.
- **ADR-0008 pipeline** (enumerate → sniff → hash → verify): the total number of
  candidate files is known **after enumeration**; hashing/verification dominate
  runtime. The denominator/phases are user-facing labels, not an implementation
  contract — do not let the display prescribe the pipeline.
- Coexists with the human report (TASK-0307) and `--json` (TASK-0308); like both,
  it is a pure output concern over the same run, changing nothing that is scanned,
  grouped, or removed.

## Acceptance criteria

Test-first (ADR-0005). Assert on captured stdout/stderr and on the run outcome.

- **AC-20 (FR-19, stdout purity):** with progress enabled (default), a completed
  run — default report **and** `--json` — writes **exactly** the report/JSON to
  stdout and **no** progress text there; all progress appears on stderr. A
  stdout-redirected run yields a clean report/JSON (the S2 contract holds).
- **AC-21 (FR-19, TTY):** when stderr is a TTY, a **live single-line** indicator on
  stderr reports the phase, `x of y` files, and a percentage, with `y` = the count
  of candidate files to check (known after enumeration).
- **AC-22 (FR-19, non-TTY fallback):** when stderr is **not** a TTY, **no** in-place
  redraw / carriage-return / ANSI control sequences are emitted; progress instead
  appears as occasional plain, newline-terminated lines (log-friendly, bounded), or
  is otherwise absent.
- **AC-23 (FR-19, opt-out):** with `--no-progress`, **nothing** progress-related is
  written to stderr, and stdout is unchanged.
- **AC-24 (FR-19, output-only):** for identical inputs/flags, the groups, keeper,
  intended removals, and the **stdout report/JSON are byte-identical** whether
  progress is on, off, or on the non-TTY fallback path — progress affects neither
  results nor stdout.

## Notes & constraints

- **Default-on**, opt-out via `--no-progress` (add to the clap surface next to the
  other output flags; wire the exit-code constants/flags style already in the
  crate). Consider honoring a non-TTY environment automatically rather than adding
  more flags — keep the surface minimal.
- **TTY detection on stderr specifically** (not stdout). When stderr is not a TTY,
  disable the in-place updater and fall back to occasional plain lines at coarse
  milestones (phase transitions + bounded percentage steps) so redirected logs stay
  readable and bounded.
- Before enumeration completes, `y` is unknown — show a counting/indeterminate
  state, then switch to `x of y` + percentage once the denominator is fixed.
- Keep progress **decoupled** from the compute pipeline: a reporting sink the
  pipeline notifies, so results/ordering/determinism are untouched and progress can
  be disabled with zero behavioral effect. Concurrency (rayon, TASK-0306) means
  the counter must be updated safely without changing results.
- Do **not** prescribe throughput/paint-rate specifics here; pick a sane redraw
  cadence that avoids flooding a TTY (e.g. throttle repaints) — an architect
  design note if a crate (e.g. a progress-bar library) is proposed.
- No dedicated `--output` flag — saving results is via stdout redirection
  (PRD-0001 §4 non-goal + Amendment 2026-07-19).

## Out of scope

- The `--output <file>` flag (deferred; redirection is the mechanism).
- The stored-results documentation, which documents **existing** behavior and
  ships now (README + `docs/guide/output-and-exit-codes.md`) — not part of this
  build unit.
- Any change to what is scanned, grouped, or removed, or to the report/JSON schema.
