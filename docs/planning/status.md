# Project Status

Live snapshot of where the project stands. Update this in every PR. This is the
first file to read when resuming work.

**Current phase:** Phase 0 — Foundation
**Last updated:** 2026-06-13

## In progress

- **TASK-0002** — Create the private GitHub repo (`intruder1912/imgdedup`) and
  push `main`. Unblocked (`gh` re-authenticated).

## Up next

- **TASK-0003 → TASK-0007** — branch protection + squash-only settings, then the
  SDLC smoke-test PR, then Phase 0 wrap-up. See [`backlog.md`](backlog.md).

## Done

- **TASK-0001** — Bootstrap repo foundation (docs, crew, structures, configs).
  Committed locally on `main` (two commits); local validation green. _Not pushed
  yet — pushed as part of TASK-0002._

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
