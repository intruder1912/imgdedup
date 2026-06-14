# Backlog

Fine-grained, repo-tracked work items. This is the canonical to-do list. Each
non-trivial task gets a spec file in [`tasks/`](tasks/) named `TASK-XXXX-<slug>.md`.
Status mirrors [`status.md`](status.md).

## Columns

- **ID** — `TASK-XXXX`, sequential.
- **Task** — short description (links to the task file when one exists).
- **Phase** — roadmap phase.
- **Role** — primary owner role (`product-owner`, `architect`, `implementer`, `tester`).
- **Status** — `todo` / `in-progress` / `blocked` / `done`.
- **Parallel** — `yes` if it can be worked concurrently with other `yes` tasks
  (disjoint files, no ordering dependency); `no` otherwise.
- **Depends on** — task IDs that must complete first.

## Phase 0 — Foundation

| ID | Task | Role | Status | Parallel | Depends on |
|---|---|---|---|---|---|
| TASK-0001 | Bootstrap repo foundation (docs, crew, structures, configs) | architect | done | no | — |
| TASK-0002 | Create private GitHub repo and push `main` | architect | done | no | TASK-0001 |
| TASK-0003 | Configure branch protection + squash-only merge settings | architect | done | no | TASK-0002 |
| TASK-0004 | CI skeleton workflow (md-lint, spell, links, commitlint, structure) | architect | done | no | TASK-0001 |
| TASK-0005 | Dependabot + security scan + release skeleton | architect | done | no | TASK-0001 |
| TASK-0006 | SDLC smoke-test PR (prove review + merge end-to-end) — PR #6 | architect | done | no | TASK-0003, TASK-0004, TASK-0005 |
| TASK-0007 | Phase 0 wrap-up: mark foundation complete, queue Phase 1 | architect | done | no | TASK-0006 |

## Phase 1 — Product definition

| ID | Task | Role | Status | Parallel | Depends on |
|---|---|---|---|---|---|
| TASK-0101 | Brainstorm + author [PRD-0001](../prd/PRD-0001-exact-image-deduplication.md) (exact-dedup scope, UX, safety) | product-owner | done | no | TASK-0007 |

## Phase 2 — Architecture & toolchain

Includes design ADRs and the concrete items deliberately deferred from Phase 0.
Several of these are stack-independent and can start as soon as Phase 0 is
wrapped (TASK-0007); the rest depend on the stack-selection ADR (TASK-0201).

| ID | Task | Role | Status | Parallel | Depends on |
|---|---|---|---|---|---|
| TASK-0201 | Author technology-stack selection ADR (fully open comparison) | architect | done | no | TASK-0101 |
| TASK-0202 | Author licensing ADR (decide license before any public release) | architect | todo | yes | TASK-0007 |
| TASK-0203 | Wire full semantic-release publishing; resolve push to protected `main` (release bot / GitHub App or release-as-asset) — flips dry-run off | architect | todo | no | TASK-0201 |
| TASK-0204 | Pin all GitHub Actions to commit SHAs (supply-chain hardening) | architect | todo | yes | TASK-0007 |
| TASK-0205 | Evaluate replacing cspell with `typos` for spell checking | architect | todo | yes | TASK-0007 |
| TASK-0206 | Promote link check (lychee) from informational to a required CI gate | architect | todo | yes | TASK-0007 |
| TASK-0207 | Add language toolchain: build, format, lint, test, coverage threshold, local git hooks; wire into `make validate` and CI — **folded into [TASK-0301](tasks/TASK-0301-crate-scaffold-cli-surface-toolchain.md)** | implementer | done (by TASK-0301) | no | TASK-0201 |
| TASK-0208 | Add CodeQL + dependency audit for the chosen language | architect | todo | no | TASK-0201 |
| TASK-0209 | Add Dependabot package ecosystem for the chosen stack | architect | todo | yes | TASK-0201 |
| TASK-0210 | Add project `.claude/settings.json` permission allowlist (deferred in Phase 0; via harness config tooling) | architect | todo | yes | TASK-0007 |
| TASK-0211 | Attach prebuilt cross-platform binaries to GitHub Releases | architect | todo | no | TASK-0203, TASK-0207 |
| TASK-0212 | [Unit-of-work cadence & planning gate](tasks/TASK-0212-process-unit-of-work-cadence.md) (implements [ADR-0011](../adr/0011-unit-of-work-cadence-and-planning-gate.md); `ci-gate` + path-filtered Rust matrix) | architect | done | no | — |

> **Phase 2 design ADRs landed:** TASK-0201 produced
> [ADR-0007](../adr/0007-technology-stack-rust-clap.md) (stack = Rust + clap) plus
> the core design ADRs [ADR-0008](../adr/0008-core-deduplication-algorithm-and-semantics.md)
> (dedup algorithm & safety semantics), [ADR-0009](../adr/0009-cross-platform-trash-and-recycle.md)
> (trash/recycle), and [ADR-0010](../adr/0010-concurrency-and-memory-model.md)
> (concurrency & memory). Note: **TASK-0208 is reframed** to clippy + cargo-audit +
> cargo-deny (CodeQL has no first-class Rust support — ADR-0007).
>
> Further Phase 1+ tasks are added as PRDs and ADRs are written. Keep this table
> and the `tasks/` specs in sync with reality in every PR.

## Phase 3 — Implementation (TDD)

The ADR-0008 pipeline, built test-first (ADR-0005). **Core first, then fan-out:**
the sequential core (TASK-0301 → enumeration/sniff → engine → keeper → concurrency)
must land before the parallelizable output/action surfaces. TASK-0301 fulfils
TASK-0207 (toolchain). `make validate` (rustfmt, clippy `-D warnings`, cargo test,
cargo-llvm-cov) gates every PR in this phase from TASK-0301 onward.

| ID | Task | Role | Status | Parallel | Depends on |
|---|---|---|---|---|---|
| TASK-0301 | [Crate scaffold, CLI surface & toolchain](tasks/TASK-0301-crate-scaffold-cli-surface-toolchain.md) (fulfils TASK-0207; blocks all Phase 3) | implementer | done | no | — |
| TASK-0302 | [File enumeration & scope](tasks/TASK-0302-file-enumeration-and-scope.md) (FR-1/2/6, NFR-6) | implementer | todo | yes | TASK-0301 |
| TASK-0303 | [Magic-byte image confirmation](tasks/TASK-0303-magic-byte-image-confirmation.md) (FR-3, NFR-2) | implementer | todo | yes | TASK-0301 |
| TASK-0304 | [Core dedup engine](tasks/TASK-0304-core-dedup-engine.md) — size→hash→verify→identity (FR-4/5/17, NFR-1) | implementer | todo | no | TASK-0302, TASK-0303 |
| TASK-0305 | [Keeper selection](tasks/TASK-0305-keeper-selection.md) (FR-7/8, NFR-3) | implementer | todo | no | TASK-0304 |
| TASK-0306 | [Concurrency & determinism](tasks/TASK-0306-concurrency-and-determinism.md) — rayon + canonical sort (NFR-3/4) | implementer | todo | no | TASK-0304 |
| TASK-0307 | [Human-readable report](tasks/TASK-0307-human-readable-report.md) (FR-9/13, FR-17) | implementer | todo | yes | TASK-0305 |
| TASK-0308 | [JSON output](tasks/TASK-0308-json-output.md) (FR-14) | implementer | todo | yes | TASK-0305 |
| TASK-0309 | [Trash apply + re-validation](tasks/TASK-0309-trash-apply-and-revalidation.md) (FR-10/12/16, NFR-1/6) | implementer | todo | no | TASK-0305 |
| TASK-0310 | [Permanent delete](tasks/TASK-0310-permanent-delete.md) (FR-11) | implementer | todo | no | TASK-0309 |
| TASK-0311 | [Exit-code semantics & `--exit-code`](tasks/TASK-0311-exit-code-semantics.md) (FR-15/18) | implementer | todo | no | TASK-0307 |
| TASK-0312 | [E2E acceptance & NFR-4 scale/benchmark](tasks/TASK-0312-e2e-acceptance-and-scale-benchmark.md) (AC-1..19, NFR-4) | tester | todo | no | TASK-0307, TASK-0308, TASK-0309, TASK-0310, TASK-0311 |
