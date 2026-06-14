# Project Status

Live snapshot of where the project stands. Update this in every PR. This is the
first file to read when resuming work.

**Current phase:** Phase 3 — Implementation (TDD), tasks seeded
**Last updated:** 2026-06-14

## In progress

- **Phase 3 task-seeding** — this PR. Twelve fine-grained implementation specs
  (TASK-0301..TASK-0312) decompose the ADR-0008 pipeline; backlog + status updated
  in the same PR. No product code yet.

## Up next

- **TASK-0301 — Crate scaffold, CLI surface & toolchain.** The Phase 3 **root**:
  Cargo crate + `rust-toolchain.toml`, the full PRD §8 clap surface (incl.
  `--exit-code` and the `--delete`-requires-`--apply` guard), exit-code constants
  0/1/2/3 (ADR-0008), and `make validate`/CI wiring (rustfmt, clippy `-D warnings`,
  cargo test, cargo-llvm-cov). It **fulfils TASK-0207** and **blocks every other
  Phase 3 task**. Start here.
- **Core-first → fan-out plan.** After TASK-0301: enumeration (TASK-0302) and
  magic-byte sniff (TASK-0303) run in parallel; both feed the dedup engine
  (TASK-0304, the safety heart: size → hash → byte-verify → identity); then keeper
  selection (TASK-0305) and concurrency/determinism (TASK-0306). Once the core +
  keeper exist, the fan-out surfaces parallelize: human report (TASK-0307) and JSON
  (TASK-0308) together; trash apply + re-validation (TASK-0309) → permanent delete
  (TASK-0310, shared executor); exit codes (TASK-0311); and the tester-led E2E +
  NFR-4 scale benchmark (TASK-0312) closes the phase.
- **Toolchain & gate wiring depending on the stack:** **TASK-0207 is folded into
  TASK-0301** (done by it). **TASK-0208 reframed** to clippy + cargo-audit +
  cargo-deny (CodeQL has no first-class Rust support — see ADR-0007), TASK-0209
  (cargo Dependabot ecosystem), TASK-0203/TASK-0211 (release + prebuilt
  cross-platform binaries).
- **Product follow-up (resolved):** the duplicates-found exit code is the opt-in
  `--exit-code` → `3` (FR-18, ADR-0008); wired in TASK-0311.
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
