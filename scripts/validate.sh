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

echo "==> Markdown lint"
npx -y markdownlint-cli2 "**/*.md"

echo "==> Spell check"
npx -y cspell --no-progress "**/*.md"

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

echo "✓ local validation passed"
