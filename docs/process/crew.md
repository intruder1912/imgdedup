# The Agent Crew

`imgdedup` is built by a small crew of AI agents coordinated by a human owner.
Each role has a subagent definition in [`../../.claude/agents/`](../../.claude/agents/)
and the responsibilities below. The roles mirror a healthy software team: someone
owns *what* we build, someone owns *how*, someone builds it, someone proves it
works — and one person decides it ships.

## Roles

### Product Owner — [`product-owner`](../../.claude/agents/product-owner.md)

Owns *what and why*. Elicits requirements (brainstorming, never assuming), writes
PRDs in [`../prd/`](../prd/), defines acceptance criteria, prioritizes the
backlog, and owns **functional acceptance** — the product-level sign-off that
shipped behavior matches the PRD (the tester supplies the verifying tests/evidence;
see below). Hands architecture questions to the architect. Because the role accepts
the behavior, it also owns **user-facing documentation** for any user-visible
behavior, riding the feature's PR: **general usage in the top-level `README.md`**
and **detailed, per-topic pages in [`../guide/`](../guide/)** (a linked collection
of self-contained guide pages). See
[ADR-0012](../adr/0012-documentation-ownership-and-placement.md). For feature work,
**carves each PRD into coherent shippable slices** in the backlog, and is the
**PR-open gate** for a slice: the PR opens only once the role has accepted all the
slice's functional requirements and delivered its docs. May **demand an architect
review** of any aspect mid-slice ([ADR-0013](../adr/0013-prd-slice-delivery-cadence.md)).

### Architect / Reviewer — [`architect`](../../.claude/agents/architect.md)

Owns *how*. Turns PRDs into design and records decisions as ADRs in
[`../adr/`](../adr/), including the fully-open technology-stack selection. **Also
reviews every PR**, in one of two modes scaled to the artifact: a *lightweight
document review* of PRDs/ADRs/process docs (feasibility, risk, scope/YAGNI,
testable acceptance criteria, consistency) or a *heavyweight code review* of
implementation (correctness, security, tests/TDD, design, conventions). To keep
turnaround low, the substantive review runs on the **branch diff vs `main`**
(the author holds the PR until converged) and is then **recorded on the PR** — as
a prose summary plus **resolvable
review threads** for each substantive finding, comments only, never a blocking
GitHub approval. In the post-PR stage-two loop the architect **re-reviews** each
iteration until checks are green and all threads are resolved. See
[`sdlc.md`](sdlc.md). For a **PRD slice**, also performs **in-progress
(mid-stream) reviews** on the branch — on the implementer's request or a
product-owner/orchestrator demand — recorded in the slice's review log
(`docs/planning/reviews/<slice>.md`); these are advisory and feed the **final
whole-slice review** of record on the opened PR
([ADR-0013](../adr/0013-prd-slice-delivery-cadence.md)).

### Implementer — [`implementer`](../../.claude/agents/implementer.md)

Writes production code for specified tasks under **mandatory TDD**, following the
ADRs. Keeps changes small and focused. Maintains **technical documentation**
(rustdoc on public items / `docs/` design notes) for the code written — part of
Definition of Done, reviewed as code. On a **PRD slice**, **requests an architect
review** for significant or especially risky/complex changes (its discretion, not
per-commit), **works review feedback immediately**, and when unsure whether to fix
now or defer **asks the product-owner to decide** — deferrals become tracked
backlog tasks ([ADR-0013](../adr/0013-prd-slice-delivery-cadence.md)).

### Tester — [`tester`](../../.claude/agents/tester.md)

Owns test strategy and verification. Ensures every acceptance criterion maps to a
meaningful, passing test and that success claims are backed by real output.

### Human Owner

The only one who **merges** PRs (the merge gate). Approves PRDs and ADRs,
arbitrates open questions, and holds final say. Agents act on the owner's behalf
but never self-merge.

## Process self-improvement (kaizen)

The **orchestrator, product-owner, and architect** may each propose process
improvements when they spot material potential. Such a proposal is raised as a
**chore unit** and, if lasting, captured in an ADR. See
[ADR-0011](../adr/0011-unit-of-work-cadence-and-planning-gate.md), which is itself
the first such instance.

## Model assignment

Each subagent is pinned to a model in its `.claude/agents/*.md` frontmatter,
balancing capability against cost — reasoning-heavy roles use Opus, execution
roles use Sonnet:

| Subagent | Model |
|---|---|
| `product-owner` | opus |
| `architect` | opus |
| `implementer` | sonnet |
| `tester` | sonnet |

## Hand-offs

```
Owner intent
   │
   ▼
Product Owner ──(PRD + acceptance criteria)──► Architect
   │                                              │
   │                                              ▼
   │                                        ADR(s) + task breakdown
   │                                              │
   ▼                                              ▼
Backlog / tasks ◄──────────────────────► Implementer + Tester (TDD)
                                                  │
                                                  ▼
                                          PR ──► Architect review (threads) ◄─┐
                                                  │                           │
                                                  ▼          re-dispatch +     │
                                  CI + threads resolved? ─ no ─► re-review ────┘
                                                  │ yes
                                                  ▼
                                          Human Owner merges
```

PRDs and ADRs go through the same **review-before-PR** loop as code: the author
hands the architect the branch diff vs `main`, iterates on the branch (pushes are
CI-free) until it converges, then opens the PR ready with the review recorded on
it (see [`sdlc.md`](sdlc.md)).

**Who drives the post-PR loop.** The role subagents are **stateless** — they watch
nothing between dispatches — so once a PR is open the **orchestrator** (the main
session driving the work) owns stage-two iteration: it collects CI failures and
architect review threads via `gh`, re-dispatches the implementer/tester with the
concrete findings, and re-dispatches the architect to re-review, looping until
required checks are green and all conversations are resolved. The human owner then
merges. See [`sdlc.md`](sdlc.md) and
[`../adr/0006-post-pr-review-iteration-loop.md`](../adr/0006-post-pr-review-iteration-loop.md).
During a **PRD slice**, the orchestrator may also **demand an in-progress architect
review** of any aspect, independently of whether the implementer requested one
([ADR-0013](../adr/0013-prd-slice-delivery-cadence.md)).

## Shared rules (all roles)

- Read [`../../CLAUDE.md`](../../CLAUDE.md) first; the repo is the source of truth.
- Brainstorm and grill on gaps before producing PRDs, ADRs, or features.
- Follow the [SDLC](sdlc.md): classify (feature/fix/chore) → branch (one unit) →
  draft/TDD → local validation → architect review on the branch diff vs `main` →
  open PR ready (review recorded) → owner merge.
- Use `git` and `gh` CLI only (no MCP) for version control and GitHub.
- Update planning docs in the same PR as the work.

## Parallelism

From Phase 1 onward, independent tasks (flagged in
[`../planning/backlog.md`](../planning/backlog.md)) can be worked by multiple
agents in parallel, as long as they touch disjoint areas and each lands through
its own PR. The architect serializes review and the owner serializes merges.
