# ADR-0012: Documentation ownership and placement

- **Status:** accepted
- **Date:** 2026-06-14
- **Deciders:** architect, human owner
- **Related:** [ADR-0011](0011-unit-of-work-cadence-and-planning-gate.md) (unit-of-work
  cadence & "docs ride the feature" DoD), [ADR-0005](0005-mandatory-tdd-and-security-gate.md),
  [`docs/process/crew.md`](../process/crew.md), [`docs/process/sdlc.md`](../process/sdlc.md),
  [`.claude/agents/product-owner.md`](../../.claude/agents/product-owner.md)

## Context and problem statement

[ADR-0011](0011-unit-of-work-cadence-and-planning-gate.md) established that a unit
of work carries its **user-facing documentation in the same PR** and folded that
responsibility onto the **product-owner**. What it left unspecified is **how user
documentation is structured and where detailed docs live** — the product-owner
rule and the Definition of Done say "CLI/usage guide, relevant README sections"
without a home for anything longer than a README section.

In practice this gap already bit us: the top-level `README.md` drifted badly out
of date (it described the project as "Phase 0 — not yet implemented" with the
"stack undecided" well into Phase 3) and there was no place to grow detailed,
per-feature usage documentation. We need a durable convention so that, as each
user-visible feature lands, its documentation has an obvious owner and an obvious
location — and the repository stays the single source of truth.

A second force is **accountability**. The product-owner owns the product's
**functional acceptance** — the sign-off that shipped behavior matches the PRD's
acceptance criteria (the tester supplies the verifying tests; see
[`crew.md`](../process/crew.md)). Documenting *what the accepted behavior is, for
users* is the same act of describing accepted behavior. Placing user
documentation with the role that accepts the behavior keeps the two from
diverging and gives "is this feature done?" a single answer that includes its
docs.

## Decision drivers

- **Single, clear ownership** of user documentation — avoid "everyone's job =
  nobody's job".
- **Tie documentation to functional acceptance** so docs cannot lag behavior:
  the role that accepts the behavior documents it.
- **Repository as the single source of truth** (ADR-0002); resumable from the
  repo alone.
- **Layered, discoverable docs**: quick orientation vs. in-depth reference, each
  appropriate to its audience.
- **Scalable and future-friendly**: detailed docs should grow as a structured
  collection (and remain amenable to later transformation into a static guide
  site) rather than a single sprawling file.
- **Consistency with the existing cadence** (ADR-0011): docs ride the feature
  PR; this ADR only refines *who* and *where*.

## Considered options

1. **Product-owner owns all user documentation; README = general orientation,
   `docs/guide/` = detailed self-contained pages.** Refines ADR-0011 with an
   explicit owner-rationale and a documentation home.
2. **Implementer owns user documentation** alongside technical docs (rustdoc /
   design notes).
3. **Defer user documentation to a release-phase push** (the original roadmap
   framing — "finalize user documentation" in Phase 5).

## Decision outcome

**Chosen option:** Option 1 — **the product-owner owns user-facing
documentation**, split by depth:

- **General usage lives in the top-level [`README.md`](../../README.md)** — what
  the tool is, the safety model, install, a usage overview, and a link into the
  detailed guide. README is the orientation surface.
- **Detailed documentation lives in [`docs/guide/`](../../docs/guide/)** as a
  **linked collection of self-contained guide pages** — one page per coherent
  topic (e.g. scanning & scope, choosing what to keep, removing duplicates
  safely, output & exit codes), with [`docs/guide/README.md`](../../docs/guide/README.md)
  as the index. Each page stands on its own and cross-links to the others, so the
  collection reads as a user guide rather than one monolithic file.
- The README's "detailed guide" section **links into `docs/guide/`** so the two
  layers are connected and neither duplicates the other.

This is justified by the drivers: it gives user docs a single accountable owner
tied to functional acceptance, a clear layered home that keeps the README short
and the depth discoverable, and a structure that scales per-feature and stays
amenable to a future static-site rendering — all while preserving ADR-0011's
"docs ride the feature" cadence.

This ADR **extends** ADR-0011; it does not supersede it. ADR-0011's DoD (user
docs land in the feature's own PR) remains in force; ADR-0012 specifies the
ownership rationale and the README/`docs/guide/` placement.

### Consequences

- Positive: every user-visible feature has an unambiguous documentation owner
  (product-owner) and location; README stays concise and current; detailed docs
  grow as a coherent, navigable guide; documentation can't silently lag accepted
  behavior.
- Negative / trade-offs: the product-owner must touch `docs/guide/` (and the
  README where relevant) on every user-visible feature, and must keep the README
  overview and the guide in sync — a modest, recurring per-feature cost.
- Follow-ups / new tasks created: establish the `docs/guide/` structure with an
  index and seed pages migrated from the README's detailed content; propagate the
  ownership/placement rule into `product-owner.md`, `crew.md`, `sdlc.md` (DoD +
  unit-of-work table), `CLAUDE.md` (roles), and the PRD template's acceptance
  expectations. Tracked as TASK-0213.

## Pros and cons of the options

### Option 1 — product-owner owns; README general + `docs/guide/` detailed

- Good: ownership tied to functional acceptance; layered and discoverable;
  scales per-feature; future-friendly (static-site amenable); consistent with
  ADR-0011.
- Bad: recurring per-feature documentation load on the product-owner; two layers
  to keep in sync.

### Option 2 — implementer owns user documentation

- Good: docs written by whoever knows the code best; co-located with the change.
- Bad: conflates *user-facing* docs (what the behavior is, for users) with
  *technical* docs (how the code works); the implementer does not own acceptance,
  so user docs would not be anchored to the accepted behavior; weaker product
  voice.

### Option 3 — defer to a release-phase documentation push

- Good: less per-feature overhead; one focused effort.
- Bad: docs lag behavior for the whole of development (exactly how the README
  rotted); contradicts ADR-0011's "docs ride the feature"; a large, error-prone
  catch-up at the end.

## Links

- [ADR-0011](0011-unit-of-work-cadence-and-planning-gate.md) — unit-of-work
  cadence; the "docs ride the feature" DoD this ADR refines.
- [ADR-0002](0002-in-repo-markdown-as-canonical-planning.md) — repo as the single
  source of truth.
- [`docs/guide/`](../../docs/guide/) — the detailed user-guide collection this ADR
  establishes.
