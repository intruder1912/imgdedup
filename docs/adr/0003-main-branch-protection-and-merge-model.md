# ADR-0003: `main` branch protection & merge model

- **Status:** accepted
- **Date:** 2026-06-13
- **Deciders:** architect, human owner
- **Related:** [ADR-0004](0004-conventional-commits-semver-and-releases.md)

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
when protection is applied and recorded in `docs/planning/status.md`.

## Links

- GitHub: "About protected branches" and review restrictions.
- [`../process/branching.md`](../process/branching.md)
