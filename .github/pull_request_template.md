<!--
PR title MUST be a Conventional Commit (e.g. "feat(scan): add recursive walk").
Squash-merge uses the title as the commit on `main` and drives the release.
See docs/process/commits.md.
-->

## What & why

<!-- What does this change do, and why? Link the issue/PRD/ADR. -->

- Issue: <!-- Closes #123 -->
- Related PRD/ADR: <!-- if any -->

## Changes

<!-- Bullet the notable changes. -->

-

## Testing & validation

- [ ] `make validate` (or `scripts/validate.sh`) passes locally
- [ ] Tests added/updated (TDD) and passing, or the change has no code
- [ ] CI is green

## Checklist

- [ ] PR title is a valid Conventional Commit
- [ ] Docs updated (README/CLAUDE/process as needed)
- [ ] Closes its issue (`Closes #…` above), or is a trivial fix that needs none (CONTRIBUTING.md)
- [ ] No secrets committed; security considerations reviewed
- [ ] (Maintainer; leave unchecked as an outside contributor, the owner runs
      the review.) Architect review requested — or the dispatch skipped and the
      orchestrator's direct verification recorded above, per the
      [ADR-0011 amendment](https://github.com/intruder1912/imgdedup/blob/main/docs/adr/0011-unit-of-work-cadence-and-planning-gate.md#amendment-2026-08-02--architect-review-skip-exception-for-verified-low-risk-units)

## Notes for the reviewer

<!-- Anything the architect/owner should focus on. -->
