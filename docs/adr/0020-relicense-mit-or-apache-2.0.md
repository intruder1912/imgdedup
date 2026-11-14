# ADR-0020: Relicense to MIT OR Apache-2.0

- **Status:** accepted
- **Date:** 2026-11-14
- **Deciders:** human owner
- **Supersedes:** [ADR-0014](0014-project-license-gpl-3.0-only.md)
- **Related:** TASK-0347, TASK-0336, `LICENSE-MIT`, `LICENSE-APACHE`,
  `Cargo.toml`, `deny.toml`,
  [ADR-0015](0015-release-publishing-tag-and-github-release.md)

## Context and problem statement

`imgdedup` has been licensed `GPL-3.0-only` since
[ADR-0014](0014-project-license-gpl-3.0-only.md) (2026-08-02). On 2026-11-14 the
owner decided to relicense the project under **`MIT OR Apache-2.0`**. The
decision came in a conversation about what going public and distribution
through package managers would take; that is the setting, not a stated
reason.

ADR-0014 named the condition under which this is the owner's decision alone:
the project has a single copyright holder until third-party contributions
arrive. That still holds — no pull request by anyone other than the owner has
been merged — and the repository is still private.

## Decision drivers

The owner gave the instruction without a stated reason. The first bullet is
the decision; the rest are considerations recorded alongside it, not the
owner's motives.

- **Owner decision:** a permissive license in place of strong copyleft.
- **Ecosystem convention:** `MIT OR Apache-2.0` is the usual choice for Rust
  crates and command-line tools; Apache-2.0 adds an explicit patent grant
  that MIT lacks, and MIT keeps compatibility with GPLv2-only code that
  Apache-2.0 alone does not have.
- **Timing:** relicensing needs every copyright holder's agreement, so it is
  cheapest before the repository is public and before the first outside
  contribution.
- **Dependencies:** every crate in the tree is already permissively licensed
  (`cargo deny list`, 2026-11-14: `imgdedup` itself was the only GPL entry),
  so nothing in the dependency graph constrains the choice.

## Considered options

1. **Stay on `GPL-3.0-only`** (ADR-0014).
2. **`MIT`** alone.
3. **`MIT OR Apache-2.0`** — the recipient chooses either.

## Decision outcome

**Option 3: `imgdedup` is licensed under `MIT OR Apache-2.0`, starting with
the first release after `v0.6.0`.**

- **License files.** `LICENSE` (the GPLv3 text) is removed. `LICENSE-MIT` and
  `LICENSE-APACHE` are added at the repository root, the file names the Rust
  ecosystem uses. `LICENSE-APACHE` is the unmodified text from
  `https://www.apache.org/licenses/LICENSE-2.0.txt` (SHA-256
  `cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30`).
  `LICENSE-MIT` carries `Copyright (c) 2026 the imgdedup authors`, the same
  holder line `--version` prints.
- **Metadata.** `Cargo.toml`, `package.json`, `package-lock.json` and the
  Claude plugin's `plugin.json` say `MIT OR Apache-2.0`.
- **`--version`.** The block ADR-0014's 2026-08-05 amendment introduced
  stays, with one line changed. `-V` is untouched.

  ```text
  imgdedup <version>
  Copyright (C) 2026 the imgdedup authors
  License MIT OR Apache-2.0: see LICENSE-MIT and LICENSE-APACHE
  This is free software: you are free to change and redistribute it.
  There is NO WARRANTY, to the extent permitted by law.
  Source: https://github.com/intruder1912/imgdedup
  ```

  Both licenses disclaim warranty, so the two sentences remain true. The
  `Source:` line is kept as a convenience; no license term requires it any
  more.
- **Release archives** bundle `LICENSE-MIT` and `LICENSE-APACHE` next to the
  binary (`scripts/build-release-artifact.sh`). Both licenses ask for their
  text and the copyright notice to accompany a copy.
  *Note, 2026-11-14 (TASK-0348):* from the release after `v0.7.0` the
  archives also bundle `THIRD-PARTY-LICENSES.txt`, the license notices of the
  crates the binary is built from; see
  [`release.md`](../process/release.md#third-party-license-notices).
- **Contributions.** `README.md` and `CONTRIBUTING.md` carry the usual
  statement: a contribution submitted for inclusion is dual licensed as
  above unless its author says otherwise.
- **Dependency policy.** The `deny.toml` allow-list is unchanged: it already
  admits only permissive licenses. Its reason changes from "GPLv3-compatible"
  to "does not put copyleft terms on a permissively licensed binary".

### What becomes of ADR-0014's rulings

| ADR-0014 ruling | Now |
|---|---|
| SPDX `GPL-3.0-only`, not `-or-later` | Moot. |
| Add the license file while private | Kept in spirit: the new files are added before the repository is public. |
| No per-file license headers (amendment §1) | Unchanged. Neither license requires them, and the reasons given there (scanners read `Cargo.toml` and the license files; an unenforced header convention rots) do not depend on the license. |
| `--version` block (amendment §2) | Kept, license line changed. |
| GPLv3 §6(d) Corresponding Source directions (amendment §3) | Moot for new releases. The README's "Corresponding Source" section is removed. |
| Re-check `-only` vs `-or-later` at the first outside PR (amendment §4) | Moot. |

### Releases already published

`v0.1.0` to `v0.6.0` were published under `GPL-3.0-only`. A license already
granted is not withdrawn, so those releases stay available under it to anyone
who received them; their archives contain the GPL `LICENSE` file, and
`v0.6.0`'s `--version` output names `GPL-3.0-only` (earlier releases predate
that block). Nothing is republished or edited. While the
repository is private the recipients are the owner and any collaborator with
access. The README states this in one sentence.

> *Note (2026-11-14, after `v0.7.0` was published):* on the owner's
> instruction the GitHub Releases `v0.1.0` to `v0.6.0` and their binaries
> were deleted; the git tags are kept. No binary under `GPL-3.0-only` is
> offered for download any more. Copies already received stay under that
> license.

### Consequences

- **Positive.** The license matches what Rust users and package managers
  expect. The source-availability obligations of GPLv3 §6, and the
  flip-time verification TASK-0336 existed for, no longer apply to new
  releases. The code can be reused, including as a library, without
  copyleft terms.
- **Negative / trade-offs.** ADR-0014's stated owner intent — derivatives
  stay free — is given up: anyone may distribute a modified, closed-source
  version. This is the substance of the decision.
- **Follow-ups.** TASK-0336 shrinks to what is not about the GPL: the
  private-repository download instructions in the guide at the flip, and a
  DCO decision at the first outside pull request. If the old releases are
  still listed when the repository goes public, they remain GPL-licensed
  artifacts beside MIT/Apache ones; whether to keep or delete them is an
  owner decision for the flip. *(Decided 2026-11-14: deleted; see the note
  above.)*

## Pros and cons of the options

### Stay on `GPL-3.0-only`

- Good: derivatives stay free.
- Bad: no longer the owner's intent; carries the §6 obligations and the
  flip-time checks.

### `MIT`

- Good: shortest and most widely understood.
- Bad: no explicit patent grant.

### `MIT OR Apache-2.0` (chosen)

- Good: the ecosystem's convention; a patent grant for those who take
  Apache-2.0; GPLv2 compatibility for those who take MIT.
- Bad: two license files to ship.

## Links

- [ADR-0014](0014-project-license-gpl-3.0-only.md) — superseded.
- [`../../LICENSE-MIT`](../../LICENSE-MIT),
  [`../../LICENSE-APACHE`](../../LICENSE-APACHE).
- [`../process/release.md`](../process/release.md) — licensing note.
