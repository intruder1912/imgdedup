# Release Process

`imgdedup` uses [Semantic Versioning](https://semver.org/) with **automated**
releases derived from the [Conventional Commit](commits.md) history, via
[semantic-release](https://semantic-release.gitbook.io/) — **published on
demand, not on merge** (owner ruling, 2026-08-05). The mechanism is decided in
[ADR-0004](../adr/0004-conventional-commits-semver-and-releases.md) and
[ADR-0015](../adr/0015-release-publishing-tag-and-github-release.md).

## How it works

1. PRs are squash-merged into `main`; each merge adds one Conventional Commit.
   **A merge publishes nothing by itself.** The `CI` and `Security` workflows
   run on the push to `main`; that is all.
2. When a release is wanted, someone dispatches the release workflow
   ([`../../.github/workflows/release.yml`](../../.github/workflows/release.yml))
   against `main`:

   ```sh
   gh workflow run release.yml --ref main   # then: gh run watch
   ```

3. Its first step **refuses to release a commit that has not passed CI**: it
   checks that the dispatched ref is `refs/heads/main` and that the required
   contexts `ci-gate` and `security` are both recorded `completed` +
   `success` for that exact commit, and fails with a clear error otherwise
   (still running, red, never ran, cancelled — all refused). This is the
   explicit replacement for the old "trigger on green CI" behaviour; see the
   [2026-08-05 amendment to ADR-0015](../adr/0015-release-publishing-tag-and-github-release.md#amendment-2026-08-05--manual-only-release-trigger-task-0333b).
4. It then runs semantic-release against that commit:
   - analyzes commits since the last release,
   - computes the next SemVer version (`fix:` → patch, `feat:` → minor,
     `BREAKING CHANGE` → major),
   - creates the `v<version>` Git tag,
   - creates the **GitHub Release** with the generated notes,
   - attaches prebuilt cross-platform binaries (TASK-0211; see
     [Distribution targets](#distribution-targets) below).

No release happens when only release-neutral commits (`docs`, `chore`, `ci`,
`test`, `refactor`, `style`) are present since the last tag — the run finishes
green having published nothing.

### Why on demand

Every `feat:`/`fix:` merge used to publish, which produced 12 releases in four
days and made the four-platform `assets` matrix 27% of the repository's entire
Actions bill (485 billed minutes over that window) for a private project where
nobody was waiting on any of those versions. Batching costs nothing in version
arithmetic: semantic-release computes the next version from **all** commits
since the last tag, so one dispatch releases everything accumulated. The cost
is that a merge worth shipping sits unreleased until someone asks — so ask,
when it is worth shipping.

If the dispatch is refused because `main`'s tip has no green CI (this is
possible without anything being broken: `main` is no longer `strict`, and CI on
`main` may still be running), wait for it or re-run it, then dispatch again.

Two more ways a run can finish **green having published nothing**, both worth
checking the log for rather than assuming a version was cut:

- only release-neutral commits accumulated since the last tag (above); or
- a merge landed in the seconds between the dispatch and the checkout, so the
  commit the run resolved to is now behind `main` and semantic-release skips.
  Nothing retries automatically now that merging does not publish — just
  dispatch again.

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
  is the tag. The `assets` job stamps the version into the binaries at build
  time (TASK-0211), so a published binary reports its own version even though
  the in-tree value never changes.

## Versioning policy

- Pre-`1.0.0`: the API/CLI is considered unstable; `feat:` bumps the minor,
  `fix:` the patch. A `BREAKING CHANGE` before 1.0.0 would jump straight to
  `1.0.0`, so mark breaking changes deliberately while in 0.x.
- `1.0.0` marks the first stable, supported release (target: end of Phase 5).

## Status

Automated publishing is **wired, enabled, and has shipped its first release**:
`v0.0.0` was pushed as the one-time baseline tag on 2026-08-02, and the next
green CI run on `main` published **v0.1.0** the same day — both steps verified
in advance by dry run (see below). The bootstrap guard (skip green, with a
`::notice::`, while no `v*` tag exists) stays in the workflow permanently as a
harmless no-op safeguard; it never triggers again once tags accumulate.

Releases up to and including **v0.5.1** (2026-08-05) were published
automatically on merge. From 2026-08-05 the workflow is dispatched manually, as
described above.

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
- **Prebuilt cross-platform binaries** attached to each release (TASK-0211;
  design recorded in the
  [2026-08 amendment to ADR-0015](../adr/0015-release-publishing-tag-and-github-release.md#amendment-2026-08-02--binary-artifacts-task-0211)).
  Built inside the `release` workflow's `assets` job (a tag pushed with
  `GITHUB_TOKEN` triggers no further workflow, so there is no separate
  `on: push: tags:` build), gated on the `release` job detecting that a new
  tag was actually published.

### Artifacts

Four targets, each built natively (no cross-compilation) on a GitHub-hosted
runner:

| Target | Runner | Archive |
|---|---|---|
| `x86_64-unknown-linux-gnu` | `ubuntu-24.04` | `.tar.gz` |
| `aarch64-apple-darwin` | `macos-latest` (Apple Silicon) | `.tar.gz` |
| `x86_64-apple-darwin` | `macos-15-intel` | `.tar.gz` |
| `x86_64-pc-windows-msvc` | `windows-latest` | `.zip` |

The Linux artifact carries a **glibc ≥ 2.39** floor (Ubuntu 24.04/Debian
13/Fedora 40+). `musl` static Linux and any `aarch64` Linux/Windows targets are
intentionally **not** shipped yet — tracked as a follow-up
(TASK-0326) to reconsider before a public release, not blocking this private
one.

Each archive is named `imgdedup-v<version>-<target-triple>.{tar.gz,zip}` and
contains one top-level directory of the same name holding the `imgdedup`
binary, `LICENSE`, and `README.md`. A same-named `.sha256` file (one per
artifact, `sha256sum -c` format) accompanies each archive:

```sh
# verify a downloaded artifact (macOS lacks sha256sum: use `shasum -a 256 -c`)
sha256sum -c imgdedup-v0.1.0-x86_64-unknown-linux-gnu.tar.gz.sha256
```

The binary's own version is stamped with the real released version at build
time (see the Cargo.toml comment above `version` and the `assets` job in
`release.yml`) — Cargo.toml's in-tree value never changes. This is asserted
via `imgdedup -V`, which prints exactly `imgdedup <version>`; `imgdedup
--version` additionally carries a GNU-style license/copyright block (ADR-0014's
TASK-0219 amendment) whose first line is checked against the same stamped
value.

**Not in scope:** GPG signing, macOS codesigning/notarization, Windows
Authenticode. Downloaded binaries are unsigned: macOS Gatekeeper will quarantine
the archive (clear it with `xattr -d com.apple.quarantine <file>` after
verifying the checksum) and Windows SmartScreen will warn. Revisit only if/when
this ships publicly with an owner-managed signing identity.

**Recovering a failed or missing asset leg.** If one platform's build/upload
fails, re-run just that job from the Actions UI ("Re-run failed jobs") — the
`release` job's outputs (the tag/version) are preserved from the original run.
To attach binaries to a release that has none at all (as v0.1.0 did — it was
published before this mechanism existed), dispatch the `Release` workflow
manually with the `tag` input set to the existing tag (e.g. `v0.1.0`): this
skips semantic-release entirely and runs only the `assets` job against that
tag.

### Local pre-flight rehearsal

Before dispatching a real release, `scripts/release-preflight.sh` (or `make
release-preflight`) rehearses the build/package/verify mechanics on this
machine, without creating a tag, touching `main`, or uploading anything:

```sh
make release-preflight                    # or: scripts/release-preflight.sh
scripts/release-preflight.sh --ref v0.5.1 --version 0.5.1 --tag v0.5.1  # rehearse a specific released ref
scripts/release-preflight.sh --keep       # keep the worktree on exit, for inspecting a failure
```

`--ref` builds from that ref's **committed** state — uncommitted working-tree
changes are never rehearsed, matching what a real release actually publishes
(`release.yml` checks out a tag, never a working tree).

It builds two of the four legs — a glibc Linux target inside a rootless
[podman](https://podman.io/) container, and `aarch64-apple-darwin` natively
when run on Apple Silicon macOS — calling
[`scripts/build-release-artifact.sh`](../../scripts/build-release-artifact.sh),
the same script the `assets` job itself calls, so the two cannot drift apart on
stamping, building, the `--version` assertion, or packaging. Windows and Intel
macOS stay CI-only (no local host for either here). Everything runs inside a
disposable `git worktree`; the real working tree, including `Cargo.toml`, is
never touched.

**On an Apple Silicon dev machine, the Linux leg is not
`x86_64-unknown-linux-gnu`.** Measured directly: running the pinned x86_64 Rust
toolchain under podman's qemu-based emulation is not just slow but unreliable
— `rustc` itself segfaulted under emulation, before any of this project's code
was even compiled. The script does not force that emulation; it lets the
container build for its own native platform instead (`aarch64-unknown-linux-gnu`
on an Apple Silicon host) and says so plainly in its output. That still
exercises the real build/package/verify pipeline end-to-end — it just isn't the
specific leg `release.yml` ships. Run it on an x86_64 host (or trust CI) for a
byte-for-byte rehearsal of that leg. See
[ADR-0015's 2026-08-05 TASK-0334 amendment](../adr/0015-release-publishing-tag-and-github-release.md#amendment-2026-08-05--local-release-pipeline-pre-flight-task-0334)
for the full reasoning.

### Licensing note (GPLv3 §6)

Distributing a binary triggers the GPLv3 §6 obligation to make the
corresponding source available. While this repository is **private**, every
recipient of a release asset already has repository (source) access by the
same grant, and GitHub additionally attaches an auto-generated source archive
to every Release — so no separate written offer or source bundle is needed
right now. `LICENSE` is still bundled in each archive as GPLv3 §4/§5 hygiene.
This position **must be re-checked before the repository goes public**
(TASK-0219), at which point already-published assets become anonymously
downloadable and the §6 analysis above no longer holds — see
[ADR-0014](../adr/0014-project-license-gpl-3.0-only.md).
