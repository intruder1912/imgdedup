# Project Status

Live snapshot of where the project stands. Update this in every PR. This is the
first file to read when resuming work.

**Current phase:** Phase 0 — Foundation
**Last updated:** 2026-06-13

## In progress

- **TASK-0001** — Bootstrap repo foundation (docs, crew, structures, configs).
  Orchestrator-executed. Local scaffolding being created.

## Up next

- **TASK-0002** — Create the private GitHub repo and push `main`
  (**blocked on `gh` re-authentication** by the owner: `gh auth refresh -h github.com`).
- **TASK-0003 → TASK-0007** — branch protection, CI skeleton, security/release
  skeleton, SDLC smoke test, Phase 0 wrap-up. See [`backlog.md`](backlog.md).

## Done

- _Nothing merged yet — Phase 0 in progress._

## Blockers / waiting on owner

- **`gh` auth invalid.** GitHub steps (TASK-0002+) cannot run until the owner
  re-authenticates the `gh` CLI.
- **Project `.claude/settings.json`** (permission allowlist) intentionally
  deferred — add later via the harness config tooling if desired.

## Open decisions (tracked for later)

- Technology stack — to be selected in a fully-open ADR (Phase 2).
- Licensing — to be recorded in an ADR before any public release.
- semantic-release vs protected `main` push — wiring decided in Phase 2 (see
  [`../process/release.md`](../process/release.md)).
