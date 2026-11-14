# ADR-0002: In-repo Markdown as canonical planning & tracking

- **Status:** accepted; superseded in part by
  [ADR-0021](0021-issue-based-planning-for-a-public-repository.md)
  (2026-11-14): tasks and status moved to GitHub Issues. PRDs, ADRs and
  process documents stay in-repo Markdown.
- **Date:** 2026-06-13
- **Deciders:** architect, human owner
- **Related:** [ADR-0001](0001-record-architecture-decisions.md)

## Context and problem statement

The crew must be able to pick up work at any time from the repository itself,
without relying on Claude session state, and humans should be able to follow
progress without switching to a different tool. We need a single, unambiguous
source of truth for planning and status.

## Decision drivers

- Resumability from repo content alone.
- One source of truth (avoid drift between systems).
- Human- and agent-readable; versioned and reviewable alongside code.

## Considered options

1. **In-repo Markdown** (`docs/planning/`: roadmap, backlog, status, task specs)
   as canonical.
2. **GitHub Issues + Projects** as canonical.
3. **Both**, with Markdown mirrored to Issues.

## Decision outcome

**Chosen option:** In-repo Markdown is canonical. `docs/planning/` holds the
roadmap, backlog, status snapshot, and per-task specs. Planning docs are updated
in the same PR as the work they describe.

### Consequences

- Positive: status travels with the code; no external dependency; fully
  versioned; resumable from a clone.
- Negative: no board UI out of the box; manual discipline to keep docs current
  (enforced by review and Definition of Done).
- GitHub Issues may still be used opportunistically (e.g., security advisories,
  Dependabot), but they are not the source of truth for project status.

## Links

- [`../planning/`](../planning/)
