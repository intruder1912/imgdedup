# ADR-0005: Mandatory TDD & security gate

- **Status:** accepted
- **Date:** 2026-06-13
- **Deciders:** architect, human owner
- **Related:** [ADR-0003](0003-main-branch-protection-and-merge-model.md), [`../../SECURITY.md`](../../SECURITY.md)

## Context and problem statement

The project must meet high quality and security standards. We need explicit,
enforceable discipline for how product code is written and what must be true
before anything merges.

## Decision drivers

- Correctness and regression safety.
- Security by default, caught early and continuously.
- Discipline that AI agents can follow unambiguously.

## Considered options

1. **Mandatory TDD + a required security gate** on every PR.
2. TDD encouraged but not enforced; security scanning only.
3. Tests required (coverage threshold) but test-first not mandated.

## Decision outcome

**Chosen option 1.**

### Test-Driven Development (mandatory for product code)

- Write a failing test, make it pass with the minimum code, then refactor.
- Every acceptance criterion maps to a meaningful, asserting test.
- A coverage threshold is enforced once the stack exists (set in Phase 2); it is
  a floor, not a substitute for meaningful assertions.
- Difficulty writing a test is treated as a design signal (consult the architect).

### Security gate (required before every merge)

- **No secrets in the repo**; secret scanning runs in CI (gitleaks now;
  language-specific scanners added in Phase 2).
- **Dependabot** keeps dependencies and GitHub Actions current. (GitHub
  dependency-review requires Advanced Security, unavailable on free private repos;
  a language-level dependency audit is added in Phase 2 — TASK-0208.)
- **Security review** by the architect on every PR (untrusted input, safe
  file/path handling, least-privilege CI tokens).
- Language-specific static analysis (e.g., CodeQL, dependency audit) added with
  the stack in Phase 2 — see [2026-08-05 amendment](#amendment-2026-08-05--shell-script-and-github-actions-workflow-static-analysis-task-0335)
  for the tooling/automation surface's own equivalent (`shellcheck`,
  `actionlint`).

### Consequences

- Positive: regressions and vulnerabilities are caught early; quality is built in.
- Negative: slower per-change throughput; agents must be held to the discipline
  (reinforced in the agent definitions, SDLC, and review).
- Some gate components (coverage, CodeQL) are necessarily deferred to Phase 2
  because they are stack-specific; the principles apply from day one.

## Amendment 2026-08-05 — shell script and GitHub Actions workflow static analysis (TASK-0335)

The Phase 2 static-analysis bullet above named the *product* code's stack
(Rust: clippy, cargo-deny). It did not name this repository's other
executable surface — the shell scripts under `scripts/` and the GitHub
Actions workflows under `.github/workflows/` — and neither was linted
anywhere, locally or in CI, until this amendment. The gap was not
hypothetical: the architect review for TASK-0334 found a real
command-injection defect in `scripts/release-preflight.sh` (unquoted
interpolation of a user-supplied value into a container's shell) purely by
running `shellcheck` by hand, because nothing gated on it.

**Added:** `shellcheck` (shell scripts) and `actionlint` (workflow YAML,
including the shell embedded in every `run:` block, via actionlint's own
shellcheck integration) — both installed pinned-and-checksum-verified in
`ci.yml`'s `checks` job, the same way `typos` already was, and available
locally via `make shellcheck`/`make actionlint`/`make validate`. Documented
operationally in `scripts/validate.sh`, [`docs/process/sdlc.md`](../process/sdlc.md),
and [`CONTRIBUTING.md`](../../CONTRIBUTING.md).

This closes the gap for this specific tooling surface; it does not change
the Phase 2 bullet's own scope (Rust static analysis, CodeQL, dependency
audit), which stands as originally recorded.

## Links

- [`../process/sdlc.md`](../process/sdlc.md), [`../../SECURITY.md`](../../SECURITY.md)
