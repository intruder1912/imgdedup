# ADR-0006: Post-PR review & CI iteration loop

- **Status:** accepted
- **Date:** 2026-06-14
- **Deciders:** architect, human owner
- **Related:** [ADR-0003](0003-main-branch-protection-and-merge-model.md)
  (merge model & conversation resolution), [ADR-0005](0005-mandatory-tdd-and-security-gate.md)
  (TDD & security gate), [`../process/sdlc.md`](../process/sdlc.md)

## Context and problem statement

The SDLC front-loads the *substantive* review onto the **uncommitted working-tree
diff** ([`sdlc.md`](../process/sdlc.md) "Review before commit"): author drafts →
`make validate` → architect reviews → revise → **repeat until converged** → single
commit → open PR. That **stage-one** loop is well specified.

**Stage two — after the PR is open — has no loop.** The SDLC said only that CI
"must pass" and to "address any remaining comments." It did not say *who* fetches
CI failures or review findings, *how* (mechanism), whether fixes trigger
re-review, or what the convergence / exit criterion is. The agent definitions
inherited the same gap ("address architect review comments" with no mechanism).

In our crew the role subagents are **stateless** — they watch nothing and resume
no context between dispatches — so when CI fails or the architect's confirmation
pass surfaces a finding on the PR, **nobody owns the loop** that carries that
finding back to a fix. This is the friction observed in practice: real PRs still
get CI-only failures and confirmation-pass nits that stage one cannot preempt.

Assumptions confirmed with the owner: the loop should be **orchestrator-driven**,
findings should be **GitHub review threads** (not just prose), and the decision is
worth recording as an ADR plus doc updates.

## Decision drivers

- **Traceability** — every finding and its resolution must survive in the repo /
  PR, not in a chat session.
- **Low turnaround** — stage two must stay *lightweight*; stage one exists
  precisely so stage two is the small remainder.
- **`gh`-only tooling** — version control / GitHub interactions use `git` and the
  `gh` CLI only (no MCP), per `CLAUDE.md`.
- **Reuse the existing gate** — `main` already requires *conversation resolution*
  and *required status checks* (ADR-0003); prefer wiring the loop to that gate
  over inventing new process.

## Considered options

1. **Orchestrator-driven loop, findings as GitHub review threads** — the main
   "tech-lead" session collects CI + review findings via `gh` and re-dispatches
   role agents until the merge gate opens.
2. **Implementer/tester self-drives** — the role agent itself polls `gh`, fixes,
   resolves threads, and re-requests review in a loop.
3. **Human owner manually shepherds** — the human relays each CI failure and
   review comment to agents and decides every iteration.

## Decision outcome

**Chosen option:** Option 1 — an **orchestrator-driven** stage-two loop with
findings posted as **GitHub review threads**, because it fits the stateless-crew
model and reuses the ADR-0003 gate as a machine-checkable exit criterion.

The orchestrator session (the main session acting as tech-lead):

1. Collects findings via `gh`:
   - CI: `gh pr checks <n>`, `gh run view <id> --log-failed`.
   - Review: `gh pr view <n> --comments`, `gh api repos/{owner}/{repo}/pulls/<n>/comments`.
2. Re-dispatches `implementer` / `tester` with the **concrete findings** — fix
   under TDD → `make validate` → push → **resolve the corresponding threads**.
   Apply `receiving-code-review` discipline: verify each finding and push back when
   it is wrong, rather than blind-complying.
3. Re-dispatches the `architect` to **re-review** the new diff.
4. **Repeats until required checks are green and all review conversations are
   resolved** — the ADR-0003 merge gate. The human owner then squash-merges.

The architect still **never blocks via a GitHub approval** (required approvals
remain 0); findings are *resolvable threads*, and conversation-resolution — not an
approval — is what gates the merge.

### Consequences

- Positive: enforced convergence (the loop ends exactly when the merge gate
  opens); a machine-readable audit trail of findings → resolutions; a named owner
  for an otherwise orphaned loop; no new gate or tooling required.
- Negative / trade-offs: slightly more ceremony than a single prose comment;
  requires discipline that stage two stays *lightweight* and does not re-litigate
  stage-one decisions.
- Follow-ups: update `sdlc.md`, `crew.md`, and the `implementer` / `tester` /
  `architect` definitions to match (same PR).

## Pros and cons of the options

### Option 1 — Orchestrator-driven, threads

- Good: matches the stateless-subagent reality; one place owns the loop; reuses the
  conversation-resolution gate; findings are independently resolvable.
- Bad: the orchestrator must marshal context to each dispatch.

### Option 2 — Self-driving role agent

- Good: fewer hops.
- Bad: pushes orchestration into a role agent that is meant to be a stateless
  task-executor; harder to keep stage two lightweight; a single agent both
  authoring and re-requesting review blurs roles.

### Option 3 — Human shepherds

- Good: maximum control.
- Bad: least automation; reintroduces the chat-as-source-of-truth failure mode the
  repo-canonical principle (ADR-0002) exists to avoid.

## Links

- [ADR-0003](0003-main-branch-protection-and-merge-model.md) — required status
  checks and conversation resolution are the gate this loop converges on.
- [ADR-0002](0002-in-repo-markdown-as-canonical-planning.md) — the repo-canonical
  principle this loop preserves (findings live on the PR, not in a chat session).
- Skills: `superpowers:receiving-code-review`, `superpowers:requesting-code-review`.
