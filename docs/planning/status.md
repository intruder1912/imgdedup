# Project Status

Live snapshot of where the project stands. Update this in every PR. This is the
first file to read when resuming work.

**Current phase:** Phase 3 — Implementation (TDD)
**Last updated:** 2026-06-15

## In progress

- _(none)_

## Up next

**Slice cadence is now in force (ADR-0013).** The remaining PRD-0001 work is no
longer shipped one PR per task: feature delivery is carved into **four coherent
shippable slices**, each a single branch → granular commits → one squash PR/release.
The existing TASK-0305..0312 specs stay as the unit of design/TDD; a slice _groups_
tasks. The full slice map (contained tasks, per-slice user value, and PR-open
acceptance — FRs + the README/`docs/guide/` docs each slice must deliver) lives in
the "PRD-0001 delivery slices" subsection of
[`backlog.md`](backlog.md). The core slice (TASK-0301..0304) already merged
task-by-task under the pre-ADR-0013 cadence.

- **Next slice: S2 — JSON output** (`json-output`; TASK-0308). An independent
  deterministic machine-readable formatter (`--json`) over the same canonical
  model S1 built (groups, keeper, intended actions, totals), enabling scripting.
  Opens its PR once the product-owner has accepted FR-14 + NFR-3 and documented
  the `--json` field schema in `docs/guide/output-and-exit-codes.md` (ADR-0012/0013).
- **Then, in order:** **S3 — Destructive actions** (`apply-delete`; TASK-0309 →
  TASK-0310) → **S4 — E2E acceptance & scale** (`e2e-scale`; TASK-0312,
  tester-led, AC-1..AC-19 and NFR-4 cross-platform proof). S2 and S3 both depend
  only on S1 and could be worked concurrently on separate branches, but the safe
  default ships JSON before the destructive surfaces. Each slice rebases on `main`
  after the previous merges. **The `--exit-code` binary-level E2E test (AC-19) is
  deferred to S4** per the S1 architect review (see
  [`reviews/detect-report.md`](reviews/detect-report.md)).
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

- **Slice S1 — Detect & report (read-only)** (`detect-report`; TASK-0305 +
  TASK-0306 + TASK-0307 + TASK-0311). The first end-to-end usable increment:
  `imgdedup <paths>` now scans, detects exact-duplicate groups, and prints a
  deterministic **dry-run report** (keeper, would-be removals, group/file counts,
  reclaimable space, hard-link notes), with a real `--keep {shortest|oldest|newest}`
  rule and the opt-in `--exit-code`. **Nothing on disk changes.** Delivered:
  - TASK-0305 — real keeper rule in `src/keeper.rs` (ADR-0008 total orders;
    byte-wise/case-sensitive path tie-break, whole-second mtime), replacing the
    engine's `placeholder_keeper_order`; mtime threaded `Candidate` →
    `ConfirmedFile` → `FileRec` from the existing enumeration stat.
  - TASK-0306 — rayon parallel hashing within size cohorts + a canonical sort as
    the sole determinism mechanism (ADR-0010); identical output across thread
    widths. Adds `rayon`.
  - TASK-0307 — `src/report.rs` human-readable report (FR-9/13/17, NFR-6);
    reclaimable space excludes collapsed hard-link members; warnings surfaced.
  - TASK-0311 — `exit_code::compute` pure mapping (0/1/2/3; `--exit-code` → 3 on
    a successful run with duplicates; run-level error 1 takes precedence).
  - **Anchor rule fix (ADR-0008 Amendment 2026-06-15):** a collapsed representative
    whose identity set had ≥2 in-scope members is now an **anchor** — never
    removable, never in reclaimable. `DuplicateGroup` gains `pub protected:
    Vec<FileRec>`; `--keep` selects the keeper from genuinely-removable non-anchor
    candidates only; the report renders a `PROTECT` row per anchor. All-anchor
    groups emit no `DuplicateGroup` (0 reclaimable). Engine and report fully
    TDD'd; 10 new tests (engine + report), 151 unit + 14 integration tests green.
  Two architect reviews (in-progress + final APPROVE) under the ADR-0006
  resolve-all gate; the in-progress review found and resolved a hard-link-note
  determinism blocker (B1) and an orphaned-note edge case (S1). Product-owner
  accepted FR-7/8/9/13/15/17/18 + NFR-3/4/6 and synced the README +
  `docs/guide/` pages. `make validate` passes. Review log:
  [`reviews/detect-report.md`](reviews/detect-report.md). **`--exit-code`
  binary-level E2E (AC-19) deferred to S4.**

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
