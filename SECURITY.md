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
  (see [`.gitignore`](.gitignore)); a secret-scanning check (`gitleaks`, full
  git history, every push and PR) runs in CI.
- **Dependency audit.** `cargo-deny` runs in CI, covering RUSTSEC advisories
  (supersedes a standalone `cargo-audit`), license compliance, banned crates,
  and source allow-listing (config in [`deny.toml`](deny.toml)), plus a weekly
  scheduled run so a new advisory against an untouched `Cargo.lock` is still
  caught. **Dependabot** proposes the resulting dependency and GitHub Actions
  updates. (GitHub's own `dependency-review-action` requires GitHub Advanced
  Security, unavailable on free private repositories; re-evaluated if the
  repository goes public.)
- **Security review** is part of the architect's review on every PR.
- **Least privilege** for CI: workflow tokens are scoped to the minimum
  permissions required; only one job in the repository (`release.yml`'s
  `assets` upload) holds `contents: write`, and it runs only on manual
  dispatch, never from a pull request.

See [`docs/adr/0005-mandatory-tdd-and-security-gate.md`](docs/adr/0005-mandatory-tdd-and-security-gate.md)
for the full security-gate decision.
