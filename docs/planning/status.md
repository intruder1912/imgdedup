# Project Status

Live snapshot of where the project stands. Update this in every PR. This is the
first file to read when resuming work.

**Current phase:** Phase 3 — Implementation (TDD), executed per the
owner-approved **[batch plan](batch-plan.md)** (2026-07-19): housekeeping →
S2 → test tooling → S3 → S4 + field validation → v0.1.0 private release →
docs pass → AI automation, batched within Claude usage limits.
**Last updated:** 2026-07-19

## In progress

- **Batch 2 (refresh half) / TASK-0216 — toolchain & dependency refresh
  (this PR).** Rust 1.93.1 → **1.97.1** (latest stable), edition 2021 →
  **2024** (`cargo fix --edition` needed zero source changes; new style
  edition applied via rustfmt; one new let-chains-era clippy lint fixed by
  collapsing a redundant nested `if` in `engine.rs` — the outer `any` check
  duplicated the inner `find`). All direct deps confirmed already at latest
  stable (crates.io check); `cargo update` refreshed 26 transitives and
  slimmed the lockfile 114 → 87 packages (an obsolete wasm/wit build-tooling
  tree dropped out). fmt/clippy `-D warnings`/tests/deny all green. **The resilience/error-path review half of TASK-0216 runs next
  window** per the batch plan's usage-limit split.

## Up next

Batch order per the [batch plan](batch-plan.md): Batch 1 (security & supply
chain: TASK-0204/0208/0209/0210) → Batch 2 (dependency/toolchain refresh +
resilience review: TASK-0216) → then the feature slices below. Owner decisions
2026-07-19: **license = GPL-3.0** (repo private for now), test oracle =
generated golden set (TASK-0313), real photos only for read-only field
validation (TASK-0314), AI automation = skill + command + operator agent
(TASK-0218).

**Slice cadence is now in force (ADR-0013).** S1 (detect-report) is shipped. Remaining PRD-0001 work is delivered as **three remaining coherent shippable slices** (S2, S3, S4), each a single branch → granular commits → one squash PR/release. The full slice map (contained tasks, per-slice user value, and PR-open acceptance) lives in the "PRD-0001 delivery slices" subsection of
[`backlog.md`](backlog.md).

- **Next slice: S2 — JSON output** (`json-output`; TASK-0308). Independent deterministic machine-readable formatter (`--json`) over the same canonical model S1 built (groups, keeper, intended actions, totals), enabling scripting. Opens PR once product-owner accepts FR-14 + NFR-3 and documents `--json` field schema in `docs/guide/output-and-exit-codes.md` (ADR-0012/0013).
- **Then, in order:** **S3 — Destructive actions** (`apply-delete`; TASK-0309 + TASK-0310) → **S4 — E2E acceptance & scale** (`e2e-scale`; TASK-0312, tester-led, AC-1..AC-19 and NFR-4 cross-platform proof). S2 and S3 both depend only on S1 and could be worked concurrently on separate branches, but safe default ships JSON before destructive surfaces. Each slice rebases on `main` after previous merges. **The `--exit-code` binary-level E2E test (AC-19) is deferred to S4** per S1 architect review.
- **In-progress reviews (ADR-0013):** within a slice the implementer requests an
  architect review for significant/risky changes (not per-commit; the
  product-owner or orchestrator may demand one), logged in
  `docs/planning/reviews/<slice>.md`. Uncertain now-vs-defer calls go to the
  product-owner, and deferrals are promoted to tracked backlog tasks.
- **TASK-0207 fulfilled by TASK-0301.** TASK-0208 reframed to clippy, cargo-audit,
  and cargo-deny (see ADR-0007). TASK-0209 (cargo Dependabot), TASK-0203/TASK-0211
  (release and prebuilt binaries) remain.
- Stack-independent Phase 2 tasks still available in parallel: TASK-0202
  (licensing ADR), TASK-0204 (SHA-pin actions), TASK-0205 (cspell→typos),
  TASK-0206 (link-check gate), TASK-0210 (`.claude/settings.json`).

## Done

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

- _None currently._ (ADR-0011 branch-protection swap completed 2026-06-14; `main`
  now requires `ci-gate` plus the doc/security checks.)

## Deferred from Phase 0 — tracked as backlog tasks

Every item intentionally deferred during the bootstrap now has a task in
[`backlog.md`](backlog.md) so nothing is lost in prose:

- Technology stack selection — **TASK-0201** (fully-open ADR).
- Licensing decision — **TASK-0202** (ADR before any public release).
- Full semantic-release wiring + protected-`main` push mechanism — **TASK-0203**
  (see [`../process/release.md`](../process/release.md), ADR-0004).
- Pin GitHub Actions to commit SHAs — **TASK-0204**.
- Re-evaluate cspell → `typos` — **TASK-0205**.
- Promote link check to a required gate — **TASK-0206**.
- Language toolchain (build/lint/fmt/test/coverage/hooks) — **TASK-0207**
  (fulfilled by TASK-0301, merged in PR #14).
- CodeQL + dependency audit — **TASK-0208**.
- Dependabot package ecosystem for the stack — **TASK-0209**.
- Project `.claude/settings.json` permission allowlist — **TASK-0210**.
- Prebuilt binaries on releases — **TASK-0211**.
