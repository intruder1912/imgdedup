# Architecture Decision Records (ADRs)

ADRs capture significant **architecture and process decisions** — the context,
the options considered, the decision, and its consequences. They use a
[MADR](https://adr.github.io/madr/)-style format
([`TEMPLATE.md`](TEMPLATE.md)). ADRs are owned by the
[architect](../../.claude/agents/architect.md).

## Process

1. **Brainstorm first.** Lay out options and trade-offs; confirm constraints with
   the owner; never assume.
2. Copy [`TEMPLATE.md`](TEMPLATE.md) to `XXXX-<slug>.md` (next number).
3. Record context, options, decision, and consequences.
4. Owner reviews; land via the normal PR flow; update this index.
5. Decisions are immutable once accepted — to change one, write a new ADR that
   **supersedes** it (and mark the old one superseded).

## Index

| ADR | Title | Status |
|---|---|---|
| [0001](0001-record-architecture-decisions.md) | Record architecture decisions | accepted |
| [0002](0002-in-repo-markdown-as-canonical-planning.md) | In-repo Markdown as canonical planning & tracking | accepted |
| [0003](0003-main-branch-protection-and-merge-model.md) | `main` branch protection & merge model | accepted |
| [0004](0004-conventional-commits-semver-and-releases.md) | Conventional Commits, SemVer & automated releases | accepted |
| [0005](0005-mandatory-tdd-and-security-gate.md) | Mandatory TDD & security gate | accepted |
| [0006](0006-post-pr-review-iteration-loop.md) | Post-PR review & CI iteration loop | accepted |

> Planned: technology-stack selection and key design ADRs (Phase 2); licensing
> ADR (before any public release).
>
> Status values: `proposed` → `accepted` → `superseded` → `deprecated`.
