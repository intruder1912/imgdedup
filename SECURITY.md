# Security Policy

## Reporting a vulnerability

This repository is **private**. If you discover a security issue, report it
privately to the repository owner — do **not** open a public issue or PR that
describes the vulnerability.

- Preferred: open a [GitHub Security Advisory](https://docs.github.com/en/code-security/security-advisories)
  ("Report a vulnerability") on this repository.
- Alternatively: contact the repository owner directly.

Please include reproduction steps, affected versions/commits, and impact. We aim
to acknowledge reports promptly and will coordinate a fix and disclosure timeline
with you.

## Supported versions

Only the **latest published release** is supported with security fixes — there
is no long-term-support branch. Check the
[latest release](https://github.com/intruder1912/imgdedup/releases/latest) and
upgrade before reporting an issue that may already be fixed.

## Security practices in this project

Security is a gate on every change, not an afterthought:

- **No secrets in the repository.** Secrets are kept out of version control
  (see [`.gitignore`](.gitignore)); a secret-scanning check (`gitleaks`, over
  the **full git history** rather than just the diff) runs in CI on every pull
  request and every push to `main`.
- **Dependency audit.** `cargo-deny` runs in CI whenever a dependency input
  changes (`Cargo.toml`, `Cargo.lock`, `deny.toml`, or the workflow itself),
  covering RUSTSEC advisories (supersedes a standalone `cargo-audit`), license
  compliance, banned crates, and source allow-listing (config in
  [`deny.toml`](deny.toml)) — plus a weekly scheduled run, so a new advisory
  against an untouched `Cargo.lock` is still caught. **Dependabot** proposes the
  resulting dependency and GitHub Actions updates. (GitHub's own
  `dependency-review-action` requires GitHub Advanced Security, unavailable on
  free private repositories; re-evaluated if the repository goes public.)
- **Static analysis.** `clippy` with `-D warnings` over all targets runs in CI
  on every code change. CodeQL is deliberately *not* used: it has no
  first-class Rust support, so clippy takes its place
  ([ADR-0007](docs/adr/0007-technology-stack-rust-clap.md)).
- **Security review** is part of the architect's review on every PR.
- **Least privilege** for CI: workflows default to `contents: read` and jobs
  widen only where they must. The only `contents: write` in the repository is
  in [`release.yml`](.github/workflows/release.yml) — its `release` job creates
  the tag and GitHub Release, its `assets` job uploads the built binaries — and
  that workflow is **manual-dispatch only**, so no pull request and no push can
  start it.

See [`docs/adr/0005-mandatory-tdd-and-security-gate.md`](docs/adr/0005-mandatory-tdd-and-security-gate.md)
for the full security-gate decision.
