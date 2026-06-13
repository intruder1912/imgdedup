# ADR-0004: Conventional Commits, SemVer & automated releases

- **Status:** accepted
- **Date:** 2026-06-13
- **Deciders:** architect, human owner
- **Related:** [ADR-0003](0003-main-branch-protection-and-merge-model.md)

## Context and problem statement

We want predictable versioning and a clear, low-effort release process that
publishes successful new versions automatically, with a human in control of when
work merges.

## Decision drivers

- Predictable, automated versioning from change history.
- Minimal manual release toil.
- Clean history (we squash-merge — ADR-0003).
- Distribution as GitHub Releases with prebuilt binaries.

## Considered options

1. **Conventional Commits + SemVer + semantic-release.** Releases published
   automatically on qualifying merges to `main`.
2. **Conventional Commits + release-please.** A "release PR" accumulates changes;
   merging it cuts the release (extra human gate, slower cadence).
3. Manual tagging and release notes.

## Decision outcome

**Chosen:** Conventional Commits + Semantic Versioning + **semantic-release**.

- Every commit/PR title is a [Conventional Commit](../process/commits.md);
  because merges are squash-only, the **PR title becomes the release-driving
  commit** on `main` and is linted in CI.
- semantic-release analyzes commits, computes the next SemVer version, generates
  `CHANGELOG.md` and **GitHub Release** notes, tags the release, and (from
  Phase 2) attaches prebuilt cross-platform binaries.
- The human gate is at **merge time** (ADR-0003): nothing reaches `main` without
  the owner merging, so "auto-publish on merge" is still owner-controlled.

### Phase 0 status

The release workflow is a **manual-dispatch placeholder** that does not run on
push. semantic-release verifies push access to `main` even in dry-run, and branch
protection blocks that — so no release runs until the push mechanism below is
wired (TASK-0203). The semantic-release config lives in `.releaserc.json`.

### Known constraint — pushing to protected `main`

`main` forbids direct pushes (ADR-0003), but semantic-release needs to push a tag
(and optionally an updated `CHANGELOG.md`). To be resolved in Phase 2:

- Use a **release bot / GitHub App or fine-grained PAT** with a branch-protection
  bypass scoped to the release job; **or**
- Configure semantic-release to publish the tag/release and changelog **as
  release artifacts/notes only**, without committing back to `main`.

The chosen mechanism will be recorded here (or in a superseding ADR) when wired.

### Consequences

- Positive: hands-off, consistent releases tied directly to change history.
- Negative: semantic-release is a Node tool, so the **release/CI tooling uses
  Node regardless of the product language** (a deliberate, isolated meta-tooling
  choice — it does not pre-decide the product stack).
- Requires resolving the protected-branch push constraint before real publishing.

## Links

- [Conventional Commits](https://www.conventionalcommits.org/), [SemVer](https://semver.org/),
  [semantic-release](https://semantic-release.gitbook.io/)
- [`../process/release.md`](../process/release.md)
