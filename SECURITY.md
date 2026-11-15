# Security Policy

## Reporting a vulnerability

If you discover a security issue, report it privately — do **not** open a
public issue or pull request that describes the vulnerability.

- Use GitHub's private vulnerability reporting: on this repository's
  **Security** tab, choose **Report a vulnerability**
  ([direct link](https://github.com/intruder1912/imgdedup/security/advisories/new)).
  The report stays private between you and the maintainer.
- If you cannot use that form, open an issue that says only that you have a
  security report and need a private channel. Leave the details out.

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
  `dependency-review-action` is not used. It was re-evaluated for the public
  repository: its vulnerability and license checks repeat what `cargo-deny`
  already enforces from `Cargo.lock` and `deny.toml`, and GitHub's advisory
  alerts do not cover actions pinned by commit SHA, which every action here
  is. See the
  [ADR-0005 amendment](docs/adr/0005-mandatory-tdd-and-security-gate.md#amendment-2026-11-15--dependency-review-action-not-adopted-on-the-public-repository-117).)
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
