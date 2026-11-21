# Contributing to `imgdedup`

This guide applies to **both human contributors and AI agents**. The repository
is the single source of truth: if a rule matters, it is written down here or in
[`docs/process/`](docs/process/).

**New here?** Start with [Issues and pull requests](#issues-and-pull-requests):
open an issue first, and send a pull request from a fork once the issue is
accepted (typo and link fixes can go straight to a pull request). The owner
runs the architect review on it; you do not need the agent setup to
contribute.

## Ground rules

1. **Read [`CLAUDE.md`](CLAUDE.md) and [`docs/process/sdlc.md`](docs/process/sdlc.md) first.**
   Agents must; human contributors can skim `sdlc.md` and skip the
   agent-specific parts.
2. **Brainstorm before building.** Clarify intent and requirements; never assume.
   Surface gaps and hidden assumptions to the owner before writing code or specs.
3. **Test-Driven Development is mandatory** for product code.
4. **Run local validation before every push** and fix all findings.
5. **Conventional Commits** for commits and PR titles; **Semantic Versioning**.

## Development workflow

See [`docs/process/sdlc.md`](docs/process/sdlc.md) for the authoritative version.
Summary:

1. **Start from an accepted issue** (see
   [Issues and pull requests](#issues-and-pull-requests)). Work is tracked in
   [GitHub Issues](https://github.com/intruder1912/imgdedup/issues); the unit
   being worked on carries the `in progress` label.
2. **Create a branch**: `feature/<slug>`, `fix/<slug>`, `chore/<slug>`,
   `docs/<slug>`. See [`docs/process/branching.md`](docs/process/branching.md).
3. **Implement** using TDD (failing test → pass → refactor).
4. **Validate locally**: run `make validate` (Markdown/spell/link check, shell
   script and workflow lint, format check, lint, tests, dependency audit).
   Fix everything.
5. **Commit** with a Conventional Commit message
   (see [`docs/process/commits.md`](docs/process/commits.md)).
6. **Push** and **open a PR** with a Conventional Commit title and a description
   that follows [`.github/pull_request_template.md`](.github/pull_request_template.md).
7. **CI must pass.** The **architect** agent reviews (as PR comments).
8. **The human owner merges** (squash). The branch auto-deletes.
9. **The PR closes its issue** (`Closes #123` in the description). The PR
   description is the record of the unit: what was verified, what the review
   found, what was not done.

## Issues and pull requests

- **Issues are welcome**: bug reports, feature requests, questions about
  behavior. The owner triages them. A new issue carries `needs-triage`; one
  the owner wants done gets `accepted`.
- **Open a pull request only for an accepted issue.** A feature here needs
  written requirements and tests first, so agreeing on the change before the
  code saves you work. A pull request without an accepted issue will be
  pointed here and closed or left until the issue is settled.
- **Exception:** typos, broken links and one-line documentation corrections
  can be sent as a pull request directly.
- **Security problems do not go in issues.** See [`SECURITY.md`](SECURITY.md).

The decision is recorded in
[ADR-0021](docs/adr/0021-issue-based-planning-for-a-public-repository.md).

## Local validation

Run:

```sh
make validate
```

This runs Markdown lint, spell check, link check, shell script lint
(`shellcheck`), GitHub Actions workflow lint (`actionlint`), `cargo fmt`/
`clippy`, the full Rust test suite, and the dependency audit (`cargo deny`,
which needs `cargo-deny` installed). See [`scripts/`](scripts/) and the
[`Makefile`](Makefile). If a tool is not installed, the script tells you how to
install it. CI runs the same checks and is the enforcement boundary.

**No real images are needed to build or test this project.** The test suite
(`tests/golden_set.rs` and friends) synthesizes its own fixtures at test time:
`tests/support/images.rs` generates, deterministically, the smallest byte
sequences the pipeline accepts as images for every supported format signature,
and `tests/support/tree.rs` builds the fixture trees around them — nothing is
downloaded and no real-world image is checked into the repository. A handful of
larger scale/benchmark tests (`tests/scale.rs`) are `#[ignore]`d by default and
only run via `make scale`; a plain `cargo test` or `make validate` is fast and
fully self-contained.

## Commits & PRs

- Format: `type(scope): subject` — e.g. `feat(scan): add recursive directory walk`.
- Allowed types and the full guide: [`docs/process/commits.md`](docs/process/commits.md).
- Because merges are **squash-only**, the **PR title becomes the commit on
  `main`** and drives the automated release — it MUST be a valid Conventional
  Commit.

## Reviews

The **architect** agent reviews every PR for correctness, design, security, and
convention adherence, leaving comments. Reviews are advisory in GitHub terms
(0 required approvals) — the **human owner** is the merge gate and makes the
final call. See [`docs/adr/0003-main-branch-protection-and-merge-model.md`](docs/adr/0003-main-branch-protection-and-merge-model.md).

## Licensing of contributions

`imgdedup` is licensed under **`MIT OR Apache-2.0`**
([`LICENSE-MIT`](LICENSE-MIT), [`LICENSE-APACHE`](LICENSE-APACHE),
[ADR-0020](docs/adr/0020-relicense-mit-or-apache-2.0.md)). Unless you
explicitly state otherwise, any contribution intentionally submitted for
inclusion in `imgdedup` by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions. Do not
paste in code whose license is unknown or not permissive — including
dependencies: new crates must satisfy the allow-list in
[`deny.toml`](deny.toml), which CI enforces. A change to the dependency set
also means regenerating
[`THIRD-PARTY-LICENSES.txt`](THIRD-PARTY-LICENSES.txt) with `make notices`
(needs `cargo-about`); CI fails when that file is out of date.

## Security

Never commit secrets. Report vulnerabilities per [`SECURITY.md`](SECURITY.md).
The security gate (secret + dependency scanning, security review) must pass
before merge.
