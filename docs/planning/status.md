# Project Status

Live snapshot of where the project stands. Update this in every PR. This is the
first file to read when resuming work.

**Current phase:** Phase 3 — Implementation (TDD), executed per the
owner-approved **[batch plan](batch-plan.md)** (2026-07-19): housekeeping →
S2 → test tooling → S3 → S4 + field validation → v0.1.0 private release →
docs pass → AI automation, batched within Claude usage limits.
**Last updated:** 2026-11-14

## In progress

- _Session 41 (chore: third-party license notices in release archives,
  TASK-0348), 2026-11-14 — branch `chore/third-party-license-notices`._
  Every release archive now also carries `THIRD-PARTY-LICENSES.txt`, the
  license texts of the crates linked into the binary. `v0.7.0` does not have
  it; the next release is the first that does.

  Owner decision (asked before building): **commit the file and check it in
  CI**, rather than generate it on each release leg. The release script only
  copies it, so no new tool runs on the path that builds published binaries,
  and a dependency's license change shows up as a diff in review. The accepted
  cost: a Dependabot cargo PR that changes the notices fails the `security`
  job until `make notices` is run on its branch. Crate versions are left out
  of the file to keep that rare.

  What landed: `about.toml` and a plain-text template
  (`scripts/notices/`); `scripts/notices.sh` (`make notices`,
  `make notices-check`); a pinned, checksum-verified `cargo-about` 0.9.3 and
  the check in `security.yml`, under the same path filter as `cargo-deny`;
  the copy in `scripts/build-release-artifact.sh`; the CI structure check;
  `release.md`, README, CONTRIBUTING and the getting-started guide. The README
  and guide lines are one-sentence factual additions written by the
  orchestrator, not by the product-owner.

  Checked by hand: a Linux container (`cargo-about` 0.9.3, aarch64 musl
  build) reports the macOS-generated file as up to date, so the CI check does
  not depend on the generating machine; a rehearsal archive built with
  `build-release-artifact.sh` from a throwaway copy contains the file with
  mode `0644`. The file is `.txt`, not Markdown as first proposed to the
  owner: license texts would fail the Markdown lint, spell check and link
  check.

  Known limits, also in `release.md`: procedural-macro crates are listed
  although they are not in the binary; `blake3` and the three `objc2*` crates
  appear with the standard text of their license because their packages hold
  no license file the tool finds. The 12 `windows*` crates needed an explicit
  pointer to their `license-mit` file in `about.toml`, pinned by checksum.

  Architect review of the uncommitted diff: changes required, all applied.
  It confirmed the file lists every third-party crate the four targets link,
  that no dependency ships an Apache `NOTICE` file, and that the workflow
  conditions fail closed. Fixed: (F1) `rustup show` does not install the
  pinned toolchain since rustup 1.28, so the first cargo call would have
  installed it with its output discarded — the step now runs
  `rustup toolchain install` and the script tests for `cargo-about` without
  calling cargo; (F2) the Rust standard library, linked into every binary,
  was not mentioned — the file now names it in a fixed paragraph and
  `release.md` lists it as a limit; (F3) the `objc2*` section shows the MIT
  text's `<year> <copyright holders>` placeholder, now explained in the
  file; (F4) two inaccurate sentences in `release.md`; (F5) ADR-0020 gained a
  dated note; (F6) the guide and `release.md` now say the file ships from the
  release after `v0.7.0`; (F7) the check has its own path filter, so editing
  only the template does not also run `cargo-deny`. `ci.yml`,
  `release.yml` and the release pre-flight had the same `rustup show` line;
  on the owner's instruction they were changed to `rustup toolchain install`
  in this PR too. The `release.yml` and pre-flight changes have not been run;
  `ci.yml`'s and `security.yml`'s run on the PR.

  Not run yet: the `security.yml` steps themselves. They run for the first
  time on this PR.

- _Session 40 (docs: record `v0.7.0` and the release clean-up; planning
  sync), 2026-11-14 — branch `docs/record-v0.7.0-and-release-cleanup`._
  PR #114 (TASK-0347, session 39) merged as squash `fa5bb81`. On the owner's
  instruction the release workflow was then dispatched against it:
  **`v0.7.0` is published, the first release under `MIT OR Apache-2.0`**.
  Checked on the downloaded macOS archive: the checksum matches, the archive
  holds `LICENSE-MIT` and `LICENSE-APACHE`, and `--version` names the new
  license.

  Also on the owner's instruction, **the 16 GitHub Releases `v0.1.0` to
  `v0.6.0` were deleted** with their binaries. Their git tags are kept: the
  release notes' compare links and ADR-0014 refer to them, and the source at
  those tags is in the history regardless. `v0.7.0` is the only release
  listed. ADR-0020 and TASK-0336 are updated to say so; that closes
  TASK-0336's "keep or delete the GPL releases" question.

  **Resume point:** no unit is in progress. Open rows: TASK-0346 (owner
  brainstorm), TASK-0348 (third-party notices), TASK-0339 (architect ADR),
  TASK-0337 / TASK-0336 / TASK-0326 (going public).

- _Session 39 (feat: relicense to `MIT OR Apache-2.0`, TASK-0347),
  2026-11-14 — branch `feat/relicense-mit-or-apache`._ Owner decision (given
  without a stated reason, in a conversation about going public and
  package-manager distribution): the project moves from `GPL-3.0-only` to `MIT OR Apache-2.0`.
  [ADR-0020](../adr/0020-relicense-mit-or-apache-2.0.md) supersedes ADR-0014.
  `LICENSE` is replaced by `LICENSE-MIT` and `LICENSE-APACHE` (the Apache
  text is the canonical file, checksum recorded in the ADR); the manifests,
  the release script, the CI structure check, README, CONTRIBUTING, the
  release docs and `deny.toml`'s comments follow. The one code change is the
  license line of `--version`, with both pinning tests changed first and seen
  to fail. TASK-0336 is narrowed: its GPL checks no longer apply. The ADR was
  written by the orchestrator from the owner's ruling and reviewed by the
  architect with the rest of the diff.

  Architect review: changes required, all applied — the code, license files,
  metadata and scripts were confirmed correct; the three must-fixes were in
  the ADR and planning prose (a false claim that every old release's
  `--version` names the GPL, when only `v0.6.0` does; considerations written
  as if they were the owner's reasons; a missing full stop). ADR-0015's GPL
  paragraph gained a dated note. Raised as TASK-0348: release archives carry
  no third-party license notices. Not fixed here: `docs/process/commits.md`
  says a breaking change bumps the major version only after `1.0.0`, but the
  configured analyzer would go from `0.x` straight to `1.0.0`.

  **Also on 2026-11-14: `v0.6.0` was published** (release workflow dispatched
  on the owner's instruction against `4878c89`; four platform archives with
  checksums). It is the last release under `GPL-3.0-only`; `v0.1.0`–`v0.6.0`
  stay under that license.

- _Session 38 (docs: report paging designed, then withdrawn — TASK-0340 /
  FR-22), 2026-11-13 and 2026-11-14 — branch `docs/withdraw-report-paging`
  (started as `feat/report-paging`)._ **Outcome: `imgdedup` will not page its
  report. No code was written; shipped behavior is unchanged.**

  What happened: on 2026-11-13 the owner brainstormed and approved a
  [design](../superpowers/specs/2026-11-13-report-paging-design.md) (a pager
  opened at the end of a long report, on by default, with a closing status
  left in the terminal). On 2026-11-14 the architect wrote
  [ADR-0019](../adr/0019-report-paging.md) and the product-owner drafted the
  PRD amendment. The owner then withdrew the feature the same day:

  - forgetting `--output` or a redirect before a large run is accepted as the
    user's cost;
  - the owner wants to drive the project more towards **separate analysis,
    reporting and user-interface functions**, so that usability problems of
    this kind can be solved differently in the future. That direction is unspecified and starts with
    an owner brainstorm: new **TASK-0346**.

  What this unit leaves in the repo: PRD-0001 Amendment 2026-11-14 withdraws
  FR-22 and AC-33..AC-37 (numbers not reused); ADR-0019 is kept as `rejected`
  for its measurements on `less` and external pagers; the design is marked
  withdrawn; the implementation plan is deleted; TASK-0340 is closed. The
  product-owner's FR-22 fold-in draft was not kept (it stays reachable from
  this PR's commit list on GitHub after the squash merge). The withdrawal text
  in the PRD was written by the orchestrator from the owner's ruling, not by
  the product-owner. The architect's lightweight document review of the
  withdrawal required four text changes, all applied: three places where the
  orchestrator's wording gave the owner motives or design direction the owner
  had not stated, and a stale sentence in TASK-0340's row.

  **Resume point:** no unit is in progress. Open rows: TASK-0346 (owner
  brainstorm), TASK-0339 (needs an architect ADR), TASK-0326 / 0336 / 0337
  (wait on the repo going public).

  Also recorded here: PR #110 (TASK-0345, session 37) merged 2026-11-13 as
  squash `cc74999`; its usage was 22% of a 5h window against a 15–20%
  estimate.

  Usage, 2026-11-14 (5h window from 6%, weekly from 14%): the design batch —
  architect 200k and product-owner 164k subagent tokens — was estimated at
  25–35% of the window and cost **32%** (6% → 38%; weekly 14% → 18%). All of
  it went into a design that was then withdrawn. The withdrawal itself —
  written inline, plus one architect review pass (47k subagent tokens) and the
  owner's question-and-answer round — cost a further 13%, ending at **51%** of
  the window and 20% weekly (owner's `/usage` reading at the point the PR was
  opened). Lesson for the batch plan (the orchestrator's observation, not the
  owner's stated reason): the owner withdrew the feature only after the full
  ADR and PRD passes. For a feature whose value is in doubt, putting a
  one-page mechanism sketch and the direction question to the owner before
  those passes would have cost a few percent instead of 32.

- _Session 37 (fix: say why a report could not be written, TASK-0345),
  2026-11-13 — branch `fix/undelivered-report-message`._ Since TASK-0342 a
  report that cannot be written to stdout exits `1` in silence. That is
  right when the reader has gone away (`imgdedup … | head`), but
  `imgdedup … > file` on a full disk ended the same way with no explanation.
  `streams::write_stdout` now returns the error, and every cause except a
  broken pipe prints one line on stderr:
  `error: could not write the report to standard output: <reason>`.
  PRD-0001 FR-15 is clarified and gains AC-47; the guide and ADR-0018's
  "Writers that cannot panic" section say the same. The architect's review
  of the diff converged with doc nits only. The wording was the architect's
  suggestion from the TASK-0342 review; the product-owner approved it
  unchanged in a short document review and tightened AC-47 (the line is one
  among any other stderr lines of the run; exit `1` also beats `--exit-code`'s
  `3`). Tests written first: an
  end-to-end test that forces a real write failure with `ulimit -f` on a
  redirected stdout, and an assertion that a broken pipe stays silent.

- _Session 36 (feat: `--output` overwrite safety net, TASK-0342, plus two
  owner-requested riders), 2026-11-13 — branch
  `feat/output-overwrite-safety-net`._ Brainstormed with the owner the same
  day ([design](../superpowers/specs/2026-11-13-output-overwrite-safety-net-design.md),
  [plan](../superpowers/plans/2026-11-13-output-overwrite-safety-net.md)).
  Owner rulings: a dedup tool must not destroy **any** file unasked; replace
  only the tool's own records without a flag; keep what is replaced as
  `.bak`, one generation; exit `1` when the report cannot be written to
  stdout.

  **Merged 2026-11-13 as PR #109 (squash `e1c40d0`)**, CI green on all
  checks including the Windows leg; the architect's confirmation pass came
  back clean before the PR was opened.

  - **Overwrite rule** (`src/output_file.rs`, `--overwrite-output`): an
    earlier record is moved to `<file>.bak` and replaced; an empty file is
    written; any other existing file is refused with exit `2` before
    scanning unless the flag is given. A non-record `.bak` blocks the run
    with or without the flag. A symlinked target is backed up by copy.
  - **Closed stderr / stdout** (rider): `main.rs` no longer uses `eprintln!`
    or `println!`; a diagnostic to a closed stderr is dropped, and a report
    that cannot be written exits `1` after the record is completed. This was
    the PR #107 review's follow-up F5.
  - **Product-owner pass:** PRD-0001 Amendment 2026-11-13 (FR-20 overwrite
    rule, FR-15 closed streams, AC-38..AC-46 — AC-33..AC-37 are FR-22's),
    guide and README. Rulings applied in code: the refusal message leads with
    "choose another path"; the blocked-backup message says what to do; a
    backup that cannot be made is its own refusal; `--overwrite-output` is
    part of the record's invocation; no notice is printed for a backup.
  - **Architect review: changes required, all applied.** Two defects it
    reproduced with the binary: (F1) `--timings` with a closed stderr still
    panicked, because `tracing-subscriber` reports a failed write with its
    own `eprintln!`; (F2) record detection by prefix took a Markdown note
    titled `# imgdedup …` for a record — moved to `.bak` on one run and
    replaced for good on the next. Detection now matches the whole header
    line the tool writes. Hardening: (F3) the symlink copy creates the backup
    anew instead of writing through whatever sits at that name; (F4) a
    Windows remove-before-rename block was removed (std's `rename` already
    replaces, and the block could lose a backup generation); (F6, F7) the
    empty-file and non-regular-file paths no longer truncate or write over
    content that appeared after the check. The ADR-0018 amendment carries the
    architect's wording, including an honest "residual windows" bullet: a
    file created at `<file>.bak` between its check and the rename is lost.
    Deferred as TASK-0345 (F8): an undelivered report that is not a broken
    pipe exits `1` without saying why.

  State: `make validate` passes; `cargo test` green (374 unit + 104
  integration on macOS). Process note: `output_file.rs`'s first unit tests
  and implementation were written together rather than test-first; the
  end-to-end tests, the closed-pipe tests and the tests for the review
  findings were written before their fixes.

  Usage (fresh 5h window from 3% for Task 5): two full agent passes
  (product-owner 113k tokens, architect 92k) plus one confirmation pass;
  estimate 20–25% of the window, actual **37%** (3% → 40%, owner's `/usage`
  reading). The estimate priced only the dispatches; the review returned
  seven findings, and the inline fix round plus the confirmation pass cost
  about as much again. For comparison, PR #107's close-out phase was
  estimated at 40–60% and came in at 42%.

- _Session 35 (fix: FR-20/FR-21 close-out tail — TASK-0343, TASK-0344 and
  review follow-ups), 2026-11-13 — branch `fix/output-record-close-out-tail`._
  One owner-requested collective PR for the small items left over from
  PR #107, three in product code and four chores. **Merged 2026-11-13 as
  PR #108 (squash `310fc6c`)**, including the later-added rewording of the
  startup error to `cannot open output file`:

  - **TASK-0344 (done).** The human-readable record header gains a third
    `# invocation:` line (`json=`, `no-progress=`, `timings=`, `output=`), so
    it carries the same invocation as the NDJSON header; the `--output` path
    is escaped like every other path. `--help` and `-h` no longer cite
    internal identifiers (`FR-`, `AC-`, `ADR-`); they moved to `//` comments
    beside each flag, and an integration test keeps them out.
  - **No stray blank line.** A record-write failure after progress has
    finished (an apply-time or result write) printed an empty line before the
    error on a terminal. The line break is now written only while a progress
    line is live. Checked by hand on a pseudo-terminal; the text itself is
    unit-tested (`streams::interrupting_line`).
  - **Shared stage labels.** `"enumerate"`, `"sniff"` and `"engine"` are
    constants in `json.rs`, used by the document and by the record's call
    sites. No behavior change.
  - **TASK-0343 (done).** A byte-pinned golden test for `--json` stdout
    (`tests/golden/json_dry_run.json`: two groups, a protected hard link, an
    already-deduplicated set, one warning) replaces the tautological
    `document_to_pretty_is_byte_identical_to_render`. Unix only.
  - **Wording.** ADR-0018 note 9, PRD-0001 FR-20 and the guide said a partial
    NDJSON `result` line "does not parse"; a write that fails on exactly the
    final newline leaves a parseable line, so the text now adds that a
    finished line ends with a newline.
  - **Distinct `--output` errors (owner decision, added after the PR
    opened).** The startup failure now reads `error: cannot open output file
    <path>: <reason>` instead of `cannot write to`, which was one word away
    from the mid-run `could not write to`. The product-owner's acceptance of
    PR #107 had declined this rewording; the owner asked for it. The mid-run
    message, which PRD-0001 and the tests pin, is unchanged (ADR-0018
    implementation note 12).
  - **Planning.** Session 34's entry records the merge; the implementation
    plan's steps are ticked; ADR-0018 gains implementation notes 10 to 12.

  Not in this unit, by decision: `eprintln!` panicking when stderr is a
  closed pipe, and TASK-0342 — both picked up in session 36.

- _Session 34 (TASK-0340/TASK-0341 implementation, slice 1: FR-20 + FR-21 +
  FR-19 elapsed time), 2026-11-12 — branch
  `feat/output-record-and-stream-routing`._ Executing
  [the implementation plan](../superpowers/plans/2026-11-12-output-record-and-stream-routing.md)
  in usage-budgeted batches, implemented inline with the architect/tester/
  product-owner reviews dispatched at the end. FR-22 (paging) is not part of
  this unit.

  **Merged 2026-11-13 as PR #107 (squash `497e976`)** with CI green
  (`checks`, `ci-gate`, `security`, Rust on Ubuntu and Windows) and the
  architect's confirmation pass done. TASK-0341 is closed; TASK-0340 stays
  open for FR-22 only. Window 1 (2026-11-12) landed plan
  Tasks 0–10 inline; window 2 ran Task 11: the tester pass, the product-owner
  guide docs, the ADR-0018 / backlog close-out, the architect's code review on
  the branch diff, and product-owner acceptance.

  Window 2 outcomes:

  - **Tester** (AC-20..AC-32 + AC-28a): no product defect. Four tests added —
    AC-32 for scanning's determinate state and for the `(Ns elapsed)` /
    `(Ns total)` suffixes end to end, and two AC-28a tests that force a real
    `EFBIG` through `ulimit -f` so the continue / report-once / exit-1 path
    runs on macOS as well as through the Linux-only `/dev/full` tests.
    `make scale` passes. `--output` adds no measurable wall time on the
    2,021-file corpus (about 500 ms with or without it).
  - **Product-owner docs:** README, `output-and-exit-codes.md`,
    `getting-started.md` and the guide index, built from the reverted
    2026-08-06 draft with its review findings applied and every example taken
    from the built binary. The draft's "two clarity gaps" were never itemized
    anywhere, so they were addressed by best guess.
  - **Architect review: code converged, docs changes required and applied.**
    All five plan decisions, the latch-under-mutex variant and the other
    recorded deviations were approved. Findings fixed on the branch: (M1) a
    failed write of the completion document can leave the `===== RESULT`
    banner and a cut-short report in the human record — reproduced by the
    architect; documented in the guide and in ADR-0018 implementation note 9
    rather than changed in code, because an end trailer would break AC-26;
    (M2) the ADR index still said `proposed`; (M3) two inaccurate
    implementation notes; (S1) the `ulimit` test assumed the failure lands
    before the result write, which holds only for long temporary paths — it
    now asserts the record does not end with the complete report; (S2) the
    record-failure message is written to stderr in one call so a progress
    repaint cannot split it.
  - **ADR-0018 §6 overclaimed.** It says the record "contains every scanned
    path"; the record holds the roots, group and hard-link-set members, warned
    files, error paths and action paths. The architect ruled the narrower
    record correct per FR-20; implementation note 8 and the guide say so.

  - **Product-owner acceptance: FR-21 and the FR-19 amendment accepted; FR-20
    accepted on one condition, met in this PR** — a PRD-0001 wording fold-in
    (FR-20's write-failure bullet and AC-28a) saying that after a reported
    write failure the human record may hold a partial completion document and
    that exit code `1` plus the error message are the signal. Follow-ups
    raised as backlog rows: TASK-0342 (safety net for `--output` naming an
    existing image or scan root, P2, needs an owner brainstorm), TASK-0343
    (byte-pinned `--json` golden test, P2), TASK-0344 (record-header and
    `--help` polish, P3). Declined as follow-ups: a "stopped" marker for a
    refused `--apply`, and rewording `cannot write to` / `could not write to`.

  State: `make validate` passes; `cargo test` green (348 unit + 91
  integration on macOS, 93 on CI's Ubuntu leg, which also runs the two
  Linux-only tests; the 4 scale tests ignored as always); clippy `-D warnings`,
  `cargo fmt --check` and `cargo deny check` clean. The architect also built
  `main` and compared stdout byte for byte against this branch, with and
  without `--output`, in default and `--json` modes and under
  `--apply --delete`: identical.

  Verified by hand, evidence in the commit messages of `37c1fb4` and
  `be94d61`: (1) FR-21 routing on a real pseudo-terminal (a throwaway Python
  `pty` harness, not kept in the repo) in four stream configurations, all
  matching ADR-0018 §8's table; (2) AC-25 against real `SIGKILL`s on the
  2,021-file `test-images/` corpus — header + milestones, no completion
  banner; NDJSON all complete lines parseable; a completed run's record ends
  byte-for-byte with its stdout.

  Known gaps for the reviewers to judge, not rediscover: the two `/dev/full`
  end-to-end tests (`tests/output_record.rs`) are Linux-only; they ran for the
  first time on CI's Ubuntu leg for PR #107 and passed; no kill has landed in the
  verify phase on a real corpus (`test-images/` has no duplicates), so
  "already-written groups survive a kill" rests on fixture tests; there is no
  automated pty test.

  Deviations from the plan as written, for the architect: the five design
  decisions listed at the top of the plan (owner approved #1, the
  elapsed-time wording; #2–#5 await the architect); the report-once latch is
  set under the writer mutex rather than by `compare_exchange` (Task 6);
  `enumerate.rs` records at three sites, not two (walk errors below the root
  also warn); Task 9 landed as one commit (an unused parameter would have
  failed clippy on its own); `json::render` is now `#[cfg(test)]` (production
  goes through `document()` + `to_pretty()`); the existing test
  `removals_are_streamed_as_they_happen` was replaced by
  `removal_lines_are_not_written_when_stdout_is_not_a_terminal`, asserting the
  opposite per FR-21; `document_to_pretty_is_byte_identical_to_render` is
  near-tautological — the real evidence that `--json` stdout bytes did not
  move is the unchanged json/acceptance/golden suites.

  Usage, estimate vs. actual (5h window; inline implementation, no agent
  dispatches): Tasks 0–3 est. 25–35%, actual 11%; Tasks 4–7 est. 12–18%,
  actual 14%; Tasks 8–9 est. 7–8%, actual 9%; Task 10 est. 8–10%, actual not
  read. Lesson for the batch plan: **inline TDD from a plan that already
  carries the code costs roughly a third of the dispatch-based estimate**;
  the plan's original 40–70%-per-batch figures assumed implementer
  dispatches. Task 11 is dispatch-heavy (tester, product-owner, architect
  ×1–2): budget 40–60% of a fresh window, and do not start the architect
  review with less than ~35% left.

  Window 2 (2026-11-13; fresh 5h window, weekly at 12% at the start): four
  agent dispatches — tester, product-owner docs, architect review,
  product-owner acceptance — about 490k subagent tokens in total, the docs
  pass and the architect review about 195k each. Estimate 40–60% of the
  window; actual **42%** at the point the PR was opened (owner's `/usage`
  reading; the weekly figure was not read). The low end of the estimate
  held: a dispatch-heavy close-out of this size costs roughly 10% of a
  window per full-depth agent pass.

- _Session 33 (TASK-0340/TASK-0341 joint design: PRD-0001 amendment + ADR-0018),
  2026-08-06 — branch `docs/prd-0001-pager-and-output-model`, fresh window
  (5h 2% → 83%)._ Design-only continuation of session 32's backlog rows.
  Product-owner's first draft (deferred `--output <file>` again, kept the S2
  stream split unchanged) was **superseded mid-session by direct owner product
  direction**: a continuously-written `--output <file>` record, elapsed time on
  every progress render, and normal output on stdout with stderr reserved for
  genuine failures only. Two product-owner round-trips plus two architect
  round-trips (five agent dispatches total) reconciled that direction against
  the shipped NFR-3 stdout-purity contract and TASK-0308/ADR-0016's dependence
  on it. Landed, in two commits on the branch (PRD, then ADR):
  1. **PRD-0001 Amendment 2026-08-06** — **FR-20** (`--output <file>`:
     continuous durable record, human-readable by default / NDJSON under
     `--json`, additive not a replacement) and **FR-21** (stream model: stdout
     carries progress/status in default mode when it's an interactive
     terminal, falls back to stderr when redirected/piped; `--json` mode
     completely unchanged — stdout stays one document, progress stays on
     stderr, so `--json > result.json` and the ADR-0016 plugin are
     unaffected). FR-19 amended for elapsed time everywhere (no ETA — hashing
     dominates real runs per TASK-0338's numbers, so an estimate would be
     confidently wrong). NFR-3 narrowed to the result document specifically.
     New AC-25..AC-32 + AC-28a. **FR-22 (auto-pager, ex-TASK-0340) demoted to
     a tentative follow-on slice** — the durable record now solves the
     data-loss half of the original field complaint, so paging is no longer
     urgent, just still useful for scrollback; one owner question (default-on
     vs. opt-in, Windows gap) blocks it from folding into the PRD body.
  2. **[ADR-0018](../adr/0018-durable-run-record-and-output-stream-routing.md)**
     — the mechanism design PRD-0001 hands to the architect. Key finding: a
     duplicate group's **membership is provably final before the run
     completes** (the candidate set closes before grouping starts, and
     `group_by_size`/`group_by_hash` only ever append) — so FR-20's "as soon
     as known final" claim holds structurally, no product rework needed.
     Resolved three owner-adjudicated questions the design surfaced:
     apply-time `trashed:`/`deleted:` lines get **no stderr carve-out** (they
     were never load-bearing there — FR-14's `actions` object and FR-20's
     `action` NDJSON event already cover them); `--json` without `--output`
     having **no incremental trace is a deliberate accepted decision**, not a
     gap (chainability for `jq`/the plugin outweighs it, and a crashed pipe is
     comparably fragile regardless); and a mid-run record-write failure
     **continues the run, reports once, and exits 1** (an `AtomicBool` latch
     keeps a full disk cheap on the rayon hot path). A consequence note on
     ADR-0008 records the finalization property as a pipeline-level fact.
  **Window 1 stopped at 83% of the 5h window** (est. 40–55% for the design
  step; actual cost roughly doubled by the mid-session direction change adding
  two full extra round-trips) — logged as a batch-plan lesson: **a
  human-directed pivot mid-design costs about as much as starting the design
  over**, not a small delta on top of the original estimate.

  **Window 2, same day.** Product-owner drafted the guide-doc pass
  (`README.md`, `output-and-exit-codes.md`, `getting-started.md`) against the
  confirmed FR-19/FR-20/FR-21 text, marked "not in a release yet" since
  `--output` doesn't exist in `src/cli.rs`. The required whole-branch-diff
  architect review ([`docs/process/sdlc.md`](../process/sdlc.md), ADR-0011)
  found it **not ready**: a stale self-contradiction in this file (this
  paragraph, previously claiming the guide pass as undone after it had
  landed), two factual-drift findings verified against `src/apply.rs` and
  `src/main.rs` (a stderr category with nothing that writes to it; the apply
  skip-line reclassification stated as an unqualified "genuine failure" for
  all ten `SkipReason` variants, when four — including the two
  `--follow-symlinks`-reachable symlink variants — are documented in source as
  routine, not failures), plus a policy question: every prior guide-doc update
  in this repo rides its feature's _implementation_ PR: should this one break
  that precedent by documenting unshipped behavior with disclaimers, or hold?
  **Owner decision: hold** — the guide-doc commit was reverted
  (`a2fd3d2`, reverting `1325211`) rather than kept with disclaimers, so this
  PR stays PRD + ADR only, and the six findings' fixes are moot for now but
  recoverable from `1325211` (a good starting draft) when TASK-0340/0341's
  implementation slice writes the guide for real. The architect's own
  ADR-0018 had the same gap the guide inherited (`§9 #7`'s skip-line hedge
  only named one of the four routine `SkipReason` variants) — fixed directly
  (`48b5700`), widening the hedge to all four with the concrete evidence from
  `apply.rs`, decision unchanged. This unit is now **PRD-0001 Amendment
  2026-08-06 + ADR-0018 only**, architect-reviewed, merged as PR #98 (2026-08-06);
  TASK-0340/0341's backlog rows point at the reverted guide-doc draft for
  reuse. Implementation (TDD, both FR-20/FR-21 first, FR-22 paging as a later
  slice, guide docs riding that PR per precedent) is the rest of this backlog
  item, scheduled for a fresh window.

- _Session 32 (new TASK-0339/TASK-0340/TASK-0341, real-world dogfooding
  follow-ups; planning-only), 2026-08-06 — merged [PR #97](https://github.com/intruder1912/imgdedup/pull/97)._
  Owner ran the local release binary
  (built and validated on the test-images corpus in session 31) unattended
  against a real ~69k-file corpus on an SMB-mounted Synology NAS volume.
  Runtime was much longer than the test-images corpus (no overall-time figure
  was captured) but the run completed; the two things worth recording came
  from watching it run, not from the result.
  1. **macOS appeared to throttle or effectively hibernate the process after
     a period of no user activity, despite an indefinitely-running
     Amphetamine session** that should have kept the system awake — not yet
     root-caused (App Nap, system idle/display sleep, SMB session idle
     disconnect, NAS disk spindown, Wi-Fi/interface power-save, and other
     network-specific throttling are all plausible and only distinguishable
     by testing).
     Recorded as **TASK-0339**: diagnose the mechanism first, then an
     opt-in, default-off, cross-platform "stay awake" flag — macOS
     `IOPMAssertion`/`NSProcessInfo.beginActivity`, Linux
     `systemd-inhibit`/the logind inhibitor API, Windows
     `SetThreadExecutionState`.
  2. **The FR-19 progress indicator (phase + %) earned its keep at this
     scale** — the owner could see the run was alive and how far along it
     was — but two related gaps surfaced only because this was a large,
     easy-to-forget-to-redirect run: the final report has no size bound and
     can overrun terminal scrollback if stdout was not redirected
     (**TASK-0340**), and the wider stdout/stderr split — stdout silent for
     the run's entire duration, everything visible on stderr, the report
     flushed once at the very end — reads backwards against normal CLI
     expectations and loses everything if a run is interrupted before
     completion (**TASK-0341**, explicitly flagged as needing resolution
     before the repository goes public, alongside TASK-0337).
  All three are `todo`; TASK-0340/TASK-0341 need a product-owner PRD-0001
  amendment and TASK-0339 an architect ADR before any code is written — this
  session only records the observations and the backlog rows (see
  `backlog.md`) so they survive past chat history, per
  [`CLAUDE.md`](../../CLAUDE.md) §2. No code, no CI-relevant files touched.

- _Session 31 (TASK-0338, performance instrumentation — implementation), 2026-08-05
  — fresh window._ Finishes what session 30 deliberately stopped at (the design
  step, [ADR-0017](../adr/0017-performance-instrumentation-tracing-spans.md)):
  the TDD implementation, its architect review, the real-corpus measurement the
  whole task exists to produce, and the product-owner docs pass.
  **Housekeeping first:** 34 local branches, each individually verified via
  `gh pr list --head <branch>` to correspond to an already-merged (or, in one
  case, closed-without-merging) PR, force-deleted with explicit confirmation;
  one stale remote branch (`docs/sync-planning-task-0301`, PR #15, closed not
  merged, content already superseded in `main`) removed too. Only `main` and
  `chore/instrument-with-tracing` remain, locally and on `origin`.
  **Implementer (TDD), on `chore/instrument-with-tracing`:** all 11 spans from
  ADR-0017's table, the `--timings` flag, the progress-off interaction, and the
  security rule (no span field via `%`/Display) — matching the ADR exactly, per
  independent verification below. One real deviation from the ADR's illustrative
  code, flagged and reasoned: `apply::apply()` gained a `mode: ActionMode`
  parameter (used only to record the `apply` span's `mode` field; removal
  behavior is unchanged and still driven entirely by the injected `Remover`) —
  the ADR's own text hadn't threaded `mode` into that call, so a real signature
  change was needed to record the field the ADR's table asks for. Also replaced
  a per-test `tracing::subscriber::with_default` helper with a process-global
  one (`src/trace_test_support.rs`, `#[cfg(test)]`-only): a genuine
  `tracing-core` per-callsite `Interest`-cache race under `cargo test`'s default
  parallel runner, reproduced directly (a nested span intermittently produced no
  close line under default parallelism, never under `--test-threads=1`) —
  confirmed by the architect review below against `tracing-core` source, not
  just re-run stability.
  **Independent architect review (one dispatch, converged on the branch): one
  real defect found and fixed, everything else confirmed by direct measurement
  rather than reading the diff — continues the run noted in
  [[imgdedup-architect-review-value]].** `FmtSpan::CLOSE` fires when a span's
  **value** (not its guard) drops; `group_by_size`/`hash`/`verify`/`sort_canonical`
  (engine) and `render` (main) had created that value in function scope while
  only entering/exiting the guard per-stage, so their close lines were deferred
  to end-of-function and printed in **reverse** pipeline order with each
  other's wall time bleeding into `time.idle` — reproduced live pre-fix
  (`sort_canonical`, `verify`, `hash`, `group_by_size`, in that order, all
  within microseconds of the engine returning). Fixed by scoping each stage's
  span value to its own block; two new regression tests (CLI-level and
  engine-unit-level), each independently verified to fail against the pre-fix
  code, not just pass post-fix. Also independently re-verified rather than
  trusted: all 11 spans at the right sites with the right fields (via an
  end-to-end run exercising all 11 at once); zero spans inside any rayon
  closure and zero per-file/entry/root/group spans (exhaustive grep); the root
  `run` guard genuinely lives inside `run()` (last-stderr-line test, confirmed
  across five invocation modes); the security rule holds crate-wide (every
  production span field is a count, size, or enum — never a path); `--timings`
  forces progress off with no bypass (`args.no_progress` has exactly one
  reader in the whole tree); stdout byte-identical with/without `--timings` on
  a 4,000-file fixture and on `--json` (866 KB, `cmp`-verified); `cargo deny
  check` clean, no `deny.toml` edit, `Cargo.lock` 123→130 exactly as predicted;
  cost when unused measured directly (3.563 s vs 3.564 s over 4,000 files);
  binary size +225 KB (ADR estimated ~228 KB); stderr line count is O(1) per
  run regardless of file count (9 lines on both a 6-file and a 4,000-file
  tree). Should-fix nits noted, none blocking: `hash`'s `files_hashed` counts
  attempts while `bytes_hashed` only accumulates on success (asymmetric, not
  wrong — worth a comment); `apply(mode, remover)` can in principle be passed
  an inconsistent pair (accepted, since deriving the label from `Remover`
  instead would make it unrepresentable); the crate-wide `%`-vs-`?` security
  guard is currently a manual grep, not a CI-enforced one (a future task, not
  this one, if ever wanted).
  **The real-corpus run this whole task exists to produce, done directly
  (not delegated) against the same 2,021-file / 5.72 GB real photo corpus
  sessions 26/29 used (`test-images/`, gitignored, still present on this
  machine):** 3 runs, warm cache, default scope. **`hash` dominates at ~86% of
  wall time** (401–417 ms of ~480 ms total); `sniff` ~11% (48–57 ms);
  `enumerate` ~2% (8–11 ms); `verify` ~0 (0 duplicate groups in this corpus, so
  nothing to verify). **This closes TASK-0321's open question
  (["is `enumerate` now the dominant serial stage?"](../adr/0010-concurrency-and-memory-model.md#consequence-note--the-scan-stage-is-no-longer-serial-2026-08-03-task-0321)):
  no** — recorded as a new consequence note on ADR-0010, with the honest
  caveat that this is one corpus shape (avg. ~2.8 MB/file, every file clearing
  `RAYON_HASH_THRESHOLD` — the smallest is 1.19 MB) and a tiny-file-heavy
  corpus could shift the `enumerate`/`hash` ratio; not claimed to generalize.
  **Product-owner pass:** concurs with ADR-0017's "no PRD-0001 FR/AC" ruling
  (checked the reasoning rather than rubber-stamping it: `--timings` emits
  nothing on stdout and changes no grouping/removal decision). Wrote a new
  `--timings` section in
  [`docs/guide/output-and-exit-codes.md`](../guide/output-and-exit-codes.md)
  (how to read the nested stage lines, that it replaces progress, combining
  with `--json`, diagnostic-not-benchmark caveat) plus a README usage mention
  and a guide-index pointer. Two findings routed back rather than fixed
  unilaterally (product-owner has no shell/write access to `.rs` files by
  design): the `--timings` `--help` text was one ~8-line paragraph — the
  longest entry in `--help` for the least important flag — and used internal
  jargon (backtick-quoted crate names, an ADR citation) instead of user-facing
  wording; rewritten to the project's two-tier summary/detail convention
  (`--keep`'s precedent), verified the one existing `--help` test only checks
  for the substring `"--timings"` so this was a safe non-behavioral change.
  Also caught: ADR-0017's "output is pure ASCII" claim is wrong —
  `tracing-subscriber`'s own `Duration` formatting renders the microsecond unit
  as `µ` (U+00B5), confirmed in a real capture (`time.busy=207µs`); corrected
  in the ADR with the reasoning for why this is accepted rather than worked
  around (a numeric-field unit glyph carries none of the injection risk
  `progress.rs`'s stricter rule exists to prevent). One item deliberately
  deferred, not fixed, as disproportionate inside a chore unit: AC-23's
  literal wording names only `--no-progress` as the way progress is
  suppressed, and a future reader could wonder whether `--timings` (a second,
  incidental way) is in scope — closed with a same-day dated PRD-0001
  amendment (no behavior change; AC-24 already covered it by phrasing) rather
  than an in-place edit, matching this document's own convention.
  `make validate` (mdlint, spell, links, shellcheck, actionlint, fmt, clippy
  `-D warnings`, full test suite) green throughout — 301 unit + 24 acceptance +
  45 CLI-integration + 7 golden-set tests, `scale.rs`'s 4 correctly ignored.
  **Final architect confirmation pass over the four post-review commits
  (measured, not read):** the corpus run reproduced independently (3 fresh
  runs — `hash` 435–447 ms, `sniff` 50–60 ms, `enumerate` 8–15 ms, i.e. the
  note's ~86 / ~11 / ~2% split holds); `µ` confirmed as U+00B5 in real output
  and `nu-ansi-term` confirmed absent from `Cargo.lock`, so ADR-0017's
  corrected UTF-8-not-ASCII claim is right on both halves; the binary-size
  delta re-derived by building `main` in a scratch worktree (1 724 176 →
  1 954 688 bytes = +225 KiB); lock 123 → 130 and `cargo deny check`
  advisories/bans/licenses/sources all ok; 9 stderr lines on both a 6-file and
  a 4 000-file tree; stdout `cmp`-identical with and without `--timings` on
  both report and `--json`; `--timings` still in `--help` (and now a single
  line under `-h`); `make validate` plus `cargo test --release` green.
  **One real correction made in this pass:** the ADR-0010 note quoted
  "~2.2 GB/s aggregate" for the `hash` stage — that is session 29's
  _per-CPU-second_ rate over the whole 5.72 GB corpus, not this stage's
  wall-clock rate, which is ~5.2 GB/s (`bytes_hashed=2168910223` over 417 ms);
  corrected there and the avg-file-size parenthetical tightened to the
  measured ~2.8 MB/file (5.72 GB / 2 021) in both the ADR and this entry.
  Session 29's own "2.76 MB/file" is the same arithmetic slip but is left
  as-is: it is a dated historical record, and no conclusion turns on it.
  Next: PR. This PR.

- _Session 30 (TASK-0338, performance instrumentation — design step), 2026-08-05
  — same window as sessions 26–29._
  **The design landed as new
  [ADR-0017](../adr/0017-performance-instrumentation-tracing-spans.md); the TDD
  implementation pass is the rest of this unit** (new **TASK-0338**, chore,
  `chore/instrument-with-tracing`). Prompted directly by session 29's numbers
  immediately above: 0.50 s of wall clock over 2,021 real photos, with **no way
  to say how much of it was the walk, the sniff, the hashing, or the verify** —
  the same blind spot that made TASK-0312, TASK-0321 and TASK-0322 each build a
  throwaway control fixture or standalone harness, and that leaves ADR-0010's
  "is `enumerate` (still serial `walkdir`) the dominant stage now that sniff is
  parallel?" open **in writing**.
  **The `tracing`-over-OpenTelemetry call was the owner's, made in
  conversation** (this ADR records it properly rather than deciding it): OTLP
  exporters pull `tonic`/`hyper` → **`tokio`**, i.e. an async runtime added to a
  fully synchronous, rayon-parallel CLI purely to serve a backend almost no run
  will connect to. `tracing` is the ecosystem's zero-cost layer, and
  `tracing-opentelemetry` keeps the OTLP door open later with **zero annotation
  churn**. **Measured rather than asserted** (release build, M2 Pro, harness in
  the scratchpad, not checked in): 3M span create/enter/exit cycles with **no
  subscriber** cost 3.73 ms vs. a 3.29 ms span-free baseline — **~0.15 ns per
  span**, at or below noise, which is what makes "always compiled in, inert by
  default" honest.
  **Four design questions the owner's framing left genuinely open, and how they
  were closed.** (1) **Span boundaries mirror the ADR-0008 _pipeline_, not the
  display `progress::Phase`** — 11 coarse spans (`run`, `enumerate`,
  `symlink_alias_dedupe`, `sniff`, `dedup`, `group_by_size`, `hash`, `verify`,
  `sort_canonical`, `apply`, `render`), each opened by the function that owns
  its stage. `Phase` merges `enumerate` and `sniff` into "Scanning" **by product
  decision** (AC-21/21a), which is exactly the split that needs measuring, and
  it covers nothing after verification — mirroring it would make the
  instrumentation structurally unable to answer the one question it exists for.
  Per-file/per-entry spans are **rejected on a measurement**: ~1.75 µs each with
  a subscriber attached, i.e. ~52 ms and **30,000 printed lines** on a 30k-entry
  walk — the exact tension TASK-0332's `advance(0)` tick and TASK-0317/0328's
  repaint throttling already navigated in this same hot loop. Counts go on the
  stage span as fields (`files_hashed`, `bytes_hashed`, …) instead: O(1) events
  per run, not O(files). (2) **A day-one consumer ships in the same unit** — a
  new `--timings` flag installing a `tracing-subscriber` `fmt` subscriber on
  stderr. Spans with nothing subscribed produce _literally nothing observable_,
  so "instrumentation only" would deliver none of the stated goal and would
  leave the PR with no behavior to assert under mandatory TDD. `RUST_LOG` was
  rejected: `env-filter` pulls `regex` for one boolean's worth of intent.
  (3) **Dependency footprint, measured on a scratch copy of this exact crate**:
  normal graph 51 → 60, `Cargo.lock` 123 → 130 (only 7 genuinely new — the
  proc-macro chain is already there for clap/serde derives), all
  MIT / MIT-OR-Apache-2.0, and **`cargo deny check` run green**
  (`advisories ok, bans ok, licenses ok, sources ok`) with **no `deny.toml`
  edit**; `default-features = false, features = ["fmt"]` is doing real work
  (63/134 with defaults vs. 60/130 without) and keeps ANSI machinery out of a
  tool that hand-writes its own terminal control. (4) **`--timings` implies
  progress-off** — not a nicety: a stage-close line written while the TTY
  progress line is live lands _inside_ it, the exact corruption `progress.rs`'s
  module doc forbids. Usage-error, buffer-until-`finish()`, TTY-sniffing and
  routing-through-`Progress` were each weighed and rejected in the ADR.
  **Two traps recorded for the implementer/reviewer, both verified this
  session:** the root span guard must live _inside_ `run()`, since `main()` ends
  in `process::exit` and runs no destructors (otherwise the run-total line
  silently never prints); and **no span field may carry a path via `%`/Display**
  — a filename containing `ESC[2J` was demonstrated reaching the terminal raw
  through `fmt`, precisely the injection TASK-0318/`safe_text.rs` exists to
  stop (`?`/Debug escapes it; shown side by side in the ADR).
  **Product scope:** no PRD-0001 FR/AC is added — `--timings` changes no result
  and writes nothing to stdout, so it is engineering tooling in service of
  NFR-4, not product capability; the flag's _name/help text_ still wants a
  lightweight product-owner sign-off and its user docs are product-owner-owned
  (ADR-0012). `tests/scale.rs` remains the formal NFR-4 benchmark — this is a
  diagnostic, not a regression gate. Design step only; no production code
  written. **Deliberately stopped here, not carried into the same window's
  implementation:** the architect's own size estimate ("medium," 7 production
  files, and — the part not worth cutting — a follow-up run against the real
  corpus to actually record the `enumerate`/`sniff` split ADR-0010 leaves
  open) came with an explicit "if less than ~50% of the current 5h window
  remains, start it fresh" line; the owner checked `/usage` at exactly that
  point — **5h=62%, 7d=30%** — and confirmed the cut. TASK-0338's backlog row
  and this ADR are the resumption point; no PR opens yet, per
  [`branching.md`](../process/branching.md)'s "hold the PR until the unit is
  converged" — this is a `chore/instrument-with-tracing` branch pushed with
  the design step only — implementation follows in a fresh session, then the
  whole branch (design + TDD code) opens as one PR and gets one review pass,
  the same shape TASK-0219 used earlier this session.

- _Session 29 (TASK-0314 addendum: informal real-world performance numbers;
  planning-only), 2026-08-05 — same window as sessions 26–28._ Captured on
  request, on the same corpus and machine as session 26's field validation
  (2,021 real JPEGs, 5.72 GB, one 2009 vacation-trip tree, Apple M2 Pro, 10
  cores) — not part of TASK-0314's own scope (that task is correctness-only)
  and **explicitly not a formal benchmark**: single machine, single hardware
  configuration, no `main`-baseline comparison, warm-OS-cache only (the one
  cold-cache run measured, 1.84 s wall, was not repeated rigorously), and one
  corpus shape (avg. 2.76 MB/file — every file in it clears TASK-0322's 1 MiB
  `RAYON_HASH_THRESHOLD`, so this run exercises the parallel intra-file
  hashing path on essentially every file, unlike a corpus with many small
  images). `tests/scale.rs` (`make scale`) remains the project's actual
  controlled regression benchmark (NFR-4); this is a single illustrative data
  point on genuine camera files, recorded here rather than left in chat only.
  **Numbers** (`/usr/bin/time -l`, 3 runs, warm cache):

  | Run | Wall | User | Sys | Peak RSS |
  |---|---|---|---|---|
  | plain report | 0.57 s | 1.62 s | 1.04 s | 46 MB |
  | `--json` | 0.50 s | 1.61 s | 0.95 s | 61 MB |
  | plain report (repeat) | 0.50 s | 1.61 s | 0.96 s | 65 MB |

  ~11 GB/s wall-clock, ~2.2 GB/s per CPU-second (user+sys spread across
  parallel cores, not a single-core figure), ~4,000 files/sec. **Peak memory
  46–65 MB against 5.72 GB of input** is the number worth keeping: it's a
  real-data confirmation that ADR-0010's bounded-memory design
  (`O(2 × pool width × buffer)`, TASK-0322's amendment) holds outside the
  synthetic fixtures it was originally measured against. This PR.

- _Session 28 (new TASK-0337, public-readiness pre-flight; planning-only),
  2026-08-05 — same window as sessions 26–27._ The public-readiness
  conversation that produced session 27's docs fix also produced a set of
  verified findings (fork-PR/Actions safety, repo permissions, secret/PII
  history, git-history handling, test-data privacy — see that conversation,
  not a task file) that were at risk of being lost to chat history rather than
  the repo, the one thing [`CLAUDE.md`](../../CLAUDE.md) §2 says never to rely
  on. Recorded as new **TASK-0337**, wired as a new dependency of both
  **TASK-0326** and **TASK-0336** (both already gated on "the repository goes
  public" — TASK-0337 is the pre-flip half, TASK-0336's Trigger A the
  at-the-flip half). Most of TASK-0337 is already closed by the audit itself
  (full findings in its backlog row): full-history gitleaks scanning since the
  repo's first CI commit, no `pull_request_target` anywhere, no custom
  secrets, both `contents: write` jobs gated behind manual dispatch, no
  PII/absolute-path leaks, no real image binaries ever tracked, and a
  deliberate decision **not** to rewrite git history (the all-or-nothing
  nature of GitHub's visibility toggle means the only way to actually "limit"
  it is a rewrite or a fresh repo, and the audit found nothing that needs
  hiding). Two items genuinely still open and left for a pre-flip session:
  tightening GitHub's fork-PR-workflow-approval setting to "require approval
  for all outside collaborators" (owner-only, not settable via the `gh api`
  surface available here) and the still-undecided planning-docs restructuring
  question (`status.md`'s session-narration volume vs. a public first
  impression). Docs-only, no code, no write-permission CI touched — claimed
  under ADR-0011's low-risk skip exception as a **planning/status sync**
  (the exception's own named category), self-verified rather than
  architect-dispatched: every claim in the new backlog row restates something
  already independently verified live in this session or session 27's
  architect review, nothing new is asserted. This PR.

- _Session 27 (`SECURITY.md`/`CONTRIBUTING.md` staleness fix, chore), 2026-08-05
  — same window as session 26._ Surfaced during a public-readiness discussion
  (Actions/fork-PR safety, repo permissions, secret-history coverage, git
  history limits, and test-data privacy — none of which needed code changes;
  see this session's conversation, not a task file) that predates any actual
  decision to go public: two docs were simply **wrong today**, independent of
  visibility. `SECURITY.md` still said "pre-release (Phase 0), no versions
  supported" and described the language-level dependency audit as a Phase-2
  future item — the project is in Phase 3 with `v0.5.1` released and
  `cargo-deny` (RUSTSEC/license/bans/sources, weekly-scheduled) live since
  TASK-0208. Fixed: a "latest release only" supported-versions policy, an
  accurate description of the existing `cargo-deny`/Dependabot/gitleaks
  security surface (including the `contents: write`-is-manual-dispatch-only
  fact, relevant to the fork-PR-safety question raised in conversation), and a
  static-analysis bullet recording clippy-in-place-of-CodeQL (ADR-0007), which
  the dropped Phase-2 sentence had promised as future work.
  `CONTRIBUTING.md`'s "Local validation" section still read "until the product
  stack is chosen (Phase 2)... documentation quality... (once code exists) the
  language test suite" — also predates the Rust crate existing. Fixed to
  describe what `make validate` actually runs today (mdlint/spellcheck/
  linkcheck/shellcheck/actionlint/fmt/clippy/full test suite), plus a new
  paragraph stating explicitly that no real images are needed to build or test
  the project — the test suite is 100% synthetic (`tests/support/images.rs`
  generates the smallest byte sequences the pipeline accepts per format
  signature; nothing real-world is checked into the repo), directly answering
  a contributor-privacy question raised in the same conversation (verified
  live: zero image binaries tracked in git, zero references to the owner's
  local `test-images/` corpus anywhere
  in code/CI/Makefile). Docs-only, no product code, no write-permission/secret
  CI surface — same shape as TASK-0217's docs-consistency precedent — but not
  claimed under ADR-0011's low-risk skip exception, since that exception is
  explicitly scoped to "a planning/status sync or a CI-tooling chore... nothing
  else by analogy," and neither file is either; goes through the normal
  architect review. This PR.

- _Session 26 (TASK-0314, real-photo field validation), 2026-08-05 — fresh
  window._ Closes the last dependency-clear item this repo had been carrying
  as blocked since sessions 4/5: the owner supplied a real photo corpus by
  copying it into a new gitignored `test-images/` directory inside the repo
  itself (chosen for simplicity over a path elsewhere on the owner's
  machine — `.gitignore` gained a `test-images/` entry so it can never be
  committed). **The corpus:** 2,021 real JPEGs (5.5 GB) from a single 2009
  vacation trip, shot across three distinct camera bodies (filenames carry
  their own EXIF-adjacent evidence of this: Canon EOS 300, IXUS50, IXUS850),
  plus ~46 unrelated non-image files (mp3/doc/db/ini/avi) mixed into the same
  subfolders — a deliberately messier shape than the golden-set generator's
  synthetic fixtures, closer to what a real photo library actually looks
  like. Only one subfolder existed; the owner had described a second,
  further ~1k-photo copy as "underway" but confirmed live that what was
  already present was the complete corpus before this ran.
  **Both required passes ran clean, read-only, against the real corpus, no
  `--apply` anywhere:** plain dry-run and `--json` dry-run each exited 0 in
  ~1.8s, zero warnings, zero `root_errors`, the JSON valid against the
  documented schema (`schema_version: 1`). Result: **0 duplicate groups** —
  plausible for one trip's worth of distinct shots, but a bare "found
  nothing" is weak evidence on its own (a broken hashing path would report
  the same thing), so it was independently checked rather than trusted: a
  full `shasum -a 256` over all 2,021 files, entirely outside imgdedup's own
  code, found 0 matching hashes — confirming the negative result against
  ground truth from a completely separate implementation, not just replaying
  the tool's own logic back at itself.
  **A negative result alone doesn't exercise the grouping/keeper logic, so a
  positive control followed, in a scratch directory outside the repository
  and outside `test-images/` (which stayed untouched throughout, satisfying
  the task's "originals untouched" requirement even though this copy was
  already once removed from any original):** 8 real photos sampled evenly
  across the corpus, 4 copied once more into a second directory to create 4
  known byte-identical pairs (verified with `shasum` before running
  imgdedup) alongside 4 deliberately-unique singles. `imgdedup --json`
  against the mixed set reported **exactly** 4 duplicate groups — correct
  keeper, correct `reclaimable_bytes`, correct per-file sizes — and correctly
  excluded all 4 singles, matching the `shasum`-derived ground truth exactly.
  This is the first time the detection pipeline has run against real camera
  JPEGs with genuine, non-synthetic EXIF/metadata variety across three
  different camera models rather than the golden-set generator's minimal
  valid fixtures, and it held up without any surprises. No product code
  change resulted — nothing this task was designed to catch, caught anything —
  so per ADR-0011's low-risk-unit exception (no code diff, no write-permission
  CI surface, same shape as TASK-0327) this unit skips an architect dispatch.
  Scratch positive-control files deleted after the run; only `.gitignore`
  (new `test-images/` entry) and the planning docs are part of this diff.
  This PR.

- _Session 25 (TASK-0219, public-release licensing readiness), 2026-08-05 —
  fresh window._
  **The architect's four rulings landed as a dated
  [amendment to ADR-0014](../adr/0014-project-license-gpl-3.0-only.md#amendment-2026-08-05--public-release-licensing-readiness-task-0219);
  the TDD implementation pass is the rest of this unit.** ADR-0014 deliberately
  left four edges open for this task, and all four are now closed as
  _decisions_ rather than as a checklist to re-derive on public-release day.
  **(1) Per-file GPL headers: no** — a reasoned decision, not a deferral.
  GPLv3's per-file guidance is a non-normative appendix; this project conveys
  repositories and release archives, never single files (one crate,
  `publish = false`, `[[bin]]` only, no library target); the scanners that
  actually read this repo (GitHub license detection, `cargo-deny`,
  `cargo metadata`, crates.io) read `Cargo.toml`/`LICENSE` and never read Rust
  comments; and an ungated header convention rots into _worse_ evidence than
  none — a scanner finding headers on 12 of 20 files reads the repository as
  partially licensed. Revisit triggers are named (a library target or
  file-level vendoring, a second copyright holder, or adopting REUSE for its
  own sake), and if it ever flips it flips to **full REUSE with a `reuse lint`
  CI job**, not a piecemeal `src/*.rs` pass. **(2) `--version`: yes**, a
  GNU-conventional block (copyright, `License GPL-3.0-only:` with the canonical
  URL, the two recognizable warranty sentences, and a `Source:` line), added
  via clap's `long_version` so that **`-V` keeps printing exactly
  `imgdedup <version>`**. That split is not cosmetic: the one-line form is
  load-bearing for `scripts/build-release-artifact.sh`, which asserts
  version-stamp correctness by _exact equality_ on it (the check that stops a
  mislabeled binary from being published, ADR-0015). The only other in-repo
  caller — the Claude plugin's preflight — runs `--version` and checks only
  that it succeeds, so the block reaches it harmlessly and no plugin change is
  needed. clap 4's `version`/`long_version` split was
  verified by direct experiment rather than read off the docs, including that
  clap prepends the binary name itself. Wording decided rather than left to
  taste: ASCII `(C)` not `©` (legacy Windows console code pages — the class of
  problem `safe_text.rs` exists for), the SPDX identifier rather than GNU's
  `+`-suffix "or later" convention, ≤80 columns per line so the license URL
  never re-wraps, and the `Source:` URL taken from `CARGO_PKG_REPOSITORY` so it
  cannot drift from `Cargo.toml`. Recorded explicitly as **voluntary**: GPLv3
  §5(d) binds _interactive_ user interfaces, and this CLI reads no stdin at all
  (verified across `src/`). **(3) GPLv3 §6: the route is §6(d), and only the
  "clear directions" half needs writing.** GitHub attaches an auto-generated
  source archive to every Release and keeps the tag fetchable, which _is_
  equivalent access to the Corresponding Source from the same place; §6(d)'s
  one human-supplied requirement is directions next to the object code, and
  those ship **now** in the two artifacts that travel with the binary — the
  `Source:` line inside `--version`, and a Corresponding Source paragraph in
  `README.md`, which the release archives already bundle next to `LICENSE`
  (zero new files, zero packaging changes). A §6(b) **written offer is
  deliberately not written**: that is the artifact that would actually mislead,
  advertising a three-year source-delivery channel nobody operates. The
  directions, by contrast, are honest today — they say where the source is,
  which is already true for every possible recipient of a private release asset
  (TASK-0211's position) and becomes true for everyone at the flip with no edit.
  **(4) `-only` vs `-or-later`: re-checked, unchanged** — and recorded rather
  than silently skipped. Whole-history `git log` shows a single copyright holder
  (two owner identities on the same GitHub no-reply address, plus
  `dependabot[bot]` manifest bumps carrying no copyrightable authorship of
  project code) and no outside contribution has ever existed, so ADR-0014's
  reasoning applies verbatim. No CLA; DCO only if outside PRs actually appear;
  re-check trigger is the first PR authored by anyone else — **not** the public
  flip, which creates readers, not copyright holders. **Residual work is
  trigger-bound and split out as new TASK-0336** (verify at the flip that every
  published release's auto-generated source archive is really present and
  anonymously downloadable, plus the guide/process wording that assumes a
  private repo; and re-run the `-only` check before merging a first outside
  PR).
  **Implementation (TDD), same session:** `src/cli.rs` gained a `LONG_VERSION`
  `concat!` constant (starting with the bare version — clap prepends the
  binary name itself to `long_version`'s render, verified empirically against
  a scratch clap 4.6.3 crate before touching production code) wired via
  `#[command(version, long_version = LONG_VERSION, ...)]`; `-V` is unchanged
  (`imgdedup <version>`), `--version` now prints the full 6-line block, both
  confirmed against a real `cargo run --`. Five new `src/cli.rs` unit tests
  (the exact block, ASCII-only, ≤80 columns/line, first-line/`-V` drift guard,
  and a regression pin on the pre-existing `-V` contract — the last
  deliberately not forced red, since it guards behavior that already existed)
  plus two `tests/cli_integration.rs` tests exercising the real binary via
  `assert_cmd` for both flags. `scripts/build-release-artifact.sh`'s
  exact-equality version-stamp assertion moved from `--version` to `-V`
  (the one-line form that assertion depends on),
  with a second check that `--version`'s first line matches the same stamped
  value — sliced from a captured variable via bash parameter expansion rather
  than piped through `head`, avoiding a `set -o pipefail` SIGPIPE footgun
  under the script's `set -euo pipefail`. `docs/process/release.md` updated to
  match. `make validate` green (289+24+38+7 tests passing, `scale.rs`
  correctly skipped). The README Corresponding Source paragraph (§6(d),
  product-owner-owned under ADR-0012) was routed to the product-owner rather
  than landed unilaterally: three short paragraphs rather than the ADR's one
  dense sentence, corrected to no longer describe `--version` as printing a
  bare version string (it now prints the license block, which the paragraph
  cites as the _other_ half of the §6(d) directions), and a pointer to the
  ADR amendment rather than restating its legal reasoning inline. Explicitly
  left out of both passes, per the ADR's own rejections: per-file SPDX
  headers, a `NOTICE` file, a §6(b) written offer, a `--license` flag, and any
  build.rs/vergen commit stamp.
  **Architect review of the branch diff vs `main` (2026-08-05), independent of
  the instance that designed it — reproduced rather than read.** Verified:
  `--version`/`-V` byte-for-byte against the running binary (stdout only,
  stderr empty, exit 0, ASCII-only, longest line 79 columns), and stable under
  a renamed executable (clap uses `#[command(name)]`, not `argv[0]`); the five
  unit tests and two integration tests were **mutation-tested** — a year
  change, a non-ASCII `©`, an over-80-column URL and a `Cargo.toml`
  `repository` change each turned the intended test red and nothing else; the
  new shell assertion block was harness-run against three fake binaries (good,
  `-V`-skewed, `--version`-skewed) and the `${var%%$'\n'*}` slice re-verified
  under **bash 3.2** as well as 5.x, since `#!/usr/bin/env bash` can resolve to
  3.2 on a macOS runner; `shellcheck` clean; `make validate` green. Removing
  the `head` pipe _eliminates_ the `pipefail` exposure rather than moving it —
  the new code has no pipeline at all. `make release-preflight` was judged
  unnecessary: the changed lines are two assertions, exercised directly by the
  harness above, and the rest of the script is untouched. Four findings, all
  fixed in place by the reviewer, none in product code: (a) the ADR/backlog/
  status all claimed the Claude plugin's preflight depends on the one-line
  form — it actually runs `--version` and parses nothing, so the rationale was
  resting on a wrong fact (the `-V` rationale stands on
  `build-release-artifact.sh` alone); (b) ruling 3's load-bearing "GitHub
  attaches a source archive" claim was asserted, not checked — now verified
  over the API (15/15 releases carry `tarball_url`/`zipball_url`; the `v0.5.1`
  tarball inspected, 150 entries, complete tree; `.gitattributes` carries no
  `export-ignore`, the one setting that could silently falsify it) and
  recorded; (c) ruling 3 didn't mention that the tag's `Cargo.toml` holds the
  `0.1.0` placeholder while the released binary is stamped — noted, with why
  §6(d) still holds (§1's "scripts to control compilation" are in the archive);
  (d) ruling 4's `%an`/`%ae` evidence doesn't see the 78 `Co-authored-by:
  Claude` trailers — the only thing in this history that reads like
  co-authorship — so why they create no second copyright holder is now
  recorded. Plus one stale usage line in `scripts/release-preflight.sh`
  (`--version` → `-V`). Deliberately **not** touched: the
  "`--version` assertion" phrase in `release.yml`'s comment (still true, and
  not worth extending this unit's diff into the repo's only `contents: write`
  workflow for a comment). No blocking findings; the single architect dispatch
  is the right depth here — nothing in the unit holds a write permission or a
  secret. Next: PR. This PR.

- _Session 24 (TASK-0332, scanning's walk liveness cue), 2026-08-05 —
  continuing session 23's window (5h=49% at start; owner explicitly
  authorized running this session up to 95%, above the usual 60–70%
  target/75% hard stop — a real-time override, not a process miss, recorded
  here per the session-12 precedent for the same kind of override)._ Closes
  the residual TASK-0328's own ruling named and explicitly declined to fix:
  on a tree where very few entries match the extension scope (a source
  checkout, a backup volume), the tree-walk sub-phase of Scanning can grind
  for a long stretch while `scanning... 3 files found` sits static — truthful,
  but indistinguishable on screen from a hung process.
  **Product-owner ruling** (new PRD-0001 **AC-21b**, [2026-08-05
  amendment](../prd/PRD-0001-exact-image-deduplication.md#amendment-2026-08-05--walk-liveness-cue-product-owner-ruling-task-0332)):
  show elapsed time since the scan started, appended to the existing
  indeterminate line — `scanning... 3 files found (14s elapsed)` — from the
  first render, not only past some threshold. Two named alternatives from the
  backlog row, both rejected with reasons recorded in the PRD amendment: an
  entries-visited tick (no denominator exists for "entries visited", so a
  growing raw number there would read as quantified progress it isn't — the
  exact concern the backlog row itself raised) and a spinner (strictly weaker
  than elapsed time for equal implementation cost — both are liveness-only,
  but a clock also says _how long_, which a spinner discards, and this tool's
  other progress text is plain counts/percentages throughout, not
  iconography).
  **Implementer (TDD):** the walk liveness cue can only tick if something
  keeps triggering repaints while the walk finds nothing — today, only
  `advance()` does that, and on a low-match tree those calls are exactly the
  ones that go quiet. `enumerate_root`'s walk loop (`enumerate.rs`) now calls
  a zero-count `advance(0)` on **every** directory entry it visits, accepted
  or not, before any of the existing `continue`s — a "tick" that changes no
  counter but still runs through the exact same throttled repaint check a
  real advance does (≤1 repaint per 100 ms), so a fast walk over a dense tree
  pays nothing extra it wasn't already paying. `progress.rs`'s `line()` gained
  an `elapsed_secs` parameter, computed once per repaint in `paint()` from the
  existing `now_millis()` clock, rendered only in the TTY indeterminate
  branch — non-TTY is unaffected, for the identical reason AC-22 already
  keeps the found-so-far count TTY-only (a non-redrawn line could only ever
  record a stale value). 6 new unit tests initially (progress.rs: elapsed
  time shown alongside the count and from the very first render; `advance(0)`
  repaints without changing or regressing the displayed count; non-TTY never
  shows elapsed time — plus enumerate.rs: a tick fires once per directory
  entry visited, separately from the one real advance per accepted candidate,
  proven with a sink that records every call's `n` rather than just summing
  it). **Verified against the real release binary, not just the unit
  suite:** built a 400,000-file synthetic tree (all `.dat`, nothing an image
  extension) and ran both `main`'s binary and this branch's under a real
  pseudo-TTY (`script`). `main` writes the indeterminate scanning line
  exactly once for the whole walk (frozen, since nothing is ever accepted as
  a candidate — the exact defect this task exists to fix); this branch
  writes it 4 times over the identical walk. The elapsed value itself stayed
  at `0s` in that run only because the walk finished in under a second on
  this machine's SSD — a real-wall-clock fact the deterministic unit tests
  don't depend on.
  **Independent architect review (one dispatch, converged on the branch):
  no blockers, one real test-coverage gap found by mutation, plus should-fix
  doc/wording gaps — all resolved. Continues the run of sessions where the
  reserved concurrency-adjacent review dispatch finds real substance, per the
  TASK-0317/TASK-0328 precedent this task itself deliberately followed.**
  **(1)** The enumerate.rs tick-placement test didn't actually pin the
  placement it claimed to: the reviewer mutated the tick to fire _after_ the
  hidden-entry skip instead of before it, and the entire 284-test suite
  still passed — silently re-freezing the indicator on exactly the
  motivating case (a source checkout, where `.git/` is a long run of hidden
  entries filtered out post-yield). Fixed by adding a hidden subtree to the
  fixture (verified: the same mutation now fails). **(2)** The `Err`-entry
  half of "the tick fires for every walk entry, not just accepted ones" was
  asserted in a code comment and the PRD but pinned by no test; added one
  using this file's existing `#[cfg(unix)]`/`chmod`-based precedent
  (a permission-denied subdirectory), empirically confirming walkdir yields
  it as _two_ entries (the directory itself, then a separate read error) —
  4 ticks, not the 3 first guessed. **(3)** The elapsed-seconds divisor
  (`now_millis() / 1000`) was completely untested: every prior test observes
  a process running for single-digit milliseconds, so it could only ever
  assert `0s` — a regression to raw milliseconds would have shipped green.
  Closed two ways: a new `Progress::new_with_interval_ms_and_elapsed_ms`
  (`#[cfg(test)]`, unlike its always-compiled `new_with_interval_ms` sibling,
  since no production caller has a legitimate reason to fake its own start
  time) injects a deterministic past start time, pinning `61_500ms → "61s
  elapsed"`; plus a narrower test calling `line()` directly. Also fixed:
  `Progress::line`'s and `ProgressSink::advance`'s rustdoc, and
  `enumerate.rs`'s module/function docs, all still described the pre-tick
  world (`advance` "called once per candidate... not once per directory
  entry visited" — now false for the `n == 0` tick, still true for the
  displayed count); AC-21b reworded, since its Given/When phrasing read as
  "the suffix appears in response to a slow tree" when it's actually
  unconditional on every indeterminate repaint; FR-19 gained the short clause
  the amendment's "reflected in FR-19 and AC-21b" claim required but hadn't
  actually done; the amendment's residual paragraph now also names
  `dedupe_roots`/`dedupe_symlink_aliases_across_roots` as the same class of
  untickable stretch, not just a blocked directory read; and the guide's
  "ticking up as proof the tool has not hung" softened to "evidence" (a
  blocked read is a named, accepted gap, not something this cue rules out).
  `docs/guide/output-and-exit-codes.md` updated throughout (the walk's
  worked example now shows the suffix as part of the normal line, not a
  slow-tree-only addition, and the non-TTY note). This PR.

- _Session 23 (TASK-0335, lint shell scripts and GitHub Actions workflows),
  2026-08-05 — continuing session 22's window (5h≈30% at start)._ Direct
  follow-up from TASK-0334's own review: neither `shellcheck` nor `actionlint`
  ran anywhere, locally or in CI, so the real command-injection defect that
  review found in `scripts/release-preflight.sh` was caught only because a
  human ran the tools by hand. Closes that gap. New `make shellcheck`/`make
  actionlint` targets folded into `validate`; `scripts/validate.sh` gets
  matching steps and install hints in the same style as its existing
  typos/lychee checks; `ci.yml`'s `checks` job installs both tools the same
  way it already installs typos — downloaded into `RUNNER_TEMP`,
  checksum-verified against a hand-pinned sha256 (shellcheck's v0.11.0 release
  publishes no separate checksums file to verify against, matching the
  `typos` precedent; actionlint's own `checksums.txt` is fetched but not
  trusted as the verification source, since an attacker able to tamper with
  one release asset could tamper with the checksums file beside it — both
  hand-pinned values were independently re-derived from the real upstream
  assets, not copied from anywhere). `actionlint` also picks up the installed
  `shellcheck` on `PATH`, so it re-lints every workflow's embedded shell for
  free. Both tools lint `git ls-files '*.sh'` rather than a `scripts/*.sh`
  glob, so a script added below the top level of `scripts/` (which already
  has a subdirectory, `scripts/release-preflight/`) can never go silently
  unlinted. One pre-existing finding worked around rather than fixed: `typos
  $(git ls-files '*.md')`'s word-splitting is intentional (typos wants one
  filename per argument; every tracked path here is plain ASCII with no
  spaces or glob metacharacters), silenced with an explanatory comment at its
  two call sites (`ci.yml`, `scripts/validate.sh`) rather than restructured.
  Docs synced: `CONTRIBUTING.md`, `docs/process/sdlc.md`.
  **One mechanical fix landed first, in its own commit, on the repository's
  only `contents: write` workflow:** `release.yml` carried a pre-existing
  SC2129 finding (two sequences of individual `>> "$GITHUB_OUTPUT"` redirects,
  shellcheck's own suggestion is to group them) that would otherwise have
  forced an exclusion on the new gate's very first run. Classified `style:`,
  not `fix:` — no behavior change, verified directly (output content/order,
  exit-code propagation under `set -euo pipefail`, and the surrounding
  branches are all identical before and after) — specifically so it carries
  no release semantics; the architect review's own should-fix flagged that the
  first draft of this commit had used `fix:`, which would have put a
  lint-only chore into the next release's user-facing changelog as a patch
  bump.
  **Independent architect review (one dispatch, converged on the branch),
  required regardless of size because the unit touches `release.yml` per
  ADR-0011's carve-out — even though the actual `release.yml` change is a
  two-line style fix.** Confirmed correct rather than assumed: re-derived both
  tools' pinned checksums independently against the live upstream releases
  (matched); reproduced the SC2129 diff's behavior-equivalence by hand
  (matched); ran both linters against a clean snapshot of `main` and confirmed
  they surface exactly the two findings this unit resolves and nothing else,
  so no baseline/exclusion file was needed. Two real, if small, gaps found and
  fixed: **(1)** the first draft's `shellcheck scripts/*.sh` glob would have
  silently missed any script added below `scripts/`'s top level — a live gap,
  since `scripts/release-preflight/` already exists — fixed to lint
  `git ls-files '*.sh'` instead. **(2)** `actionlint` silently drops its
  embedded-shell rule (exits 0, no error) if it can't find `shellcheck` on
  `PATH`, so a future reordering of the two install steps could quietly halve
  workflow coverage with no signal; the workflow step now asserts the pairing
  explicitly before running. Also caught and fixed: `fix:` → `style:` on the
  `release.yml` commit (above), a stale `docs/process/sdlc.md` line
  enumerating the old `make validate` contents, and an ADR-0005 amendment
  recording the addition — the ADR's own security-gate bullet named only the
  Rust stack's static analysis and had gone stale the moment this repository's
  other executable surface (scripts, workflows) went unlinted. Nits noted but
  not acted on: the local install hint for `actionlint` had no non-Homebrew
  path (added `go install`); no automated bump path exists yet for the two
  new hand-pinned tool versions, same as `typos` already has (accepted
  residual, not a new one this unit introduces). This continues the run of
  sessions where the reserved review dispatch finds real substance on a unit
  that looked mechanical from the outside. This PR.

- _Session 22 (TASK-0334, local release-pipeline pre-flight), 2026-08-05 —
  fresh window (5h=0%, 7d=17% at start)._ Owner picked this as the next unit
  from three unblocked `todo` items (TASK-0219, TASK-0332, TASK-0334) after
  the owner's explicit priority list (321→320) plus the CI-minutes Kaizen
  chain (330/331/333) both ran out. Closes the piece TASK-0333(b) deliberately
  deferred as "a separate, larger idea": the `assets` job (the repository's
  only `contents: write` workflow) had no local rehearsal at all, so a
  stamping/packaging/`--version` mismatch was discovered only in that job,
  after the tag and the GitHub Release already existed.
  **Design, resolving the backlog's two open questions before any code:**
  (1) shared script vs. parallel rehearsal — resolved as extract-only-the-
  non-credentialed-half. New `scripts/build-release-artifact.sh` (stamp
  `Cargo.toml`, `cargo build --release`, assert `--version`, package
  archive+`.sha256`, the architecture-match guard) is called by both
  `release.yml`'s `assets` job and the new `scripts/release-preflight.sh`;
  the checks-green verification and `gh release upload` stay inline in the
  workflow and the shared script never receives `GITHUB_TOKEN`, so nothing on
  the published-binary path gains privilege it didn't have. (2) glibc-floor
  checking — out of scope, stays with TASK-0326.
  **A real, measured finding shaped the implementation, not an assumption:**
  the first design forced `--platform linux/amd64` under podman so the local
  rehearsal would reproduce the exact `x86_64-unknown-linux-gnu` leg on this
  Apple-Silicon dev machine. `rustc -vV` — not a build, just the compiler
  printing its own version — segfaulted under that qemu emulation,
  reproducibly, and reducing `cargo build -j 1` didn't help (the crash was in
  `rustc` itself). `scripts/release-preflight.sh` does not paper over this: it
  drops the forced `--platform`, lets the container build for podman's own
  native platform instead, and reports on stderr exactly which target it
  built rather than mislabeling the rehearsal — on an x86_64 host (or CI) this
  limitation doesn't apply and the leg is `x86_64-unknown-linux-gnu`
  natively. The tool rehearses two of the four legs (this glibc Linux target
  via rootless podman, `aarch64-apple-darwin` natively on Apple Silicon
  macOS); Windows and Intel macOS stay CI-only, per the backlog's own scope.
  Everything runs inside a disposable `git worktree` — the real working tree,
  including `Cargo.toml`, is never touched (`build-release-artifact.sh`
  stamps the version in place, which must never happen to a developer's own
  checkout). `make release-preflight` added. Verified end-to-end for real,
  repeatedly: default run, `--version`/`--tag` overrides, `--keep`, an
  invalid `--ref` (fails before any worktree exists), and an invalid
  `--version` (worktree still cleaned up on failure) — `git worktree list`
  confirmed clean after every run, and the real working tree's `Cargo.toml`
  confirmed untouched throughout.
  **Independent architect review (one dispatch, converged on the branch): one
  real security-relevant defect, four other should-fixes, all fixed —
  continuing the run of sessions where the reserved review dispatch finds
  real substance, not nits.** `release-preflight.sh` interpolated
  `--version`/`--tag` **unquoted** into the podman container's `bash -c`
  string; the review demonstrated live that a crafted `--version` could break
  out of the string and execute an arbitrary command inside the container
  (`1.0.0$(touch /tmp/PWNED)"; echo INJECTED` created the file) — local-only,
  never reachable from `release.yml` or any credentialed path, but exactly
  the unquoted-interpolation shape this repo's own workflows otherwise avoid.
  Fixed by passing both values as environment variables (`-e
  PREFLIGHT_VERSION=... -e PREFLIGHT_TAG=...`) instead of string-building the
  inner script; re-verified live post-fix that the same crafted input no
  longer executes anything (`sed` now just fails cleanly on the garbage
  version string, worktree still cleaned up). Also fixed: the packaging stage
  in `build-release-artifact.sh` used a system tmp dir before moving into
  `out-dir`, a cross-volume-move risk on the Windows leg (staged inside
  `out-dir` itself instead, restoring the pre-extraction `$RUNNER_TEMP`
  same-volume behavior); the local macOS leg silently inherited the
  developer's ambient `CARGO_TARGET_DIR`/etc. (now forced to `target`, a
  no-op in CI); and `release.yml` reconstructed the artifact's filename
  independently of what the script actually produced — the one real seam in
  the ADR's "one source of truth" claim — closed by having the script write a
  `.artifact-filename` manifest the workflow reads back instead. Plus
  worktree-safety/UX nits: `$$`-suffixed worktree path (two concurrent local
  runs could otherwise `rm -rf`/`worktree remove --force` each other's tree),
  `ref^{commit}` resolution (exact for annotated tags, today's `v*` tags are
  lightweight so this was latent), the output directory cleared at the start
  of each run (a stale prior artifact would otherwise be listed alongside the
  new one), a `--tag` flag (rehearsing a real `vX.Y.Z`-shaped filename), and
  missing-value validation on `--ref`/`--version`/`--tag`. One should-fix
  deliberately left for a follow-up rather than fixed here: no CI job lints
  the new shell scripts or the workflows themselves (every finding in this
  review, including the injection, was caught by a human/architect running
  `shellcheck`/`actionlint` by hand) — tracked as new **TASK-0335**.
  Recorded in the
  [2026-08-05 amendment to ADR-0015](../adr/0015-release-publishing-tag-and-github-release.md#amendment-2026-08-05--local-release-pipeline-pre-flight-task-0334);
  operational docs in
  [`docs/process/release.md`](../process/release.md#local-pre-flight-rehearsal).
  This PR.

- _Session 21 (TASK-0333, the two CI-minutes policy calls TASK-0331 deferred),
  2026-08-05 — continuing session 20's window._ Both halves needed an owner
  ruling before any code, obtained directly this session rather than assumed:
  **(c) branch protection `strict: true` → `false`** — applied immediately by
  the orchestrator via `gh api` on explicit real-time authorization (the
  owner answered "yes" to the direct question), not written up as a
  deferred owner-action command like TASK-0330/0331's precedent, since this
  was live confirmation in the same turn rather than a design the owner
  would review later. Verified live before and after; the three required
  contexts (`checks`, `ci-gate`, `security`) stayed app-pinned, unaffected.
  **(b) release cadence — manual-only trigger.** Owner ruling: stop
  auto-publishing on every green `push: main` (12 releases in 4 days, 485 of
  ~1790 billed CI-minutes/4 days, 27%, in the `assets` matrix alone) and
  switch to `workflow_dispatch`-only, batching whatever accumulated since
  the last tag. A separate, larger idea raised in the same conversation —
  local podman/macOS release builds usable for testing and installation, not
  just CI-check — was explicitly scoped out as its own follow-up: **new
  TASK-0334**.
  The trigger swap looked mechanical but wasn't: the removed `workflow_run`
  trigger had one load-bearing property (ADR-0005/ADR-0015) — a release can
  never publish from a commit whose checks are red or still running — that a
  bare `workflow_dispatch` would have silently dropped. Implemented as an
  explicit step, before checkout, querying the checks API for the dispatched
  commit and failing closed unless `ci-gate` and `security` (this repo's
  required contexts; `checks` isn't queried separately since `ci-gate`
  already aggregates it) are both a completed success from GitHub Actions
  (`app_id`-pinned, matching branch protection's own pinning). Also
  incidentally closes ADR-0015's recorded gap of not gating on the Security
  workflow. New `checks: read` permission on the job (rejected the broader
  `actions: read` alternative). `release`'s job-level `if:` — previously
  sorting `workflow_run` payloads — was removed outright rather than left
  inert, since none of those cases can occur once `workflow_run` is gone.
  **Because this is the repository's only `contents: write` job, this unit
  took two architect dispatches** (design+implement, then an independent
  review), per ADR-0011's explicit carve-out that write-permission jobs
  never qualify for the low-risk review-skip exception regardless of size —
  the same rigor TASK-0331 applied to the `security` job fold.
  **The independent review found and fixed one real, security-relevant
  defect: `.check_runs[0]` depended on an array order the REST API does not
  document.** `filter=latest` deduplicates only _within_ a check suite, and
  GitHub Actions creates one suite per workflow run — so two different runs
  reporting the same check name against one commit both come back, and
  reading index `[0]` made the verdict depend on which one the API happened
  to return first. Proven live, independently, by both the reviewer and the
  orchestrator: every recent commit on `main` carries two or three `release`
  check runs, and (the case that mattered) a red `release` run sits next to
  a green one on the exact same commit with no documented ordering between
  them. The fix requires **every** returned check run to be green, not just
  the first — order-independent, still clears via a re-run (which lands in
  its own suite's `latest`), and closes a related latent hole (a red result
  could no longer be laundered by a green run beside it, removing the need
  for a `(manual)`-suffix-style workaround should `security.yml` ever gain
  its own dispatch trigger). This is the exact failure class the
  two-dispatch precedent for `contents: write` jobs (ADR-0011) exists to
  catch: invisible
  to a live spot-check the orchestrator itself ran mid-review (`main`'s
  current tip happened to carry only one `security` check run at the time —
  the ordering hazard only shows up once a second run exists, e.g. after the
  first Monday cron), and the review reasoned about the API's actual
  documented guarantees rather than trusting today's data shape. Two stale
  doc claims caught in the same pass and corrected (`docs/adr/0015-...md`,
  `docs/process/release.md`).
  **Caveat carried into the PR, not resolved by review:** the check-runs
  query has never executed under a job-scoped `GITHUB_TOKEN` with
  `contents: write` + `checks: read` on this private repo — verification so
  far is the live API queried directly plus careful reading, which is the
  strongest evidence available pre-merge, but the mechanism is fail-closed
  if the permission scoping turns out to be wrong, so the first real
  dispatch is the true test. This PR.

- _Session 20 (TASK-0331, CI-minutes: macOS cadence + security-job fold),
  2026-08-05 — continuing session 19's window (5h=39%, 7d=11% at start)._
  Picks up the two structural levers TASK-0330 left on the table that are
  engineering calls rather than policy calls: **(a)** the `rust` job's macOS
  leg (390 of ~1790 billed min/4 days, 22% — the single largest line item,
  driven entirely by GitHub's 10x macOS multiplier on a leg that already
  finishes inside the one-minute floor) ran on every PR push; **(d)**
  `secret-scan` and `cargo-deny` were separate jobs, each paying its own
  floor for 14s/65s of real work. Two other TASK-0331 items — (b) release
  cadence and (c) branch protection `strict: true` — are policy calls, not
  engineering ones, and were **not** attempted here: split out as new
  **TASK-0333** with their measured numbers carried forward. (e), `ci-gate`'s
  own floor cost, is closed as structurally unavoidable.
  **(a) required an owner call first, obtained before any code was written:**
  asked directly how much per-PR macOS signal was worth keeping; ruling
  (2026-08-05): **macOS moves to `push: main` only**, ubuntu/windows
  unchanged on every PR (est. −250 to −300 billed min/4 days). Implemented by
  varying the `rust` job's **matrix width** by event
  (`fromJSON(github.event_name == 'pull_request' && … || …)`) rather than
  splitting out a separate macOS job with a job-level `if:` — deliberately,
  so `rust` stays exactly one job with one aggregated result and `ci-gate`'s
  `needs.rust.result` check gains no new skip case to reason about (a
  separate job would report `skipped` on every PR, the same `needs:`/`if:`
  sharp edge TASK-0330's `checks` job already had to name). A
  `workflow_dispatch` trigger was added as the pre-merge escape hatch for a
  macOS-only failure, checked safe against `release.yml`'s `push`-only
  trigger condition before being added.
  **(d):** `secret-scan` + `cargo-deny` folded into one `security` job.
  ADR-0005 constraints held explicitly: secret scanning stays unconditional,
  never path-filtered (a secret can land in any file); cargo-deny is
  path-filtered at the **step** level inside an always-running job, never a
  job-level `if:`, so this required check always reports a real
  success/failure rather than "skipped" on most PRs. Est. ~167 → ~85 billed
  min/4 days.
  **Independent architect review (one dispatch, converged on the branch):
  three real defects found, all fixed, none visible from the YAML alone.**
  (1) Path-filtering cargo-deny made the audit _event_-driven while RUSTSEC
  advisories are _time_-driven: a new advisory against an untouched
  `Cargo.lock` would have gone unreported until something happened to touch
  a dependency file — silently weakening ADR-0005's "caught early and
  continuously" for the CI-minutes saving. Fixed with a weekly `schedule:`
  trigger and an explicit `github.event_name == 'schedule'` clause in the
  cargo-deny condition (paths-filter has no diff base on a scheduled event
  and would otherwise report `deps: false`, turning the weekly audit into a
  no-op); concurrency keyed on event name too, so a `push: main` landing in
  the schedule's ~1-minute window cannot cancel the one audit not tied to a
  change. (2) A dispatched `workflow_dispatch` run's check runs land on the
  branch head SHA — the same SHA an open PR from that branch is gated on —
  so the unsuffixed `checks`/`ci-gate` names would have collided with the
  required contexts (and the PR-title lint, which needs a PR context, can't
  run on a dispatch at all). Fixed with a `(manual)` name suffix on that
  event, making dispatched runs inert to branch protection. (3) The
  branch-protection rename command drafted for the owner used the REST API's
  deprecated `contexts[]` form, which binds each name to "the app that
  recently set it" — for the brand-new `security` context, no app, so it
  would have landed **unpinned** and reportable by any integration holding
  `checks: write`, the wrong property for the check that is the ADR-0005
  gate itself. Fixed with the app-pinned `checks` form, explicit
  `app_id: 15368` (GitHub Actions) on all three replacement contexts.
  **Requires the same manual owner step as TASK-0330/TASK-0206** before/at
  merge — exact command in "Blockers / waiting on owner" below. This PR.

- _Session 19 (TASK-0328, animate scanning's determinate sniff sub-phase),
  2026-08-05 — fresh window (5h=0%, 7d=7% at start)._ Closes the residual
  TASK-0317 (session 11b) left open: TASK-0317 animated only the tree-walk
  sub-phase of `Scanning` with a live indeterminate `scanning... N files
  found` counter, but its own architect review had measured that walking is
  only ≈10% of the scanning window (0.03s) while magic-byte confirmation
  (`sniff::filter_candidates`) is ≈90% (0.88s, 30k files/60 dirs/~117MB,
  release build) — the larger part of the wait stayed frozen. **Product-owner
  ruling first** (this was a product decision, not an implementer one, since
  AC-21 specified Scanning as a single indeterminate state): animate it
  _determinately_, as a **second state of the same phase**, not a new phase
  name. Once the walk finishes with `N` candidates, the indicator leaves the
  indeterminate count for `scanning: x of N (p%)`, advancing once per file
  examined (confirmed or rejected alike) and ending at `N of N (100%)` before
  hashing begins; `N == 0` skips the state entirely. Two alternatives were
  considered and rejected: a new sub-phase label (every candidate word was a
  near-synonym of the existing `Verifying` phase, which does something
  different, so the vocabulary would mislead) and a shared indeterminate
  counter run through both stages (cheapest, but silently changes what the
  count _means_ mid-run with nothing on screen to distinguish "discovered"
  from "examined"). `N` is deliberately not the run-wide candidate total `y`
  (`N ≥ y`, since rejected files are examined but never become candidates)
  and carries no `(N candidates)` suffix, unlike Verifying — `y` is genuinely
  unknown at that point. New PRD-0001 **AC-21a** and a 2026-08-05 amendment;
  the residual this ruling explicitly declines to fix — a walk that grinds
  through a tree with very few extension-matching entries, leaving
  `scanning... 3 files found` static for the ≈10% walk portion — is named and
  tracked as new **TASK-0332**, not folded in. **Implementer (TDD):** new
  `sniff::filter_candidates_with_progress` (the plain `filter_candidates`
  survives as a `#[cfg(test)]`-only wrapper, mirroring the
  `enumerate`/`enumerate_with_progress` and
  `find_duplicates`/`find_duplicates_with_progress` precedent) calls
  `sink.advance(1)` once per candidate from inside the existing rayon
  closure — the same lever already proven safe for concurrent callers by
  `engine.rs`'s hashing phase; `main.rs` wired with `retotal(N)` +
  `set_phase(Phase::Scanning)` (same phase value — no `progress.rs` trait or
  renderer changes were needed at all: `set_phase` already force-repaints
  regardless of whether the phase value actually changed, which turned out to
  be exactly the AC-21a entry mechanism).
  **Architect review (one dispatch, converged on the branch): one real defect
  found and fixed, everything else held up.** The _closing_ render of the new
  state had nothing forcing it — the state was left to whichever `advance`
  call happened to land on a throttle-due tick, which on a real 20,000-file
  run rendered `scanning: 18216 of 20000 (91%)` immediately followed by
  `done: 0 of 0 (100%)`, silently skipping the AC-21a-mandated `N of N
  (100%)` closing frame the PRD amendment, the backlog row, and the user
  guide's worked example all promised. The unit test that should have caught
  it couldn't: it used `immediate()` (throttle disabled), where every
  `advance` repaints, so the gap was invisible in isolation and only showed
  up under the real default throttle. Fixed with a second forced
  `set_phase(Phase::Scanning)` call after the sniff stage returns, pinned by
  a new CLI-level test (fails on the pre-fix commit, passes after, verified
  both directions) plus two more closing an actual test-coverage gap: the
  only prior CLI-level test happened to use a fixture where every examined
  file was also confirmed (`N == y`), so nothing pinned the `N != y` clause
  that is the entire point of AC-21a — new tests cover `N > y` (some
  rejected) and `N > 0, y == 0` (all rejected, `done: 0 of 0 (100%)`
  reachable from a non-empty scan). This continues the run noted in
  [[imgdedup-architect-review-value]]: the reserved dispatch found a real,
  user-visible defect on a unit that looked simple and had already passed
  `make validate` clean going in. No `docs/planning/reviews/` log — that
  directory is PRD-slice-cadence only (confirmed against TASK-0317/TASK-0329
  precedent, neither of which has one). This PR.

- _Session 18 (TASK-0329, cross-root `--follow-symlinks` alias collisions)
  closes the residual TASK-0320's own architect review left open: the same
  anchor-rule-masking-a-real-duplicate defect, reachable across two scan
  roots instead of within one. Reproduced fresh with the real release binary
  before any dispatch, matching this project's practice: `A/g.png` (a genuine
  duplicate) + `B/deep/photo.png`, aliased a second time as `A/l/photo.png`
  via `A/l -> B/deep`, reported `hard_link_sets: [A/l/photo.png,
  B/deep/photo.png]` and `reclaimable_bytes: 0` when 498 real bytes were
  reclaimable. **Product-owner ruling (2026-08-04): fix now, not leave as a
  documented residual** — dispatched with the reproduction and the two
  backlog options. The deciding factor: the root-scoped boundary isn't
  explainable in the user's model (a user thinks in terms of _a scan_, not
  per-root walks), and the guide's own workaround for the residual ("scan the
  paths separately") was self-defeating, since it destroys the very
  cross-root duplicate detection a user passes multiple roots to get. The
  ruling also set an explicit, non-negotiable performance constraint before
  any code was written: a root whose own walk saw no symlink must not pay
  materially more than it did when the pass was still root-scoped, since a
  naive widening would otherwise force a full canonicalize sweep of an
  unrelated large sibling root the moment any other root has a symlink.
  **Fix:** `enumerate::dedupe_symlink_aliases` widened into
  `enumerate::dedupe_symlink_aliases_across_roots` — one pass over every
  deduped root's candidates, grouped by canonical path, "direct" (no symlink
  hop) evaluated per candidate against _its own_ root rather than a single
  global one. The performance constraint is met without a special case: a
  symlink-free root's candidate canonical path is derived arithmetically
  (`root_canonical.join(suffix)`, no syscall) since no component below such a
  root can differ from that form, while a root that itself saw a symlink
  still pays the real per-candidate `Path::canonicalize` syscall, exactly as
  before this widening. Measured directly (release binary, warm cache,
  30,000-file symlink-free root sharing a run with a one-file
  symlink-containing sibling): ~5.7 s, against a pre-widening `main` baseline
  of ~5.6 s and a no-sibling run at ~5.63 s over the identical fixture — the
  three are within measurement noise of each other. Generalized to **N
  roots, not 2** (pinned by a 3-root unit test — a rule that worked for two
  roots but not three would not have been defensible) and root-argument order
  is asserted, not assumed, to leave the result — including which physical
  path survives — unchanged (AC-14b/NFR-3/AC-16). The single-root
  `dedupe_symlink_aliases` survives as a thin test-only wrapper so TASK-0320's
  own unit tests remain exact regression pins. Also verified, per the
  product-owner's explicit instruction not to inherit the claim: the prior
  `enumerate_root` doc comment's justification for not flagging a symlinked
  root itself as `saw_symlink` (every candidate in that root's walk shares
  the one hop) still holds under the cross-root widening, because
  `dedupe_roots` already guarantees any two deliberately-kept roots have
  non-overlapping walks. New PRD-0001 AC-14b; new ADR-0008 amendment closing
  TASK-0320's residual (1); `docs/guide/scanning-and-scope.md`'s cross-root
  caveat and "scan separately" workaround removed — the alias-collapse
  behavior now holds regardless of how many paths are passed. Classified
  `fix:` (defect), not a feature.
  **Architect review (one dispatch, converged on the branch): no blockers,
  two should-fixes, both resolved.** The correctness questions the review was
  asked to independently verify all held up — including with fixture shapes
  the orchestrator hadn't tried, notably a case-insensitive-filesystem root
  name and the one configuration where two deliberately-kept sibling roots
  (an explicit hidden root plus an ordinary symlink in its ancestor pointing
  at it) can still meet at one physical file, which the review both confirmed
  correct with the real binary and asked to be pinned by a test. **(1)** the
  first cut's `root_of` lookup was an O(roots) linear scan per candidate,
  reintroducing — one stage later — the exact quadratic `dedupe_roots` had
  already been measured and fixed away from at 20,000 roots; replaced with an
  O(n) precomputed index. **(2)** the claim that `dedupe_roots`'s
  deliberately-kept sibling roots never overlap had no test pinning the one
  configuration where their walks _can_ still meet (hidden root + ancestor
  symlink into it) — added as a dedicated unit test. Also tightened: the
  ADR's performance evidence, replaced with a pass-runs-vs-pass-skipped
  measurement that isolates the constraint (delta −0.1%, within noise) rather
  than an end-to-end comparison too coarse to resolve it, plus a named but
  unmeasured memory caveat and the available mitigation lever; the
  `enumerate_root` non-overlap justification, given the third case (a root
  that fails to canonicalize) the first cut's argument didn't cover; and the
  PRD's AC-14a wording, which had drifted to claim its own residual was
  "unchanged" after this PR changed it. This PR._

- _Session 17 (TASK-0330, CI-minutes optimization, Kaizen chore) — spare-cycle
  tooling maintenance flagged by the owner: GitHub Actions usage was tracking
  toward exhausting the Pro plan's 3000 included minutes/month with only 4 days
  into the billing cycle. Measured from the Actions API rather than estimated —
  932 jobs across 210 workflow runs since 2026-08-01, per-job
  `started_at`/`completed_at`, billed rounded up to the whole minute per job,
  GitHub's own OS multipliers applied (Linux 1x, Windows 2x, macOS 10x):
  **~1790 billed minutes in four days**, 60% of the monthly cap in ~13% of the
  month. No caches existed anywhere (cache-usage API: 0 active). The finding
  that reshaped the fix, and which the first cut of this unit had missed:
  **~1000 of those 1790 minutes are the per-job one-minute billing floor**, so
  56% of the bill is bought by how many jobs run, not by how long any job
  takes — which caps what any speed-up can reach and makes removing whole jobs
  worth more than making jobs faster. Shipped accordingly:_
  - _**Six ubuntu jobs consolidated into one `checks` job** (`markdownlint`,
    `spellcheck`, `structure`, `linkcheck`, `changes`, `pr-title`): 378 billed
    min (21% of spend) for 4-15 seconds of real work each. Removing five floor
    charges is the unit's largest certain win, ~280-310 min per four days.
    `rust`/`ci-gate` now depend on `checks`. Two consequences handled
    explicitly: every step carries `if: ${{ !cancelled() }}`, because GitHub
    otherwise abandons a job at its first failing step and one markdown error
    would hide the other five checks — turning a single red run into up to
    five sequential ones and costing more than the consolidation saves; and
    `dorny/paths-filter` moved first so `rust`'s gating output exists even
    when a later check fails. A lint failure now does skip the `rust` matrix
    (a job with `needs:` and a plain `if:` still requires success), which is
    kept deliberately — the matrix is the biggest line item and nothing is
    hidden by deferring it one run._
  - _**`actions/cache` on `ci.yml`'s `rust` job, Linux and Windows only.**
    Excluding macOS is the counter-intuitive part and is what the measurement
    forced: `rust (macos-latest)` is the single most expensive job in the
    repository (390 billed min) precisely because of the 10x multiplier, but it
    finishes in 53s — already well inside the one-minute floor, so at most 70 of
    its 390 minutes are recoverable, while restoring a several-hundred-megabyte
    `target` on a macOS runner would push it past 60s and double the bill.
    Caching there would have made the largest line item worse. Cache contents
    are narrowed to the checksum-verified parts of `CARGO_HOME`
    (`registry/index`, `registry/cache`, `git/db`) plus `target` — never
    `registry/src`, which cargo does not re-verify once unpacked — with
    `CARGO_INCREMENTAL=0` to keep ~1 GB of runner-local junk out of the
    repository's 10 GB cache budget._
  - _**`release.yml`'s `assets` job deliberately left uncached.** Two of its
    four legs already finish inside the floor, and on the other two the fixed
    costs (checkout, `rustup show` installing the pinned toolchain, packaging,
    `gh release upload`) plus a cache restore leave the job in the same billing
    minute — expected saving ~0. Against that, a restored `target` would sit on
    the path that produces the published binaries, which this repository
    SHA-pins and cargo-deny-gates specifically to keep auditable. Not a trade
    worth making for no measured gain._
  - _Realistic effect: **~1790 → ~1420-1450 billed minutes per four days, a
    ~20% cut** — materially less than the "30-40% of cap" the unit's first cut
    projected, because the floor is not reachable by caching. The levers that
    are actually larger than this whole unit (chiefly `rust (macos-latest)`'s
    390 min and `assets`' 485 min, both a function of run count and release
    cadence rather than job speed) are written up with numbers as **TASK-0331**
    rather than half-done here. `cargo-deny` untouched. This PR._

- _Session 16 (TASK-0320, `--follow-symlinks` candidacy ruling) closes a
  residual gap TASK-0319 explicitly named as out of its own scope:
  overlapping/repeated scan roots were fixed at the root-list level, but
  `--follow-symlinks` can create the same misleading-report symptom via
  filesystem aliasing **within** a single root's own walk — e.g. a symlinked
  directory pointing at content also reachable directly — which no change to
  `dedupe_roots` could fix. Reproduced with the real binary first, matching
  TASK-0319's own practice: `A/real_dir/photo.png` plus `A/link_dir ->
  real_dir` made `imgdedup --follow-symlinks` report the single real file as
  an `already_deduplicated` hard-link set (mislabeled — a true hard link is
  two independent directory entries naming one inode; this is one entry,
  reached twice). **Adding a genuine second copy alongside it surfaced a
  worse defect, found before any product ruling was sought:** the anchor
  rule (correct for true hard links — removing one name reclaims zero bytes)
  then protected both aliased representatives from removal, reporting `0`
  reclaimable bytes when 486 real bytes were genuinely reclaimable. This,
  not just the labeling, is why the fix could not be scoped as a docs-only
  clarification of the existing dry-run/apply parity exception.
  **Product-owner ruling (dispatched with the reproduction findings and the
  architect's on-record preference from TASK-0319's review):** rejected the
  "drop symlink-derived candidates from candidacy entirely" reading of that
  preference — it would contradict AC-14's own existing test
  (`follow_symlinks_flag_includes_symlink_targets`, where the target lives
  outside every scan root and must still be included) and would make
  `--follow-symlinks ./library` report nothing at all for an ordinary link
  farm. Ruled instead: canonicalize and collapse only proven alias
  collisions — two candidates resolving to the same real path — never a
  lone symlink whose target isn't independently in scope. New
  `enumerate::dedupe_symlink_aliases`, run once per deduped root (never
  across roots — the backlog's own framing scoped this to one root's walk),
  only under `--follow-symlinks` (structurally impossible otherwise, so the
  default path pays nothing): candidates canonicalizing to the same real path
  collapse to one survivor, preferring the non-symlink-routed path when
  exactly one exists, else falling back to the existing FR-7 `shortest` order
  — reused via a newly-extracted `keeper::cmp_shortest_by` rather than
  inventing a second ordering (the ruling's explicit instruction). A
  candidate that cannot be canonicalized (broken link, unreadable ancestor)
  is kept unchanged — fail-open, mirroring `dedupe_roots`. True hard links
  are untouched (two independent directory entries each canonicalize to
  themselves, so they're never grouped here), continuing through the
  existing, unmodified anchor-protection path.
  **A subtlety the tie-break logic had to get right, caught while writing
  its own test on this development machine:** "was this candidate reached
  through a symlink" is tested **relative to each root's own canonical
  form**, not the process's absolute view of the filesystem — because macOS's
  `/tmp` is itself a symlink to `/private/tmp`, comparing against a global
  canonical form would make every candidate under a `/tmp`-based tempdir
  look "routed through a symlink" regardless of whether the scan-relevant
  part of its path involved one at all, silently degrading the "prefer the
  real path" preference to the shortest-order tie-break everywhere on such
  systems. Scoping the comparison to each root's own canonicalized form
  neutralizes that irrelevant ancestor symlink entirely — verified directly:
  a test asserting the shorter of two equally-indirect routes wins the
  tie-break passed cleanly on this (symlinked-tmp) machine.
  New PRD-0001 **AC-14a** and a matching acceptance test
  (`ac14a_follow_symlinks_alias_is_counted_once_and_never_masks_a_real_duplicate`)
  reproducing the exact defect end-to-end through the real binary. New
  ADR-0008 amendment, a direct companion to TASK-0319's. Both guide pages
  updated: `removing-duplicates-safely.md`'s parity exception is narrowed to
  the one case it still applies to (symlink target not independently in
  scope), and `scanning-and-scope.md`'s symlink section now documents the
  aliasing behavior. **Accepted residual, unchanged from before and named
  explicitly rather than silently narrowed:** a leaf symlink whose target is
  not independently enumerated in this run (outside every scan root, or
  filtered by `--ext`/hidden rules) is still a candidate at the link path,
  still declined at apply time with the existing skip reasons — safe,
  truthfully reported, reachable only via an opt-in flag.
  **Architect review (one dispatch, converged on the branch before the PR).**
  The collapse logic itself held up under adversarial probing with the real
  binary — relative roots (`.`, `A`), a root containing `..`, a root that is
  itself a symlinked directory, a symlink loop, a direct route _deeper_ than
  its alias, and 3-route groups all behave correctly, and the root-relative
  "direct" test generalizes as designed. Three findings, all resolved here:
  **(1)** the _same_ defect — the anchor rule masking 498 genuinely
  reclaimable bytes — was reproduced **across two scan roots**
  (`--follow-symlinks A B` with `A/l -> B`), which the root-scoped ruling
  leaves open. Not a regression and not made worse, but it was closed _by
  omission_: the amendment named only the leaf-symlink residual, and AC-14a as
  first written ("one physical file reachable by two paths") would not have
  held. AC-14a is now scoped to a single scan root, ADR-0008 and
  `scanning-and-scope.md` name the cross-root case explicitly, and
  **TASK-0329** tracks the product-owner call on whether to widen the pass
  (option (a) there is mechanically small — one pass over all candidates with
  "direct" evaluated per candidate against its own root).
  **(2)** Performance: the pass canonicalizes every candidate and was measured
  at ~0.7 s on 30k files at depth 4 (release, warm cache) — **100%** of the
  `--follow-symlinks` wall-clock delta over `main` (6.59 s → 7.28 s), and paid
  even on trees containing no symlinks at all, which is the common case for a
  user who passes the flag defensively. `enumerate_root` now returns whether
  its walk saw a symlink and the pass is skipped when it did not: 6.57 s,
  exactly `main`'s cost and the no-flag cost, with the four alias tests still
  green (they would fail if the guard failed to detect either symlink shape).
  **(3)** Test/doc gaps: added a >2-route alias-group test and an explicit
  input-order-independence test for the survivor (NFR-3/AC-16 — the previous
  tests all ran in natural walk order); corrected the `enumerate_with_progress`
  docstring, whose "the count only ever grows by files that will actually reach
  the next pipeline stage" claim an alias collapse falsifies (cosmetic — the
  Scanning phase is indeterminate and the engine sets its own total), the
  module header's scope-rule and "no canonicalization here" notes, and the
  "same directory entry" phrasing that was accurate for a symlinked ancestor
  but not for a leaf symlink. `keeper::cmp_shortest_by`'s extraction was
  verified expression-identical to the previous `cmp_shortest` and is already
  pinned by keeper's four existing per-tier `shortest` tests, so no new test
  was warranted there. This PR._

- _Session 15 (TASK-0322, intra-file parallel hashing) closes the second
  unexploited throughput lever the TASK-0312 NFR-4 measurement surfaced (the
  first, the scan stage, was TASK-0321): `blake3` was compiled without its
  `rayon` feature, and `engine.rs`'s `hash_file` called the serial `update()`
  unconditionally, so one large file was hashed by exactly one core no matter
  how many idle cores the run had. This ADR (0010) had already asserted "we use
  the blake3 crate's parallel update" as a design decision back in Phase 2 —
  never actually implemented, and this task's brief was explicitly "measure
  before adopting," not to assume that framing was still right by the time
  Phase 3 got here.
  **Measured on a 10-core machine, isolating the real question: nested rayon
  parallelism.** `engine.rs` already runs one rayon parallel iterator over
  each same-size cohort (outer level); the open question was whether adding a
  second, intra-file rayon call (inner level, `update_rayon`) inside that
  closure helps, hurts, or does nothing — and how that depends on how many
  files the outer level currently has in flight. Four scenarios: (1) single
  large file with no outer contention (the case ADR-0010 originally named) —
  `update_rayon` (4 MiB buffer) sustained ~3.1–3.9x the throughput of serial
  `update()` (64 KiB buffer) from 10–500 MiB. (2) Small files in isolation —
  below ~128 KiB the per-call join overhead isn't recovered (measured ~2x
  slower at 32 KiB); from 256 KiB up it's already a net win. (3) A cohort of
  N same-size large (200 MiB) files, outer rayon spreading them across
  cores — wins at N=1–4 (fewer files than cores), converges to parity by
  N=8–16 (at/past core count), never regressing beyond ~6%. (4) A cohort of N
  same-size medium (4 MiB) files — the realistic case, since most duplicate
  groups are a handful of ordinary photos, not dozens of huge files — wins
  outright at N=2–8, bounded ≤6% regression even at the extreme N=512.
  **Adopted, threshold-gated, not unconditional:** `hash_file` now branches on
  a new `RAYON_HASH_THRESHOLD` (1 MiB, comfortably past the measured ~128–256
  KiB breakeven) — below it, the original serial/64 KiB path is untouched
  (where the per-call overhead is a real if small loss, and where the
  overwhelming majority of ordinary photo files live); at/above it,
  `update_rayon` with a `min(file size, 4 MiB)` buffer (the measured
  throughput knee, capped so a file just over the threshold doesn't pay a
  4 MiB buffer to hash 1 MiB). This keeps the
  common case byte-for-byte on the previously-measured path while capturing
  the upside for the case ADR-0010 flagged as unaddressed: a handful of large
  duplicate files (RAW/TIFF/PSD/video) sharing one core when idle cores were
  available. **Architect review (one dispatch, converged) re-ran the
  measurement independently and changed two things:** the buffer is now capped
  at the file size (fixed 4 MiB measured both slower under cohort contention
  and ~2.8x heavier in peak RSS for 1 MiB files), and ADR-0010's memory claim
  is corrected rather than merely inherited — a rayon worker blocked inside
  `update_rayon`'s `join` can steal another file's outer task and hold a
  second buffer live, so live buffers run ~1.5–2x pool width (measured peak
  RSS 70–91 MB at 10 workers vs ~12 MB on the serial path). Still bounded by
  machine width, never by data volume, so NFR-4 holds — but materially larger
  than before and now written down as `O(2 × pool width × buffer)`, which
  supersedes the TASK-0312 "300 MB over 3 files costs 7.5 MB" figure for files
  at/above the threshold: re-measured on the real release binary, 3 × 100 MiB
  duplicates went 7.6 MB → 20.4 MB and 8 × 100 MiB went 7.9 MB → 41.4 MB
  (exactly one buffer per file in flight), with **byte-identical JSON** from
  the `main` and branch binaries on both fixtures and no wall-time regression.
  `tests/scale.rs`'s recorded comparison was corrected to match. The review
  also measured the missing control (serial `update()` using a 4 MiB buffer:
  1.02–1.06x), confirming the win is `update_rayon` and not the bigger read,
  and noted that the headline single-file 3.9x is benchmark-only — the engine
  never hashes a size-singleton, so the reachable best case is a cohort of 2.
  `blake3`'s `rayon` feature enabled in `Cargo.toml` (adds
  `rayon-core` as a transitive dep of `blake3` — already present transitively
  via the crate's own top-level `rayon` dependency; `cargo-deny` clean, same
  pre-existing duplicate-crate warnings as `main`, none new). Byte-verification
  (`files_equal`, ADR-0008's sole removal authority) is a raw comparison, not
  a hash, so out of scope — TASK-0322 named hashing specifically. Correctness
  pinned by a new regression test
  (`hash_file_matches_reference_hash_across_threshold_boundary`) asserting
  `hash_file` agrees with BLAKE3's own one-shot hasher just below, exactly at,
  and just above the threshold, plus (review addition) at a size past the
  buffer length so the parallel path is exercised with more than one
  `update_rayon` call and an unaligned tail — BLAKE3 guarantees the digest
  depends only on the bytes, never on chunking, so the risk was this module's
  own buffering logic, not the algorithm. Recorded in the
  [2026-08-04 amendment to ADR-0010](../adr/0010-concurrency-and-memory-model.md#consequence-note--intra-file-parallel-hashing-adopted-threshold-gated-2026-08-04-task-0322).
  This PR._

- _Session 14 (TASK-0319, dedupe overlapping/repeated scan roots) closes an
  architect finding from the S3 review: `imgdedup ./d ./d` (or overlapping
  roots like `./d ./d/sub`) walked the same physical file more than once,
  so every file appeared hard-linked to itself in the dry-run report
  (`hard-linked: a.png, a.png`; "no duplicate groups to remove; 1
  already-deduplicated hard-link set") — fail-safe (apply-time Check 4,
  ADR-0008, already made this safe to run) but misleading, since the tree
  had never actually been compared against anything. Reproduced first with
  the real binary (`./target/debug/imgdedup /tmp/tdup /tmp/tdup`) to confirm
  the exact symptom before writing a failing test. **Fix is enumeration-stage,
  not engine-stage:** new `enumerate::dedupe_roots` canonicalizes the
  (normally small) list of user-supplied root arguments and drops any root
  that is an exact duplicate of, or nested inside, another — regardless of
  argument order — before any file is walked. Single-root invocations (the
  common case) skip canonicalization entirely (`paths.len() <= 1` early
  return), so this adds zero cost for the overwhelmingly common case and no
  per-candidate cost even with multiple roots — deliberately avoiding
  `same-file::Handle`-based dedup across the full candidate set, which would
  violate `collapse_identity`'s NFR-4 invariant (bounded open-handle count)
  at scale. A root that cannot be canonicalized (e.g. it does not exist)
  passes through unchanged so the existing root-error path (ADR-0008
  exit-code semantics) is untouched — verified by a regression test.
  **Three architect dispatches — the first two each found real, reproducible
  blockers; the third converged clean.** Another session in the run where
  review found real substance, not nits — until it finally didn't.

  **Round 1: 3 blockers + a should-fix.** (1) With `--no-recurse`, an
  ancestor root's walk is capped at depth 1, so it never reaches a nested
  root's contents — collapsing the nested root as "covered" silently dropped
  its files entirely. (2) A hidden path passed _explicitly_ as a scan root is
  always included (opt-in-by-explicit-root,
  `docs/guide/scanning-and-scope.md`) — collapsing an explicitly-named hidden
  nested root into its ancestor silently un-opted it back out, since the
  ancestor's own walk skips hidden entries by default. (3) The original
  all-pairs `starts_with` comparison was O(n²) — review measured a 25×
  slowdown at 20,000 roots (73s vs. `main`'s 2.97s). **Should-fix:** the
  `overlapping_roots_do_not_double_count_and_still_find_real_duplicates` CLI
  test's only assertion also matched the pre-fix buggy output, so it wasn't
  actually pinning the bug it was named for.

  **Round 1 fixes, and what round 2 found wrong with them.** The first pass
  fixed the `--no-recurse` and hidden cases by disabling non-exact nesting
  collapse under `--no-recurse` entirely and skipping collapse when the
  nested root is hidden relative to its ancestor; the O(n²) comparison became
  a sort (`Path`'s `Ord` is lexicographic over components, so a path's
  descendants sit in a contiguous range right after it once sorted) plus a
  linear pass tracking a single "last kept ancestor." **Round 2 found two
  more real holes in that pass, both the exact TASK-0319 symptom again, both
  reproduced against the release binary:** (1) a single "last kept" slot lets
  a kept-but-not-collapsed nested root (e.g. an explicit hidden root) shadow
  its _own_ ancestor from a later, unrelated sibling that sorts after it —
  `imgdedup A A/.cache A/sub` masked a real duplicate in `A/sub` down to "0
  bytes reclaimable." (2) Disabling nesting collapse entirely under
  `--no-recurse` over-corrected: a depth-1 walk _does_ reach the ancestor's
  own direct-child files, so a direct-child file given as its own root was
  still double-walked. **Fixed:** `dedupe_roots` now tracks a **stack** of
  still-relevant kept ancestors (not one slot), and the `--no-recurse`
  coverage check is depth-aware (`descendant.parent() == Some(ancestor) &&
  descendant.is_file()`) rather than a blanket disable — both changes proved
  sufficient to check only the top of the stack, argued in the code comment
  and the ADR amendment. 3 more `enumerate.rs` unit tests reproduce both
  round-2 holes plus a multi-level-nesting case, on top of the round-1 tests
  (7 new tests total across both rounds) and the original 4. Both rounds'
  fixes re-verified against the real binary.

  **Round 3 (convergence check): approved, no blockers.** Independently
  re-derived the "checking only the top of the stack is sufficient" proof
  rather than trusting the code comment, then tried to falsify it with two
  differential-testing sweeps against a brute-force oracle (`enumerate` of
  the deduped roots must equal the set union of `enumerate` on every root
  individually) — 65,520 configurations total across mixed hidden/non-hidden
  multi-level trees and a 6-deep chain forcing 3–4 stack pops per iteration,
  zero mismatches; the same harness found 13,020 failures when pointed at
  the round-1 single-slot implementation, confirming the harness itself is
  meaningful. Re-verified performance (no O(n²) reintroduced; ~7% flat
  overhead from canonicalizing 20,000 roots, and 1.7× _faster_ than `main`
  in the case where roots actually collapse, since redundant walks are
  elided). Five non-blocking nits, all folded in: a garbled `covers()` doc
  comment, a guide sentence that overstated the hidden-root exception
  (`--hidden` does still collapse it), and two known-residual-gap notes
  added to the ADR amendment and TASK-0320's backlog entry (an unreadable
  ancestor directory fails loudly rather than silently, so out of scope; a
  `--follow-symlinks` filesystem-aliasing case reproduces this task's exact
  symptom with a single root and no repeated arguments at all, so it isn't
  fixable at the root-list level and belongs with TASK-0320 instead).
  Recorded in the
  [2026-08-03 amendment to ADR-0008](../adr/0008-core-deduplication-algorithm-and-semantics.md#amendment-2026-08-03--overlappingrepeated-scan-roots-are-no-longer-double-enumerated-task-0319).
  This PR._

- _Session 13, follow-up fix (TASK-0327 regression) — found by actually
  running `make validate` locally, not by reading the config: TASK-0327
  folded `make links`/`scripts/validate.sh` onto lychee, using the exact same
  invocation as CI's `linkcheck` job, `'./**/*.md'`. CI's job runs on a fresh
  checkout with no `npm ci` step, so `node_modules` never exists there —
  locally, after this session's own `npm ci --ignore-scripts` (run to
  dry-run-verify TASK-0324's `semantic-release` config change), the same glob
  picked up thousands of vendored markdown files with root-relative links
  lychee cannot resolve, producing 1,645 spurious errors. Fixed with
  `lychee.toml`'s `exclude_path = ["node_modules"]` — a no-op in CI (the
  directory doesn't exist there) but load-bearing for any contributor whose
  working tree has it. This PR._

- _Session 13, second unit (TASK-0324 + TASK-0325, release.yml nits) closes
  two architect-review nits from TASK-0203 against the write-permission
  release job. **TASK-0324:** `.releaserc.json`'s `@semantic-release/github`
  options used the deprecated `successComment: false` / `failTitle: false`
  form; the backlog wording said to wait "for the next semantic-release
  major bump," but checking the currently-pinned `@semantic-release/github@
  12.0.9` source directly (`lib/success.js`, `lib/fail.js`) showed the
  deprecation warning already fires today, and `successCommentCondition:
  false` / `failCommentCondition: false` already short-circuit the same
  commenting/issue-creation paths without it — no reason to wait, so this
  landed now instead of being deferred again. `semantic-release --dry-run`
  confirms the new config loads and validates cleanly; it does **not**
  exercise the `success`/`fail` steps themselves (verified against
  `semantic-release`'s own `lib/plugins/normalize.js`: those steps are
  `dryRun: false` and are skipped in dry-run mode) — the source read is the
  actual evidence for behavioral equivalence, the dry-run only rules out a
  config-validation regression. **TASK-0325:** added
  `github.event.workflow_run.head_repository.full_name == github.repository`
  to `release.yml`'s trigger condition, with a comment explaining it's a
  redundant-today, defense-in-depth gate (a forked PR's CI run happens in
  _this_ repo and can report `head_branch == 'main'` — the fork's own
  default branch — but `event == 'push'` already excludes that case) rather
  than a load-bearing fix. `actionlint` shows no new findings (one
  pre-existing, unrelated shellcheck style nit present on `main` too).
  **Architect review: approve with one should-fix + 4 nits, all resolved.**
  Both functional edits verified independently correct (equivalence proven
  from `@semantic-release/github`'s source; `head_repository` confirmed a
  required, non-nullable `workflow_run` field that fails closed if ever
  absent). Should-fix: [ADR-0015](../adr/0015-release-publishing-tag-and-github-release.md)
  — the ADR this task's own review originated from — had gone stale
  (still described the deprecated option spelling and the four-clause
  trigger); closed with a
  [dated amendment](../adr/0015-release-publishing-tag-and-github-release.md#amendment-2026-08-03--non-deprecated-plugin-option-spelling-and-an-explicit-fork-gate-task-0324task-0325)
  per the ADR's own immutability rule. Nits: tightened the `release.yml`
  comment's first sentence (it described the wrong mechanism), scoped the
  dry-run claim above, and fixed two backlog wording issues (options live in
  `.releaserc.json`, not `release.yml`; the superseded "wait for a major
  bump" directive is now struck through rather than silently contradicted by
  its own done-note). Second architect dispatch kept per the session-6
  precedent: this is CI automation carrying `contents: write`, so the
  security gate applies at this size too — and, as in every prior instance of
  that precedent, it found real, if small, issues. This PR._

- _Session 13, first unit (TASK-0327, local link-check consistency) folds
  `make links` and `scripts/validate.sh` onto lychee: both now run
  `lychee --config lychee.toml --no-progress './**/*.md'`, the exact
  invocation CI's required `linkcheck` job uses, instead of the old
  `markdown-link-check`-based command (a different tool, a different config
  file, and — until now — not wired into `validate` at all, so local
  validation never actually checked what CI enforces). `.markdown-link-
  check.json` is removed as unused. Verified locally: 0 errors, output
  matches CI's last-known-good run. Docs-only tooling change verified
  directly by the orchestrator (no product-code change, no write-permission
  CI touched); architect dispatch skipped per the
  [2026-08-02 ADR-0011 skip exception](../adr/0011-unit-of-work-cadence-and-planning-gate.md#amendment-2026-08-02--architect-review-skip-exception-for-verified-low-risk-units).
  Landed as PR #70._

- _Session 12 (TASK-0323, exit-code reconciliation) resolves the `--delete`
  without `--apply` exit-code disagreement found by the S4 acceptance suite:
  code exited `1`, the guide documented `2`. Owner ruling (2026-08-03): `2`
  (usage error) — a refused flag combination joins clap's own parse-time
  errors, not run-level failures like an unreadable root. `exit_code::CLAP`
  renamed to `USAGE` (no longer clap-exclusive) and wired into `main.rs`'s
  `args.validate()` failure branch in place of `ERROR`. `cli.rs`, `main.rs`,
  and the test suite (`cli_integration.rs`, `acceptance.rs`) all reconciled;
  the guide needed no change, it was already right. This PR._

- _Session 11b (TASK-0317, animate the scanning phase) fixes the frozen
  `scanning...` indicator for the **enumeration** sub-phase (magic-byte
  sniffing is a separate follow-up, TASK-0328 — see below). Architect review
  found two real defects before merge, not just nits: (1) `progress.rs`'s
  `paint()` unconditionally clamped the numerator to `phase_total`, which is
  0 until a total is known, so any `advance()` during the indeterminate
  phase was invisible. (2) `set_total` never reset the numerator, so
  enumeration's "files found" count leaked straight into Hashing, pinning
  it at `x of x (100%)` from Hashing's very first repaint — a genuine AC-21
  violation the green suite did not catch. Both fixed, with regression
  tests at the unit and CLI level. The indeterminate branch renders
  `"scanning... N file(s) found"` — **TTY only**: review also found that
  showing the count on non-TTY output would freeze at a permanently false
  `"0 files found"` (non-TTY repaints an indeterminate phase exactly once,
  at phase entry, before anything is counted), which is worse than the old
  neutral `"scanning..."`; non-TTY stays bare. New
  `enumerate::enumerate_with_progress` (mirrors `engine.rs`'s
  progress-aware/progress-free split) advances the counter once per
  accepted candidate during the serial walkdir traversal.
  **Not animating magic-byte sniffing was initially justified on the wrong
  grounds** (claimed "double-counting" made it technically hard; review
  showed `retotal` already solves that, same as Verifying does) **and an
  unmeasured one** (claimed TASK-0321 made sniff no longer dominant;
  review measured the opposite on a release build — enumeration is only
  ≈10% of the scanning window, sniff ≈90%). Corrected: the real reason is
  an AC-21 product-owner call (Scanning is currently spec'd as one
  indeterminate state), tracked as **TASK-0328** with the measurement
  recorded. This PR._

- _Session 11 (TASK-0318, control-character sanitization) closes a security
  note from the TASK-0315 review: hostile filenames (or error messages that
  embed one, e.g. `walkdir`'s and `trash`'s `Display` impls) could reach
  stderr/stdout with raw control bytes — ANSI escapes injecting terminal
  control sequences, or a raw `\n` forging an extra report line. New
  `src/safe_text.rs::escape_control_chars` (Unicode `Cc` only — C0/DEL/C1 —
  so non-Latin scripts/emoji/combining marks are never touched) is applied
  at all 3 `main.rs` stderr sites and, once the CLI integration test caught
  that the backlog's "stderr" wording undersold a scope already recorded in
  [`reviews/apply-delete.md`](reviews/apply-delete.md) (finding 12–17), all
  12 `report.rs` stdout sites too. One architect dispatch: 2 blockers (the
  `report.rs` scope gap; this planning update itself hadn't landed) + 5
  nits, all resolved and converged in one round — including a demonstrated
  line-forgery repro (a warning path containing `\nSummary: 0 duplicate
  groups, forged` rendered as two lines pre-fix, one post-fix) and a
  verified claim about `trash::Error`'s `Debug`-based `Display` embedding
  paths that `std::io::Error` normally would not. `json.rs` is an accepted
  residual — serde already escapes C0 per RFC 8259. This PR._

- _Session 10 (TASK-0321, scan-stage parallelization) parallelized
  `sniff::filter_candidates` with rayon — the throughput lever the TASK-0312
  NFR-4 measurement identified (a control fixture where no file is hashed
  costs the same as one hashing ~1,800, because `enumerate`/`sniff` are
  serial). Same pattern already used for hashing in `engine.rs`; rayon's
  indexed `collect` preserves input order by construction, verified against
  the crate source during architect review and pinned by a new unit test
  asserting exact order (not just "the widths agree") at 1 vs. 8 threads.
  Measured on a synthetic 60k-file all-rejected fixture, 10-core machine:
  cold-cache 158.7s → 21.5s (~7.4x), warm-cache 6.1s → 1.8s (~3.3x). One
  architect dispatch (budgeted for concurrency-critical code per the batch
  plan): approved with one blocker (this ADR/planning doc set was
  initially missing — now resolved in this same PR) and should-fixes
  (strengthened the order test to assert exact input order rather than
  just cross-width agreement; fixed an asymmetric `Vec::with_capacity`
  call; corrected an inaccurate ordering comment in `engine.rs`). Recorded
  in the [2026-08-03 amendment to ADR-0010](../adr/0010-concurrency-and-memory-model.md#consequence-note--the-scan-stage-is-no-longer-serial-2026-08-03-task-0321).
  Follow-ups: TASK-0317's indicator work can now proceed against the
  parallel scan stage (its debug-build sizing note is stale); `enumerate`
  (still fully serial) is a plausible new dominant stage worth a future
  control-fixture re-measurement, not confirmed here. This PR._

- _Session 9 (AI automation support, TASK-0218) delivered the `imgdedup`
  Claude Code plugin (`claude-plugin/`): a skill (`using-imgdedup`) defining
  the dry-run-first safety protocol, the `/imgdedup:dedupe` slash command,
  and a read-only `imgdedup-operator` subagent for large scans — a pure
  wrapper over the existing `--json`/`--apply`/`--delete` CLI contract, no
  new CLI behavior. Formalized in
  [ADR-0016](../adr/0016-ai-agent-automation-plugin.md), documented for users
  in [`docs/guide/using-with-claude.md`](../guide/using-with-claude.md) and
  README, and recorded as a supported capability in the
  [2026-08-02 amendment to PRD-0001](../prd/PRD-0001-exact-image-deduplication.md#amendment-2026-08-02--ai-agent-automation-as-a-supported-capability).
  This PR._

- _Session 8 (docs/tooling consistency) is complete: TASK-0206 (PR #56),
  TASK-0205 (PR #57), the follow-up link-exclude fix (PR #58 — whose `fix:`
  squash commit auto-published **v0.2.1**, binaries and all), and
  **TASK-0217** all done. TASK-0217 was preceded by a
  deliberate **read-only recon pass** (17 files: README, all 6 `docs/guide/`
  pages, all 5 `docs/process/` docs, all 4 agent files) before touching
  anything, which turned an open-ended "consistency pass" into three
  precisely-scoped pieces — most of the surface checked out clean. Owner
  explicitly accepted overage risk for this session (not a standing rule) to
  finish it in one go rather than deferring further. See the Done entry below
  for what changed._

- Session 7b (TASK-0211, prebuilt binaries) merged as **PR #54**. **Week 1
  target reached: v0.1.0 shipped**, with binaries backfilled onto it
  manually. Unplanned bonus: PR #54's own squash commit was a `feat:` type
  (adding the binary-attachment capability), which — correctly, per the
  automation this same PR built — triggered **another** real release,
  **v0.2.0**, and its `assets` job ran the fully-automatic path end-to-end for
  the first time with no manual intervention: all four legs green, uploaded
  to the live release, spot-checked (Linux artifact downloaded,
  checksum-verified). The pipeline is now proven both ways: manual backfill
  and automatic on-merge. **Operational note for future sessions:** any PR
  landing as `feat:`/`fix:` now triggers a real release _and_ a four-target
  binary build, not just a tag — keep `docs:`/`chore:`/etc. commit types for
  planning-only or non-shipping changes, as the convention already requires
  ([`commits.md`](../process/commits.md)).

## Up next

Session order per the [batch plan](batch-plan.md) — **1:** re-plan + batch 4b
(TASK-0315) — **done** · **2:** TASK-0316 + S3 core (TASK-0309) — **done** ·
**3:** S3 finish (TASK-0310, acceptance, docs, review, PR) — **done** ·
**4+5:** S4 acceptance + scale (TASK-0312) — **done (PR #50)**, combined into
one session/PR · **6:** release plumbing (TASK-0202, TASK-0203) — **done
(PR #52)** · **7:** binaries + v0.1.0 (TASK-0211) — **done (PR #54)** ·
**8:** docs consistency (TASK-0217/0205/0206) — **done (PRs #56, #57, #58, #59)**
· **9 (fresh window):** AI automation (TASK-0218) — **done (this PR)**.

**TASK-0314 (real-photo field validation) did not land in PR #50** — it needs
the owner's ~500–1k photo corpus, which per the 2026-08-01 plan was allowed to
slip out of the S4 session. It turned out not to gate anything: **v0.1.0
published automatically** (see below) without it, so pick it up whenever the
owner can supply the corpus.

**v0.1.0 published automatically, before TASK-0211 could attach binaries to
it.** The `v0.0.0` baseline tag (pushed 2026-08-02) bootstrapped the version
line as designed; the very next green CI run on `main` (the docs-sync merge,
PR #53) triggered the release workflow, which found the bootstrap tag, ran
semantic-release, and published **v0.1.0 with zero release assets** — this
predates the `assets` job TASK-0211 adds. `docs/process/release.md` and
[ADR-0015](../adr/0015-release-publishing-tag-and-github-release.md) explain
why binaries must be built **inside** the release job (a `GITHUB_TOKEN`-pushed
tag triggers no other workflow) and how `chore/prebuilt-binaries` backfills
v0.1.0's missing assets via a manual `workflow_dispatch` input rather than
re-running semantic-release.

Owner decisions
2026-07-19: **license = GPL-3.0** (repo private for now), test oracle =
generated golden set (TASK-0313), real photos only for read-only field
validation (TASK-0314), AI automation = skill + command + operator agent
(TASK-0218).

**Slice cadence is now in force (ADR-0013).** S1 (detect-report) is shipped. Remaining PRD-0001 work is delivered as **three remaining coherent shippable slices** (S2, S3, S4), each a single branch → granular commits → one squash PR/release. The full slice map (contained tasks, per-slice user value, and PR-open acceptance) lives in the "PRD-0001 delivery slices" subsection of
[`backlog.md`](backlog.md).

- **Next: session 2 — TASK-0316** (hard-link surfacing-vs-wording decision from
  the batch-4 golden-set finding; product-owner work whose cost depends on the
  outcome — a wording narrowing is docs-only, surfacing the sets touches the
  engine, the JSON schema and the golden manifest) **then the S3 core**
  (TASK-0309), per the release-path slices below.
- **Then, in order:** **S3 — Destructive actions** (`apply-delete`; TASK-0309 + TASK-0310) → **S4 — E2E acceptance & scale** (`e2e-scale`; TASK-0312, tester-led, AC-1..AC-19 and NFR-4 cross-platform proof). Each slice rebases on `main` after previous merges. **The `--exit-code` binary-level E2E test (AC-19) is deferred to S4** per S1 architect review.
- **New: Batch 4b — progress indication (FR-19, TASK-0315).** Owner field feedback (2026-07-19): the running tool shows no terminal output. Now spec'd as [PRD-0001 Amendment 2026-07-19](../prd/PRD-0001-exact-image-deduplication.md#amendment-2026-07-19--progress-indication--stored-results): a live **stderr** progress indicator (phase, `x of y`, %), default-on, TTY-aware with a plain non-TTY fallback and `--no-progress` opt-out; stdout stays pure in all modes (incl. `--json`). Ships as its **own one-unit feature branch** after batch 3 (coexists with `--json`), independent of the S3/S4 release path but ideally before v0.1.0. Owner decision: **build in a fresh usage window** — do not start ahead of it. Stored results use **stdout redirection** (`--json > result.json`), no new flag; a `--output` flag was considered and deferred. The redirection **doc note ships now** (documents existing behavior).
- **In-progress reviews (ADR-0013):** within a slice the implementer requests an
  architect review for significant/risky changes (not per-commit; the
  product-owner or orchestrator may demand one), logged in
  `docs/planning/reviews/<slice>.md`. Uncertain now-vs-defer calls go to the
  product-owner, and deferrals are promoted to tracked backlog tasks.
- **TASK-0207 fulfilled by TASK-0301.** TASK-0208 reframed to clippy, cargo-audit,
  and cargo-deny (see ADR-0007). TASK-0209 (cargo Dependabot), TASK-0203
  (release wiring), **TASK-0211** (prebuilt binaries), **TASK-0206** (link
  check promoted to a required gate — **PR #56, merged**), and **TASK-0205**
  (cspell → typos migration — PR #57) are done.
- TASK-0219 (public-release licensing readiness) — **design ruled 2026-08-05
  (ADR-0014 amendment), implementation in flight in session 25**; the
  trigger-bound remainder is now **TASK-0336**. **TASK-0327** (`Makefile`'s
  `links` target folded onto lychee,
  wired into `validate`) — **done, PR #70**. **TASK-0324/TASK-0325**
  (release.yml deprecation + trigger-condition nits) — **done, this PR**.

## Done

- **TASK-0217 — docs consistency pass (PR #59, 2026-08-02).** Preceded by a read-only
  recon (17 files) before any edit, which found the scope was narrower than
  "consistency pass" implied — bundled as one PR since all three pieces trace
  to this one backlog task. **(1) Prebuilt binaries were undocumented:**
  README's "Installing" and `docs/guide/getting-started.md`'s "Install"
  sections both still said "no prebuilt binaries yet" — stale since TASK-0211
  (PR #54). Added a download/checksum-verify/extract walkthrough (with the
  macOS quarantine and Windows SmartScreen caveats), linking to
  `docs/process/release.md#distribution-targets` for full detail rather than
  duplicating it. **(2) The architect-review policy didn't match practice:**
  `sdlc.md`, `crew.md`, and `architect.md` all described every PR getting an
  architect dispatch, but 9 units had already skipped it (docs-only syncs;
  low-risk `contents: read` CI chores) under an unwritten cost/benefit
  exception. Formalized as a dated
  [amendment to ADR-0011](../adr/0011-unit-of-work-cadence-and-planning-gate.md#amendment-2026-08-02--architect-review-skip-exception-for-verified-low-risk-units)
  with three explicit criteria (no product-code changes; no write-permission/
  secret-touching CI; correctness independently verified by the orchestrator,
  not asserted), then propagated as short pointers into the three docs above
  — the exception lowers the floor, never a ceiling; the architect/
  product-owner/orchestrator can still always demand a review. **(3) Stderr
  diagnostic prefix unified** (resilience-review nit): `cli.rs::validate()`
  now returns the bare message; `main.rs` adds `"error: "` uniformly at its
  one call site, the same way every other diagnostic in the run already did
  — no user-visible behavior change (`refactor:`), regression-locked by a new
  assertion on the existing test. Owner explicitly accepted overage risk to
  finish this session in one go (not a standing rule).

- **Session 8 — docs/tooling consistency (2026-08-02).** Three small
  independent units, each its own PR (per the one-unit-one-PR chore cadence).
  **TASK-0206 (PR #56):** `linkcheck` promoted from informational to a
  required gate (`fail: true`, renamed from `linkcheck (informational)`).
  Flipping `fail: true` immediately surfaced **two** real, previously-invisible
  defects. First: `lychee.toml` failed to _load_ under the pinned lychee
  v0.24.2 (`exclude_mail` was removed from its config schema — the same class
  of drift as the markdownlint-cli2 issue from Batch 0); fixed (field
  removed). Second, once loading: `www.gnu.org/licenses/{gpl-3.0.txt,
  gpl-howto.html}` (in ADR-0014) consistently timed out from GitHub-hosted
  runners across two separate CI runs despite `max_retries = 3`, while
  resolving instantly (`curl -I`, HTTP 200, <2s) from a non-CI IP — gnu.org
  rate-limits/blocks cloud CI ranges, so this is the check environment being
  unreachable, not a broken link; excluded rather than "fixed". With
  `fail: false` both defects were silently swallowed, so link checking had
  never actually run in CI, only reported green. Verified locally with the
  exact pinned version: 439 links, 436 OK, 0 errors, 3 excluded. Also
  corrected a stale claim
  that `cargo-deny` was a required check since 2026-07-19 — live-verified via
  the GitHub API that it was not, until the owner added it (and `linkcheck`)
  live during this session. The owner then renamed the required-check
  context from `linkcheck (informational)` to `linkcheck` to match (the
  agent's own `gh api` attempt was blocked by the permission classifier, as
  intended for branch-protection changes) — verified live afterward; no
  blocker remains. **PR #56 merged.**
  **TASK-0205 (PR #57):** migrated spell-checking from cspell
  to `typos`. Decided by running both tools against this repo's Markdown and
  a deliberately-misspelled scratch file: equal catch rate on six real typos,
  but `typos` needed only **one** exception (`PNGs`, a tokenizer artifact)
  versus cspell's 100+ curated custom words — typos matches known
  misspelling _patterns_ rather than checking every word against a
  dictionary, so jargon needs no allowlist. Rejected the community
  `crate-ci/typos` GitHub Action: its entrypoint builds a `**` glob under
  plain bash with no `shopt -s globstar`, so it silently under-matches
  recursively — confirmed by direct test. CI instead installs the pinned
  binary directly (SHA256-verified, since upstream publishes no checksums
  file) and runs it against `git ls-files '*.md'`, matching cspell's old
  scope exactly. Follow-up: **TASK-0327** — `Makefile`'s `links` target still
  uses `markdown-link-check`, diverging from TASK-0206's lychee-based gate
  and not wired into `validate` at all; found while touching the Makefile for
  this task, not fixed here to keep the unit focused.

- **Prebuilt release binaries (TASK-0211, PR #54, session 7b, 2026-08-02).** An
  `assets` job in `release.yml` now builds four native targets
  (`x86_64-unknown-linux-gnu` on `ubuntu-24.04`, `aarch64-apple-darwin` on
  `macos-latest`, `x86_64-apple-darwin` on `macos-15-intel`,
  `x86_64-pc-windows-msvc` on `windows-latest`) after the `release` job
  detects a newly published tag (diffing `git tag --list 'v*'`
  before/after `npm run release`, since semantic-release exposes no such
  output itself), stamps the released version transiently into each build
  runner's `Cargo.toml` (never committed — `imgdedup --version` now reports
  the real release version without the in-tree placeholder moving),
  packages a per-target `.tar.gz`/`.zip` with `LICENSE` + `README.md` + a
  `.sha256` checksum, and uploads via `gh release upload --clobber`.
  Design recorded in a
  [2026-08-02 amendment to ADR-0015](../adr/0015-release-publishing-tag-and-github-release.md#amendment-2026-08-02--binary-artifacts-task-0211);
  operational detail in
  [`process/release.md`](../process/release.md#distribution-targets).
  **v0.1.0 turned out to have already published automatically** (the
  `v0.0.0` bootstrap tag + PR #53's green CI run triggered a real release,
  correctly, before this job existed) with zero assets — a `tag` input was
  added to `workflow_dispatch` so the workflow can be run against an
  already-published tag without re-invoking semantic-release; used once to
  backfill v0.1.0 (all four legs green, macOS-arm64 artifact independently
  downloaded/checksum-verified/run). **Two architect dispatches** (design,
  then an independent review-only pass — this touches the same
  write-permission `GITHUB_TOKEN` job as TASK-0202/0203, so the CLAUDE.md
  security gate applied the same way as PR #52): the review found and fixed
  an inverted comment about GitHub Actions null/empty-string coercion, and
  closed a tag-name shell-expression-injection channel (git ref names
  legally permit `$`/backticks/parens; a `require_semver_tag` guard now
  constrains the value at the point it originates) — not a privilege
  escalation given it needs write access already, but a closed channel over
  an argued-harmless one. Follow-up: TASK-0326 (musl/`aarch64` targets,
  deferred to a public-release gate; TASK-0219 already gates the GPLv3 §6
  position on the same event). **Proven both paths the same day:** merging
  this PR's own `feat:`-typed squash commit triggered a real, unplanned
  **v0.2.0** release, whose `assets` job ran fully automatically (no manual
  dispatch) for the first time — all four legs green, uploaded, spot-checked.

- **Release plumbing — licensing + publishing (TASK-0202, TASK-0203, session 6 /
  batch 7a, 2026-08-02).** One chore unit, two decisions.
  **[ADR-0014](../adr/0014-project-license-gpl-3.0-only.md): `GPL-3.0-only`.**
  The owner's 2026-07-19 "GPL-3.0" is formalized with the canonical text in
  `LICENSE`; `-only` rather than `-or-later` because a sole copyright holder can
  always widen the grant later but can never narrow one already conveyed (and
  bare `GPL-3.0` is a deprecated SPDX id). The ADR records what the license does
  and does not mean while the repo is private: privacy is access control, not
  licensing, so anyone given repository access receives a GPL copy they may
  redistribute, and conveying a **binary** triggers the GPLv3 §6 source
  obligation — a live constraint for TASK-0211. Reconciled `Cargo.toml`
  (`license = "TBD"` → the SPDX id), README, CONTRIBUTING, `deny.toml`, and the
  CI structure check.
  **[ADR-0015](../adr/0015-release-publishing-tag-and-github-release.md):
  publish the tag + GitHub Release, never commit back to `main`.** ADR-0004's
  premise ("branch protection blocks the tag push") turned out to conflate two
  refs: tags live in `refs/tags/*`, protection targets `refs/heads/main`, and
  this repo has no tag rulesets — verified against semantic-release's source and
  the live branch-protection API. So publishing needs **no bot, PAT, or
  protection bypass**, only the workflow's own `GITHUB_TOKEN` with
  `contents: write`, and ADR-0003 is untouched. `release.yml` now runs
  semantic-release gated on a **green CI run for the merged commit**
  (`workflow_run`), with the tooling pinned by `package-lock.json` +
  `npm ci --ignore-scripts` because this is the only job holding write
  permission. **A dry run caught a live trap:** with no tag in the repo,
  semantic-release computes the first release as **v1.0.0** — merging the wiring
  would have published 1.0.0 by accident. The job now skips until a `v*` tag
  exists; the documented `v0.0.0` baseline then yields v0.1.0 (also verified by
  dry run). Costs of the choice, all recorded: `CHANGELOG.md` is no longer
  generated (it is now a signpost to the Releases page), `Cargo.toml`'s version
  is not bumped by a release (TASK-0211 stamps it at build time), and a tag
  pushed with `GITHUB_TOKEN` triggers no further workflow — so TASK-0211 must
  build binaries **inside** the release job. New follow-up: **TASK-0219**
  (public-release licensing readiness).
  **One defect fell out of the wiring:** `exit_code_flag_is_accepted` asserted
  `imgdedup . --exit-code == 0`, i.e. "the repository working tree contains no
  duplicates" — true only by luck, and false as soon as `npm ci` puts duplicate
  PNGs in `node_modules/`. Fixed to scan an empty temp dir, per the rule the
  destructive-action tests in that file already state: no test scans a directory
  it does not own.
- **Slice S4 — E2E acceptance & scale (TASK-0312, PR #50, sessions 4–5 combined,
  2026-08-02).**
  `tests/acceptance.rs`: 21 tests, one per PRD criterion (AC-1..AC-19, AC-18a),
  driving the compiled binary so coverage is traceable criterion by criterion.
  AC-19's binary-level test lands here after being deferred out of S1.
  `tests/scale.rs` + `make scale`: `#[ignore]`d NFR-4 fixtures. **Measured:**
  peak RSS is ~7.5 MB + ~350 B/file and is independent of data volume (3 files
  at 3 MB and at 300 MB both peak at 7.5 MB), confirming the streaming-read
  design; ~0.36 GB projected at 1M files. **Not settled:** the concurrency
  width — the benchmark could not see the knob, because `enumerate` and `sniff`
  are serial and dominate small-file wall time, so rayon's default stands as
  status quo rather than measured optimum. Architect review: 2 blockers (both
  against claims in the measurement write-up, including a false statement about
  BLAKE3 parallelism in an ADR note) + 13 should-fixes, all resolved; four
  acceptance assertions could not have failed. Follow-ups: TASK-0321 (parallel
  scan — also the lever behind TASK-0317), TASK-0322 (intra-file parallelism for
  large files), TASK-0323 (exit-code doc divergence). See
  [`reviews/e2e-scale.md`](reviews/e2e-scale.md).

- **Slice S3 — Destructive actions (TASK-0309 + TASK-0310, sessions 2–3,
  2026-08-01).** `--apply` moves non-keepers to the OS trash and
  `--apply --delete` removes them permanently, both through one executor with
  the removal mechanism injected, so the irreversible path gets exactly the
  same treatment as the recoverable one. Apply-time re-validation (FR-16) runs
  immediately before each removal and refuses on anything it cannot prove —
  including a fourth check beyond ADR-0008's list, that the candidate is not the
  same physical file as the keeper. A failure never escalates: there is no path
  anywhere from a trash failure to a deletion. Output now reports what happened
  (`mode` = `dry-run`/`trash`/`delete`, plus an additive `actions` object and a
  report section listing every removed path and every skip with its reason).
  **Three defects found by running the tool rather than reading it:** the
  `trash` crate's macOS default hangs forever (it drives Finder via `osascript`,
  needing Automation permission that a non-interactive shell cannot grant); a
  CLI test pointed `--apply --delete` at the repository working directory; and
  re-validation proved metadata of one file and content of possibly another.
  Architect whole-slice review: 1 blocker + 10 should-fixes, all resolved;
  reviewer found no way to destroy a keeper or empty a group. Follow-ups:
  TASK-0319 (repeated scan roots report a tree as clean when it is not),
  TASK-0320 (`--follow-symlinks` candidacy). See
  [`reviews/apply-delete.md`](reviews/apply-delete.md).

- **Batch 4 / TASK-0313 — test-data generator & golden-set oracle (PR #41,
  merged 2026-07-19).** Deterministic tiny-image generator for all 8
  supported magic-byte signatures (`image_bytes(format, seed, len)` — pure
  function, byte-identical across runs/platforms), golden-tree builder with
  explicit mtimes + hard links, and an oracle harness diffing real
  `imgdedup --json` runs against **hand-derived** expected manifests
  (`tests/support/` + `tests/golden_set.rs`; test-only unit). 7 scenarios:
  per-format pipeline coverage, near-misses with positive control,
  mislabeled/empty files, all three `--keep` rules, hard-link
  protected/already-deduplicated semantics, NFR-3 byte-identical stdout,
  generator meta-tests. Architect review approve-with-nits → converged +
  pre-merge confirmation; first JSON-level hard-link assertions green on
  Windows CI. **Oracle's first run found a doc-vs-behavior gap → TASK-0316**
  (lone fully-hard-linked sets absent from output vs guide
  `already_deduplicated` wording; product-owner to decide surface-vs-reword;
  current behavior pinned in the golden manifest). S4 (TASK-0312) reuses the
  generator for AC/scale coverage. See
  [`tasks/TASK-0313-golden-set-oracle.md`](tasks/TASK-0313-golden-set-oracle.md).

- **Batch 3 / Slice S2 — JSON output (TASK-0308, PR #38, merged
  2026-07-19).** `--json` emits a stable schema_version-1 document (groups
  with keeper/removable/protected/hard_link_sets/reclaimable_bytes,
  already_deduplicated sets, totals, and — per resilience F3 — first-class
  `warnings` + `root_errors`) as a second formatter over the same canonical
  model as the human report. Product-owner accepted FR-14/NFR-3; full
  `--json` schema reference in `docs/guide/output-and-exit-codes.md` +
  README usage. Architect whole-slice review approve-with-nits → converged
  (F1 representative-selection doc fix incl. stale `engine.rs` docstring,
  F2 wording, F3 edge-case JSON tests). 191 tests on all 3 OS platforms.
  See [`reviews/json-output.md`](reviews/json-output.md).

- **Batch 2 (final) / TASK-0216 — resilience review + F1 fix (PR #37,
  2026-07-19).** Whole-pipeline error-path review recorded in
  [`reviews/resilience-2026-07.md`](reviews/resilience-2026-07.md). Core
  verdict: all I/O failures degrade safely (warn + never-remove). One defect
  (F1): a nonexistent/unreadable scan root exited 1 **silently** — fixed:
  root errors now print to stderr (`error: <path>: <reason>`), regression
  test added. F3 added the JSON-diagnostics acceptance note to TASK-0308
  (satisfied in S2); F8 rides S4 field validation.

- **Batch 2 (refresh half) / TASK-0216 — toolchain & dependency refresh
  (PR #35, 2026-07-19).** Rust 1.93.1 → **1.97.1**, edition 2021 → **2024**
  (zero source changes needed; style-edition rustfmt applied; one new clippy
  `collapsible_if` fixed in `engine.rs`, architect-verified
  semantics-preserving). All direct deps at latest stable; `cargo update`
  slimmed the lockfile 114 → 87 packages. The resilience/error-path review
  half of TASK-0216 remains next.

- **Batch 1 / TASK-0204 + 0208 + 0209 + 0210 — security & supply chain
  (PRs #31, #34, 2026-07-19).** All actions SHA-pinned + gitleaks image
  digest-pinned; cargo Dependabot enabled (first fruits: clap 4.6.2 in #32,
  infer 0.22.0 in #33, both merged); committed `.claude/settings.json`
  allowlist; cargo-deny audit gate (`deny.toml`, `make deny`, CI job) whose
  first run caught and fixed **RUSTSEC-2026-0204** (crossbeam-epoch → 0.9.20).
  **Owner added `cargo-deny` to `main`'s required checks (2026-07-19)** —
  the audit is now a hard merge gate.

- **Batch 0 / TASK-0215 — `main` unblocked (PR #30, 2026-07-19).** Fixed the
  MD032 that a drifted markdownlint release flagged in
  `reviews/detect-report.md`; pinned markdownlint-cli2\@0.23.1,
  cspell\@10.0.1, markdown-link-check\@3.14.2 across CI/Makefile/validate.sh;
  CI Node 20 → 22 (cspell\@10 requires ≥22.18). Landed the owner-approved
  [batch plan](batch-plan.md). Dependabot PR #29 (setup-node 6→7) went green
  and merged right after.

- **Slice S1 — Detect & report (read-only)** (TASK-0305 + TASK-0306 + TASK-0307 + TASK-0311) — **MERGED in PR #25 (2026-06-25)**. First end-to-end usable increment: `imgdedup <paths>` scans, detects exact-duplicate groups, prints deterministic dry-run report with real `--keep {shortest|oldest|newest}` rule and opt-in `--exit-code`. Nothing on disk changes. Delivered all FRs/NFRs and resolved architect Review 3 with **ADR-0008 Amendment 2026-06-15** (anchor rule: collapsed hard-link representatives with in-scope siblings are never removable, protected in output). 178 tests passing (all 3 OS platforms). See [`reviews/detect-report.md`](reviews/detect-report.md).

- **TASK-0214 — PRD-slice delivery cadence & in-progress review protocol
  (ADR-0013).** For **feature delivery**, the unit of work is now a **coherent
  shippable slice** of a PRD (product-owner carves, architect sanity-checks) rather
  than a single task; `fix`/`chore` keep the ADR-0011 small-PR cadence. A slice is
  one branch + granular commits, **PR-less and CI-free until complete**, squash-
  merged (ADR-0003/0004 unchanged). **In-progress reviews:** the implementer
  requests an architect review for significant/risky changes (not per-commit; the
  product-owner and orchestrator may demand one), logged in
  `docs/planning/reviews/<slice>.md`; feedback is worked immediately, with
  uncertain now-vs-defer decided by the product-owner and **deferrals promoted to
  tracked tasks**. **PR-open gate:** the product-owner has accepted all the slice's
  functional requirements and delivered its docs (ADR-0012); then a **full whole-
  slice architect review** builds on the log, under the ADR-0006 resolve-all gate
  (the 3-OS matrix runs at slice end — an accepted, contained residual risk).
  Authored [ADR-0013](../adr/0013-prd-slice-delivery-cadence.md) + the
  `docs/planning/reviews/` location, and propagated the cadence into `sdlc.md`,
  `branching.md`, `crew.md`, the implementer/product-owner/architect role defs, and
  `CLAUDE.md`. Docs-only; markdownlint + cspell green, links resolve.

- **TASK-0213 — Documentation ownership & guide structure (ADR-0012).** Formalized
  that the **product-owner owns user-facing documentation** (justified by owning
  functional acceptance), split by depth: **general usage in `README.md`**,
  **detailed per-topic pages in `docs/guide/`** (a linked collection of
  self-contained guide pages with an index). Authored
  [ADR-0012](../adr/0012-documentation-ownership-and-placement.md) (extends
  ADR-0011), established `docs/guide/` (index + getting-started, scanning & scope,
  choosing what to keep, removing duplicates safely, output & exit codes — the
  detailed reference migrated out of the README), slimmed the README to general
  usage + a guide link, and propagated the rule consistently into
  `product-owner.md`, `crew.md`, `sdlc.md` (DoD + unit-of-work table), `CLAUDE.md`
  (roles + repo map), and the PRD template. Docs-only unit; markdownlint + cspell
  green, links resolve.

- **TASK-0304 — Core dedup engine (the safety heart).** New module `src/engine.rs`
  with `find_duplicates(files, keeper_order) -> DedupOutcome` implementing the
  ADR-0008 pipeline: size pre-grouping (singletons dropped without hashing, AC-9)
  → BLAKE3 hash pre-filter (streamed 64 KiB buffer, never whole-file) → hash
  grouping → hard-link/identity collapse via `same-file::Handle` (POSIX
  `(st_dev,st_ino)` / Windows volume-serial+file-id; computed only within a
  hash-group and dropped promptly, bounding FD pressure) → **star byte-for-byte
  verify each non-keeper against the keeper** as the sole removal authority
  (FR-4/NFR-1). A forced-collision test (constant hasher) proves correctness does
  not depend on BLAKE3 being collision-proof; hard-link siblings are collapsed and
  never proposed for removal (AC-18/FR-17). Keeper is the minimum under an
  **injected ordering** (`placeholder_keeper_order` lexicographic for now; TASK-0305
  plugs in the real rule). Pure analysis — no filesystem mutation. **Carrier
  thread:** `sniff::filter_candidates` now consumes `Vec<enumerate::Candidate>` and
  emits `Vec<sniff::ConfirmedFile{path,size}>`, threading the enumerated `size`
  through (no re-stat). Adds `blake3` + `same-file` deps; wired into `main.rs` as
  stage 3 behind a `TODO(TASK-0307)` for the report. 15 new engine tests + size-
  carrier test; all 102 tests green; `make validate` (fmt/clippy `-D warnings`/
  test/coverage) passes. Resolved design recorded in
  [`tasks/TASK-0304-core-dedup-engine.md`](tasks/TASK-0304-core-dedup-engine.md).

- **TASK-0303 — Magic-byte image confirmation.** New module `src/sniff.rs`
  with `confirm_image(path) -> bool` (reads at most 64-byte header, uses
  `infer` crate) and `filter_candidates(paths) -> (confirmed, warnings)`.
  Confirms JPEG, PNG, GIF87a/GIF89a, WebP, BMP, TIFF-LE/TIFF-BE by magic
  bytes. Mislabeled files (e.g. text named `photo.jpg`) are rejected and
  collected as `SniffWarning` records — never grouped or acted on (FR-3,
  NFR-2). Wired into `main.rs::run()` as stage 2 after enumeration; removed
  the now-unnecessary `#[allow(dead_code)]` on `Candidate.path`. Adds
  `infer 0.16` to `Cargo.toml`. 14 unit tests (one per format + AC-4 +
  bounded-read invariant + `filter_candidates` behavior); all 86 tests
  green; fmt/clippy clean.

- **TASK-0302 — File enumeration & scope.** Traversal module (`src/enumerate.rs`)
  implementing the first pipeline stage (ADR-0008). Delivers:
  `ScopeOptions` (no_recurse, include_hidden, follow_symlinks, extensions),
  `Candidate` (path + size), `EnumerateWarning` (path + reason),
  `CandidateSet`, and `enumerate()` entry point. walkdir added as a dependency;
  tempfile added as a dev-dependency. 23 unit tests covering AC-15, FR-2, AC-14,
  AC-10; all 66 tests green; `make validate` passes. Key implementation note:
  hidden-file check is relative to the scan root (not the full absolute path) so
  that temp-directory paths starting with `.tmp` are not spuriously treated as
  hidden. Symlink-specific tests are `#[cfg(unix)]`.

- **Process — unit-of-work cadence & planning gate (ADR-0011 / TASK-0212).** One
  branch now carries a whole unit of work (design, TDD code, technical/user docs,
  and the planning/status update) into a single squash PR, classified up front as
  feature/fix/chore; a required `ci-gate` job fronts a path-filtered Rust matrix so
  docs/chore-only units skip the 3-OS compile. Reaffirms ADR-0002; ADR-0003/0006
  intact. See [ADR-0011](../adr/0011-unit-of-work-cadence-and-planning-gate.md) and
  [`tasks/TASK-0212-process-unit-of-work-cadence.md`](tasks/TASK-0212-process-unit-of-work-cadence.md).
  Merged in PR #16; owner completed the branch-protection swap — `main`'s required
  CI check is now `ci-gate` (the three `rust (…-latest)` checks are gated behind it
  and skipped on docs/chore-only PRs).
- **TASK-0301 — Crate scaffold, CLI surface & toolchain.** Merged in PR #14
  (commit `2e12148`); **fulfils TASK-0207** (Rust toolchain). Delivered: Rust
  binary crate (`Cargo.toml`, `Cargo.lock`), `rust-toolchain.toml` pinning the
  toolchain with rustfmt+clippy, full PRD §8 clap-derive surface, exit-code
  constants module (`0`/`1`/`2`/`3`), unit + integration tests covering the CLI
  surface and exit codes, `make validate` Rust targets, `scripts/validate.sh`
  Rust gate, and a 3-OS CI `rust` job (now a required check).
- **TASK-0101 — PRD-0001 (exact image deduplication) accepted.** Merged in PR #9;
  status now `accepted`. Scope: exact (content-hash) duplicates only; dry-run
  default with `--apply`→OS trash and opt-in `--delete`; deterministic keeper rule
  (shallowest path → oldest mtime → lexicographic) with `--keep` override; layered
  file scope (extension allow-list → magic-byte confirmation before any action).
  See [`../prd/PRD-0001-exact-image-deduplication.md`](../prd/PRD-0001-exact-image-deduplication.md).
- **TASK-0201 — Technology stack chosen: Rust + clap.** Fully-open comparison in
  [ADR-0007](../adr/0007-technology-stack-rust-clap.md) (Go runner-up; decided on
  native cross-platform trash and compiler-enforced safety). Phase 2 core design
  ADRs landed alongside: [ADR-0008](../adr/0008-core-deduplication-algorithm-and-semantics.md)
  (dedup algorithm & safety semantics — BLAKE3 pre-filter + byte-verify authority,
  size pre-grouping, hard-link/TOCTOU handling, keeper rule, exit codes),
  [ADR-0009](../adr/0009-cross-platform-trash-and-recycle.md) (the `trash` crate
  for `--apply`, direct delete for `--delete`), and
  [ADR-0010](../adr/0010-concurrency-and-memory-model.md) (rayon parallel hashing,
  bounded streaming memory, canonical-sort determinism). **Phase 2 architecture is
  largely complete**; Phase 3 implementation (TDD) is next.

- **TASK-0001..0007 — Phase 0 Foundation complete.** Private repo
  `intruder1912/imgdedup`; `main` protected (required checks `markdownlint`,
  `spellcheck`, `structure`, `pr-title`, `secret-scan`; PR-required; 0 approvals;
  linear history; squash-only; auto-delete branches). Agent crew, process docs,
  PRD/ADR/planning structures, and CI/security all in place. SDLC proven by
  smoke-test PR #6 (owner reviewed + merged — self-approval deadlock confirmed
  resolved).

## Housekeeping (owner action, optional)

- **Dependabot PRs** opened during bootstrap: #1, #3, #5 are obsolete (they bump
  actions removed in #6) and can be closed; #2 (amannn) and #4 (checkout) are
  valid version bumps to rebase + merge. (Agents are blocked from acting on
  Dependabot-authored PRs without explicit authorization.)

## Blockers / waiting on owner

- **Resolved 2026-08-05 (was: owner action with TASK-0331's PR) — `main`'s
  required status checks are swapped.** Verified live during TASK-0333(b):
  `gh api …/branches/main/protection/required_status_checks --jq '{strict,
  checks}'` returns `checks`, `ci-gate`, `security`, each pinned to app `15368`,
  with **`strict: false`** (the owner also applied TASK-0333(c) — see its
  backlog row). Nothing is waiting on the owner here. The original entry is
  kept below for the record, since the exact command and its `checks`-vs-
  `contexts` reasoning are the template for the next rename. Note that
  TASK-0333(b) makes this list load-bearing in a second place: `release.yml`'s
  CI-verification step derives the contexts it checks (`ci-gate` + `security`,
  `checks` being aggregated by `ci-gate`) from it, so a future rename must
  update that step too — it reads check-run _results_, not the protection
  settings, so a drift would not fail loudly on its own.

  <details><summary>Original entry (superseded)</summary>

  **Owner action, with TASK-0331's PR, not after it — swap `main`'s required
  status checks.** That PR folds `secret-scan` and `cargo-deny` into a single
  `security` job (TASK-0331(d)), so both of those context names stop existing.
  Branch protection still requires them by exact string, and a required check
  that never reports sits at "Expected — waiting for status to be reported"
  forever, on this PR and every later one. `enforce_admins` is `false`, so the
  owner can still merge through it, but the swap belongs at merge time.
  Verified live via the API on 2026-08-05, the current list is `secret-scan`,
  `ci-gate`, `cargo-deny`, `checks` (`strict: true`), each pinned to app
  `15368` (GitHub Actions). The replacement — three names, since `security`
  absorbs both `secret-scan` and `cargo-deny`:

  ```sh
  gh api -X PATCH \
    /repos/intruder1912/imgdedup/branches/main/protection/required_status_checks \
    --input - <<< '{"strict":true,"checks":[{"context":"checks","app_id":15368},{"context":"ci-gate","app_id":15368},{"context":"security","app_id":15368}]}'
  ```

  This uses the `checks` form rather than the older `contexts[]` one the
  TASK-0330 entry below used, on purpose: per the REST docs, `contexts` is
  deprecated and binds each name to "the app that recently set it", which for
  the brand-new `security` context is _no app_ — it would land unpinned, i.e.
  reportable by any integration holding `checks: write`, which is precisely
  the wrong property for the check that _is_ the ADR-0005 gate. Naming the app
  id explicitly keeps all three pinned to GitHub Actions, matching what is live
  today.

  Read back with `gh api
  /repos/intruder1912/imgdedup/branches/main/protection/required_status_checks
  --jq '{strict, checks}'` (`checks`, not `contexts` — only that view shows the
  `app_id` pinning) and resolve this entry once it confirms the new list. Same
  shape as the manual renames TASK-0330 (`checks`) and TASK-0206 (`linkcheck`)
  each needed; the architect does not change repository settings.
  Note the macOS half of TASK-0331 needs **no** protection change: `rust`
  varies its matrix width by event, `ci-gate` remains the single aggregating
  required check, and none of the per-leg `rust (…)` contexts is required.

  </details>

- `main`'s required checks, verified live via the GitHub API on 2026-08-04 and
  again on 2026-08-05 (post-TASK-0330, pre-TASK-0331): `secret-scan`,
  `ci-gate`, `cargo-deny`, `checks`
  (`strict: true`) — the owner swapped in the consolidated `checks` context
  (absorbing `markdownlint`, `spellcheck`, `structure`, `pr-title`,
  `linkcheck`) at merge time, exactly as TASK-0330 requested, resolving what
  was a blocker here.
- Prior history: required checks as of 2026-08-02 were `ci-gate`,
  `markdownlint`, `spellcheck`, `structure`, `pr-title`, `secret-scan`,
  `cargo-deny`, `linkcheck` — the last two added by the owner live during
  TASK-0206 (2026-08-02), correcting a prior status.md claim that
  `cargo-deny` had already been required since 2026-07-19 (it had not). The
  owner also renamed the `linkcheck (informational)` context to `linkcheck`
  to match TASK-0206's job rename, resolving what was briefly a blocker here.

## Deferred from Phase 0 — tracked as backlog tasks

Every item intentionally deferred during the bootstrap now has a task in
[`backlog.md`](backlog.md) so nothing is lost in prose:

- Technology stack selection — **TASK-0201** (fully-open ADR).
- Licensing decision — **TASK-0202** — **done**: `GPL-3.0-only`
  ([ADR-0014](../adr/0014-project-license-gpl-3.0-only.md), `LICENSE`).
- Full semantic-release wiring + protected-`main` push mechanism — **TASK-0203**
  — **done**: tag + GitHub Release only, no commit back
  ([ADR-0015](../adr/0015-release-publishing-tag-and-github-release.md),
  [`../process/release.md`](../process/release.md)).
- Pin GitHub Actions to commit SHAs — **TASK-0204**.
- Re-evaluate cspell → `typos` — **TASK-0205** — **done**: migrated.
- Promote link check to a required gate — **TASK-0206** — **done**: PR #56.
- Language toolchain (build/lint/fmt/test/coverage/hooks) — **TASK-0207**
  (fulfilled by TASK-0301, merged in PR #14).
- CodeQL + dependency audit — **TASK-0208**.
- Dependabot package ecosystem for the stack — **TASK-0209**.
- Project `.claude/settings.json` permission allowlist — **TASK-0210**.
- Prebuilt binaries on releases — **TASK-0211** — **done**: PR #54.
