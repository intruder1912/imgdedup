# ADR-0014: Project license — GPL-3.0-only

- **Status:** superseded by [ADR-0020](0020-relicense-mit-or-apache-2.0.md)
  (2026-11-14); it still describes the license of releases `v0.1.0` to
  `v0.6.0`
- **Date:** 2026-08-02
- **Deciders:** human owner (license family), architect (SPDX refinement)
- **Related:** [TASK-0202](../planning/backlog.md), [ADR-0007](0007-technology-stack-rust-clap.md),
  [ADR-0015](0015-release-publishing-tag-and-github-release.md), `deny.toml`, `Cargo.toml`

## Context and problem statement

The repository has shipped with `license = "TBD"` in `Cargo.toml` and a README
that says "no license is published yet; all rights reserved". The owner decided
on **GPL-3.0** on 2026-07-19 (recorded in
`docs/planning/batch-plan.md` and `docs/planning/status.md`, both removed
from `main` by ADR-0021 and kept in the git history); this ADR formalizes that
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

*(Ruled 2026-08-05 — see the
[TASK-0219 amendment](#amendment-2026-08-05--public-release-licensing-readiness-task-0219):
per-file headers **no**, `--version` notice **yes**.)*

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
  *(Re-checked 2026-08-05: still a single copyright holder, `-only` stands; the
  trigger now sits on TASK-0336.)*
- Negative / trade-off: dependency choices are constrained forever after —
  a crate under a GPLv3-incompatible license (e.g. GPL-2.0-only) can never be
  added. `deny.toml`'s allow-list enforces this mechanically.
- `publish = false` stays in `Cargo.toml`: having a license does not mean
  publishing to crates.io, which is its own (unmade) decision.
- Follow-ups: **TASK-0219** — public-release licensing readiness (per-file
  headers if wanted, `--version` notice, source-offer wording for binaries,
  `-only` vs `-or-later` re-check before outside contributions). *(All four
  ruled 2026-08-05 — see the
  [TASK-0219 amendment](#amendment-2026-08-05--public-release-licensing-readiness-task-0219);
  what remains is trigger-bound verification, tracked as **TASK-0336**.)*

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

## Amendment 2026-08-05 — public-release licensing readiness (TASK-0219)

This ADR deferred four items to TASK-0219: **per-file headers**, a
**license/copyright line in `--version`**, **GPLv3 §6 source-offer wording**
wherever binaries are distributed, and a **`-only` vs `-or-later` re-check**
before the first outside contribution. All four are ruled here.

**The decision this ADR records does not change.** `GPL-3.0-only` stands, the
canonical text stays at `LICENSE`, and the private-repo analysis above is
untouched. What is decided is what ships *alongside* the license.

**State of the world when these rulings were made,** checked rather than
assumed: the repository is still **private** (`gh repo view --json visibility`
→ `PRIVATE`, 2026-08-05) and **no public-release date exists**. Each ruling
therefore says explicitly whether it applies **now** or is **trigger-bound**.
Nothing below depends on the flip happening, and nothing below has to be
redone if it never does.

### 1. Per-file GPL headers — no, deliberately; and if ever, then REUSE whole

**Ruling: do not add per-file copyright/SPDX headers.** Not to `src/*.rs`, not
to the workflows, not to the scripts. This is a decision, not a deferral.

- **Nothing requires them.** GPLv3's per-file notice guidance lives in the
  non-normative appendix *"How to Apply These Terms to Your New Programs"*.
  The normative obligations on conveying — §4's "keep intact all notices" and
  "give all recipients a copy of this License", §5's marking of modified
  works — are satisfied here by `LICENSE` at the repository root, the
  `license = "GPL-3.0-only"` field in `Cargo.toml`, the README's License
  section, and `LICENSE` being bundled inside every release archive
  (`scripts/build-release-artifact.sh`).
- **Nothing in this project conveys a single file.** There is one crate, one
  copyright holder, `publish = false`, and `[[bin]]` with no library target —
  the unit that travels is the repository or the release archive, both of
  which already carry the license. The scenario headers exist for (a file
  copied out of context) is not one this project creates.
- **A half-adopted convention is worse evidence than none.** Twelve headers
  today are twelve headers that silently rot: the thirteenth file arrives
  without one, review has a new thing to check by eye, and a scanner that
  finds headers on 12 of 20 files reads the repository as *partially*
  licensed. Keeping the claim in exactly the places tooling actually reads
  keeps it true by construction.
- **The standard that would make headers pay is [REUSE](https://reuse.software).**
  REUSE compliance means an SPDX header on *every* file (source, config,
  workflows, docs, fixtures), a `LICENSES/` directory, a `REUSE.toml`
  (formerly `.reuse/dep5`) covering the un-headerable ones, and a `reuse lint`
  job in CI to keep it
  honest. That is a real, machine-verifiable property. Adopting a fragment of
  it — SPDX lines on `src/*.rs` only — buys none of that property and pays a
  share of the cost.
- **Considered and rejected middle option:** a bare `// SPDX-License-Identifier:
  GPL-3.0-only` on `src/*.rs`, enforced by a grep in `scripts/validate.sh`.
  Cheap, but the benefit is zero *for this repository's actual scanner set* —
  GitHub's license detection, `cargo-deny`, `cargo metadata`, and crates.io
  all read `Cargo.toml`/`LICENSE`, none of them read Rust source comments —
  against a permanent per-file overhead and one more check to maintain
  (TASK-0335 has just finished paying down the cost of ungated conventions).

**Revisit triggers** (any one of these, and the answer may flip — to *full*
REUSE adoption with a CI check, not to a piecemeal header pass):

1. the project grows a library target or its source is vendored into another
   project file-by-file;
2. a second copyright holder appears (see ruling 4) and per-file attribution
   starts carrying information the repository root cannot;
3. the project adopts REUSE for its own sake (e.g. to appear in a compliance
   toolchain that requires it).

### 2. `--version` — yes: a GNU-style block on `--version`, terse `-V` retained

**Ruling: `imgdedup --version` prints the GNU-conventional block below;
`imgdedup -V` keeps printing exactly one line.** Applies **now**, not at the
flip — it is correct today and needs no revisiting when visibility changes.

```text
imgdedup 0.1.0
Copyright (C) 2026 the imgdedup authors
License GPL-3.0-only: GNU GPL version 3 <https://gnu.org/licenses/gpl-3.0.html>
This is free software: you are free to change and redistribute it.
There is NO WARRANTY, to the extent permitted by law.
Source: https://github.com/intruder1912/imgdedup
```

**Mechanism: clap's `long_version`, not a new flag and not a new subcommand.**
clap 4 renders `--version` from `long_version` and `-V` from `version` when
both are set — verified by direct experiment against clap 4 rather than read
off the docs: with `#[command(version, long_version = LONG_VERSION)]`,
`--version` printed the multi-line block and `-V` printed `imgdedup 0.1.0`,
and clap prepends the binary name itself (so the constant must start with the
version and must **not** repeat `imgdedup`). A separate `--license` flag was
rejected: it is not a convention anyone would look for, and it adds a CLI
surface to carry a string that belongs on the surface everybody already tries.

**Why the split rather than the same block on both.** GNU tools print the same
text for both only because they have no short version flag at all. Here, the
one-line form is load-bearing for exactly one consumer:
`scripts/build-release-artifact.sh` asserts the built binary's version output
equals `imgdedup <version>` *exactly* — the check that catches a
version-stamping failure before a mislabeled binary is published (ADR-0015).
That assertion moves to `-V`. The only other in-repo caller, the Claude
plugin's preflight (`claude-plugin/skills/using-imgdedup/SKILL.md` and
`claude-plugin/agents/imgdedup-operator.md`), runs `imgdedup --version` and
checks only that it *succeeds* — checked, not assumed: it parses nothing, so
the long block reaches it harmlessly (it now reads six lines of context
instead of one, which is if anything an improvement, and no plugin change is
needed). Keeping a stable machine-readable one-liner **and** a
human-conventional long form costs one constant.

**Wording, decided rather than left to taste:**

- `GPL-3.0-only:` rather than GNU's bare `GPLv3:`. GNU's convention encodes
  "or later" as a `+` suffix, so `GPLv3` (no `+`) already means only — but
  that convention is not widely known, and the SPDX identifier is what
  `Cargo.toml`, `deny.toml`, `CONTRIBUTING.md` and this ADR already say. One
  spelling everywhere, unambiguous to a reader who knows neither convention.
- `(C)` in ASCII, not `©`. The whole output must stay ASCII so it renders on a
  legacy Windows console code page; this codebase already carries `safe_text.rs`
  for exactly that class of problem, and a mojibake copyright notice is a worse
  notice. Pinned by a test (see the handoff below).
- The two warranty sentences are GNU's long-standing phrasing verbatim,
  because recognizability is the point — users skim them and know what they
  are.
- Every line ≤ 80 columns, so an 80-column terminal never re-wraps the license
  URL into something un-clickable. This is why the URL is
  `https://gnu.org/licenses/gpl-3.0.html` (79 columns with its line) rather
  than the `www.` form used in prose links; both are the same document.
- The `Source:` URL comes from `CARGO_PKG_REPOSITORY`, so it cannot drift from
  `Cargo.toml`. See ruling 3 — this line is the §6 direction that travels
  *inside the object code itself*.
- No trailing period on the three metadata lines (`Copyright`, `License`,
  `Source`) so that a URL is never selected with a stray `.`; periods stay on
  the two full sentences.
- Year `2026` is static and matches `README.md`. A year bump is a deliberate
  one-line edit, and the test pinning the string is what will surface it.

**Rejected: embedding the exact tag or commit** (via `build.rs`, `vergen`, or a
release-time env var). The released version already maps to its tag by
construction — `tagFormat` is pinned to `v${version}` (ADR-0015) — so `0.1.0`
identifies tag `v0.1.0` without help, and a build script would add
supply-chain surface *and* break ADR-0015's deliberately zero-Rust-source
version-stamping design. A locally built development binary would additionally
name a tag whose contents it is not, which is worse than saying nothing.

**This notice is voluntary, and that is worth recording precisely.** GPLv3
§5(d) requires "Appropriate Legal Notices" only from works with *interactive
user interfaces* (§0). `imgdedup` reads nothing from the user during a run —
verified: no `stdin`/`read_line` use anywhere in `src/` — so it has no
interactive interface in that sense and owes no displayed notice. It is
adopted anyway because a binary is the one artifact that can be separated from
its archive, its README and its repository, and this is the cheapest way for
it to keep saying what it is and where its source lives.

### 3. GPLv3 §6 — the route is §6(d), and the *directions* ship now

**What §6 actually requires**, since "source-offer wording" is a phrase that
hides five different obligations. Conveying object code is lawful only in one
of five ways: (a) with the Corresponding Source on a physical medium; (b) with
a **written offer**, valid three years, to supply the source; (c) by passing
along an offer received (non-commercial, occasional only); (d) by **offering
access from a designated place**, with equivalent access to the Corresponding
Source **through the same place**; or (e) peer-to-peer transmission.

**Ruling: this project's route is §6(d), and it needs no written offer — ever,
as long as binaries are conveyed from GitHub Releases.** The object code is
offered from a Release page; GitHub attaches an auto-generated source archive
to every Release and the tag itself stays fetchable, which *is* equivalent
access to the Corresponding Source from the same place. §6(d)'s one
human-supplied requirement is the rest of that sentence: *"maintain clear
directions next to the object code saying where to find the Corresponding
Source."*

**The archive half of that claim is verified, not assumed** (2026-08-05, over
the API — the repository is private, so anonymous reachability is the one part
that can only be confirmed after the flip and stays on TASK-0336):

- every one of the 15 published releases (`v0.1.0`…`v0.5.1`) carries a
  populated `tarball_url`/`zipball_url` alongside its eight binary assets, so
  the source archive really is offered from the same page as the object code;
- the `v0.5.1` tarball was downloaded and inspected: 150 entries, the complete
  tag tree including `src/`, `Cargo.toml`, `Cargo.lock`, `LICENSE`,
  `scripts/` and `.github/workflows/`;
- `.gitattributes` carries **no `export-ignore`** — the one repository-side
  setting that could silently strip files out of GitHub's generated archive
  and quietly falsify this ruling. Adding one is therefore a licensing change,
  not a formatting change.

**The stamping wrinkle, stated so nobody has to rediscover it.** The tag's
`Cargo.toml` says `version = "0.1.0"` (the permanent development placeholder —
ADR-0015 never commits a bump back), while the binary published for that tag
reports the real version, stamped at build time. A plain `cargo build` from
the source archive therefore yields a binary that prints a different version
string. This does **not** weaken §6(d): §1 defines Corresponding Source as the
source *plus the scripts to control compilation*, and those scripts —
`scripts/build-release-artifact.sh` and `.github/workflows/release.yml`, which
perform the stamp from the release version — are inside the archive too. The
recipient receives everything needed to regenerate the object code, including
the step that makes it call itself `0.5.1`.

**Ruling: those directions ship now, in the two places that travel with the
object code** — not deferred to the flip:

1. **The binary itself** — the `Source:` line in `--version` (ruling 2).
2. **The archive** — a *Corresponding Source* paragraph in `README.md`'s
   License section, which `build-release-artifact.sh` already bundles into
   every `.tar.gz`/`.zip` alongside `LICENSE`. Zero new files, zero new
   packaging steps; the directions are literally next to the object code, in
   the same archive.

The exact wording for (2), to be added as a subsection of `README.md`'s
existing License section — drafted here because it is a licensing artifact,
but `README.md` is **product-owner-owned** ([ADR-0012](0012-documentation-ownership-and-placement.md)),
so it lands with their sign-off:

```markdown
### Corresponding Source

`imgdedup` binaries are distributed only from this repository's
[GitHub Releases](https://github.com/intruder1912/imgdedup/releases). The
complete Corresponding Source for any released binary is available from the
same place: each release is built from the tag of the same name — `v<version>`,
where `<version>` is what the binary itself prints for `imgdedup --version` —
and GitHub attaches an auto-generated source archive to every release
alongside the binary archives. This is how `imgdedup` satisfies GPLv3 §6(d);
no separate written offer is made, and none is needed.
```

**Ruling: do not write a §6(b) written offer.** It is a three-year commitment
to operate a source-delivery channel that does not exist and would never be
used, it is unnecessary the moment (d) holds, and — the point of the question
— **it is the artifact that would actually be misleading.** Publishing a
"valid for three years, contact us for source" notice while the repository is
private would advertise a service nobody runs.

**Is writing §6(d) directions now, while private, honest?** Yes, and the
distinction matters. The `Source:` line and the README paragraph say *where
the source is*. That is true for every actual recipient today — everyone who
can download a private Release asset already holds the source by the same
grant, which is exactly why ADR-0015's TASK-0211 amendment could conclude "no
additional action while private" — and it becomes true for everyone at the
flip, with no edit. Neither artifact claims anonymous availability, so neither
becomes false in either direction. What would be dishonest is the *offer*, and
that is the thing not being written.

**Rejected alternatives:** a `NOTICE` file (an Apache-ecosystem convention; a
third place for the same sentence to rot, and it would not be read by anyone
who did not already read `LICENSE`); and appending a static §6 footer to the
semantic-release release notes (it would mean editing the release body from
the repository's only `contents: write` job to assert a property GitHub's own
auto-attached source archives already provide).

**Trigger-bound, carried to TASK-0336** — the flip-day checklist, because
these are facts to *verify at the flip*, not text to write now:

- every published release still has its tag, and GitHub's auto-generated
  source archives are actually present and anonymously downloadable (verify,
  do not assume — this is the load-bearing half of the §6(d) claim);
- `README.md`'s Corresponding Source paragraph and the LICENSE section are
  publicly reachable;
- `docs/guide/getting-started.md`'s "the repository is currently private"
  download instructions are updated (they already anticipate this);
- ADR-0015's TASK-0211 §6 paragraph and `docs/process/release.md`'s licensing
  note are re-stated for a public repo — the conclusion holds, but for the
  §6(d) reason rather than the "recipients already have repo access" reason.

### 4. `-only` vs `-or-later` — re-checked, unchanged

**Ruling: `GPL-3.0-only` stands.** Checked, not skipped: the condition this
ADR made the choice conditional on — a single copyright holder — still holds.
`git log --format='%an <%ae>'` over the entire history returns three
identities, of which two are the owner under the same GitHub no-reply address
(`andy.d` and `intruder1912`, both
`6000952+intruder1912@users.noreply.github.com`) and the third is
`dependabot[bot]`, whose commits are machine-generated manifest/lockfile
version bumps carrying no copyrightable authorship of this project's code.
**There has never been an outside contribution.** Every argument in the
decision above therefore applies verbatim; nothing is re-litigated here.

**The one thing `%an`/`%ae` does not see, checked separately:** 78 commits
carry `Co-authored-by: Claude <model> <noreply@anthropic.com>` trailers (plus
8 naming dependabot). These are a GitHub *attribution* convention, not a
copyright grant, and they do not add a second copyright holder: the machine is
not an author for copyright purposes, and Anthropic's commercial terms assign
whatever rights it may hold in an output to the user who prompted it. The
trailers are the only thing in this history that superficially reads as
co-authorship, so the fact that they are *not* is recorded here rather than
left for a future reader to re-derive.

- **No CLA.** It requires infrastructure, asks contributors for rights beyond
  inbound=outbound, and deters exactly the drive-by contributions a small CLI
  gets. `CONTRIBUTING.md`'s existing inbound=outbound statement ("by
  contributing you agree that your contribution is licensed under those same
  terms") is the load-bearing artifact and already exists.
- **No DCO yet.** If outside PRs actually appear, requiring `Signed-off-by`
  is the cheap next step (no rights assignment, no infrastructure, one CI
  check) — proposed then, not pre-emptively.
- **The asymmetry to keep in view:** widening to `-or-later` is a unilateral
  one-line edit *only while the owner is the sole holder*. If that flexibility
  is ever wanted, the cheapest moment is **before** the first outside
  contribution, not after.

**Trigger: re-check before merging the first PR authored by anyone other than
the owner** — recorded on TASK-0336. Deliberately *not* the public flip:
making the repository public creates readers, not copyright holders, and does
not by itself change this analysis.

### Consequences of this amendment

- Positive: the four open edges this ADR left are closed as *decisions*
  (headers: no, with named revisit triggers) or as *shipped artifacts*
  (`--version` block, §6(d) directions), rather than staying a checklist that
  must be re-derived under the time pressure of a public-release day.
- Positive: nothing here is conditional on going public, so the flip becomes a
  short verification (TASK-0336) rather than a licensing project.
- Negative / trade-off: declining per-file headers means a file copied out of
  this repository carries no in-band license statement. Accepted: copying it
  without the license is a violation with or without a header, and the
  scenario is not one this project's packaging creates.
- Negative / trade-off: `--version` and `-V` now differ, which is unusual in
  the Rust ecosystem (though standard in GNU's). Mitigated by pinning both
  forms with tests and by the release pipeline asserting the short form.
- Implementation is handed to the implementer under TDD (exact texts, files
  and tests in TASK-0219's backlog row); the flip-day checklist and the
  first-outside-contribution re-check are **TASK-0336**.

## Links

- [GNU GPL v3.0](https://www.gnu.org/licenses/gpl-3.0.txt) — the text in
  `LICENSE` (removed by ADR-0020; in the tree up to tag `v0.6.0`), verbatim
  (SHA-256 `3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986`).
- [SPDX license list](https://spdx.org/licenses/) — `GPL-3.0` is deprecated in
  favor of `GPL-3.0-only` / `GPL-3.0-or-later`.
- [FSF: how to use GNU licenses](https://www.gnu.org/licenses/gpl-howto.html)
- [ADR-0015](0015-release-publishing-tag-and-github-release.md) — release
  publishing, which is where binary distribution (and thus GPLv3 §6) becomes
  concrete.
