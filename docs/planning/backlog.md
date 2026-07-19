# Backlog

Fine-grained, repo-tracked work items. This is the canonical to-do list. Each
non-trivial task gets a spec file in [`tasks/`](tasks/) named `TASK-XXXX-<slug>.md`.
Status mirrors [`status.md`](status.md). Execution order and usage-limit
budgeting live in the [batch plan](batch-plan.md) (owner-approved 2026-07-19).

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
| TASK-0202 | Author licensing ADR — **owner decided GPL-3.0 (2026-07-19)**; repo stays private for now, ADR formalizes before any public flip | architect | todo | yes | TASK-0007 |
| TASK-0203 | Wire full semantic-release publishing; resolve push to protected `main` (release bot / GitHub App or release-as-asset) — flips dry-run off | architect | todo | no | TASK-0201 |
| TASK-0204 | Pin all GitHub Actions to commit SHAs (supply-chain hardening; incl. gitleaks image version pin) | architect | done | yes | TASK-0007 |
| TASK-0205 | Evaluate replacing cspell with `typos` for spell checking | architect | todo | yes | TASK-0007 |
| TASK-0206 | Promote link check (lychee) from informational to a required CI gate | architect | todo | yes | TASK-0007 |
| TASK-0207 | Add language toolchain: build, format, lint, test, coverage threshold, local git hooks; wire into `make validate` and CI — **folded into [TASK-0301](tasks/TASK-0301-crate-scaffold-cli-surface-toolchain.md)** | implementer | done (by TASK-0301) | no | TASK-0201 |
| TASK-0208 | Dependency audit (reframed per ADR-0007): cargo-deny in CI covering RUSTSEC advisories (supersedes standalone cargo-audit), licenses, bans, sources; `deny.toml` + `make deny`; fixed RUSTSEC-2026-0204 (crossbeam-epoch → 0.9.20) | architect | done | no | TASK-0201 |
| TASK-0209 | Add Dependabot package ecosystem for the chosen stack (cargo, weekly, `build` prefix) | architect | done | yes | TASK-0201 |
| TASK-0210 | Add project `.claude/settings.json` permission allowlist (deferred in Phase 0; via harness config tooling) | architect | done | yes | TASK-0007 |
| TASK-0211 | Attach prebuilt cross-platform binaries to GitHub Releases | architect | todo | no | TASK-0203, TASK-0207 |
| TASK-0212 | [Unit-of-work cadence & planning gate](tasks/TASK-0212-process-unit-of-work-cadence.md) (implements [ADR-0011](../adr/0011-unit-of-work-cadence-and-planning-gate.md); `ci-gate` + path-filtered Rust matrix) | architect | done | no | — |
| TASK-0213 | [Documentation ownership & guide structure](tasks/TASK-0213-documentation-ownership.md) (ADR-0012: product-owner owns user docs; README = general, `docs/guide/` = detailed) | product-owner | done | no | TASK-0212 |
| TASK-0214 | [PRD-slice delivery cadence & in-progress review protocol](tasks/TASK-0214-prd-slice-delivery-cadence.md) (ADR-0013: feature unit = PRD slice; in-progress reviews + acceptance/docs PR-open gate) | architect | done | no | TASK-0212 |
| TASK-0215 | Unblock `main`: fix MD032 in S1 review log; pin markdownlint-cli2/cspell/markdown-link-check versions in CI, Makefile, validate.sh (stops lint-tool drift) | architect | done | no | — |
| TASK-0216 | Dependency & toolchain refresh + resilience review — refresh (PR #35: Rust 1.97.1, edition 2024, transitives) + [resilience review](reviews/resilience-2026-07.md) (finding F1 silent-root-error defect fixed in the same unit; F3→TASK-0308, F8→TASK-0312/0314) | architect | done | no | TASK-0215 |
| TASK-0217 | Docs consistency pass: README + `docs/guide/` + process docs + AI docs (`CLAUDE.md`, `.claude/agents/`) reconciled with shipped behavior; unify stderr diagnostic prefix (`error:` vs bare message — resilience review nit) | product-owner | todo | no | TASK-0203 |
| TASK-0218 | AI automation support: in-repo Claude usage skill + slash command + `imgdedup-operator` agent (dry-run-first, `--json`-driven, no unattended `--delete`) | architect | todo | no | TASK-0308 |

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

> **Delivery cadence — PRD-0001 ships in slices (ADR-0013).** From here on,
> feature delivery within PRD-0001 is carved into **coherent shippable slices**
> (one branch → granular commits → one squash PR/release each), not one PR per
> task. The task rows below stay the unit of design/TDD; a **slice groups tasks**.
> See the "PRD-0001 delivery slices" subsection beneath this table for the slice
> map, per-slice user value, and PR-open acceptance. The core slice (TASK-0301..0304)
> already merged task-by-task under the pre-ADR-0013 cadence; the remaining slices
> below are the new unit of work.

| ID | Task | Role | Status | Parallel | Depends on |
|---|---|---|---|---|---|
| TASK-0301 | [Crate scaffold, CLI surface & toolchain](tasks/TASK-0301-crate-scaffold-cli-surface-toolchain.md) (fulfils TASK-0207; blocks all Phase 3) | implementer | done | no | — |
| TASK-0302 | [File enumeration & scope](tasks/TASK-0302-file-enumeration-and-scope.md) (FR-1/2/6, NFR-6) | implementer | done | yes | TASK-0301 |
| TASK-0303 | [Magic-byte image confirmation](tasks/TASK-0303-magic-byte-image-confirmation.md) (FR-3, NFR-2) | implementer | done | yes | TASK-0301 |
| TASK-0304 | [Core dedup engine](tasks/TASK-0304-core-dedup-engine.md) — size→hash→verify→identity (FR-4/5/17, NFR-1) | implementer | done | no | TASK-0302, TASK-0303 |
| TASK-0305 | [Keeper selection](tasks/TASK-0305-keeper-selection.md) (FR-7/8, NFR-3) | implementer | done | no | TASK-0304 |
| TASK-0306 | [Concurrency & determinism](tasks/TASK-0306-concurrency-and-determinism.md) — rayon + canonical sort (NFR-3/4) | implementer | done | no | TASK-0304 |
| TASK-0307 | [Human-readable report](tasks/TASK-0307-human-readable-report.md) (FR-9/13, FR-17) | implementer | done | yes | TASK-0305 |
| TASK-0308 | [JSON output](tasks/TASK-0308-json-output.md) (FR-14) — **acceptance addition (resilience F3):** JSON model must include per-file warnings and root errors | implementer | todo | yes | TASK-0305 |
| TASK-0309 | [Trash apply + re-validation](tasks/TASK-0309-trash-apply-and-revalidation.md) (FR-10/12/16, NFR-1/6) | implementer | todo | no | TASK-0305 |
| TASK-0310 | [Permanent delete](tasks/TASK-0310-permanent-delete.md) (FR-11) | implementer | todo | no | TASK-0309 |
| TASK-0311 | [Exit-code semantics & `--exit-code`](tasks/TASK-0311-exit-code-semantics.md) (FR-15/18) | implementer | done | no | TASK-0307 |
| TASK-0312 | [E2E acceptance & NFR-4 scale/benchmark](tasks/TASK-0312-e2e-acceptance-and-scale-benchmark.md) (AC-1..19, NFR-4) | tester | todo | no | TASK-0307, TASK-0308, TASK-0309, TASK-0310, TASK-0311 |
| TASK-0313 | Test-data generator & golden-set oracle harness: deterministic tiny images per supported format + duplicate/near-miss trees with expected-manifest diff against `--json` (task spec written when batch 4 starts) | tester | todo | no | TASK-0308 |
| TASK-0314 | Real-photo field validation: read-only dry-run + `--json` sanity pass on a copied ~500–1k photo subset; pre-release gate, originals untouched | tester | todo | no | TASK-0312 |

### PRD-0001 delivery slices

The remaining PRD-0001 work (TASK-0305..0312) is carved into four **shippable
slices** per [ADR-0013](../adr/0013-prd-slice-delivery-cadence.md). Each slice is
end-to-end-validatable, independently mergeable/releasable, larger than a task and
smaller than the whole PRD, and respects the backlog dependency ordering. The
product-owner carves them; the architect sanity-checks the boundaries. Each slice
opens its PR only when the product-owner has (1) accepted all the slice's listed
functional requirements and (2) delivered its user-facing docs (README +
`docs/guide/`, [ADR-0012](../adr/0012-documentation-ownership-and-placement.md)).

The core slice (TASK-0301..0304: scaffold/CLI, enumeration, sniff, dedup engine) is
**already merged** — it landed task-by-task before ADR-0013 and produced
analysis-only behavior with no user-facing report yet. The four slices below are
the new unit of work and are ordered by dependency; they ship in sequence.

| Slice | Slug | Tasks | User-visible value | PR-open acceptance (FRs + docs) |
|---|---|---|---|---|
| S1 — Detect & report (read-only) | `detect-report` | TASK-0305, TASK-0306, TASK-0307, TASK-0311 | First end-to-end usable tool: a user can scan paths and get a deterministic dry-run report of every exact-duplicate group, its keeper, the files that would be removed, counts, reclaimable space, and hard-link notes — with `--keep {shortest\|oldest\|newest}` and the opt-in `--exit-code`. Nothing is ever modified on disk. | FR-7, FR-8 (keeper rule + `--keep`), FR-9 (dry-run default), FR-13 (human report), FR-15, FR-18 (`--exit-code`), FR-17 (hard-link notes), NFR-3 (determinism), NFR-4 (streaming + parallel hashing baseline), NFR-6 (per-file skip warnings). **Docs:** README general-usage overview (scan + read the report) and the matching `docs/guide/` pages — `getting-started.md`, `scanning-and-scope.md`, `choosing-what-to-keep.md`, and the exit-code half of `output-and-exit-codes.md` — synced to shipped behavior. |
| S2 — JSON output | `json-output` | TASK-0308 | Power users can drive `imgdedup` from scripts: `--json` emits valid, deterministic machine-readable output (groups, keeper, intended actions, totals) over the same canonical model as the human report. | FR-14 (`--json`), NFR-3 (deterministic JSON). **Docs:** README mention of `--json` and the JSON-output section of `docs/guide/output-and-exit-codes.md` (documented field schema). |
| S3 — Destructive actions | `apply-delete` | TASK-0309, TASK-0310 | The tool becomes actionable: `--apply` moves non-keepers to the OS trash (recoverable) and `--apply --delete` permanently deletes them, both gated by apply-time re-validation, idempotent, and matching the dry-run for an unchanged tree. The keeper is never removed and no group is left empty. | FR-10 (`--apply` → trash), FR-11 (`--apply --delete`), FR-12 (idempotence), FR-16 (apply-time re-validation), NFR-1 (safety invariant), NFR-2 (no non-image removed), NFR-6. **Docs:** README actions overview (`--apply`/`--delete` + safety model) and `docs/guide/removing-duplicates-safely.md` synced to shipped behavior. |
| S4 — E2E acceptance & scale | `e2e-scale` | TASK-0312 | Confidence + cross-platform proof: every PRD acceptance criterion (AC-1..AC-19) is verified through the real CLI on Linux/macOS/Windows, and the NFR-4 scale targets (100k+ files, bounded memory, parallel throughput) are validated, fixing the empirical concurrency default. | Full AC-1..AC-19 acceptance, NFR-3/NFR-4/NFR-5. **Docs:** reconcile README + `docs/guide/` (incl. `docs/guide/README.md` index) with the fully verified end-to-end behavior; no user-facing gaps remain. |

**Slice ordering & boundaries (justification).**

- **S1 first** because a read-only "detect + report" increment is the smallest
  thing that is independently useful and independently validatable, and it carries
  zero destructive risk — the safest first release. It bundles keeper selection
  (TASK-0305) and concurrency/determinism (TASK-0306) because a report without a
  chosen keeper or without canonical ordering is neither correct nor
  deterministic — they are not separately shippable. `--exit-code` (TASK-0311)
  rides S1 because it only maps the already-computed dry-run outcome to a status
  code and completes the "scriptable read-only run"; it depends on the report
  (TASK-0307) and nothing destructive.
- **S2 (JSON)** is split out as its own slice: it is an independent formatter over
  S1's model (TASK-0308 is `Parallel: yes` with the human report), adds a distinct
  user capability (scripting), and is small enough to ship and release on its own
  without waiting on the destructive surfaces.
- **S3 (destructive actions)** groups trash apply (TASK-0309) and permanent delete
  (TASK-0310) because TASK-0310 reuses TASK-0309's executor and its apply-time
  re-validation — they share one safety-critical code path and one docs page, and
  splitting them would ship an incomplete actions surface. This slice is gated last
  among the feature slices so the highest-risk code lands after the read-only and
  scripting paths are proven.
- **S4 (E2E + scale)** is its own tester-led slice: it depends on every prior slice
  (AC-1..AC-19 + NFR-4) and delivers the cross-platform acceptance proof and the
  empirical concurrency default. It is the PRD-completion slice rather than a
  feature increment, but it is independently mergeable and ships the final
  doc reconciliation.

**Cross-slice dependency note.** S1 → S2 → S3 → S4 is the safe default ship order
and matches the task dependencies (TASK-0308/0309 depend on TASK-0305 in S1;
TASK-0312 depends on all). S2 and S3 both depend only on S1, so they *could* be
worked in either order or concurrently on separate branches; the table orders them
JSON-before-destructive to ship the lower-risk capability first. Each slice rebases
on `main` after the previous one merges (ADR-0013).
