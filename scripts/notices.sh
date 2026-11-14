#!/usr/bin/env bash
# Generate or check THIRD-PARTY-LICENSES.txt, the third-party license notices
# bundled in every release archive (TASK-0348).
#
# The file is committed. `make notices` rewrites it; `make notices-check` and
# the `security` CI job fail when it no longer matches what Cargo.lock,
# about.toml and the template produce. scripts/build-release-artifact.sh only
# copies it, so no extra tool runs on the path that builds published binaries.
#
# Needs cargo-about (brew install cargo-about / cargo install cargo-about).
# CI pins the version in security.yml; a different version may format the
# file differently, so use that one when regenerating.
#
# Usage: notices.sh [--check]

set -euo pipefail

mode="write"
case "${1:-}" in
"") ;;
--check) mode="check" ;;
*)
  echo "usage: $0 [--check]" >&2
  exit 2
  ;;
esac

cd "$(dirname "$0")/.."

notices="THIRD-PARTY-LICENSES.txt"
template="scripts/notices/third-party-licenses.hbs"

if ! command -v cargo-about >/dev/null; then
  echo "error: cargo-about is not installed (brew install cargo-about / cargo install cargo-about)" >&2
  exit 1
fi

# cargo-about reads license texts from the crate sources, and about.toml lists
# every release target, so the sources for all platforms must be present — not
# only the host's. After this, generation runs without the network (--frozen):
# the result depends on Cargo.lock, about.toml, the template and the
# cargo-about version, not on what a remote serves today.
cargo fetch --locked

generated="$(mktemp)"
trap 'rm -f "$generated"' EXIT

# --fail: a crate whose license cannot be determined is an error, not a
# silently missing entry.
cargo about generate --frozen --fail --output-file "$generated" "$template"

if [ "$mode" = "write" ]; then
  cat "$generated" >"$notices" # not cp: mktemp files are mode 0600
  echo "wrote $notices"
  exit 0
fi

if ! diff -u "$notices" "$generated"; then
  echo "error: $notices is out of date. Run 'make notices' and commit the result." >&2
  exit 1
fi
echo "$notices is up to date"
