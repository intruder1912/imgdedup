# ADR-0001: Record architecture decisions

- **Status:** accepted
- **Date:** 2026-06-13
- **Deciders:** architect, human owner
- **Related:** all subsequent ADRs

## Context and problem statement

`imgdedup` is built by a crew of AI agents and a human owner, and must be
resumable from the repository content alone. Decisions made only in chat sessions
are lost. We need a durable, reviewable record of significant architecture and
process decisions.

## Decision drivers

- Continuity across sessions and contributors (human and AI).
- Traceability: why a choice was made, and what was rejected.
- Low ceremony, lives in the repo next to the code.

## Considered options

1. **MADR-style ADRs in `docs/adr/`** committed via PRs.
2. A wiki or external document store.
3. No formal record (rely on commit messages / memory).

## Decision outcome

**Chosen option:** MADR-style ADRs in `docs/adr/`, one file per decision,
numbered sequentially, indexed in `README.md`, authored by the architect and
approved by the owner through the normal PR flow.

### Consequences

- Positive: decisions are versioned, reviewable, and discoverable in the repo.
- Negative: a small authoring overhead per decision.
- ADRs are immutable once accepted; changes are made by superseding ADRs.

## Links

- [MADR](https://adr.github.io/madr/)
- [`README.md`](README.md), [`TEMPLATE.md`](TEMPLATE.md)
