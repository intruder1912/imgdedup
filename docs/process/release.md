# Release Process

`imgdedup` uses [Semantic Versioning](https://semver.org/) with **automated**
releases derived from the [Conventional Commit](commits.md) history, via
[semantic-release](https://semantic-release.gitbook.io/). The mechanism is
decided in [ADR-0004](../adr/0004-conventional-commits-semver-and-releases.md)
and [ADR-0015](../adr/0015-release-publishing-tag-and-github-release.md).

## How it works

1. PRs are squash-merged into `main`; each merge adds one Conventional Commit.
2. The `CI` workflow runs on the push to `main`. When it finishes **green**, it
   triggers the release workflow
   ([`../../.github/workflows/release.yml`](../../.github/workflows/release.yml)),
   which checks out the exact commit CI validated and runs semantic-release:
   - analyzes commits since the last release,
   - computes the next SemVer version (`fix:` → patch, `feat:` → minor,
     `BREAKING CHANGE` → major),
   - creates the `v<version>` Git tag,
   - creates the **GitHub Release** with the generated notes,
   - (from TASK-0211) attaches prebuilt cross-platform binaries.

No release happens when only release-neutral commits (`docs`, `chore`, `ci`,
`test`, `refactor`, `style`) are present since the last tag.

## Why nothing is committed back to `main`

`main` is protected and forbids direct pushes (ADR-0003). semantic-release does
not need to push to it: the tag it creates lives in `refs/tags/*`, and branch
protection targets `refs/heads/main`. So the release runs on the workflow's own
`GITHUB_TOKEN` with `contents: write` — **no release bot, GitHub App, personal
access token, or protection bypass**. The plugins that would commit back
(`@semantic-release/git`, `@semantic-release/changelog`) are deliberately not
used. Full reasoning and trade-offs: [ADR-0015](../adr/0015-release-publishing-tag-and-github-release.md).

Two consequences worth remembering:

- **`CHANGELOG.md` is not generated.** Release notes live on the GitHub Release
  (`gh release view v<version>`); the in-tree file just points there.
- **`Cargo.toml`'s `version` is not bumped by a release.** The released version
  is the tag. TASK-0211 stamps the version into the binaries at build time
  inside the release job, so a published binary reports its own version.

## Versioning policy

- Pre-`1.0.0`: the API/CLI is considered unstable; `feat:` bumps the minor,
  `fix:` the patch. A `BREAKING CHANGE` before 1.0.0 would jump straight to
  `1.0.0`, so mark breaking changes deliberately while in 0.x.
- `1.0.0` marks the first stable, supported release (target: end of Phase 5).

## Status

Automated publishing is **wired and enabled** (TASK-0203): the release workflow
runs on every green CI run for `main`. One step remains before it can publish.

### One-time bootstrap of the version line (TASK-0211)

semantic-release with **no `v*` tag in the repository** treats the entire
history as the first release and publishes **v1.0.0** — verified by a dry run.
The roadmap calls for `v0.1.0`, so the release workflow **skips (green, with a
notice) while no `v*` tag exists**. To cut the first release:

```sh
# One-time: baseline the version line at the repository's first commit.
git tag v0.0.0 "$(git rev-list --max-parents=0 origin/main)"
git push origin v0.0.0
```

The next green CI run on `main` then publishes **v0.1.0** (the accumulated
`feat:` commits bump the minor from the `0.0.0` baseline) — also verified by a
dry run. Pushing a tag needs no special permissions; it is not a branch.

After that the guard never triggers again, because tags accumulate.

### Verifying a release without publishing

```sh
npm ci --ignore-scripts
GITHUB_TOKEN=$(gh auth token) npx semantic-release --dry-run --no-ci
```

A dry run creates and pushes nothing; it prints the version it *would* release
and the notes it *would* write.

### Known advisories in the release tooling

`npm audit` reports DoS advisories (`brace-expansion`, `node-tar`) inside the
`npm` CLI that ships as a dependency of `@semantic-release/npm`. That plugin is
**not** in our plugin list and the `npm` CLI is never invoked by a release, so
the code is unreachable here; it also cannot be patched independently, as it is
bundled inside the `npm` package. Re-check on each Dependabot bump of
`semantic-release`. No CI check gates on `npm audit`.

## Distribution targets

- **GitHub Releases** with generated notes (always).
- **Prebuilt cross-platform binaries** attached to each release (TASK-0211).
  Note that a tag pushed with `GITHUB_TOKEN` does **not** trigger further
  workflows, so the binaries must be built inside the release job itself rather
  than by a separate `on: push: tags:` workflow.
- Distributing a binary to anyone without repository access triggers the
  GPLv3 §6 obligation to provide the corresponding source — see
  [ADR-0014](../adr/0014-project-license-gpl-3.0-only.md).
