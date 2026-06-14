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

For **feature** work the unit is a **PRD slice** — a coherent, shippable increment
of a PRD (larger than one task, smaller than the whole PRD), reviewed continuously
on the branch and opened as a single PR when complete
([ADR-0013](../adr/0013-prd-slice-delivery-cadence.md)). `fix` and `chore` branches
keep the small one-unit-one-PR cadence. A slice branch lives longer than a task
branch, so **rebase on `main`** regularly to stay current and bounded.

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
   it opens in **ready** state (not draft). For a **PRD slice**, the architect's
   reviews happen **in-progress on the branch** (implementer-requested, or
   product-owner/orchestrator-demanded), logged under
   [`docs/planning/reviews/`](../planning/reviews/); the PR opens only when the
   product-owner has accepted the slice's requirements and delivered its docs
   ([ADR-0013](../adr/0013-prd-slice-delivery-cadence.md)).
5. The **PR title** is the Conventional Commit that lands on `main`.
6. After merge, the branch is **auto-deleted**. Delete your local copy too.

## Merge strategy

**Squash-only.** The PR title becomes the single commit on `main`, so it must be
a valid Conventional Commit. Merge commits and rebase-merge are disabled at the
repository level. See [`commits.md`](commits.md) and [`release.md`](release.md).
