#!/usr/bin/env bash
# Local validation runner — mirror of CI's required checks.
# Run before every push. Equivalent to `make validate`.
#
# Phase 0: documentation checks.
# Phase 3: Rust fmt / clippy / test added below.
set -euo pipefail

cd "$(dirname "$0")/.."

if ! command -v npx >/dev/null 2>&1; then
  echo "error: Node.js (npx) is required for local validation." >&2
  echo "       Install Node.js, then re-run." >&2
  exit 1
fi

if ! command -v typos >/dev/null 2>&1; then
  echo "error: typos is required for local validation." >&2
  echo "       Install via 'brew install typos-cli' or 'cargo install typos-cli', then re-run." >&2
  exit 1
fi

if ! command -v lychee >/dev/null 2>&1; then
  echo "error: lychee is required for local validation." >&2
  echo "       Install via 'brew install lychee' or 'cargo install lychee', then re-run." >&2
  exit 1
fi

if ! command -v shellcheck >/dev/null 2>&1; then
  echo "error: shellcheck is required for local validation." >&2
  echo "       Install via 'brew install shellcheck' or 'apt install shellcheck', then re-run." >&2
  exit 1
fi

if ! command -v actionlint >/dev/null 2>&1; then
  echo "error: actionlint is required for local validation." >&2
  echo "       Install via 'brew install actionlint' or 'go install" >&2
  echo "       github.com/rhysd/actionlint/cmd/actionlint@latest', then re-run." >&2
  exit 1
fi

echo "==> Markdown lint"
npx -y markdownlint-cli2@0.23.1 "**/*.md"

echo "==> Spell check"
# Word splitting is the point: typos takes one filename per argument, and
# every tracked path here is plain ASCII with no spaces or glob metacharacters.
# shellcheck disable=SC2046
typos $(git ls-files '*.md')

echo "==> Link check"
lychee --config lychee.toml --no-progress './**/*.md'

echo "==> Shell script lint"
# Tracked files, not a scripts/*.sh glob: a script added below the top level
# of scripts/ (e.g. scripts/release-preflight/ already exists as a directory)
# would otherwise go unlinted with no signal either here or in CI.
# shellcheck disable=SC2046
shellcheck $(git ls-files '*.sh')

echo "==> GitHub Actions workflow lint"
actionlint

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo is required for Rust validation." >&2
  echo "       Install Rust via https://rustup.rs, then re-run." >&2
  exit 1
fi

echo "==> cargo fmt --check"
cargo fmt --check

echo "==> cargo clippy"
cargo clippy --all-targets -- -D warnings

echo "==> cargo test"
cargo test

if ! command -v cargo-deny >/dev/null 2>&1; then
  echo "error: cargo-deny is required for the dependency audit." >&2
  echo "       Install: brew install cargo-deny  (or: cargo install cargo-deny --locked)" >&2
  exit 1
fi

echo "==> cargo deny check"
cargo deny check

echo "✓ local validation passed"
