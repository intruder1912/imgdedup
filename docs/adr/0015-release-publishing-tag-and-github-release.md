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
- **Trigger:** *(superseded 2026-08-05 — see the
  [manual-only trigger amendment](#amendment-2026-08-05--manual-only-release-trigger-task-0333b);
  the CI-green guarantee below is preserved, by an explicit check rather than
  by the trigger.)* `workflow_run` on the **CI** workflow completing, filtered to
  `conclusion == 'success' && event == 'push' && head_branch == 'main'`, plus
  `workflow_dispatch` for manual recovery. A plain `on: push: branches: [main]`
  would race the post-merge CI run and could publish from a commit whose checks
  are still running or already red. The job checks out
  `workflow_run.head_sha` — the exact commit CI validated. If a newer commit
  reaches `main` in the meantime, semantic-release detects that the checkout is
  behind and skips; the next merge's run then releases everything accumulated,
  so nothing is lost. *(That last clause lapsed with the trigger: there is no
  "next merge's run" any more. A dispatch whose `github.sha` fell behind
  `main`'s tip — a merge landing in the seconds between dispatch and checkout —
  still skips, but now finishes green having published nothing and retries
  nothing. Recovery is to dispatch again; noted in
  [`../process/release.md`](../process/release.md).)*
- **Not gated on the `Security` workflow.** *(No longer true as of 2026-08-05 —
  the [manual-only trigger amendment](#amendment-2026-08-05--manual-only-release-trigger-task-0333b)
  verifies `security` as well as `ci-gate`, because the reason for this gap was
  the trigger mechanism that amendment removes.)* `workflow_run` fires per workflow,
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

## Amendment 2026-08-02 — binary artifacts (TASK-0211)

This ADR named TASK-0211 as the owner of three loose ends: which binaries to
ship, how the released version reaches them given `Cargo.toml` is never
bumped, and how to build them at all given a `GITHUB_TOKEN`-pushed tag
triggers no other workflow. All three are now resolved and implemented in
`.github/workflows/release.yml`'s `assets` job.

**Platform matrix — four native builds, no cross-compilation.**
`x86_64-unknown-linux-gnu` (`ubuntu-24.04`), `aarch64-apple-darwin`
(`macos-latest`, Apple Silicon), `x86_64-apple-darwin` (`macos-15-intel`), and
`x86_64-pc-windows-msvc` (`windows-latest`). Every artifact is built and smoke
tested (`--version` check) on the architecture it targets. Deliberately
excluded: `x86_64-unknown-linux-musl` (blake3 pulls in `cc`, so a musl build
needs an untested sysroot in the most privileged job right before the first
cut), and any `aarch64` Linux/Windows target (hosted ARM runners are not free
on private repos, and nothing has asked for them). Consequence: the Linux
artifact carries a glibc ≥ 2.39 floor. `ubuntu-22.04` was rejected as a lower
floor because its deprecation (with deliberate job failures) begins
2026-09-17. Tracked as a follow-up (TASK-0326) to widen before a public
release, not before. `macos-15-intel` is a **dated** label, guaranteed only
through August 2027 — recorded so its eventual removal is a scheduled decision
rather than a surprised outage.

**Version stamping — patch `Cargo.toml` transiently in each build runner's
checkout, zero Rust source changes.** The CLI already derives `--version` from
`CARGO_PKG_VERSION` (`#[command(version, …)]`), so the `assets` job
line-anchors on `^version =` (verified to match exactly once) and rewrites it
to the tag's version before `cargo build --release`, then asserts the built
binary's `--version` output matches. This resolves the "Cargo.toml's version
is not bumped" consequence recorded above: the in-tree placeholder never
moves — only the ephemeral runner checkout is patched — while published
binaries self-report correctly. The build cannot use `--locked`, since the
stamp makes `Cargo.lock`'s own `imgdedup` entry stale by construction; cargo
rewrites just that one entry, leaving every dependency pin untouched.

**Workflow mechanics — publish, then a gated matrix job uploads, in the same
run.** The `release` job now diffs `git tag --list 'v*'` before and after
`npm run release` to detect whether a tag actually got published (semantic-
release exposes no such output natively) and exposes `released`/`tag`/
`version` as job outputs. Those two outputs are the only values in the whole
flow that are not workflow-authored constants, and the `assets` job
interpolates them into `run:` blocks and into a `sed` replacement — so the
detect step **validates the tag against `^v<major>.<minor>.<patch>[-+…]$`
before emitting it**, at the single point where it originates. Git ref names
legally permit `$`, backticks and parentheses, so without that guard a tag name
would be a shell-expression-injection channel into the repository's most
privileged job (reachable only with write access, which already grants an
equivalent token — but a closed channel beats an argued-harmless one). A new
`assets` job (`needs: release`, gated on
`released == 'true'`) matrix-builds the four targets, each checking out the
published tag, stamping the version, building, packaging into a per-target
`.tar.gz`/`.zip` (named `imgdedup-v<version>-<target-triple>.*`, containing
`imgdedup`, `LICENSE`, `README.md`) with a same-named `.sha256`, and uploading
via `gh release upload --clobber`. Chosen over building in parallel with
semantic-release and attaching via `@semantic-release/github`'s `assets`
option because that alternative runs version computation twice, adds
artifact upload/download plumbing between jobs, and touches `.releaserc.json`
— three new failure modes to buy a property not worth much here: the released
commit's CI already compiled and tested on all three OSes minutes earlier, and
a failed leg is recoverable (`--clobber` plus GitHub's "re-run failed jobs",
which preserves the `release` job's outputs). No caching action runs in the
`assets` job — a job holding `contents: write` should not restore a cache
another job could have poisoned.

## Amendment 2026-08-03 — non-deprecated plugin-option spelling and an explicit fork gate (TASK-0324/TASK-0325)

Two nits from this ADR's own review, closed together as a small chore. The
least-privilege *decision* recorded above is unchanged in both cases — only
the spelling of one config and the legibility of one condition.

**TASK-0324 — `.releaserc.json`'s `@semantic-release/github` options.** The
least-privilege configuration above was written as `successComment: false`,
`failTitle: false`; the currently-pinned `@semantic-release/github@12.0.9`
already emits a runtime `DEPRECATION` warning for that form (verified against
its source, `lib/success.js` / `lib/fail.js`) and recommends
`successCommentCondition` / `failCommentCondition` instead. Checked directly
against that same source that the replacement is behaviorally identical: both
old and new spellings short-circuit the same "skip commenting on issues and
pull requests" / "skip issue creation" branches, and the now-absent
`failTitle` value is provably unreachable — its only use site is inside the
branch `failCommentCondition: false` already returns from. `.releaserc.json`
now reads `"successCommentCondition": false, "failCommentCondition": false`
(`labels: false`, `releasedLabels: false` unchanged). This keeps the token at
`contents: write` only, exactly as before — it removes a deprecation warning
in the release job's log, nothing more.

**TASK-0325 — the `release` job's trigger `if:`.** The four-clause condition
above gained a fifth (in the `workflow_run` branch):
`github.event.workflow_run.head_repository.full_name == github.repository`.
This is **defense-in-depth, not a fix for a real hole**: the case it guards
against — a forked pull request's CI run, which happens in *this* repository
and can report `head_branch == 'main'` (the contributor's fork's own default
branch name) — is already excluded today by `event == 'push'`, since a
`push`-triggered `workflow_run` can only originate from a push to this
repository. The added clause makes that safety property self-evident to a
future reader without requiring them to re-derive `workflow_run`'s fork
semantics from scratch. Confirmed `head_repository` is a required,
non-nullable field on the `workflow_run` event (only `head_repository_id` can
be null), so the comparison never silently evaluates in an unsafe direction:
if the field were ever absent, the expression is `false`, and the job is
skipped (recoverable via the existing `workflow_dispatch` backfill path), not
run.

**Manual backfill/recovery path.** `workflow_dispatch` gained an optional
`tag` input: when set, the `release` job skips semantic-release entirely and
reports that tag as `released`, letting the `assets` job (re-)build binaries
for a release that already exists with none — the exact situation `v0.1.0`
was in, having published (correctly, per the bootstrap design above) before
this mechanism existed.

**GPLv3 §6 — no additional action while private.** A GitHub Release asset on
a private repo is downloadable only by a principal with repo read access, so
every possible recipient already holds the complete corresponding source by
the same grant; GitHub also attaches an auto-generated source archive to every
Release. No written offer, source bundle, or notice text is needed now beyond
bundling `LICENSE` in each archive (free, and good §4/§5 hygiene regardless).
**TASK-0219 is the gate**: the moment the repository goes public, already-
published assets become anonymously downloadable and this analysis no longer
holds, so TASK-0219 must complete before that flip — recorded as an explicit
dependency on its backlog row.

**Not in scope:** GPG signing, macOS codesigning/notarization, Windows
Authenticode — all require an owner-managed signing identity and long-lived
secrets, which is exactly what this ADR's chosen option (no standing
credentials beyond `GITHUB_TOKEN`) avoids. Unsigned binaries mean macOS
Gatekeeper quarantines the download and Windows SmartScreen warns; documented
as a user-facing workaround in [`../process/release.md`](../process/release.md),
revisited only for a public release.

## Amendment 2026-08-05 — manual-only release trigger (TASK-0333(b))

**Owner ruling, 2026-08-05: releases are cut on demand, not on merge.** The
`workflow_run` trigger recorded above is removed; `workflow_dispatch` becomes
the sole path to a release. The decision this ADR records — tag + GitHub
Release, no commit back, `GITHUB_TOKEN` with `contents: write` — is unchanged.
Only *when* that machinery runs changes, plus how the safety property the old
trigger carried is now obtained.

**Why.** Measured over 2026-08-01..04 (Actions API, per-job durations, billed
rounded up to the whole minute, OS multipliers applied) the `assets` matrix
cost **485 billed minutes, 27% of the repository's entire Actions bill**,
across **12 releases in four days**. That is release *cadence*, not job speed:
every merge carrying a `feat:`/`fix:` published a version, and each version
builds four native targets. TASK-0330 had already established that caching
`assets` is worth ~0 *and* undesirable (a restored `target` on the path that
produces published binaries), so the only remaining lever was releasing less
often — a product/distribution call, hence the owner's, and carried to them as
TASK-0333(b) rather than decided by the architect. Nothing is lost by batching:
semantic-release derives the next version from *all* Conventional Commits since
the last tag, so one deliberate dispatch releases everything accumulated with
the same version arithmetic a per-merge cadence would have produced in
instalments.

**The safety property does not travel with the trigger — it is re-established
explicitly.** The `workflow_run` trigger's one load-bearing job was that a
release could never be published from a commit whose checks were red or still
running (the "Trigger" bullet above, and the "gated on green CI for the exact
commit released" positive consequence). A bare `workflow_dispatch` would have
silently dropped it: the job's checkout ref fell back to `github.sha`, which on
a manual dispatch is whatever the dispatcher happened to point at, verified by
nothing. That is materially worse now than it would have been a week ago,
because `main`'s branch protection no longer sets `strict` ("require branches
to be up to date"), so `main`'s tip is no longer guaranteed to have been tested
in combination with the latest state before a merge landed — the `push: main`
CI run is what catches that class of break, and it catches it *after* the fact.
A release must therefore positively confirm that CI already passed for the
exact commit it is about to release.

`release.yml`'s first step now does exactly that, before the checkout and
before any dependency is installed:

- It refuses any ref other than `refs/heads/main`, restoring the old trigger's
  `head_branch == 'main'` clause. (`.releaserc.json`'s `branches` would refuse
  too, but only after ~400 npm packages are installed, and with a message about
  release branches rather than about the mistake made.)
- It queries `GET /repos/{owner}/{repo}/commits/{sha}/check-runs` — via `gh
  api`, this repository's established tool — once per required context, with
  `filter=latest` and `app_id=15368`, pinning the answer to GitHub Actions
  exactly as `main`'s branch protection pins those contexts. A same-named check
  run reported by any other integration holding `checks: write` cannot satisfy
  the gate (verified live: a wrong `app_id` returns zero check runs rather than
  being ignored).
- It requires **every** returned check run to be `status == "completed" &&
  conclusion == "success"`, and is fail-closed on everything else: a missing
  check run (CI never ran for that commit), one still in progress, and every
  non-success conclusion — `failure`, `cancelled`, `timed_out`,
  `action_required`, `skipped`, `neutral`. A `gh api` error fails the step too,
  via `set -euo pipefail`.
- **Every** rather than the newest, because `filter=latest` deduplicates only
  *within* a check suite (which is what makes a re-run supersede the attempt it
  re-ran), and GitHub Actions creates one check suite per workflow *run* — so
  two runs reporting the same check name against the same commit both come
  back. Verified against this repository's live data: every recent commit on
  `main` carries two or three `release` check runs for exactly that reason. The
  case it decides is the one this gate exists for: `security.yml`'s weekly
  `schedule:` run reports a second, unsuffixed `security` check run against
  `main`'s tip, so a new RUSTSEC advisory found by the cron against an
  unchanged `Cargo.lock` sits next to the push run's green `security`. Reading
  only the first element would have made that verdict depend on an array order
  the REST API does not document; requiring all of them is order-independent
  and strictly fail-closed, and still clears by re-running the red run (whose
  re-run wins its own suite's `latest`). It also removes the need for a
  `(manual)`-style suffix should `security.yml` ever gain a
  `workflow_dispatch`: a green manual run cannot launder a red one beside it.
- The name match is exact and case-sensitive, which matters: TASK-0331(a)
  suffixes a manually dispatched CI run's gate with a `(manual)` marker so it
  cannot
  stand in for a required context. It cannot stand in here either — only a real
  `push: main` run can answer.

**Permissions: `checks: read`, added to the `release` job.** `contents: write`
does not imply it (the checks API is its own scope, and a job-level
`permissions:` block grants nothing it does not name). The alternative
formulation — asking the workflow-runs endpoint whether the CI *workflow*
concluded successfully for that SHA — needs `actions: read`, which grants read
access to every workflow run, its logs and its artifacts in the repository.
Same answer, broader grant, in the repository's most privileged job: rejected.

**This closes the "not gated on the `Security` workflow" gap recorded above.**
That gap was justified purely by the mechanism: "`workflow_run` fires per
workflow, and coordinating two would mean polling the other's status." The new
step *is* polling, so the justification no longer applies and the gap is no
longer worth keeping. The verified contexts are **`ci-gate` and `security`** —
`main`'s required set minus `checks`, which `ci-gate` already aggregates (it
fails unless `checks` succeeded and `rust` succeeded or was skipped), so naming
both would assert one fact twice. `security` is aggregated by nothing, living
in a separate workflow, so it is named explicitly. The release gate is now
strictly stronger than the one this ADR originally described. One consequence
follows from that and is wanted: `security.yml`'s weekly `schedule:` run
reports an unsuffixed `security` check run against `main`'s tip, so an advisory
the cron finds against an unchanged `Cargo.lock` blocks releases of that commit
until it is dealt with — the correct direction for an ADR-0005 gate, and the
reason the step requires *every* returned check run to be green rather than the
newest one (see the bullet above).

**The backfill path is deliberately exempt.** `workflow_dispatch` with a `tag`
input repairs the assets of a release that *already exists* and was gated when
it was cut; it does not decide that anything should be released. The
verification step carries the same `if: env.BACKFILL_TAG == ''` guard every
other normal-path step in the job already has, so that path is unchanged.

**The job-level `if:` is removed rather than left inert.** Its four clauses
(including TASK-0325's `head_repository` clause) existed to sort a
`workflow_run` event; with that event impossible, they were dead code
referencing a payload that can no longer exist. Their safety content does not
lapse: a `workflow_dispatch` never originates from a fork and only a principal
with write access can start one, and the ref/CI questions are now answered by
the step above — which **fails loudly** where a job-level condition would skip
silently, the right behaviour when a human has just asked for a release.

**Consequences.** Releases now require one deliberate action
(`gh workflow run release.yml --ref main`, see
[`../process/release.md`](../process/release.md)); a merge that should have
shipped can sit unreleased until someone asks, which is the accepted cost of
the 27%. Estimated saving: most of 485 billed minutes per four days, scaling
with how much releases are actually batched. No repository-settings change is
required — no context name changes, and the required-check list is read by this
workflow, not written by it.

## Amendment 2026-08-05 — local release-pipeline pre-flight (TASK-0334)

**Deferred out of the previous amendment (TASK-0333(b)) as its own, larger
piece.** Now that releases are cut deliberately rather than on every merge, a
bad `assets` leg is discovered in the repository's single most privileged job,
after the tag and the GitHub Release already exist, in a run someone is
actually waiting on. The `assets` matrix was also the one part of the pipeline
with no local rehearsal at all. This amendment adds one: `scripts/
release-preflight.sh` (documented operationally in
[`../process/release.md`](../process/release.md#local-pre-flight-rehearsal)),
plus a refactor of the `assets` job's own steps so the CI path and the local
rehearsal cannot silently drift apart.

**Extraction, not duplication.** The stamp/build/verify/package logic that used
to live as four separate inline steps in the `assets` job (`Stamp the released
version into Cargo.toml`, `cargo build --release`, `Verify the built binary
reports the stamped version`, `Package the archive and checksum` — plus a fifth,
`Verify the runner's architecture matches this leg's target triple`, whose
check is now this script's own first, unconditional step) is now one script,
[`scripts/build-release-artifact.sh`](../../scripts/build-release-artifact.sh),
parameterized on `<target> <version> <tag> <archive> <out-dir>`. `release.yml`'s
`assets` job calls it; `scripts/release-preflight.sh` calls it too, for a local
rehearsal of the same mechanics. This is the "one source of truth" resolution
of the open question TASK-0334's own backlog entry named: **the credentialed
half never moves.** The shared script never sees `GITHUB_TOKEN` and never runs
`gh release upload` — that step, and the "verify required checks are green"
step earlier in the `release` job, stay inline in the workflow, exactly as
`release.yml`'s no-cache note (TASK-0330) already argued for keeping this job's
footprint minimal and directly auditable in one file. What moved into the
script is specifically the part that is expensive to get wrong twice (version
stamping, archive layout, checksum format) and carries no privilege — running
it locally can build a binary, but it cannot publish one.

**Glibc-floor / musl / aarch64-Linux checking is explicitly out of scope
here**, per TASK-0334's own backlog note — that question belongs to TASK-0326
(deferred to the public-release decision), not to this pre-flight tool.

**What the tool actually rehearses, and the one honest gap in it.** Two of the
four release legs have a local host at all: a glibc Linux target, built inside
a rootless [podman](https://podman.io/) container
([`scripts/release-preflight/Containerfile`](../../scripts/release-preflight/Containerfile),
Ubuntu 24.04 to approximate the `ubuntu-24.04` runner's glibc without claiming
byte-identical reproduction), and `aarch64-apple-darwin`, built natively when
run on Apple Silicon macOS. Windows (`x86_64-pc-windows-msvc`) and Intel macOS
(`x86_64-apple-darwin`) stay CI-only — no local host for either exists here,
and TASK-0334's own scope named exactly these two legs. Everything runs inside
a disposable `git worktree` (never the real working tree — `build-release-
artifact.sh` stamps `Cargo.toml` in place, which must never happen to this
repository's actual copy), removed on exit whether the rehearsal succeeds or
fails, unless `--keep` is passed.

**Measured, not assumed: emulating the Linux leg's exact target triple on an
Apple Silicon dev machine does not work.** The first design attempted
`--platform linux/amd64` to force a real `x86_64-unknown-linux-gnu` build under
podman's qemu-based emulation on this project's arm64 development machine.
`rustc -vV` — not a build, just invoking the compiler to print its own version
— segfaulted under that emulation, reproducibly. This is a known class of qemu
user-mode emulation limitation with LLVM-based toolchains, not a
misconfiguration to work around; retrying with reduced build parallelism
(`cargo build -j 1`) did not help, because the crash was in `rustc` itself, before
any of this project's code compiled. The tool does not paper over this: it
drops the forced `--platform` and lets podman build for its own native
platform instead — on an x86_64 host (or in CI) that natively **is**
`x86_64-unknown-linux-gnu`, a byte-for-byte rehearsal of the real leg; on an
Apple Silicon host it is `aarch64-unknown-linux-gnu` instead, which is not one
of the four targets `release.yml` ships. The script detects which target it
actually built (`rustc -vV`'s own `host:` line, the same mechanism `build-
release-artifact.sh`'s architecture guard already used) and says so plainly on
stderr rather than silently mislabeling the rehearsal as the CI leg it isn't.
The build/package/verify mechanics are still exercised for real either way —
only the specific target triple is host-dependent. One honest note on the
architecture guard itself: on the podman leg the container detects its own
host and passes that value *as* the requested target, so `build-release-
artifact.sh`'s host-vs-target check can never actually fire there — it stays
live (and load-bearing) on the macOS leg and in `release.yml` itself, where
target is fixed by the matrix rather than self-reported.

**Consequences.** A release build can now be exercised on this machine before
dispatching the real workflow, at zero cost to the credentialed path (the
shared script carries no permissions and is exercised in CI exactly as before,
just from one file instead of four inline steps). The known gap — no reliable
local rehearsal of the literal `x86_64-unknown-linux-gnu` leg on Apple Silicon
— is named rather than hidden, and does not regress anything: CI still builds
and verifies that exact leg on every real release, unaffected by this tool.

## Links

- [semantic-release configuration](https://semantic-release.gitbook.io/semantic-release/usage/configuration)
  and [`@semantic-release/github`](https://github.com/semantic-release/github#options)
- [About protected branches](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches)
  — protection rules target branches; tag protection is a separate ruleset.
- [Triggering a workflow](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows)
  — `GITHUB_TOKEN`-triggered events do not create workflow runs.
- [`../process/release.md`](../process/release.md) — the operational procedure.
