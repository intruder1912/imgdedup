# Release Process

`imgdedup` uses [Semantic Versioning](https://semver.org/) with **automated**
releases derived from the [Conventional Commit](commits.md) history, via
[semantic-release](https://semantic-release.gitbook.io/).

## How it works

1. PRs are squash-merged into `main`; each merge adds one Conventional Commit.
2. On qualifying pushes to `main`, the release workflow
   ([`../../.github/workflows/release.yml`](../../.github/workflows/release.yml))
   runs semantic-release, which:
   - analyzes commits since the last release,
   - computes the next SemVer version (`fix:` → patch, `feat:` → minor,
     `BREAKING CHANGE` → major),
   - updates `CHANGELOG.md`,
   - creates the Git tag and the **GitHub Release** with generated notes,
   - (from Phase 2) attaches prebuilt cross-platform binaries.

No release happens when only release-neutral commits (`docs`, `chore`, `ci`,
`test`, `refactor`, `style`) are present.

## Versioning policy

- Pre-`1.0.0`: the API/CLI is considered unstable; minor/patch semantics follow
  the release tool's 0.x configuration.
- `1.0.0` marks the first stable, supported release (target: end of Phase 5).

## Status — Phase 0 skeleton

The release workflow is a **manual-dispatch placeholder** and does not run on
push. Even semantic-release's dry-run verifies it can push a tag to `main`, which
branch protection blocks (no direct pushes) — so automated release cannot run
until the push mechanism below is in place. The semantic-release configuration
lives in `.releaserc.json` ready for Phase 2. Enabling real releases is
**TASK-0203**.

### Known constraint to resolve in Phase 2

`main` is a protected branch with no direct pushes. semantic-release needs to
push a tag (and, if configured, the updated `CHANGELOG.md`) to `main`. Options to
be decided in the release-tooling work of Phase 2 and recorded in
[`../adr/0004-conventional-commits-semver-and-releases.md`](../adr/0004-conventional-commits-semver-and-releases.md):

- A dedicated **release bot / GitHub App** (or fine-grained PAT) with a
  branch-protection bypass for the release job.
- Or configure semantic-release to publish the release/tag and changelog **as
  release assets/notes only**, without committing back to `main`.

Until one is chosen and wired, real publishing stays disabled (dry-run).

## Distribution targets

- **GitHub Releases** with generated notes (always).
- **Prebuilt cross-platform binaries** attached to each release (added in
  Phase 2 once the build exists; this was the agreed distribution model).
