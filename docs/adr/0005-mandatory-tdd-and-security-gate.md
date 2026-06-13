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
  the stack in Phase 2.

### Consequences

- Positive: regressions and vulnerabilities are caught early; quality is built in.
- Negative: slower per-change throughput; agents must be held to the discipline
  (reinforced in the agent definitions, SDLC, and review).
- Some gate components (coverage, CodeQL) are necessarily deferred to Phase 2
  because they are stack-specific; the principles apply from day one.

## Links

- [`../process/sdlc.md`](../process/sdlc.md), [`../../SECURITY.md`](../../SECURITY.md)
