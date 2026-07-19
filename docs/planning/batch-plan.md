# Batch plan — housekeeping to first release (2026-07)

Owner-approved execution schedule (2026-07-19) taking `imgdedup` from the
post-S1 state to a first private release. It overlays the
[roadmap](roadmap.md) phases and the [backlog](backlog.md) tasks with an
explicit **batch order sized to Claude usage limits** (Pro tier: 5-hour
rolling window + weekly caps, with a separate Fable 5 pool). This file is the
schedule; the backlog stays the canonical task list.

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

- Each batch ≈ one session, estimated as % of one Pro 5-hour window below;
  realistic pace is **2–3 batches/week** under the weekly caps.
- Start of batch: check the estimate against what remains in the window/week
  (both pools). End of batch: reconcile with `/usage` actuals and re-scope the
  remaining batches.
- Every batch keeps a mid-point cut line (a commit leaving the branch
  coherent) in case usage trends over the estimate.
- Subagent fan-out is expensive; prefer inline work. Reserve Fable 5 for
  design- and review-heavy steps; use Sonnet for mechanical TDD/doc work.
- **Actuals log:** Batch 0 — 26% of 5h window (est. 15–25; one extra CI
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
  batches/week the week supports roughly one more batch after it.

## The batches

| # | Batch | Unit | Tasks | Est. 5h % | Status |
|---|---|---|---|---|---|
| 0 | Unblock `main` + this plan | chore | TASK-0215 | 15–25 | done (PR #30; actual 26%) |
| 1 | Security & supply chain | chore | TASK-0204, TASK-0208, TASK-0209, TASK-0210 | 50–70 | done (PRs #31, #34; actual ~25 across both) |
| 2 | Versions, stability & resilience | chore | TASK-0216 | 50–70 | done (PRs #35 + F1-fix PR; actual ~25 total; review found 1 defect, fixed) |
| 3 | S2 — JSON output | feature slice | TASK-0308 | 60–80 | done (PR #38; actual ~70 across two windows) |
| 4 | Test tooling & golden set | feature | TASK-0313 | 60–80 | todo |
| 4b | Progress indication (FR-19) | feature | TASK-0315 | 45–60 | todo |
| 5 | S3 — Destructive actions | feature slice | TASK-0309, TASK-0310 | 2 × 60 | todo |
| 6 | S4 — E2E, scale & field validation | feature slice | TASK-0312, TASK-0314 | 70–90 | todo |
| 7 | Release flow → v0.1.0 (private) | chore/feature | TASK-0202, TASK-0203, TASK-0211 | 60–80 | todo |
| 8 | Docs consistency pass | chore | TASK-0217, TASK-0205, TASK-0206 | 40–60 | todo |
| 9 | AI automation support | feature | TASK-0218 | 50–70 | todo |

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

Slice mechanics (branching, in-progress reviews, PR-open gates) are unchanged
from [ADR-0013](../adr/0013-prd-slice-delivery-cadence.md); this plan only
fixes the order and the usage-limit envelope.
