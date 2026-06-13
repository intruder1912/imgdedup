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

The project is pre-release (Phase 0). No versions are supported for production use
yet. This section will be updated when the first release is published.

## Security practices in this project

Security is a gate on every change, not an afterthought:

- **No secrets in the repository.** Secrets are kept out of version control
  (see [`.gitignore`](.gitignore)); a secret-scanning check runs in CI.
- **Dependabot** proposes regular dependency and GitHub Actions updates.
  (GitHub's dependency-review requires Advanced Security, which is unavailable on
  free private repositories; a language-level dependency audit is added in
  Phase 2.)
- **Security review** is part of the architect's review on every PR.
- **Least privilege** for CI: workflow tokens are scoped to the minimum
  permissions required.

See [`docs/adr/0005-mandatory-tdd-and-security-gate.md`](docs/adr/0005-mandatory-tdd-and-security-gate.md)
for the full security-gate decision. Language-specific scanning (e.g. CodeQL,
dependency audit) is added once the stack is chosen (Phase 2).
