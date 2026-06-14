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
| [0007](0007-technology-stack-rust-clap.md) | Technology stack — Rust + clap | accepted |
| [0008](0008-core-deduplication-algorithm-and-semantics.md) | Core deduplication algorithm & semantics | accepted |
| [0009](0009-cross-platform-trash-and-recycle.md) | Cross-platform trash / recycle | accepted |
| [0010](0010-concurrency-and-memory-model.md) | Concurrency & memory model | accepted |
| [0011](0011-unit-of-work-cadence-and-planning-gate.md) | Unit-of-work cadence & planning gate | proposed |

> Planned: licensing ADR (before any public release). The technology-stack and
> core design ADRs for PRD-0001 (0007–0010) have landed.
>
> Status values: `proposed` → `accepted` → `superseded` → `deprecated`.
