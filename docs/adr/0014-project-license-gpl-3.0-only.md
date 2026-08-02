# ADR-0014: Project license — GPL-3.0-only

- **Status:** accepted
- **Date:** 2026-08-02
- **Deciders:** human owner (license family), architect (SPDX refinement)
- **Related:** [TASK-0202](../planning/backlog.md), [ADR-0007](0007-technology-stack-rust-clap.md),
  [ADR-0015](0015-release-publishing-tag-and-github-release.md), `deny.toml`, `Cargo.toml`

## Context and problem statement

The repository has shipped with `license = "TBD"` in `Cargo.toml` and a README
that says "no license is published yet; all rights reserved". The owner decided
on **GPL-3.0** on 2026-07-19 (recorded in
[`../planning/batch-plan.md`](../planning/batch-plan.md) and
[`../planning/status.md`](../planning/status.md)); this ADR formalizes that
decision, adds the license text, and settles the two things the bare phrase
"GPL-3.0" leaves open:

1. **Which SPDX identifier** — `GPL-3.0` on its own is a *deprecated* SPDX
   identifier and must be written as either `GPL-3.0-only` or
   `GPL-3.0-or-later`. Tooling that parses the field (cargo, cargo-deny,
   crates.io, GitHub's license detection) needs the current form.
2. **What the license means while the repository is private** — the owner's
   decision explicitly keeps the repo private for now; going public is a
   separate, later decision that this ADR does **not** make.

The license family itself (copyleft vs permissive) is **not reopened here** —
that was the owner's call. What follows records it, its consequences, and the
open edges it creates.

## Decision drivers

- **Owner intent:** strong copyleft — derivatives of `imgdedup` stay free.
- **Correctness of metadata:** a valid, non-deprecated SPDX expression that
  `cargo`, `cargo-deny`, and GitHub all understand.
- **Dependency compatibility:** every crate in the tree must be
  GPLv3-compatible (`deny.toml` already restricts the allow-list to permissive
  licenses on this assumption).
- **Reversibility:** the project has a **single copyright holder**, so any
  future relicensing is the owner's unilateral decision — until third-party
  contributions arrive.
- **Honesty about the private-repo state:** the license must not imply
  obligations or freedoms that do not yet apply, nor hide the ones that do.

## Considered options

### Which SPDX identifier

1. **`GPL-3.0-only`** — exactly GPLv3, no automatic adoption of future GPL
   versions.
2. **`GPL-3.0-or-later`** — GPLv3 "or (at your option) any later version"; the
   FSF's own recommendation.
3. `GPL-3.0` (bare) — deprecated in SPDX; rejected outright as a tooling
   hazard (ambiguous, and warned about by license scanners).

### When to add the `LICENSE` file

1. **Now, while private** — the license is in place before any copy leaves the
   owner's control.
2. **Only at the moment the repo goes public** — keep "all rights reserved"
   until then.

## Decision outcome

**Chosen:** **`GPL-3.0-only`**, with the full canonical GPLv3 text added to the
repository root as `LICENSE` **now**, while the repository stays private.

`GPL-3.0-only` because:

- It licenses exactly the terms the owner read and chose. `-or-later` hands
  every recipient a standing option to use the terms of a **future** GPL version
  that nobody involved has read; that is a delegation of a licensing decision,
  not a technicality.
- With a **single copyright holder**, the flexibility argument for `-or-later`
  is weak: the owner can relicense to `-or-later`, dual-license, or grant
  exceptions at any time by editing one line. The reverse (narrowing
  `-or-later` to `-only`) is not possible for copies already conveyed. Choosing
  the narrower option keeps the decision in the owner's hands and stays
  cheaply reversible **as long as the project has no outside contributors**.
- It is a current, unambiguous SPDX identifier.

The text is added **now** rather than at publication because a license that
does not exist cannot be relied on by anyone who receives a copy, and the first
release (v0.1.0, private) is imminent — see
[ADR-0015](0015-release-publishing-tag-and-github-release.md).

### What "private repository + GPL-3.0-only" actually means today

This is the nuance most easily glossed over, so it is recorded explicitly.

- **The GPL is triggered by conveying, not by existing.** As long as the owner
  is the only person with a copy, no GPL obligation is ever exercised: there is
  no recipient to grant freedoms to. The `LICENSE` file is a **standing offer**
  that takes effect for whoever receives a copy.
- **Granting repository access is conveying a copy.** Anyone the owner adds as
  a collaborator on the private repository — human or otherwise — receives the
  source *under GPL-3.0-only* and may therefore lawfully redistribute it,
  including publicly. Adding the license file while private **does** change
  what a collaborator may do with what they get. The repository's privacy is
  an access-control measure, not a licensing one.
- **Distributing a binary triggers the source obligation.** GPLv3 §6 requires
  the "Corresponding Source" to accompany any conveyed object code. Today the
  first release (see ADR-0015) is a private GitHub Release visible only to
  people who already have source access, so the obligation is satisfied
  trivially. **If a binary is ever sent to someone without repository access,
  the source must go with it (or be offered).** That is a hard constraint on
  any future "share the binary with a friend" step, and a strong practical
  argument for making the repo public before distributing binaries widely.
- **Going public is still a separate decision.** Nothing here commits the owner
  to publishing. It only means that *if and when* a copy leaves, the terms are
  already fixed and known.
- **Not legal advice.** This ADR records an engineering decision and the
  project's working understanding of it; it is not a legal opinion.

### Copyright notice and headers

The copyright holder is the repository owner. GPLv3 *recommends* per-file
notices but does not require them; adding a header to every source file is
churn that would land in every future diff for little benefit while the project
is a single-author private repo. Decision: keep the notice in `LICENSE` and
`README.md` only for now, and revisit per-file headers plus a license line in
`imgdedup --version` output as part of the public-release readiness checklist
(**TASK-0219**).

### Consequences

- Positive: the licensing question is closed; `Cargo.toml`, `deny.toml`,
  `README.md`, and `CONTRIBUTING.md` now agree with each other and with a real
  license file; GitHub will detect the license; the dependency allow-list in
  `deny.toml` has an actual policy behind it instead of a pending decision.
- Positive: contributions are now unambiguously accepted under
  `GPL-3.0-only` (stated in `CONTRIBUTING.md`), so the copyright story stays
  simple as long as that is honored.
- Negative / trade-off: **copyleft narrows adoption.** Nobody can vendor
  `imgdedup`'s source into a proprietary tool. For an end-user CLI this is
  mostly theoretical, but it does rule out, e.g., embedding the crate in a
  closed-source product. This was the owner's deliberate choice.
- Negative / trade-off: **`-only` costs future flexibility** the moment a
  second copyright holder appears; relicensing then needs their consent.
  Mitigation: revisit before accepting the first outside contribution
  (TASK-0219) — either stay `-only`, move to `-or-later`, or adopt a CLA/DCO.
- Negative / trade-off: dependency choices are constrained forever after —
  a crate under a GPLv3-incompatible license (e.g. GPL-2.0-only) can never be
  added. `deny.toml`'s allow-list enforces this mechanically.
- `publish = false` stays in `Cargo.toml`: having a license does not mean
  publishing to crates.io, which is its own (unmade) decision.
- Follow-ups: **TASK-0219** — public-release licensing readiness (per-file
  headers if wanted, `--version` notice, source-offer wording for binaries,
  `-only` vs `-or-later` re-check before outside contributions).

## Pros and cons of the options

### `GPL-3.0-only`

- Good: the terms are exactly what was read and chosen; no future-version
  drift; sole owner can always widen it later.
- Good: unambiguous, current SPDX identifier.
- Bad: if a GPLv4 ever appears, downstream cannot move to it without the
  owner's action, and code cannot be combined with `GPL-4.0-only`-style code.
- Bad: contradicts the FSF's own recommendation, which matters if the project
  ever wants to be maximally idiomatic in the GNU ecosystem.

### `GPL-3.0-or-later`

- Good: FSF-recommended; maximal forward compatibility with future GPL
  versions and with `-or-later` codebases; no coordination needed later.
- Bad: grants recipients the option of terms that do not exist yet and cannot
  be reviewed today.
- Bad: irrevocable for copies already conveyed — the strictly less reversible
  choice, which is the wrong default for a decision this cheap to revisit.

### Adding `LICENSE` now vs. at publication

- Now — Good: no window in which a collaborator holds a copy under unclear
  terms; metadata, scanners, and docs are consistent immediately.
- Now — Bad: a collaborator may redistribute (see above); the owner must be
  aware that access = a GPL grant.
- At publication — Good: keeps maximal control while private.
- At publication — Bad: leaves `license = "TBD"`, an untrue README, and a
  `deny.toml` policy with no decision behind it; and the "all rights reserved"
  state is the one that actually needs care, since it is the ambiguous one for
  anyone given access.

## Links

- [GNU GPL v3.0](https://www.gnu.org/licenses/gpl-3.0.txt) — the text in
  [`LICENSE`](../../LICENSE), verbatim
  (SHA-256 `3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986`).
- [SPDX license list](https://spdx.org/licenses/) — `GPL-3.0` is deprecated in
  favor of `GPL-3.0-only` / `GPL-3.0-or-later`.
- [FSF: how to use GNU licenses](https://www.gnu.org/licenses/gpl-howto.html)
- [ADR-0015](0015-release-publishing-tag-and-github-release.md) — release
  publishing, which is where binary distribution (and thus GPLv3 §6) becomes
  concrete.
