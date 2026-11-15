# ADR-0003: `main` branch protection & merge model

- **Status:** accepted
- **Date:** 2026-06-13
- **Deciders:** architect, human owner
- **Related:** [ADR-0004](0004-conventional-commits-semver-and-releases.md),
  [ADR-0015](0015-release-publishing-tag-and-github-release.md)

## Context and problem statement

We want a strict, high-quality git workflow where the **human owner is the merge
gate**, while most work is performed by AI agents acting with the owner's
credentials. A naïve "require ≥1 approving review" rule deadlocks a solo
repository: **GitHub forbids a PR author from approving their own PR**, and since
agents open PRs as the owner, the owner could neither self-approve nor merge.
This has bitten the owner before and must not recur.

## Decision drivers

- The owner must reliably be able to **review and merge** every PR.
- Quality/security gating via required CI checks.
- Clean, linear history suitable for automated releases.
- Avoid the self-approval deadlock without standing up extra identities now.

## Considered options

1. **Required CI status checks + PR-required + linear history + conversation
   resolution, with required approvals = 0.** The owner is the sole merger; the
   architect agent reviews via comments.
2. **Set up a bot identity / GitHub App** so agent PRs are authored by the bot and
   the owner formally approves (enforced approvals work, heavier setup).
3. **Minimal protection** (CI checks only), decide later.

## Decision outcome

**Chosen option 1.** Protect `main` with:

- **Require a pull request before merging**; no direct pushes.
- **Required status checks** (the CI jobs), strict / branches up to date.
- **Require conversation resolution** before merging.
- **Require linear history.**
- **Block force-pushes and deletions.**
- **Required approving reviews = 0.**

Merge settings: **squash-merge only** (merge commits and rebase-merge disabled);
squash commit message defaults to the PR title; **auto-delete head branches**.

The architect agent's review is delivered as **PR comments**, not a blocking
GitHub approval. The **human owner is the single merge gate** and clicks Merge
once CI is green and review is addressed.

### Consequences

- Positive: no self-approval deadlock; the owner always controls merges; CI still
  gates quality; history stays linear for releases.
- Negative: GitHub records no formal "approval" object; the review-quality
  guarantee is process-based (owner discipline + architect comments) rather than
  enforced by an approval count.
- Revisit if collaborators are added or enforced approvals become desirable →
  then adopt option 2 (bot identity / GitHub App) in a superseding ADR.
- The Phase 0 **SDLC smoke test** explicitly verifies the owner can open and merge
  a PR end-to-end, proving the deadlock is gone.

### Note on `include administrators`

Required status checks only gate the merge button when the checks actually run on
the PR. We require only checks that are present in CI to avoid blocking merges on
non-existent checks. Whether to "include administrators" in enforcement is chosen
when protection is applied and recorded in `docs/planning/status.md` (removed from `main` by ADR-0021; in the git history).

## Amendment 2026-11-15 — repository rulesets replace the classic rule; release tags are protected (#129)

The protection on `main` moves from a classic branch protection rule to a
**repository ruleset**, and a second ruleset protects release tags. The merge
model above is unchanged: the human owner is the single merge gate, required
approvals stay at 0, and the architect's review is delivered as comments.

### Why

- The repository is public. A ruleset is readable by anyone, so a contributor
  can see the rules a pull request must meet; a classic rule's settings are
  mostly visible to admins only.
- A ruleset can be exported as JSON and kept in the repository, where a change
  to it is reviewed like any other change.
- Rulesets can target tags. [ADR-0015](0015-release-publishing-tag-and-github-release.md)'s
  release model relies on a `v*` tag never moving once published, and until
  now nothing enforced that: `refs/tags/*` was unguarded.

### The `main` ruleset

A branch ruleset named `main`, targeting `refs/heads/main`, carries the classic
rule's settings one to one, as they stood on 2026-11-15:

| Rule | Setting |
|---|---|
| Pull request required | 0 approving reviews; no code-owner review; conversation resolution required; squash is the only allowed merge method |
| Required status checks | `ci-gate`, `checks`, `security`, each pinned to GitHub Actions; **not** strict |
| Linear history | required |
| Force pushes | blocked |
| Deletion | blocked |

"Not strict" corrects the decision outcome above, which lists "strict / branches
up to date". The live rule had already dropped strict by 2026-08-05 (ADR-0015's
amendment of that date relies on it), and the ruleset records the rule as it
is, not as first written.

**One addition beyond one to one: squash is pinned in the ruleset.** The
classic rule could not express a merge method; squash-only lived in the
repository's merge settings alone. Those settings are unchanged, and the
ruleset now also allows only squash, so the decision above is enforced by the
public, versioned rule and survives a merge setting being switched back on.
Nothing changes in practice today. Automatic deletion of head branches remains
a repository setting.

**Admin bypass: pull requests only.** The classic rule did not enforce its
settings on admins (`enforce_admins` off), which also let the owner push to
`main` directly. The ruleset replaces that with one explicit bypass entry: the
repository admin role, in `pull_request` mode. The owner can still merge a pull
request whose rules are not met — the case that has actually come up is a
renamed required check sitting at "Expected" until the rule is updated — but
can no longer push to `main` outside a pull request. This is deliberately
tighter than what it replaces, and it resolves the open choice the "Note on
`include administrators`" above left to be made when protection was applied.

### The `release-tags` ruleset

A tag ruleset named `release-tags`, targeting `refs/tags/v*`:

- **update blocked** and **force push blocked** — a `v*` tag cannot be moved;
- **deletion blocked** — a `v*` tag cannot be removed;
- **creation open** — the release workflow creates the tag with its own
  `GITHUB_TOKEN`, so no rule restricts creation;
- **no bypass actors**, the owner included.

Removing a wrongly published tag therefore means disabling the ruleset first:
a deliberate act, visible in the ruleset's history, and not something a stray
`git push --force --tags` can do. semantic-release's notes ref,
`refs/notes/semantic-release-v<version>`, is not a tag and matches neither
ruleset. semantic-release pushes with `git push --tags`, but the workflow
fetches every tag first, so existing tags are sent unchanged and only the new
tag reaches the server as a creation.

This does not weaken or change ADR-0015: the release still needs no bot, no
personal access token and no bypass.

### The JSON in the repository is a record

[`.github/rulesets/main.json`](../../.github/rulesets/main.json) and
[`.github/rulesets/release-tags.json`](../../.github/rulesets/release-tags.json)
hold the two rulesets in the form GitHub's ruleset import and its REST API
accept. **GitHub's settings are what is enforced; the files are the reviewed
copy.** Nothing applies them automatically. A rule change is made as one unit:
the JSON is changed in a pull request and the live ruleset is changed to match,
and the pull request records that both were done.

Fields the files leave out take GitHub's defaults, so the live `main` ruleset
shows two the file does not. Neither changes behaviour here:

- `require_extra_approval_for_unattributed_changes: true` — has no effect when
  zero approvals are required;
- `do_not_enforce_on_create: false` — concerns newly created branches, and the
  ruleset targets `main` alone.

To compare the two, read the live rulesets with
`gh api repos/{owner}/{repo}/rulesets` and
`gh api repos/{owner}/{repo}/rulesets/<id>`.

### Rollout and verification (2026-11-15)

- Both rulesets were created from the JSON files through the REST API and read
  back. The API reports the owner's bypass on `main` as pull requests only, and
  on `release-tags` as none.
- The tag rules were exercised with a throwaway tag under a temporary extra
  pattern on the same ruleset: creating the tag succeeded, moving it was
  rejected, deleting it was rejected. The pattern and the tag were then
  removed.
- **Not exercised:** a direct push to `main` by the owner (a failed test would
  have left a commit on `main`), and tag creation by the release workflow's
  `GITHUB_TOKEN`. No rule restricts creation, so the next release is expected
  to pass; it is the confirmation.
- The classic rule stays in place until the pull request for this amendment
  is merged, and is then deleted on the owner's go, recorded on #129. While
  both exist GitHub applies both.

### Consequences

- Positive: the rules are public and versioned; release tags are immutable by
  enforcement, not only by convention; the owner's exemption is narrower and
  explicit.
- Negative: two places hold the rules (GitHub and the JSON files) and they can
  drift; only the same-unit rule above keeps them together.
- Negative: an emergency fix can no longer be pushed straight to `main`. It
  goes through a pull request, which the owner may merge without waiting for
  the checks.

## Links

- GitHub: "About protected branches" and review restrictions; "About rulesets".
- [`../process/branching.md`](../process/branching.md)
