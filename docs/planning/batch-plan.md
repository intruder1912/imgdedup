# Batch plan — housekeeping to first release (2026-07 → 2026-08)

Owner-approved execution schedule (2026-07-19, **re-planned 2026-08-01**)
taking `imgdedup` from the post-S1 state to a first private release. It
overlays the [roadmap](roadmap.md) phases and the [backlog](backlog.md) tasks
with an explicit **batch order sized to Claude usage limits** (Pro tier:
5-hour rolling window + weekly caps, with a separate Fable 5 pool). This file
is the schedule; the backlog stays the canonical task list.

## Owner decisions recorded here

- **License: GPL-3.0** (TASK-0202 ADR to formalize). Repo **stays private for
  now**; going public is a later, separate decision. First release is a
  private GitHub release.
- **Test oracle = generated golden set, not real photos.** Correctness tests
  need known ground truth, so a deterministic generator produces tiny valid
  images per supported format plus duplicate/near-miss trees with an expected
  manifest (TASK-0313). Real photos serve only as a **read-only field
  validation** on a copied subset (~500–1k files) before release (TASK-0314);
  originals are never touched.
- **AI automation support** ships as an in-repo Claude **skill, slash command,
  and custom operator agent** (TASK-0218): dry-run-first, JSON-based, never
  `--delete` unattended.
- **Progress indication (FR-19, 2026-07-19), three decisions** — from the owner's
  manual field feedback that the running tool produces no terminal output. Now
  specified in the
  [PRD-0001 Amendment 2026-07-19](../prd/PRD-0001-exact-image-deduplication.md#amendment-2026-07-19--progress-indication--stored-results),
  built as batch 4b / **TASK-0315**:
  1. **Progress UX** — a live indicator on **stderr** (phase, `x of y`, %),
     **default-on**, auto-disabling its in-place render when stderr is not a TTY
     (falling back to occasional plain, log-friendly lines), with `--no-progress`
     to force it off. Stdout stays pure in all modes (including `--json`).
  2. **Stored results = shell redirection, no new flag.**
     `imgdedup … --json > result.json` — stdout carries only the report/JSON,
     diagnostics + progress go to stderr. A dedicated `--output` flag was
     **considered and explicitly deferred** (a PRD non-goal, not a task).
  3. **Scheduling** — spec now, **build in a fresh usage window as its own unit**;
     do **not** start implementation ahead of that window.

## Limits governance (all batches)

- Each batch ≈ one session, estimated as % of one Pro 5-hour window below.
- Start of batch: check the estimate against what remains in the window/week
  (both pools). End of batch: reconcile with `/usage` actuals and re-scope the
  remaining batches.
- Every batch keeps a mid-point cut line (a commit leaving the branch
  coherent) in case usage trends over the estimate.
- **Session fill target 60–70% of the 5-hour window, hard stop at 75%** —
  keeps the ~25–30% margin the global rule requires while still paying the
  fixed per-session re-entry cost (reload planning docs, rebase, first CI
  round trip) only once per unit of progress.
- Subagent fan-out is expensive; prefer inline work. Reserve Fable 5 for
  design- and review-heavy steps; use Sonnet for mechanical TDD/doc work.
- **At most one architect dispatch per feature unit** (review + confirm in the
  same dispatch). Batch 4 landed at 51% against a 60–80 estimate largely
  because it worked inline with a single review dispatch.

### Measured cost model (from the batches 0–4 actuals)

| Unit shape | 5h window | Weekly points | Evidence |
|---|---|---|---|
| Docs-only / small chore | ~10% | ~1 | batches 0–2 (four chores fit one window) |
| Feature unit, inline, one review | ~50% | ~8 | batch 4 (TASK-0313) |
| Feature slice (build + acceptance/docs/review) | ~70% | ~8–9 | batch 3 (S2, across two windows) |

Derived rule of thumb: **one full 5-hour window ≈ 10–16 weekly points**, so a
fresh week supports roughly **6 full windows**, i.e. **8–9 batches at the
60–70% fill target**. Feature work tracks its estimate closely; chore work
consistently comes in under.

### Actuals log

- Batch 0 — 26% of 5h window (est. 15–25; one extra CI
  round-trip for the cspell@10 Node-22 requirement). Week of 2026-07-19 after
  batch 0: 27% general / 23% Fable 5. Batch 1 part 1 (PR #31) — 11% (window
  at 37%, week 28/25). Both batch-1 halves ran in the same window as batch 0,
  well under the 50–70 estimate — chore batches are cheaper than planned;
  feature-slice estimates stay as-is until measured. Batch 1 part 2 (PR #34) —
  11% (window 48%, week 29/26). Batch 2 refresh half (PR #35) — 10% (window
  58%, week 30/28). Four batches' worth of chores fit one window; the
  resilience review + S2 start next window. Batch 2 final (resilience review +
  F1 fix, PR #37) — ~13%; S2 core build (parked on `feature/json-output`) —
  ~20% (window closed at 97%, week 32/31). S2 finish (acceptance, docs,
  review, PR #38, merged 2026-07-19) — ~50% of the next window (week 39/40
  after merge). Batch 3 total ≈ 70, inside its 60–80 estimate — feature
  slices track estimates far closer than chores did. Week remaining ≈ 60%
  both pools: batch 4 (est. 60–80) starts a fresh window; at 2–3
  batches/week the week supports roughly one more batch after it. Batch 4
  (PR #41, merged 2026-07-19) — **51%** of its window (est. 60–80; leaner
  because the unit was inline tester work with a single architect
  review+confirm dispatch and CI passed first try). Week after merge:
  47% general / 50% Fable 5 used. Re-scope: batch 4b (est. 45–60) does
  **not** fit the remaining 49% of this window with margin — per the
  owner decision it starts its own fresh window; the week supports it
  (~10–12 weekly points at batch-4's observed burn rate). **Addendum (missed
  in the #42 sync):** the FR-19 specification units — PR #39 (planning flip,
  ~0) and PR #40 (the PRD-0001 progress amendment, **~28%** against a ~15%
  estimate) — say to budget **25–30% for spec/amendment units**, not 15%;
  that window closed at 78% (week 42/42). **2026-08-01 —
  owner reports both pools at 0% (5-hour window and both 7-day caps)** after
  an idle stretch since the batch-4 merge; the re-plan below schedules the
  remaining batches against a full fresh week.
- **Session 1 (PR #46 re-plan + PR #47 batch 4b), 2026-08-01 — 100% of the
  5-hour window against a 53–72 estimate, running into overage; the week
  finished at just 10% (all models).** Two corrections fall out of this, both
  reversing assumptions in the re-plan above:
  1. **The 5-hour window is the binding constraint, not the weekly cap.** One
     full window costs ~10 weekly points — the bottom of the 10–16 estimate —
     so the week affords ~9 more full windows. Weekly capacity is not what
     needs managing; per-session sizing is.
  2. **A subagent-driven feature unit costs roughly 2× the same unit worked
     inline.** Batch 4 was inline with a single architect dispatch: 51%.
     Session 1 ran ~1.65M subagent tokens across eight dispatches and resumes
     (implementer ×4, architect ×2, product-owner ×2, tester ×1) and hit 100%.
     Model split of the spend was 56% Sonnet / 44% Opus with a 96% cache-hit
     rate, so the cost is the number of agent round-trips, not the model tier.
     This quantifies the "prefer inline work" rule that was already in this
     file but was unpriced.

  The quality return was real — the review rounds found 21 findings including
  three user-visible defects a green suite had missed — so the lesson is not
  "stop using subagents" but "budget them explicitly". **Revised sizing for the
  remaining sessions: default to inline TDD plus one architect dispatch, and
  spend extra dispatches only on concurrency- or safety-critical code.** S3
  (destructive actions) is the one place that clearly warrants two, which is
  why its estimate rises below.
- **Session 2 (PR #48 TASK-0316 + the parked S3 core), 2026-08-01 — 47% of the
  window against a 55–75 estimate; the week moved 10% → 15%.** The inline
  re-pricing is confirmed on both axes:
  1. **Inline work costs roughly what batch 4 did.** Session 2 delivered a
     complete merged unit *and* the S3 core on two subagent dispatches (one
     product-owner ruling, one architect review) versus session 1's eight, for
     less than half the window. Estimates for inline units can stay where they
     are; it was the subagent multiplier that was priced wrong, not the units.
  2. **~10 weekly points per full window holds.** 47% of a window cost 5
     weekly points, matching session 1's 100% → ~10. Weekly capacity remains
     ample (85% left ≈ 8 windows); the per-session window stays the constraint.
- **Session 3 (PR #49, slice S3 finish), 2026-08-01 — 55% of the window against
  a 60–75 estimate; the week moved 15% → 20%.** Third session in a row under
  estimate on the inline model, and the ~10-weekly-points-per-window ratio holds
  for a third data point. Two things worth carrying forward:
  1. **A review of safety-critical code generates more rework than the estimate
     assumes.** The whole-slice architect review returned 17 findings over ~1000
     lines of destructive code, and working them was the larger half of the
     session — more than the build itself. It was worth it (three of the
     findings were user-visible defects, one a panic that would have aborted
     mid-delete), but "review + fix round" should be budgeted as one unit, not
     as a review with a tail.
  2. **The estimate was raised for two architect dispatches and only one was
     spent.** The single whole-slice review, run after the product-owner had
     accepted and the docs were written, found everything an in-progress review
     would have — because it could exercise the finished binary. For a slice
     whose risk is behavioral rather than architectural, one late review beats
     two early ones.
- **Sessions 4+5 (PR #50, S4 acceptance + scale), 2026-08-02 — combined into one
  session/PR against a 100–130 combined estimate; the week moved 20% → 25% (~5
  weekly points, well under the ~16 combined estimate for two sessions).** Two
  things worth carrying forward:
  1. **A pure-test unit (no product code) is far cheaper than a feature-slice
     build.** S4 added `tests/acceptance.rs` + `tests/scale.rs` only — no
     `src/` changes — so it tracked closer to the "feature unit, inline, one
     review" cost model (~50%/~8 weekly) than to two separate slice-sized
     sessions. Test-only units should be priced with their own line in the
     cost model rather than reusing the feature-slice estimate.
  2. **TASK-0314 (real-photo field validation) slipped out, as the plan
     anticipated.** It needs the owner's ~500–1k photo corpus, which has not
     been supplied; it is not on the session 6/7a critical path (only the
     v0.1.0 tag in 7b needs it) and can land whenever the corpus is ready.
- **PR #51 (docs sync for the S4/PR #50 merge) + session 6 (batch 7a — PR #52,
  TASK-0202 + TASK-0203), 2026-08-02 — 50% of the window combined against a
  50–65 estimate for session 6 alone (i.e. squarely at the low end once the
  free docs-sync is folded in); the week moved 25% → 30% (~5 weekly points,
  same order as prior chore sessions).** Scope delivered: ADR-0014, `LICENSE`,
  and the `Cargo.toml`/README/CONTRIBUTING/`deny.toml` reconciliation; ADR-0015,
  `.releaserc.json`, `release.yml`, `package.json`/`package-lock.json`,
  Dependabot npm, `CHANGELOG.md`, `docs/process/release.md`, and the ADR-0004
  loop-closure; two planning syncs (#51, and this unit's own). Two things worth
  carrying forward:
  1. **This chore ran two architect dispatches (author, then an independent
     review-only pass), not the usual one** — deliberately, because it's CI
     automation carrying a write-permission `GITHUB_TOKEN` and CLAUDE.md's
     security gate (rule 3) applies regardless of unit size. It still landed
     at the low end of the estimate, so **security-sensitive chores can afford
     the second dispatch without blowing the window** — budget for it rather
     than defaulting to one.
  2. **The expensive part was verification, not authoring.** Reading
     semantic-release's source, querying the live branch-protection and
     ruleset APIs, and running the tool in `--dry-run` against `main` is what
     turned two plausible assumptions into facts — one of which (ADR-0004's
     "protection blocks the tag push") was wrong, and another of which (the
     first release computing as v1.0.0) would have published a wrong version
     on merge. Budget chore units that touch release or security plumbing for
     a verification pass, not just for writing.

## Re-plan 2026-08-01 — both limit pools fresh

The owner reported **0% used on both the 5-hour window and the 7-day caps**
(general and Fable 5) on 2026-08-01, after an idle stretch since the batch-4
merge on 2026-07-19. `main` is green and no PRs are open, so the remaining
plan starts from a clean slate. Two consequences:

1. **The weekly cap is now the binding constraint, not the 5-hour window.** A
   full week affords ~6 windows of work, and the whole remaining path to
   v0.1.0 (batches 4b → 7b) costs ~4 windows ≈ **40–65 weekly points**. The
   release is therefore reachable **within this week** with margin — provided
   sessions stay at the fill target rather than sprawling.
2. **Prefer fewer, fuller sessions.** Each session pays a fixed re-entry cost;
   splitting the same work across more, thinner sessions raises the weekly
   burn without buying window safety.

The two remaining batches that were sized above one comfortable session
(batch 5 at `2 × 60`, batch 6 at `70–90`) are **split into explicit halves**
(5a/5b, 6a/6b), and the release batch is split into plumbing (7a) and the
tagged release (7b), so that every scheduled session fits the 60–70% target.

## The batches

Done history first, then the re-planned sessions in execution order. "Weekly"
is the estimated cost in 7-day-cap points, per the measured cost model above.

| # | Batch | Unit | Tasks | Est. 5h % | Status |
|---|---|---|---|---|---|
| 0 | Unblock `main` + this plan | chore | TASK-0215 | 15–25 | done (PR #30; actual 26%) |
| 1 | Security & supply chain | chore | TASK-0204, TASK-0208, TASK-0209, TASK-0210 | 50–70 | done (PRs #31, #34; actual ~25 across both) |
| 2 | Versions, stability & resilience | chore | TASK-0216 | 50–70 | done (PRs #35 + F1-fix PR; actual ~25 total; review found 1 defect, fixed) |
| 3 | S2 — JSON output | feature slice | TASK-0308 | 60–80 | done (PR #38; actual ~70 across two windows) |
| 4 | Test tooling & golden set | feature | TASK-0313 | 60–80 | done (PR #41; actual 51%) |

### Scheduled sessions (from 2026-08-01)

| Session | Batch | Unit(s) | Tasks | Est. 5h % | Est. weekly | Cut line |
|---|---|---|---|---|---|---|
| 1 | re-plan + 4b | chore, then feature | this file (PR #46), then TASK-0315 | 8–12 + 45–60 | ~9 | **done 2026-08-01** — both units landed, but at **100%** of the window (subagent-driven; see the actuals log) |
| 2 | 4c + 5a | fix-or-docs, then feature slice | TASK-0316, then TASK-0309 | 10–20 + 45–55 | ~7 | **done 2026-08-01** — PR #48 merged, `feature/apply-delete` parked at the cut line; actual **47%** |
| 3 | 5b | feature slice (finish) | TASK-0310 + S3 acceptance, docs, whole-slice review, PR | 60–75 (safety-critical: budget **two** architect dispatches) | ~8 | **done 2026-08-01** — PR #49; actual **55%** |
| 4 | 6a | feature slice | TASK-0312 (AC-1..AC-19 through the real CLI, incl. the deferred AC-19 `--exit-code` binary test) | 50–60 | ~8 | **done 2026-08-02** — landed together with session 5 in PR #50 |
| 5 | 6b | feature slice (finish) | NFR-4 scale/benchmark + TASK-0314 field validation + doc reconciliation, review, PR | 50–70 | ~8 | **done 2026-08-02 — PR #50** (AC's + NFR-4 measurement + architect review); **TASK-0314 field validation slipped as anticipated** (needs the owner's photo corpus, not yet supplied) — no user-facing doc reconciliation was needed since S4 added no product code |
| 6 | 7a | chore | TASK-0202 (GPL-3.0 ADR + `LICENSE`), TASK-0203 (semantic-release wiring, protected-`main` push mechanism) | 50–65 | ~7 | **done 2026-08-02** — ADR-0014 (`GPL-3.0-only` + `LICENSE`) and ADR-0015 (tag + GitHub Release only; no bot/PAT/bypass needed); see the actuals log |
| 7 | 7b | chore/feature | TASK-0211 prebuilt binaries; cut **v0.1.0** (private) | 40–55 | ~6 | `v0.0.0` baseline tag **pushed and verified 2026-08-02**; **next** — TASK-0211 itself, in a fresh window (session 6 closed this one at ~50%) |
| 8 | 8 | chore | TASK-0217, TASK-0205, TASK-0206 | 40–60 | ~5 | next week |
| 9 | 9 | feature | TASK-0218 | 50–70 | ~7 | next week |

**Week 1 target (2026-08-01 →): sessions 1–7, i.e. v0.1.0 shipped**, at an
estimated 40–65 weekly points against a 100-point cap. **Week 2: sessions
8–9** (docs consistency + AI automation), which deliberately sit after the
release so they document and automate final behavior.

### Checkpoints and the cut line for the week

- Reconcile `/usage` after **every** session and append to the actuals log.
- **Trigger (revised after session 1):** the weekly cap is no longer the live
  risk — one window costs ~10 weekly points, so the week comfortably holds the
  remaining sessions. The real trigger is per-session: **if a session passes
  60% of its window with the unit not yet reviewable, stop at the cut line**
  rather than pushing into overage as session 1 did. Sessions 6a/6b and 7a/7b
  keep their order regardless; the current week resets 2026-08-04.
- **Trigger:** if a session passes 75% of its window mid-unit, stop at the cut
  line in its row above and resume in the next window — never run into the cap
  mid-review.
- TASK-0316 (session 2) is sized 10–20% because the outcome is unknown until
  the product-owner decides: a wording narrowing is a docs-only `fix`, while
  surfacing lone hard-linked sets touches the engine, the JSON schema, and the
  golden manifest. If it turns out to be the engine path, it is its own unit
  and batch 5a slips to session 3, cascading the rest by one session.

## Ordering constraints

- Batch 0 first — `main` is red (unpinned markdownlint drift) and blocks every
  PR, including Dependabot #29.
- Batches 1–2 before 5 — supply-chain and dependency hardening land before the
  destructive-actions code.
- Batch 3 before 4 — the test oracle diffs `--json` output.
- Batch 4 before 6 — S4 uses the generator for AC/scale coverage.
- Batch 7 after 6 — release gate is S4 green plus field validation.
- Batch 9 after 3 — the operator agent drives the tool via `--json`.
- Batch 8 can slot anywhere after 7's docs impact is known; kept late so the
  pass covers final behavior.
- **Batch 4b (progress, FR-19) after batch 3** — progress must coexist with the
  `--json` stdout-purity contract shipped in S2 (batch 3), so it starts only once
  JSON output has landed (depends on TASK-0308). It is **independent of the
  S3/S4 release-critical path** and can float relative to batches 4/5/6 — but it
  should land **before batch 7 (v0.1.0)** so the first release ships decent
  interactive UX. The owner reorders at will; per the owner it is scheduled to
  build in a **fresh usage window as its own unit** (do not start ahead of that).
  The stored-results doc note (redirection) documents existing behavior and ships
  now, decoupled from this batch.

- The 2026-08-01 splits inherit their parent's constraints: 5a → 5b and
  6a → 6b are strictly sequential halves of one slice branch (one PR each for
  S3 and S4, not one per half), and 7a precedes 7b because the release job
  cannot be tagged before the licensing ADR and the publishing mechanism land.

Slice mechanics (branching, in-progress reviews, PR-open gates) are unchanged
from [ADR-0013](../adr/0013-prd-slice-delivery-cadence.md); this plan only
fixes the order and the usage-limit envelope.
