# Roadmap

The path from an empty repository to a released v1.0 of `imgdedup`. Phases are
sequential at the milestone level; tasks **within** later phases are
parallelizable. Each phase is refined in its own brainstorm → PRD/ADR → plan →
implement → review cycle. This file is the high-level map. The work itself
is tracked in [GitHub Issues](https://github.com/intruder1912/imgdedup/issues)
([ADR-0021](../adr/0021-issue-based-planning-for-a-public-repository.md));
[`backlog.md`](backlog.md) is the closed list of tasks up to 2026-11-14.

## Phase 0 — Foundation *(complete — 2026-06-13)*

Establish governance, process, AI crew, documentation, planning structures, and
stack-agnostic CI/release scaffolding so all later work is repo-driven.

Exit criteria (all met): private repo on GitHub; protected `main` with required
CI checks (`markdownlint`, `spellcheck`, `structure`, `pr-title`, `secret-scan`)
and squash-only merges; agent crew + process docs + PRD/ADR/planning structures
in place; SDLC proven end-to-end by smoke-test PR #6 (owner reviewed and merged —
no self-approval deadlock).

## Phase 1 — Product definition

Product Owner authors the first PRD(s) defining the dedup product: detection
scope (exact vs perceptual/near-duplicate), supported formats, CLI UX
(commands/flags/output), performance targets, and safety model (dry-run,
trash vs. delete, undo). Output: `docs/prd/PRD-0001-*` (and others as needed).

Exit criteria: approved PRD(s) with testable acceptance criteria; backlog seeded
with implementation tasks; open architecture questions handed to the architect.

## Phase 2 — Architecture & toolchain

Architect writes the **fully-open technology-stack selection ADR** plus design
ADRs (hashing/detection strategy, index/storage, concurrency model, CLI
framework, error model). Then the concrete toolchain is established for the
chosen stack: build, format, lint, test, coverage, local git hooks; CodeQL and
dependency audit; Dependabot package ecosystem; and **full semantic-release
wiring + binary publishing**, resolving the protected-`main` push constraint.

Exit criteria: stack ADR + key design ADRs approved; `make validate` runs the
full language toolchain locally and in CI; release pipeline able to publish for
real.

## Phase 3 — MVP implementation

Implementer + Tester build the core engine (scan → hash → group → act) and the
CLI surface under TDD, task by task. Tasks decomposed to short, parallelizable
units in the backlog; architect reviews each PR.

Exit criteria: PRD-0001 acceptance criteria met and tested; usable end-to-end CLI.

## Phase 4 — Hardening & UX

Performance on large image sets, additional formats, configuration, robust error
handling and edge cases, accessibility of output, and a focused security review.

Exit criteria: meets non-functional/performance targets; security review passed.

## Phase 5 — Release v1.0

Exercise the release pipeline end-to-end; publish the first stable GitHub Release
with prebuilt cross-platform binaries; finalize user documentation.

Exit criteria: `1.0.0` published with binaries and notes; install/usage docs live.
