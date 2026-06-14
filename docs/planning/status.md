# Project Status

Live snapshot of where the project stands. Update this in every PR. This is the
first file to read when resuming work.

**Current phase:** Phase 2 — Architecture & toolchain (largely complete; Phase 3 next)
**Last updated:** 2026-06-14

## In progress

- **Phase 2 architecture ADRs for PRD-0001** — this PR. ADR-0007..0010 author the
  technology stack and the core design that unblocks implementation
  (see Done below). Doc bookkeeping (PRD/ADR index/planning) updated in the same PR.

## Up next

- **Phase 3 — Implementation (TDD).** With ADR-0007..0010 accepted, the next step
  is to **seed fine-grained, parallelizable Phase 3 implementation tasks** (from
  the ADR-0008 pipeline: enumerate → sniff → size-group → hash → byte-verify →
  identity → keeper → report/apply; plus the ADR-0010 executor and an NFR-4
  benchmark task) and begin TDD. That task-seeding happens right after this PR.
- **Toolchain & gate wiring depending on the stack:** TASK-0207 (Rust toolchain:
  cargo/rustfmt/clippy `-D warnings`/cargo-llvm-cov, wired into `make validate`),
  **TASK-0208 reframed** to clippy + cargo-audit + cargo-deny (CodeQL has no
  first-class Rust support — see ADR-0007), TASK-0209 (cargo Dependabot
  ecosystem), TASK-0203/TASK-0211 (release + prebuilt cross-platform binaries).
- **Product follow-up:** the duplicates-found exit code (ADR-0008 reserves code
  `3`) is flagged back to the product-owner to decide.
- Stack-independent Phase 2 tasks remain available in parallel: TASK-0202
  (licensing ADR), TASK-0204 (SHA-pin actions), TASK-0205 (cspell→typos),
  TASK-0206 (link-check gate), TASK-0210 (`.claude/settings.json`).

## Done

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

- _None currently._ (`gh` re-authenticated 2026-06-13; GitHub steps unblocked.)

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
- Language toolchain (build/lint/fmt/test/coverage/hooks) — **TASK-0207**.
- CodeQL + dependency audit — **TASK-0208**.
- Dependabot package ecosystem for the stack — **TASK-0209**.
- Project `.claude/settings.json` permission allowlist — **TASK-0210**.
- Prebuilt binaries on releases — **TASK-0211**.
