# Contributing to `imgdedup`

This guide applies to **both human contributors and AI agents**. The repository
is the single source of truth: if a rule matters, it is written down here or in
[`docs/process/`](docs/process/).

## Ground rules

1. **Read [`CLAUDE.md`](CLAUDE.md) and [`docs/process/sdlc.md`](docs/process/sdlc.md) first.**
2. **Brainstorm before building.** Clarify intent and requirements; never assume.
   Surface gaps and hidden assumptions to the owner before writing code or specs.
3. **Test-Driven Development is mandatory** for product code.
4. **Run local validation before every push** and fix all findings.
5. **Conventional Commits** for commits and PR titles; **Semantic Versioning**.

## Development workflow

See [`docs/process/sdlc.md`](docs/process/sdlc.md) for the authoritative version.
Summary:

1. **Pick a task** from [`docs/planning/backlog.md`](docs/planning/backlog.md)
   (or its task file in [`docs/planning/tasks/`](docs/planning/tasks/)). Mark it
   in progress in [`docs/planning/status.md`](docs/planning/status.md).
2. **Create a branch**: `feature/<slug>`, `fix/<slug>`, `chore/<slug>`,
   `docs/<slug>`. See [`docs/process/branching.md`](docs/process/branching.md).
3. **Implement** using TDD (failing test → pass → refactor).
4. **Validate locally**: run `make validate` (Markdown/spell/link check, shell
   script and workflow lint, format check, lint, tests). Fix everything.
5. **Commit** with a Conventional Commit message
   (see [`docs/process/commits.md`](docs/process/commits.md)).
6. **Push** and **open a PR** with a Conventional Commit title and a description
   that follows [`.github/pull_request_template.md`](.github/pull_request_template.md).
7. **CI must pass.** The **architect** agent reviews (as PR comments).
8. **The human owner merges** (squash). The branch auto-deletes.
9. **Update the planning docs** (status/backlog) in the same PR.

## Local validation

Run:

```sh
make validate
```

This runs Markdown lint, spell check, link check, shell script lint
(`shellcheck`), GitHub Actions workflow lint (`actionlint`), `cargo fmt`/
`clippy`, and the full Rust test suite. See [`scripts/`](scripts/) and the
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

`imgdedup` is licensed under **`GPL-3.0-only`** ([`LICENSE`](LICENSE),
[ADR-0014](docs/adr/0014-project-license-gpl-3.0-only.md)). By contributing you
agree that your contribution is licensed under those same terms. Do not paste in
code whose license is unknown or GPLv3-incompatible — including dependencies:
new crates must satisfy the allow-list in [`deny.toml`](deny.toml), which CI
enforces.

## Security

Never commit secrets. Report vulnerabilities per [`SECURITY.md`](SECURITY.md).
The security gate (secret + dependency scanning, security review) must pass
before merge.
