# Branching Model

`imgdedup` uses **trunk-based development**: a single long-lived branch, `main`,
with short-lived topic branches merged via reviewed pull requests.

## `main`

- Always releasable and protected (see
  [`../adr/0003-main-branch-protection-and-merge-model.md`](../adr/0003-main-branch-protection-and-merge-model.md)).
- No direct pushes. All changes arrive through PRs.
- Linear history (squash-merge only).

## Topic branches

Create from up-to-date `main`. Keep them short-lived and focused (one task).

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
4. Open a PR; the **PR title** is the Conventional Commit that lands on `main`.
5. After merge, the branch is **auto-deleted**. Delete your local copy too.

## Merge strategy

**Squash-only.** The PR title becomes the single commit on `main`, so it must be
a valid Conventional Commit. Merge commits and rebase-merge are disabled at the
repository level. See [`commits.md`](commits.md) and [`release.md`](release.md).
