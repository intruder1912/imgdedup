# Changelog

**The changelog of record is the [GitHub Releases](https://github.com/intruder1912/imgdedup/releases)
page.** This file is a signpost, not a second copy.

Releases are published by semantic-release from the
[Conventional Commit](docs/process/commits.md) history: it computes the
[SemVer](https://semver.org/spec/v2.0.0.html) bump, creates the `v<version>` tag
and writes the release notes onto the GitHub Release. Nothing is committed back
to `main` — that is a deliberate decision
([ADR-0015](docs/adr/0015-release-publishing-tag-and-github-release.md)), taken
so that publishing needs no bot account, no personal access token and no
branch-protection bypass. The cost is exactly this: no generated changelog in
the working tree.

To read the notes without a browser:

```sh
gh release list
gh release view v0.1.0
```

Unreleased work in progress is tracked in
[`docs/planning/status.md`](docs/planning/status.md); the release procedure is
[`docs/process/release.md`](docs/process/release.md).

> **Before the first release.** No version has been published yet. Everything up
> to this point is foundation work — governance, process, planning, CI, and the
> PRD-0001 implementation slices — recorded in `docs/planning/status.md`.
