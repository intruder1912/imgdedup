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
- **PR #53 (docs sync recording the v0.0.0 tag push) landed the same day as
  session 6, ~0% marginal cost** — a purely factual planning sync, no design
  change, architect dispatch skipped per the owner-endorsed cost/benefit call
  ([[review-cost-benefit]]). Its own CI run going green then triggered the
  release workflow, which found the newly-bootstrapped `v0.0.0` tag and
  **published v0.1.0 automatically** — correct behavior per the bootstrap
  design, but a session-7 surprise: the release existed with zero assets
  before TASK-0211 had shipped anything to attach.
- **Session 7 (7b — PR #54, TASK-0211), 2026-08-02 — session started at 0%/31%
  (window/week) and closed at 32%/34%: ~32% of the window against a 40–55
  estimate (comfortably under, despite the added scope below), ~3% weekly,
  in line with the ~10-weekly-points-per-full-window ratio (32% × ~10 ≈ 3).**
  Two architect dispatches (design, then an independent review-only pass —
  security-sensitive release automation gets the second dispatch per the
  session-6 precedent, and it again found real issues: an inverted
  null-coercion comment and a tag-name shell-injection channel, both fixed in
  the review). Discovering v0.1.0's early publish mid-session added scope not
  in the original estimate — a `workflow_dispatch` backfill path, plus the
  actual backfill run and its live verification (download, checksum, extract,
  run `--version`) — and it still landed well under estimate, so "handle a
  release that already exists" is cheap in practice even though it wasn't
  priced in; no estimate revision needed on that basis alone. Merging PR #54
  then triggered a **second real release** (v0.2.0, `feat:`-typed commit) the
  same session, whose `assets` job ran fully automatically end-to-end with no
  manual step — the automatic path is now proven, not just the manual one,
  at effectively zero extra cost (it ran on GitHub's infrastructure, not this
  session's usage).
- **Session 8 (partial — PRs #56, #57), 2026-08-02 — window continued from
  34% (no fresh window this time) to 45% after TASK-0206 + TASK-0205: ~11%
  combined, matching the "docs chore ≈10%" line of the cost model almost
  exactly despite one of the two units (TASK-0206) surfacing and fixing a
  real, previously-invisible defect (`lychee.toml` had never actually loaded
  under the pinned lychee version).** Both units skipped architect dispatch
  (low-risk, `contents: read`-only CI config, evaluated/verified directly)
  — the cost/benefit call held for chore-weight CI tooling changes, not just
  for docs-only planning syncs. **TASK-0217 (docs consistency pass,
  product-owner-led) deliberately deferred**, per an explicit mid-session
  owner check-in: the batch plan's own trigger ("if a session passes 60% of
  its window with the unit not yet reviewable, stop at the cut line") was
  read forward rather than waited for — 45% with the largest, least-defined
  remaining unit still ahead was judged close enough to justify stopping
  before starting it, rather than mid-way through it. Two things worth
  carrying forward: (1) **stacking chores onto a non-fresh window still
  tracks the cost model fine** — the 34%→45% delta prices the same as if it
  had started fresh; the model prices *units*, not *sessions*. (2) **A
  mid-session usage check-in before the largest unit of a multi-unit session
  is a cheap, worthwhile pattern** — it turned an implicit judgment call
  ("is 45% too much to start a ~20-40%-estimated unit against") into an
  explicit one, at the cost of one round-trip.

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
| 7 | 7b | chore/feature | TASK-0211 prebuilt binaries; cut **v0.1.0** (private) | 40–55 | ~6 | **done 2026-08-02 — PR #54.** v0.1.0 had already auto-published (with no assets) via the workflow this session extends; the session built the `assets` job, added a `workflow_dispatch` backfill path, and used it once to attach binaries to v0.1.0 |
| 8 | 8 | chore, then product-owner | TASK-0206 + TASK-0205 done (PRs #56, #57); TASK-0217 deferred | 40–60 | ~5 | **in progress 2026-08-02** — TASK-0206 + TASK-0205 done at 45% of the window (started 34%, ~11% combined); TASK-0217 deferred to a fresh window per mid-session cut-line check-in |
| 9 | 9 | feature | TASK-0218 | 50–70 | ~7 | **done 2026-08-02 — PR #60.** Whole-branch architect review found 2 Critical + 6 Important issues (all fixed and re-verified — see PR notes); one residual Important finding introduced by the fix wave itself was closed in a follow-up fix round |

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

## Re-plan 2026-08-03 — post-v0.1.0 backlog cleanup (sessions 10+)

New window opened 2026-08-03 at **5h=0%, 7d=46%**. All release-critical work
(sessions 1–9, v0.1.0 through the AI automation plugin) is done; the remaining
backlog is independent hardening/perf/nit items with no release gate forcing
an order. Owner set an explicit priority: **321, 318, 317, 323, 327,
324/325, 319, 322, 326, 320, 314** — used as the base ordering below, adjusted
only where a real dependency requires it (TASK-0317's sizing note says to
re-measure its indicator after the scan stage changes shape, so it stays
directly after TASK-0321 rather than being reordered around it, matching the
owner's list). Each session below targets the existing **60–70% fill / 75%
hard-stop** governance from this file; the owner's instruction for this
re-plan is a looser **90% ceiling**, so 75% remains the working target and 90%
is treated as the absolute stop line, not the aim.

| Session | Unit | Tasks | Why grouped / order | Est. 5h % | Est. weekly |
|---|---|---|---|---|---|
| 10 | feature (perf), concurrency-sensitive | TASK-0321 — parallelize the scan stage (`sniff::filter_candidates`) — **done, PR #62, actual 20%** | Solo: touches ADR-0010's concurrency model and is the throughput lever TASK-0317 depends on being measured correctly; budget the extra architect dispatch the concurrency-critical-code rule reserves | 55–75 | 7–9 |
| 11a | fix | TASK-0318 (sanitize control chars in stderr **and stdout** paths) — **done, PR #64, actual 23%** | Quick security nit — grew mid-review to cover `report.rs`'s stdout sites too (see the actuals note below), still landed cheaply | 20–35 | 3 |
| 11b | feature | TASK-0317 (animate the `scanning` phase, sized post-321 on a release build) — **done, PR #66, actual 26%** | Split out of session 11 (see actuals note): 43% of the window was already spent on 11a by the time it would start, too close to the 75% hard stop to begin a feature unit with its own re-sizing uncertainty | 35–55 | 5–6 |
| 12 | product-owner fix + chore ×2 | TASK-0323 (exit-code doc/code reconciliation) — **done, PR #68, actual 14%** + TASK-0327 (fold `make links` onto lychee) + TASK-0324/0325 (release.yml deprecation + trigger-condition nits, bundled) — carried to session 13 | Three small, independent, low-risk items; 324/325 touch the write-permission release job so keep the second-dispatch precedent from session 6 even at this size | 45–65 | 5–6 |
| 13 | chore ×2 (carried from 12) | TASK-0327 (fold `make links` onto lychee, wire into `validate`) — **done, PR #70** + TASK-0324/0325 (release.yml deprecation + trigger-condition nits, bundled) — **done, this PR** | TASK-0327 is docs/tooling-only, no write-permission CI — architect dispatch skipped per the ADR-0011 exception; TASK-0324/0325 touch `release.yml`'s write-permission job, so keep the second-dispatch precedent | 10–20 (327 alone) + 15–25 (324/325) | 2–4 |
| 14 | fix, core-engine | TASK-0319 (dedupe enumerated paths by identity — same-tree-twice bug) | Solo: touches `engine.rs`'s hard-link/identity path, the safety-critical core; do not crowd it with unrelated perf work in the same session | 35–50 | 5 |
| 15 | perf evaluation | TASK-0322 (evaluate `blake3` `update_rayon` for intra-file parallelism) | Solo: "measure before adopting" — outcome-dependent (adopt vs. reject-with-rationale), same class of uncertainty as TASK-0316 | 35–50 | 5 |
| 16 | product-owner decision + fix | TASK-0320 (`--follow-symlinks` candidacy semantics) | Outcome-dependent sizing like TASK-0316: docs-only if the ruling keeps current behavior, an `enumerate.rs` change if it doesn't | 15–40 | 2–5 |

**Not scheduled — blocked, not just ordered:**

- **TASK-0326** (musl/aarch64 binary targets) — the backlog entry itself gates
  this on "the public release, not before," and the repo is still private
  with no public-release date set. Scheduling a session now would build
  ahead of a decision only the owner can make (going public) and that
  TASK-0219 also gates on. Revisit once TASK-0219 / the public-release
  decision is active.
- **TASK-0314** (real-photo field validation) — still blocked on the owner
  supplying the ~500–1k photo corpus; unchanged since sessions 4/5. Cheap
  when unblocked (test-only, read-only unit, ~20–30% / ~3 weekly) and can
  slot into any later window without disturbing this order.

At ~10 weekly points per full window, sessions 10–15 total roughly **30–40
weekly points against the 46% already used this week** — comfortably inside
the 7-day cap even without a reset, so the binding constraint stays per-session
window sizing, not the week. Only session 10 fits in the *current* window from
a cold start (5h=0%); sessions 11–15 each start their own fresh window per the
"prefer fewer, fuller sessions" rule, reconciling `/usage` actuals against
these estimates at the end of every session as usual.

**Session 10 (TASK-0321, PR #62), 2026-08-03 — 20% of the window against a
55–75 estimate, comfortably under; the week moved 46% → 48% (~2 weekly
points).** Landed inline with a single architect dispatch (blocker + 3
should-fixes, converged in one round — see the ADR-0010 amendment and
`status.md`'s session-10 entry). Two things worth carrying forward:

1. **Concurrency-critical sizing is still running high.** This is the third
   data point (after batch 1's security/supply-chain chores and batch 4's
   TASK-0313) where a unit this repo classified as needing extra review care
   landed at roughly a quarter of its estimate. The estimate reserved margin
   for review rework that a well-scoped, single-function change didn't need —
   worth remembering when sizing the TASK-0319 session (also concurrency/
   safety-adjacent, now session 14 after the session-13 insert below) rather
   than mechanically reapplying the 35–50% line.
2. **Plenty of window remains for session 11 in the same window** (20% used,
   target 60–70%, hard stop 75%) — the "sessions 11–15 each start their own
   fresh window" framing above assumed session 10 alone would consume most of
   a window, which did not happen. Per "prefer fewer, fuller sessions,"
   session 11 (TASK-0318 + TASK-0317) continues in this window rather than
   waiting for a fresh one, targeting a combined ~55–75% before the day's
   cut line.

**Session 11a (TASK-0318, PR #64), 2026-08-03 — 23% of the window (20% → 43%)
against a share of the 55–70 combined estimate; the week moved 48% → 50% (~2
weekly points).** Landed inline with a single architect dispatch (2 blockers +
5 nits, converged in one round). Two things worth carrying forward:

1. **Review-driven scope growth stayed cheap.** The architect found the fix's
   real scope was roughly 4× the diff I opened with — `report.rs`'s 12 stdout
   sites, not just `main.rs`'s 3 stderr sites — because a prior review
   ([`reviews/apply-delete.md`](reviews/apply-delete.md)) had already recorded
   that scope against this task and the backlog's "stderr" wording didn't
   reflect it. Landing at 23% despite that expansion says the cost driver for
   this class of fix is "how many call sites share one clear pattern," not
   "how many findings the review returns" — matches the batch-4 lesson that
   inline TDD absorbs rework cheaply when the fix itself is mechanical.
2. **TASK-0317 split out of session 11 rather than attempted at 43%.** The
   session-11 combined estimate (55–70%) assumed both halves; with 11a alone
   already at 43%, starting 11b (a feature unit with its own re-sizing
   uncertainty — see its backlog note) risked either the 75% hard stop or a
   messy mid-unit cut line. Re-scoped as **11b**, its own row above,
   deliberately left for a fresh-window decision rather than pushed into this
   one. `git push`'s `fix:`-typed squash commit auto-published **v0.3.1** —
   the automatic release path continues to fire correctly on every `feat:`/
   `fix:` merge, at zero marginal session cost.

**Session 11b (TASK-0317, PR #66), 2026-08-03 — 26% of the window (43% →
69%) against a 35–55 estimate, comfortably inside it despite the extra
review round; the week moved 50% → 53% (~3 weekly points).** `feat:`-typed
squash commit auto-published **v0.3.2**. One architect dispatch, but not a
clean approve: **2 blockers, both real defects, not nits.**

1. **`set_total` never reset the numerator, so Scanning's "files found"
   count leaked into Hashing** — the whole hashing phase rendered as
   already-100%-complete from its first repaint, an AC-21 violation no
   existing test caught (nothing combined "advance during an indeterminate
   phase, then set_total"). This is the second session in a row (TASK-0321
   was the first) where the review found something a green suite had missed
   by construction, not by chance — worth treating "does any test combine
   these two call sequences" as a standing question when touching shared,
   stateful infrastructure like `progress.rs`.
2. **My own scope-trim write-up was wrong on two counts**, not just
   under-detailed: I claimed a technical constraint (double-counting) that
   `retotal` already solves, and an unmeasured performance claim that
   turned out backwards (sniff is ~90% of the scanning window post-TASK-0321,
   not enumeration). Landing at 26% despite a full rewrite of two planning
   docs plus a new backlog task (TASK-0328) says **honest-write-up rework is
   cheap when caught in the same review round** — the expensive failure mode
   would have been shipping the wrong justification and someone acting on it
   later, not the cost of fixing it now.

The window closed at 69%, at the top of the 60–70% target band — the
session stops here rather than starting a third unit, per the "prefer
fewer, fuller sessions" rule's own boundary.

**Session 12 (TASK-0323, PR #68), 2026-08-03 — 14% of the window (69% →
83%) against a share of the 45–65 combined estimate; the week moved 53% →
54% (~1 weekly point).** `fix:`-typed squash commit auto-published
**v0.4.1**. The owner explicitly chose to continue past session 11b's
stated stopping point (top of the 60–70% target band) rather than wait for
a fresh window — a real-time override of the default governance, not a
process miss; recorded here so the trigger reads as "the default, absent
other direction" rather than a hard rule. One architect dispatch: **2
blockers, both completeness misses** (stale `--help` text rendered from a
`clap` doc comment; ADR-0008, the exit-code scheme's own cited authority,
still describing the pre-ruling behavior) — the same pattern as sessions
10/11b: architect review keeps finding real gaps on small, mechanical-
looking fixes, not just nits. **Session closes at 83%/54%, past the 75%
hard stop** — the owner's own signal ("waiting for next window") is the
actual stop here, not a model-computed one. TASK-0327/TASK-0324/TASK-0325
(session 12's other two planned items) did not start; carry to session 13.

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
