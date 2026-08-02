# Project Status

Live snapshot of where the project stands. Update this in every PR. This is the
first file to read when resuming work.

**Current phase:** Phase 3 — Implementation (TDD), executed per the
owner-approved **[batch plan](batch-plan.md)** (2026-07-19): housekeeping →
S2 → test tooling → S3 → S4 + field validation → v0.1.0 private release →
docs pass → AI automation, batched within Claude usage limits.
**Last updated:** 2026-08-02

## In progress

- _Session 8 (docs/tooling consistency) is 2/3 done: TASK-0206 (link check →
  required gate) merged as **PR #56**; TASK-0205 (cspell → typos) is **PR #57**,
  green, rebased onto PR #56, awaiting merge. **TASK-0217 (docs consistency
  pass) deliberately deferred to a fresh window** — usage checked in mid-session
  (5h window 34% → 45% after TASK-0206+TASK-0205, ~11% combined, tracking the
  "docs chore ≈10%" cost model closely) with the largest, product-owner-led
  unit of the session still ahead; stopping here rather than starting it
  against 45% already spent. TASK-0217 also carries its own stderr-diagnostic-
  prefix unification nit (`cli.rs::validate()` bakes "error: " into its own
  message instead of `main.rs` adding it uniformly like every other stderr
  writer does) — small, but worth doing in the same unit._

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
**8–9 (fresh window each):** docs consistency (TASK-0217/0205/0206), AI
automation (TASK-0218).

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
- TASK-0219 (public-release licensing readiness) still available in
  parallel. New follow-up from TASK-0205: **TASK-0327** — `Makefile`'s
  `links` target still uses `markdown-link-check`, diverging from CI's
  lychee-based gate and not wired into `validate` at all.

## Done

- **Session 8 — docs/tooling consistency (2026-08-02).** Three small
  independent units, each its own PR (per the one-unit-one-PR chore cadence).
  **TASK-0206 (PR #56):** `linkcheck` promoted from informational to a
  required gate (`fail: true`, renamed from `linkcheck (informational)`).
  Flipping `fail: true` immediately surfaced a real, previously-invisible
  defect: `lychee.toml` failed to _load_ under the pinned lychee v0.24.2
  (`exclude_mail` was removed from its config schema — the same class of
  drift as the markdownlint-cli2 issue from Batch 0); with `fail: false` that
  load failure was silently swallowed, so link checking had never actually
  run in CI, only reported green. Fixed and verified locally with the exact
  pinned version: 439 links, 438 OK, 0 errors. Also corrected a stale claim
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

- _None currently._ `main`'s required checks, verified live via the GitHub
  API on 2026-08-02: `ci-gate`, `markdownlint`, `spellcheck`, `structure`,
  `pr-title`, `secret-scan`, `cargo-deny`, `linkcheck` — the last two added by
  the owner live during TASK-0206 (2026-08-02), correcting a prior status.md
  claim that `cargo-deny` had already been required since 2026-07-19 (it had
  not). The owner also renamed the `linkcheck (informational)` context to
  `linkcheck` to match TASK-0206's job rename, resolving what was briefly a
  blocker here.

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
