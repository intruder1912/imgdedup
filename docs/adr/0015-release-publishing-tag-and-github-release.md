# ADR-0015: Release publishing — tag + GitHub Release, no commit back to `main`

- **Status:** accepted
- **Date:** 2026-08-02
- **Deciders:** architect, human owner
- **Related:** [ADR-0003](0003-main-branch-protection-and-merge-model.md),
  [ADR-0004](0004-conventional-commits-semver-and-releases.md),
  [ADR-0014](0014-project-license-gpl-3.0-only.md), TASK-0203, TASK-0211

## Context and problem statement

[ADR-0004](0004-conventional-commits-semver-and-releases.md) chose Conventional
Commits + SemVer + **semantic-release**, but left one question open and
publishing disabled (`dryRun: true`, a manual-dispatch placeholder workflow):

> `main` forbids direct pushes (ADR-0003), but semantic-release needs to push a
> tag (and optionally an updated `CHANGELOG.md`). […] The chosen mechanism will
> be recorded here (or in a superseding ADR) when wired.

This ADR closes that loop (TASK-0203). ADR-0004 is **not** superseded — its
decisions (Conventional Commits, SemVer, semantic-release, human-controlled
merge gate) stand unchanged; this records the publishing mechanism it deferred.

### The constraint, stated precisely

The premise in ADR-0004 — "branch protection blocks the tag push" — conflates
two different refs. Checked against this repository's actual configuration
(`gh api repos/:owner/:repo/branches/main/protection` and `…/rulesets`):

- Branch protection here is a **classic branch protection rule on `main`**,
  i.e. it targets `refs/heads/main`. Required contexts: `markdownlint`,
  `spellcheck`, `structure`, `pr-title`, `secret-scan`, `ci-gate`; strict
  (up-to-date) required; linear history; 0 required approvals.
- **There are no repository rulesets at all** (`rulesets` returns `[]`), so
  **no tag protection** exists: `refs/tags/*` is unguarded.

So a *tag* push is not a *branch* push. What semantic-release actually pushes
(verified against its source) is:

| Step | Command | Ref namespace |
|---|---|---|
| verify auth | `git push --dry-run --no-verify -- <url> HEAD:main` | none — a dry run sends no ref update |
| tag | `git tag v<version> <sha>` then `git push --tags -- <url>` | `refs/tags/*` |
| notes | `git push -- <url> refs/notes/semantic-release-v<version>` | `refs/notes/*` |

A commit is pushed to `refs/heads/main` **only** by the optional
`@semantic-release/git` plugin (the plugin that commits an updated
`CHANGELOG.md`/version files back). Without it, nothing touches the protected
ref, and the workflow's own `GITHUB_TOKEN` with `contents: write` is sufficient.

## Decision drivers

- **Do not weaken ADR-0003.** No branch-protection bypass, no exception list.
- **No owner-only secret management.** A bot account, GitHub App, or
  fine-grained PAT is a sensitive, manual, expiring, owner-only setup step and a
  standing credential with write access. Avoid if the job can run on the
  ephemeral, automatically-scoped `GITHUB_TOKEN`.
- **Least privilege** for a job that has write access and runs on `main`.
- **Supply-chain hygiene:** the release job is the most privileged job in the
  repo; its Node dependency tree must be reproducible, not resolved fresh on
  every run.
- **No accidental releases.** Turning automation on must not publish a version
  nobody chose.
- **Single source of truth for the changelog** — not two that drift.

## Considered options

1. **Release bot / GitHub App / PAT with a protection bypass**, plus
   `@semantic-release/git` committing `CHANGELOG.md` back to `main`
   (ADR-0004 option a).
2. **Tag + GitHub Release only, no commit-back** (ADR-0004 option b) — the
   changelog of record becomes the GitHub Release notes.
3. **Release-PR model** (release-please-style): a bot opens a PR carrying the
   changelog and version bump; merging it releases.

## Decision outcome

**Chosen: option 2 — publish the tag and the GitHub Release; never commit back
to `main`.**

Because the tag lives in `refs/tags/*` and the release is created through the
API, the entire flow runs on the workflow's built-in `GITHUB_TOKEN` with a
single elevated permission (`contents: write`). No bot, no App, no PAT, no
bypass, no long-lived secret, and ADR-0003 stays exactly as written.

### Shape of the implementation

- **`.releaserc.json`** — `dryRun` removed; plugins are
  `@semantic-release/commit-analyzer`, `@semantic-release/release-notes-generator`,
  and `@semantic-release/github`. This is the default plugin set **minus**
  `@semantic-release/npm` (this is a Rust project) and deliberately **without**
  `@semantic-release/changelog` + `@semantic-release/git` — those two are the
  commit-back pair whose absence is the whole point. All three used plugins
  ship as dependencies of `semantic-release` itself, so nothing extra is
  installed. `tagFormat` is pinned explicitly to `v${version}` rather than left
  implicit, because tag names are a compatibility surface (TASK-0211's binary
  assets and any future install script key off them).
- **`@semantic-release/github` options** are set to the least-privilege
  configuration: `successComment: false`, `failTitle: false`, `labels: false`,
  `releasedLabels: false`. Each of those features would require `issues: write`
  and/or `pull-requests: write`; disabling them keeps the token at
  `contents: write` only. The cost is that a failed release surfaces as a red
  workflow run rather than an auto-filed issue — acceptable for a
  single-maintainer repo where the owner watches the Actions tab.
- **`package.json` + `package-lock.json` + `npm ci --ignore-scripts`.** The
  repo's existing convention for Node meta-tooling is `npx -y <pkg>@<version>`
  (markdownlint, cspell, link-check) — an exact pin of the *top-level* package
  only, with ~443 transitive packages re-resolved on every run. That is
  tolerable for a `contents: read` lint job and **not** tolerable for the job
  that holds `contents: write`: a compromised patch release anywhere in the
  tree would execute with publish rights. A committed lockfile pins every
  transitive package by integrity hash, `npm ci` refuses to drift from it, and
  `--ignore-scripts` blocks install-time script execution (verified: the tool
  runs correctly installed this way). `package.json` is `private: true` and is
  never published; it exists only to hold the pin.
- **Trigger:** `workflow_run` on the **CI** workflow completing, filtered to
  `conclusion == 'success' && event == 'push' && head_branch == 'main'`, plus
  `workflow_dispatch` for manual recovery. A plain `on: push: branches: [main]`
  would race the post-merge CI run and could publish from a commit whose checks
  are still running or already red. The job checks out
  `workflow_run.head_sha` — the exact commit CI validated. If a newer commit
  reaches `main` in the meantime, semantic-release detects that the checkout is
  behind and skips; the next merge's run then releases everything accumulated,
  so nothing is lost.
- **Not gated on the `Security` workflow.** `workflow_run` fires per workflow,
  and coordinating two would mean polling the other's status. `secret-scan` is
  a **required** check on every PR, so no commit reaches `main` without a
  secret scan; the release publishes notes and (later) binaries, not new
  history. Recorded as a conscious gap, not an oversight.
- **Bootstrap guard.** A step checks for an existing `v*` tag and **skips**
  (green, with a `::notice::`) when there is none. See below for why this is
  load-bearing.

### The first release must be bootstrapped — verified, not assumed

A local dry run of the proposed configuration against `main` (semantic-release
25.0.8, `--dry-run --no-ci`) reported:

```text
✔  Allowed to push to the Git repository
ℹ  No git tag version found on branch main
ℹ  Found 51 commits since last release
ℹ  There is no previous release, the next release version is 1.0.0
```

So enabling automation with no tags present would publish **v1.0.0** on the very
next push to `main` — including the merge of this change — which is neither what
the roadmap says (v0.1.0) nor a version anyone chose. Re-running the same dry
run with a `v0.0.0` tag placed on the repository's initial commit produced
`The next release version is 0.1.0`, as intended.

Hence the two-part answer:

1. The workflow **refuses to publish while no `v*` tag exists** (it logs a
   notice and exits green), so merging this change is inert.
2. Cutting v0.1.0 (TASK-0211, session 7b) starts with the one-time bootstrap
   `git tag v0.0.0 <initial-commit> && git push origin v0.0.0`, documented in
   [`../process/release.md`](../process/release.md). Pushing a tag needs no
   special rights — that is the same fact this whole ADR rests on.

### Consequences

- Positive: real releases with **no new credential, no bypass, no owner-managed
  secret**, and ADR-0003 untouched. The privileged surface is one job with one
  extra permission on a pinned dependency tree.
- Positive: the release is gated on green CI for the exact commit released.
- Negative: **`CHANGELOG.md` is no longer generated.** The changelog of record
  becomes the **GitHub Releases** page. The in-tree file is reduced to a short
  pointer at it (kept because the CI `structure` job requires the file, and
  because a stale auto-generated-looking changelog is worse than a signpost).
  Anyone wanting release notes offline reads them from the Releases API.
- Negative: **the version in `Cargo.toml` is not bumped by the release.** The
  released version lives in the tag, and `imgdedup --version` (which prints
  `CARGO_PKG_VERSION`) would drift from it. This is the real price of not
  committing back, and it must be handled by **TASK-0211**: stamp the version
  into `Cargo.toml` at build time inside the release job (e.g. an
  `@semantic-release/exec` `prepareCmd` before the cross-platform build), so
  published binaries report their own release version while the in-tree value
  stays a development placeholder. Until then, treat `Cargo.toml`'s `version`
  as non-authoritative.
- Negative: **a tag pushed with `GITHUB_TOKEN` does not trigger other
  workflows** (GitHub suppresses `GITHUB_TOKEN`-originated events other than
  `workflow_dispatch`/`repository_dispatch` to prevent recursion). TASK-0211
  therefore **cannot** use an `on: push: tags:` workflow to build the binaries;
  it must build and attach them **within the same release job**, before
  `@semantic-release/github` publishes. Knowing this now avoids designing 7b
  into a dead end.
- Residual risk: semantic-release's `verifyAuth` step runs `git push --dry-run`
  against `main`. A dry-run push transmits no ref update, so branch protection
  has nothing to evaluate — and the step passed in the local dry run above,
  though with the owner's (admin) token, which does not prove the Actions token
  case. **Contingency if the first real run fails with `EGITNOPERMISSION`:**
  that is the one scenario in which option 1's credential becomes unavoidable;
  the fix would be a GitHub App token (owner action) used *only* for
  authentication — the plugin set would not change. This is cheap to detect
  (the first run is a manual dispatch away) and does not affect anything else.
- Follow-ups: TASK-0211 (binaries in-job + version stamping + the v0.0.0
  bootstrap), TASK-0219 (GPLv3 §6 source-offer wording once binaries ship —
  see [ADR-0014](0014-project-license-gpl-3.0-only.md)).

## Pros and cons of the options

### Option 1 — release bot / GitHub App / PAT with bypass

- Good: `CHANGELOG.md` and `Cargo.toml`'s version stay accurate in the tree;
  the repo is self-describing offline.
- Good: the well-trodden path — most semantic-release documentation assumes it.
- Bad: creates a **standing credential with write access to a protected
  branch** and an explicit hole in ADR-0003's protection — the single largest
  security regression available to us here, in exchange for a convenience.
- Bad: owner-only manual setup (App creation/installation or PAT issuance,
  secret storage, expiry rotation) that no agent can perform or maintain.
- Bad: the commit-back lands on `main` **outside** the PR flow with no review,
  breaking the "everything on `main` came through a reviewed PR" invariant; with
  strict required status checks it also races other merges.

### Option 2 — tag + GitHub Release only (chosen)

- Good: no new credentials, no bypass, protection model intact; ephemeral
  token, single extra permission, least privilege.
- Good: exactly one changelog (the Releases page) instead of two that drift.
- Bad: `CHANGELOG.md` and `Cargo.toml`'s version are not maintained in-tree
  (mitigations above).
- Bad: reading the changelog needs GitHub (or the API), which matters more once
  the repo is public.

### Option 3 — release-PR model

- Good: fits branch protection by construction; the human merge gate is already
  the model this project uses.
- Bad: **`GITHUB_TOKEN` cannot make it work.** Creating PRs with it requires the
  "Allow GitHub Actions to create and approve pull requests" setting, and PRs it
  opens **do not trigger workflow runs** — so the required checks would never
  report and the release PR could never be merged. It needs the same standing
  credential as option 1, without option 1's simplicity.
- Bad: ADR-0004 already rejected the release-PR cadence (an extra human gate on
  top of the merge gate that already exists).

## Links

- [semantic-release configuration](https://semantic-release.gitbook.io/semantic-release/usage/configuration)
  and [`@semantic-release/github`](https://github.com/semantic-release/github#options)
- [About protected branches](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches)
  — protection rules target branches; tag protection is a separate ruleset.
- [Triggering a workflow](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows)
  — `GITHUB_TOKEN`-triggered events do not create workflow runs.
- [`../process/release.md`](../process/release.md) — the operational procedure.
