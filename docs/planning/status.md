# Project Status

Live snapshot of where the project stands. Update this in every PR. This is the
first file to read when resuming work.

**Current phase:** Phase 3 — Implementation (TDD)
**Last updated:** 2026-06-14

## In progress

- _Nothing currently in progress._

## Up next

- **Core-first fan-out (TASK-0301 merged — now ready):** TASK-0302 (enumeration)
  and TASK-0303 (magic-byte sniff) are the immediately-ready, parallelizable next
  tasks; both feed TASK-0304 (dedup engine: size → hash → byte-verify → identity);
  then TASK-0305 (keeper) and TASK-0306 (concurrency/determinism). Fan-out
  surfaces: TASK-0307 (human report) + TASK-0308 (JSON); TASK-0309 (trash apply +
  re-validation) → TASK-0310 (permanent delete); TASK-0311 (exit codes); TASK-0312
  (tester-led E2E + scale benchmark).
- **TASK-0207 fulfilled by TASK-0301.** TASK-0208 reframed to clippy, cargo-audit,
  and cargo-deny (see ADR-0007). TASK-0209 (cargo Dependabot), TASK-0203/TASK-0211
  (release and prebuilt binaries) remain.
- Stack-independent Phase 2 tasks still available in parallel: TASK-0202
  (licensing ADR), TASK-0204 (SHA-pin actions), TASK-0205 (cspell→typos),
  TASK-0206 (link-check gate), TASK-0210 (`.claude/settings.json`).

## Done

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
- Language toolchain (build/lint/fmt/test/coverage/hooks) — **TASK-0207**
  (fulfilled by TASK-0301, merged in PR #14).
- CodeQL + dependency audit — **TASK-0208**.
- Dependabot package ecosystem for the stack — **TASK-0209**.
- Project `.claude/settings.json` permission allowlist — **TASK-0210**.
- Prebuilt binaries on releases — **TASK-0211**.
