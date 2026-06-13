# Commit Conventions

This project follows [Conventional Commits 1.0.0](https://www.conventionalcommits.org/en/v1.0.0/).
Commit messages and PR titles are linted in CI. Because merges are **squash-only**,
the **PR title is the commit that lands on `main`** and drives automated
versioning — it must always be valid.

## Format

```
type(scope): short imperative subject

[optional body explaining what and why, not how]

[optional footer(s): BREAKING CHANGE:, Refs:, Closes:]
```

- **type** — one of the allowed types below.
- **scope** — optional, lowercase area of the codebase (e.g. `scan`, `hash`,
  `cli`, `ci`, `docs`).
- **subject** — imperative mood, no trailing period, ≤ ~72 chars.

## Allowed types

| Type | Use for | SemVer effect |
|---|---|---|
| `feat` | A new feature | **minor** |
| `fix` | A bug fix | **patch** |
| `docs` | Documentation only | none |
| `test` | Adding/adjusting tests | none |
| `refactor` | Code change, no behavior change | none |
| `perf` | Performance improvement | patch |
| `build` | Build system / dependencies | none |
| `ci` | CI configuration | none |
| `chore` | Maintenance, tooling, housekeeping | none |
| `style` | Formatting only | none |
| `revert` | Reverts a previous commit | varies |

## Breaking changes

Append `!` after the type/scope **and/or** add a `BREAKING CHANGE:` footer:

```
feat(cli)!: rename --recursive to --recurse

BREAKING CHANGE: the --recursive flag is now --recurse.
```

A breaking change triggers a **major** version bump (once `1.0.0` is reached;
pre-1.0 bumps follow the SemVer 0.x conventions configured in the release tool).

## Examples

```
feat(scan): add recursive directory traversal
fix(hash): handle truncated JPEG without panicking
docs(adr): add ADR-0006 selecting the technology stack
chore(ci): pin actions to commit SHAs
```

## How commits are documented

The Conventional Commit history **is** the change log: the release tooling
(semantic-release) generates `CHANGELOG.md` and GitHub Release notes from it (see
[`release.md`](release.md)). Keep subjects accurate and scoped so the generated
notes are meaningful. The genesis commit of this repository was
`chore: bootstrap repository foundation and governance`.
