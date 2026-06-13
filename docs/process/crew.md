# The Agent Crew

`imgdedup` is built by a small crew of AI agents coordinated by a human owner.
Each role has a subagent definition in [`../../.claude/agents/`](../../.claude/agents/)
and the responsibilities below. The roles mirror a healthy software team: someone
owns *what* we build, someone owns *how*, someone builds it, someone proves it
works — and one person decides it ships.

## Roles

### Product Owner — [`product-owner`](../../.claude/agents/product-owner.md)

Owns *what and why*. Elicits requirements (brainstorming, never assuming), writes
PRDs in [`../prd/`](../prd/), defines acceptance criteria, and prioritizes the
backlog. Hands architecture questions to the architect.

### Architect / Reviewer — [`architect`](../../.claude/agents/architect.md)

Owns *how*. Turns PRDs into design and records decisions as ADRs in
[`../adr/`](../adr/), including the fully-open technology-stack selection. **Also
reviews every PR** for correctness, design, security, tests, and conventions —
leaving comments (not a blocking GitHub approval).

### Implementer — [`implementer`](../../.claude/agents/implementer.md)

Writes production code for specified tasks under **mandatory TDD**, following the
ADRs. Keeps changes small and focused.

### Tester — [`tester`](../../.claude/agents/tester.md)

Owns test strategy and verification. Ensures every acceptance criterion maps to a
meaningful, passing test and that success claims are backed by real output.

### Human Owner

The only one who **merges** PRs (the merge gate). Approves PRDs and ADRs,
arbitrates open questions, and holds final say. Agents act on the owner's behalf
but never self-merge.

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
                                          PR ──► Architect review (comments)
                                                  │
                                                  ▼
                                          Human Owner merges
```

## Shared rules (all roles)

- Read [`../../CLAUDE.md`](../../CLAUDE.md) first; the repo is the source of truth.
- Brainstorm and grill on gaps before producing PRDs, ADRs, or features.
- Follow the [SDLC](sdlc.md): branch → TDD → local validation → Conventional
  Commit → PR → review → owner merge.
- Use `git` and `gh` CLI only (no MCP) for version control and GitHub.
- Update planning docs in the same PR as the work.

## Parallelism

From Phase 1 onward, independent tasks (flagged in
[`../planning/backlog.md`](../planning/backlog.md)) can be worked by multiple
agents in parallel, as long as they touch disjoint areas and each lands through
its own PR. The architect serializes review and the owner serializes merges.
