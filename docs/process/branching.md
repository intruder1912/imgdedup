# Branching Model

`imgdedup` uses **trunk-based development**: a single long-lived branch, `main`,
with short-lived topic branches merged via reviewed pull requests.

## `main`

- Always releasable and protected (see
  [`../adr/0003-main-branch-protection-and-merge-model.md`](../adr/0003-main-branch-protection-and-merge-model.md)).
- No direct pushes. All changes arrive through PRs.
- Linear history (squash-merge only).

## Topic branches

Create from up-to-date `main`. A **branch = one unit of work**: it carries
everything that unit needs — design (PRD/ADR if applicable), TDD code, technical
and user docs, **and** the planning/status update — into one squash PR. Classify
the unit up front as **feature**, **fix**, or **chore**, which sets the cadence
(prefix, lifetime, CI weight, review kind); see
[ADR-0011](../adr/0011-unit-of-work-cadence-and-planning-gate.md). Docs that
**accompany** a feature ride the feature branch; **standalone** doc/planning work
is its own chore unit.

| Prefix | Use for |
|---|---|
| `feature/` | New product capability or behavior |
| `fix/` | Bug fix |
| `chore/` | Tooling, CI, deps, repo maintenance |
| `docs/` | Documentation, PRDs, ADRs, planning |
| `refactor/` | Internal change with no behavior change |
| `test/` | Test-only additions/improvements |

Naming: `prefix/<short-kebab-slug>`, optionally suffixed with a task id, e.g.
`feature/recursive-scan`, `docs/prd-0001-scope`, `fix/exif-orientation-1042`.

## Lifecycle

1. `git switch -c feature/<slug>` from current `main`.
2. Commit in focused increments (Conventional Commits).
3. Keep up to date with `main` (rebase preferred to preserve linear history).
4. **Hold the PR.** Pushing to a topic branch triggers no CI — work and push
   freely. Open the PR only once the unit is **converged and architect-reviewed**;
   it opens in **ready** state (not draft).
5. The **PR title** is the Conventional Commit that lands on `main`.
6. After merge, the branch is **auto-deleted**. Delete your local copy too.

## Merge strategy

**Squash-only.** The PR title becomes the single commit on `main`, so it must be
a valid Conventional Commit. Merge commits and rebase-merge are disabled at the
repository level. See [`commits.md`](commits.md) and [`release.md`](release.md).
