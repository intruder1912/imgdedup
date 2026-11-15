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
  request and every push to `main`, and is a required check. GitHub's secret
  scanning and **push protection** are also enabled on the repository: push
  protection blocks a push containing a secret of a type GitHub supports for
  it, before it reaches the repository. The person pushing can override the
  block by giving a reason, so it is a safeguard against accidents, not a
  guarantee.
- **Dependency audit.** `cargo-deny` runs in CI whenever a dependency input
  changes (`Cargo.toml`, `Cargo.lock`, `deny.toml`, or the workflow itself),
  covering RUSTSEC advisories (supersedes a standalone `cargo-audit`), license
  compliance, banned crates, and source allow-listing (config in
  [`deny.toml`](deny.toml)) — plus a weekly scheduled run, so a new advisory
  against an untouched `Cargo.lock` is still caught. **Dependabot** proposes
  version updates for dependencies and GitHub Actions, and Dependabot alerts
  and security updates are enabled. (GitHub's own
  `dependency-review-action` is not used. It was re-evaluated for the public
  repository: its vulnerability and license checks repeat what `cargo-deny`
  already enforces from `Cargo.lock` and `deny.toml`, and GitHub's advisory
  alerts do not cover actions pinned by commit SHA, which every action here
  is. See the
  [ADR-0005 amendment](docs/adr/0005-mandatory-tdd-and-security-gate.md#amendment-2026-11-15--dependency-review-action-not-adopted-on-the-public-repository-117).)
- **Static analysis.** `clippy` with `-D warnings` over all targets runs in CI
  on every code change and is the required static-analysis gate for the Rust
  code ([ADR-0007](docs/adr/0007-technology-stack-rust-clap.md)). GitHub's
  CodeQL default setup is also enabled on the repository. It scans the Rust
  code, the JavaScript release tooling and the GitHub Actions workflows; its
  results are advisory, not a required check.
- **Security review** is part of the architect's review on every PR.
- **Pinned actions.** Every GitHub Action is referenced by a full commit SHA,
  and the repository's Actions settings require it: a workflow that references
  an action by tag or branch fails to run.
- **Protected `main` and release tags.** Changes reach `main` only through a
  pull request; the required checks (`ci-gate`, `checks`, `security`) must
  pass, except that the repository admin can merge a pull request whose checks
  have not passed (kept for cases such as a renamed required check). No one
  can push to `main` outside a pull request. Published `v*` release tags
  cannot be moved or deleted by anyone, the admin included, without first
  disabling the ruleset. Both are enforced by repository rulesets, which
  anyone can read on GitHub and in [`.github/rulesets/`](.github/rulesets/).
  See [ADR-0003](docs/adr/0003-main-branch-protection-and-merge-model.md).
- **Least privilege** for CI: workflows default to `contents: read` and jobs
  widen only where they must. The only `contents: write` in the repository is
  in [`release.yml`](.github/workflows/release.yml) — its `release` job creates
  the tag and GitHub Release, its `assets` job uploads the built binaries — and
  that workflow is **manual-dispatch only**, so no pull request and no push can
  start it.

See [`docs/adr/0005-mandatory-tdd-and-security-gate.md`](docs/adr/0005-mandatory-tdd-and-security-gate.md)
for the full security-gate decision.
