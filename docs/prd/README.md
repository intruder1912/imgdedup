# Product Requirements Documents (PRDs)

PRDs define **what** `imgdedup` should do and **why**. They are owned by the
[product-owner](../../.claude/agents/product-owner.md) and are the input to
architecture ([ADRs](../adr/)) and implementation.

## Process

1. **Brainstorm first.** Explore intent with the owner; surface gaps and hidden
   assumptions; never assume. (See [`../process/sdlc.md`](../process/sdlc.md).)
2. Copy [`TEMPLATE.md`](TEMPLATE.md) to `PRD-XXXX-<slug>.md` (next number).
3. Write testable requirements with clear acceptance criteria.
4. Self-review (placeholders, contradictions, ambiguity, scope), then have the
   owner review.
5. Seed the resulting tasks into [`../planning/backlog.md`](../planning/backlog.md).
6. Land it through the normal PR flow and keep this index updated.

## Index

| PRD | Title | Status |
|---|---|---|
| [PRD-0001](PRD-0001-exact-image-deduplication.md) | Exact image deduplication (scope, UX, safety) | approved |

> Status values: `draft` → `in review` → `approved` → `superseded`.
