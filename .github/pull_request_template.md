<!--
PR title MUST be a Conventional Commit (e.g. "feat(scan): add recursive walk").
Squash-merge uses the title as the commit on `main` and drives the release.
See docs/process/commits.md.
-->

## What & why

<!-- What does this change do, and why? Link the task/PRD/ADR. -->

- Task: <!-- TASK-XXXX or link to docs/planning/tasks/… -->
- Related PRD/ADR: <!-- if any -->

## Changes

<!-- Bullet the notable changes. -->

-

## Testing & validation

- [ ] `make validate` (or `scripts/validate.sh`) passes locally
- [ ] Tests added/updated (TDD) and passing — _N/A until code exists (Phase 0)_
- [ ] CI is green

## Checklist

- [ ] PR title is a valid Conventional Commit
- [ ] Docs updated (README/CLAUDE/process as needed)
- [ ] Planning updated (`docs/planning/status.md`, `backlog.md`)
- [ ] No secrets committed; security considerations reviewed
- [ ] Architect review requested — or the dispatch skipped and the
      orchestrator's direct verification recorded above, per the
      [ADR-0011 amendment](../docs/adr/0011-unit-of-work-cadence-and-planning-gate.md#amendment-2026-08-02--architect-review-skip-exception-for-verified-low-risk-units)

## Notes for the reviewer

<!-- Anything the architect/owner should focus on. -->
