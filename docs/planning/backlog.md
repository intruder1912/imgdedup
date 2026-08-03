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
| TASK-0202 | Author licensing ADR — owner decided GPL-3.0 (2026-07-19); formalized as **`GPL-3.0-only`** in [ADR-0014](../adr/0014-project-license-gpl-3.0-only.md) + `LICENSE`; repo stays private (an access-control, not a licensing, state) | architect | done | yes | TASK-0007 |
| TASK-0203 | Wire full semantic-release publishing — [ADR-0015](../adr/0015-release-publishing-tag-and-github-release.md): tag + GitHub Release only, **no commit back to `main`**, so the built-in `GITHUB_TOKEN` suffices (no bot/PAT/bypass). Dry-run off; CI-gated `release.yml`; pinned tooling via `package-lock.json` | architect | done | no | TASK-0201 |
| TASK-0204 | Pin all GitHub Actions to commit SHAs (supply-chain hardening; incl. gitleaks image version pin) | architect | done | yes | TASK-0007 |
| TASK-0205 | Evaluate replacing cspell with `typos` for spell checking. **Decision: migrate.** Evaluated by running both tools against this repo's Markdown and against a deliberately-misspelled scratch file: `typos` caught the same six real typos cspell did, with **one** false positive on the whole repo (`PNGs` misparsed, fixed via `_typos.toml`) versus cspell's 100+ curated custom words — typos matches known misspelling patterns rather than checking every word against a dictionary, so jargon needs no exceptions. The community `crate-ci/typos` GitHub Action was tried and rejected: its entrypoint builds a `**` glob under plain bash with no `shopt -s globstar`, silently under-matching recursively; CI instead installs the pinned binary directly (checksum-verified, since upstream publishes none) and runs it against `git ls-files '*.md'` | architect | done | yes | TASK-0007 |
| TASK-0206 | Promote link check (lychee) from informational to a required CI gate: job renamed `linkcheck` (was `linkcheck (informational)`), `fail: true`, `lychee.toml`'s existing retry/timeout/accept tuning now load-bearing. **Two defects found by the promotion itself, both invisible until the gate actually ran:** (1) `lychee.toml` failed to *load* under the pinned lychee v0.24.2 (`exclude_mail` was removed from the config schema) — fixed (field removed; mail exclusion is the v0.24.2 default). (2) once loading, `www.gnu.org/licenses/{gpl-3.0.txt,gpl-howto.html}` (in ADR-0014) consistently timed out from GitHub-hosted runners across two separate CI runs despite `max_retries = 3`, while resolving instantly (`curl -I`, HTTP 200, <2s) from a non-CI IP — gnu.org rate-limits/blocks cloud CI ranges; excluded rather than "fixed" since the links themselves are fine. Because the job ran `fail: false` until now, both defects were silently swallowed and link checking had never actually run in CI. Verified locally: 439 links, 436 OK, 0 errors, 3 excluded. Owner added `linkcheck` (and, discovered stale, `cargo-deny`) to `main`'s required-checks list live 2026-08-02 and renamed the context to match this job's rename — verified live afterward, no blocker remains | architect | done | yes | TASK-0007 |
| TASK-0207 | Add language toolchain: build, format, lint, test, coverage threshold, local git hooks; wire into `make validate` and CI — **folded into [TASK-0301](tasks/TASK-0301-crate-scaffold-cli-surface-toolchain.md)** | implementer | done (by TASK-0301) | no | TASK-0201 |
| TASK-0208 | Dependency audit (reframed per ADR-0007): cargo-deny in CI covering RUSTSEC advisories (supersedes standalone cargo-audit), licenses, bans, sources; `deny.toml` + `make deny`; fixed RUSTSEC-2026-0204 (crossbeam-epoch → 0.9.20) | architect | done | no | TASK-0201 |
| TASK-0209 | Add Dependabot package ecosystem for the chosen stack (cargo, weekly, `build` prefix) | architect | done | yes | TASK-0201 |
| TASK-0210 | Add project `.claude/settings.json` permission allowlist (deferred in Phase 0; via harness config tooling) | architect | done | yes | TASK-0007 |
| TASK-0211 | Attach prebuilt cross-platform binaries to GitHub Releases (four native targets: linux-x86_64, macos-aarch64, macos-x86_64, windows-x86_64; version stamped into `Cargo.toml` transiently at build time; `assets` job in `release.yml`, gated on the `release` job detecting a newly published tag; manual `tag` backfill input added to attach binaries to `v0.1.0`, which had already published without them). See the [2026-08 amendment to ADR-0015](../adr/0015-release-publishing-tag-and-github-release.md#amendment-2026-08-02--binary-artifacts-task-0211) | architect | done | no | TASK-0203, TASK-0207 |
| TASK-0212 | [Unit-of-work cadence & planning gate](tasks/TASK-0212-process-unit-of-work-cadence.md) (implements [ADR-0011](../adr/0011-unit-of-work-cadence-and-planning-gate.md); `ci-gate` + path-filtered Rust matrix) | architect | done | no | — |
| TASK-0213 | [Documentation ownership & guide structure](tasks/TASK-0213-documentation-ownership.md) (ADR-0012: product-owner owns user docs; README = general, `docs/guide/` = detailed) | product-owner | done | no | TASK-0212 |
| TASK-0214 | [PRD-slice delivery cadence & in-progress review protocol](tasks/TASK-0214-prd-slice-delivery-cadence.md) (ADR-0013: feature unit = PRD slice; in-progress reviews + acceptance/docs PR-open gate) | architect | done | no | TASK-0212 |
| TASK-0215 | Unblock `main`: fix MD032 in S1 review log; pin markdownlint-cli2/cspell/markdown-link-check versions in CI, Makefile, validate.sh (stops lint-tool drift) | architect | done | no | — |
| TASK-0216 | Dependency & toolchain refresh + resilience review — refresh (PR #35: Rust 1.97.1, edition 2024, transitives) + [resilience review](reviews/resilience-2026-07.md) (finding F1 silent-root-error defect fixed in the same unit; F3→TASK-0308, F8→TASK-0312/0314) | architect | done | no | TASK-0215 |
| TASK-0217 | Docs consistency pass: README + `docs/guide/` + process docs + AI docs (`CLAUDE.md`, `.claude/agents/`) reconciled with shipped behavior; unify stderr diagnostic prefix (`error:` vs bare message — resilience review nit). Read-only recon found 17 files were checked, only two were actually stale: (1) README + `getting-started.md` still said "no prebuilt binaries" (stale since TASK-0211) — added a download/checksum/quarantine walkthrough; (2) the "architect reviews every PR" policy in `sdlc.md`/`crew.md`/`architect.md` didn't reflect the low-risk-unit review-skip exception already used 9 times — formalized as a dated amendment to [ADR-0011](../adr/0011-unit-of-work-cadence-and-planning-gate.md#amendment-2026-08-02--architect-review-skip-exception-for-verified-low-risk-units). The stderr-prefix nit fixed: `cli.rs::validate()` now returns the bare message, `main.rs` adds `"error: "` uniformly like every other diagnostic | product-owner | done | no | TASK-0203 |
| TASK-0218 | AI automation support: in-repo Claude usage skill + slash command + `imgdedup-operator` agent (dry-run-first, `--json`-driven, no unattended `--delete`) | architect | done (this PR) | no | TASK-0308 |
| TASK-0219 | Public-release licensing readiness ([ADR-0014](../adr/0014-project-license-gpl-3.0-only.md) follow-ups): per-file GPL headers (or a reasoned decision not to), a license/copyright line in `--version`, GPLv3 §6 source-offer wording wherever binaries are distributed, and a `-only` vs `-or-later` re-check before accepting the first outside contribution. **Must complete before the repo goes public** — TASK-0211's "no action needed on §6 yet" position relies on every binary recipient already having private-repo (source) access, which stops holding the moment the repo flips public | architect | todo | yes | TASK-0202, TASK-0211 |

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
| TASK-0308 | [JSON output](tasks/TASK-0308-json-output.md) (FR-14) — **acceptance addition (resilience F3):** JSON model must include per-file warnings and root errors (satisfied: `warnings` + `root_errors` are first-class in the schema) | implementer | done | yes | TASK-0305 |
| TASK-0309 | [Trash apply + re-validation](tasks/TASK-0309-trash-apply-and-revalidation.md) (FR-10/12/16, NFR-1/6) | implementer | done | no | TASK-0305 |
| TASK-0310 | [Permanent delete](tasks/TASK-0310-permanent-delete.md) (FR-11) | implementer | done | no | TASK-0309 |
| TASK-0311 | [Exit-code semantics & `--exit-code`](tasks/TASK-0311-exit-code-semantics.md) (FR-15/18) | implementer | done | no | TASK-0307 |
| TASK-0312 | [E2E acceptance & NFR-4 scale/benchmark](tasks/TASK-0312-e2e-acceptance-and-scale-benchmark.md) (AC-1..19, NFR-4) | tester | done | no | TASK-0307, TASK-0308, TASK-0309, TASK-0310, TASK-0311 |
| TASK-0313 | [Test-data generator & golden-set oracle harness](tasks/TASK-0313-golden-set-oracle.md): deterministic tiny images per supported format + duplicate/near-miss trees with expected-manifest diff against `--json` | tester | done | no | TASK-0308 |
| TASK-0314 | Real-photo field validation: read-only dry-run + `--json` sanity pass on a copied ~500–1k photo subset; pre-release gate, originals untouched | tester | todo | no | TASK-0312 |
| TASK-0315 | [Progress indication](tasks/TASK-0315-progress-indication.md) (FR-19) — live stderr progress (phase, x/y, %), TTY-aware with non-TTY fallback, default-on + `--no-progress`, stdout-pure incl. `--json` | implementer | done | no | TASK-0308 |
| TASK-0316 | Lone fully-hard-linked sets (one inode, no separate copy) appeared nowhere in output while the guide documented them as already-deduplicated (with a worked example the tool could not produce) — golden-set finding (TASK-0313). **Product-owner ruled 2026-08-01: a bug, not a scope call** — ADR-0008 edge case 1 already required it and both formatters already handled it; one engine guard dropped the note. Fixed as a `fix:` unit, AC-18a added | product-owner | done | no | TASK-0313 |
| TASK-0317 | Animate the `scanning` phase (FR-19 follow-up): enumeration **and** magic-byte sniffing run behind a frozen `scanning...` — ~200k system calls and 5–15 s of static indicator at 100k files. Spec-conformant (AC-21 allows an indeterminate state pre-enumeration) but the same intent gap that was fixed for `hashing`. Show a live indeterminate count (`scanning… 43,102 files found`); the renderer's no-percentage branch already exists. **Sizing note (architect measurement, 2026-08-01):** on a 1502-file / 194 MB tree the frozen `scanning...` took ~3.6 s of a 4.2 s run — roughly 9× the hashing phase, far worse than the estimate this deferral was priced on. That was a debug build and blake3 uses SIMD regardless of opt level while walkdir/sniff do not, so re-measure on a release build in S4 before sizing. **Now blocked on TASK-0321 having landed (done) rather than pending it** — re-measure against the now-parallel sniff stage, not the debug-build serial numbers above. `progress.rs`'s `advance` is already documented safe to call from rayon worker threads (it's how the hashing phase animates), so a per-file counter for the scanning phase can call it directly from inside `filter_candidates`'s parallel closure the same way `engine.rs` does | implementer | todo | yes | TASK-0315, TASK-0321 |
| TASK-0319 | Scanning the same tree twice (`imgdedup ./d ./d`, or overlapping roots like `./d ./d/sub`) makes every file appear hard-linked to itself, so the run reports `hard-linked: a.png, a.png` and finds **zero** duplicates. Fail-safe (the apply-time identity check means nothing is removed) but a user could conclude their tree is clean when it is not. Dedupe enumerated paths by file identity before grouping — architect finding, S3 review | implementer | todo | yes | TASK-0302 |
| TASK-0320 | Decide whether `--follow-symlinks` should drop symlink entries from candidacy entirely (canonicalize and dedupe by target) instead of enumerating the link path and skipping it at apply time. Current behavior is safe and truthfully reported, but a dry run can list a removal the apply then declines — a documented parity exception. Product-owner call; architect prefers dropping them | product-owner | todo | yes | TASK-0309 |
| TASK-0321 | Parallelize the scan stage: `sniff::filter_candidates` opens and reads a header for every candidate in a serial loop, and NFR-4 measurement showed it is ~100% of small-file wall time — a control fixture where **no** file is hashed costs the same as one hashing ~1,800. This is the throughput lever for large libraries, and the same serial stage behind TASK-0317's frozen `scanning...` indicator. **Done:** switched to `into_par_iter().map().collect()` (same pattern as `engine.rs` hashing); rayon's indexed collect preserves input order by construction, pinned by a new unit test asserting exact order at 1 vs. 8 threads. Measured on a synthetic 60k-file all-rejected fixture: cold-cache 158.7s → 21.5s (~7.4x), warm-cache 6.1s → 1.8s (~3.3x), 10 cores. See the [2026-08-03 amendment to ADR-0010](../adr/0010-concurrency-and-memory-model.md#consequence-note--the-scan-stage-is-no-longer-serial-2026-08-03-task-0321). Only the parallelization landed here; TASK-0317's indicator work is separate and not yet done | implementer | done | no | TASK-0312 |
| TASK-0322 | Large files have **no intra-file parallelism**: `blake3` is compiled without its `rayon` feature and `engine.rs` calls the serial `update()`, so one file is hashed by one core. Evaluate `update_rayon` above a size threshold — compatible with ADR-0010, which rules out mmap but not threaded hashing over the existing streaming buffer. Measure before adopting | implementer | todo | yes | TASK-0312 |
| TASK-0323 | `--delete` without `--apply` exits **1**, but `docs/guide/output-and-exit-codes.md` documents `2` as "usage error — invalid arguments". PRD AC-12 only requires non-zero, so neither is wrong — but the two disagree. Product-owner decides whether a refused flag *combination* is a usage error (exit 2) or a run-level error (exit 1), then code and guide are reconciled. Found by the S4 acceptance suite | product-owner | todo | yes | TASK-0312 |
| TASK-0318 | Sanitize control characters in paths printed to stderr: `main.rs` prints `err.path.display()` raw, so a hostile filename can inject terminal escape sequences. Pre-existing, but FR-19 made stderr a stream we actively draw on, which raises the value of escaping it. Architect security note from the TASK-0315 review | implementer | todo | yes | TASK-0315 |
| TASK-0324 | `release.yml`'s `@semantic-release/github` options use the deprecated `successComment`/`failTitle: false` form (v12 `DEPRECATION` warning); switch to `successCommentCondition`/`failCommentCondition: false` on the next `semantic-release` major bump so a future release doesn't silently start trying to comment without the scope to do it. Architect review nit, TASK-0203 | implementer | todo | yes | TASK-0203 |
| TASK-0325 | Add `github.event.workflow_run.head_repository.full_name == github.repository` to `release.yml`'s trigger `if:` — redundant with the existing `event == 'push'` fork-exclusion today, but makes the safety property self-evident to a future reader who doesn't want to re-derive the `workflow_run` event semantics. Architect review nit, TASK-0203 | implementer | todo | yes | TASK-0203 |
| TASK-0326 | Widen the release binary matrix: `x86_64-unknown-linux-musl` (static, no glibc floor — blake3 needs a `cc`/musl sysroot on the runner, deliberately deferred from TASK-0211) and `aarch64` Linux/Windows targets (need cross-compilation or paid ARM runners on a private repo). Reconsider as a gate on the public release, not before | architect | todo | yes | TASK-0211 |
| TASK-0327 | `Makefile`'s `links` target (and `.markdown-link-check.json`) uses `markdown-link-check`, diverging from CI's lychee-based `linkcheck` gate (TASK-0206) — local validation doesn't check what CI actually enforces, and `links` isn't even wired into the `validate` composite target. Found while touching the Makefile for TASK-0205. Fold local link-checking onto lychee + `lychee.toml` and add it to `validate` | architect | todo | yes | TASK-0206 |

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

**Post-S1 amendment feature — FR-19 progress indication (TASK-0315).** The
[Amendment 2026-07-19](../prd/PRD-0001-exact-image-deduplication.md#amendment-2026-07-19--progress-indication--stored-results)
added FR-19 (live stderr progress) after the original slice carve. It is a single
coherent FR, so it ships as **its own one-unit feature branch/PR** rather than
joining a slice — it depends on TASK-0308 (S2) because progress must coexist with
the `--json` stdout-purity contract. It is independent of the S3/S4
release-critical path (see the [batch plan](batch-plan.md) batch 4b).
