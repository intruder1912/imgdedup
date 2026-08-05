#!/usr/bin/env bash
# Stamp, build, verify and package one release artifact for one target triple.
#
# This is the shared implementation behind .github/workflows/release.yml's
# `assets` job and scripts/release-preflight.sh (TASK-0334). It knows nothing
# about GitHub Actions or podman — it only assumes a checked-out working tree
# at CWD, a Rust toolchain matching this repo's rust-toolchain.toml already on
# PATH, and (on the linux-gnu leg) the C toolchain blake3 needs to build.
# Extracting exactly this logic — never the checks-green verification or the
# `gh release upload` step, which stay inline in release.yml — means the local
# pre-flight tool and the CI job that publishes real binaries cannot drift
# apart on the part that is expensive to get wrong twice (version stamping,
# archive layout, checksum format) while the privileged, credentialed steps
# never run outside the workflow.
#
# Usage: build-release-artifact.sh <target> <version> <tag> <archive> <out-dir>
#   target   Rust target triple, e.g. x86_64-unknown-linux-gnu. Must match the
#            host's own triple (rustc -vV); this script never cross-compiles.
#   version  SemVer to stamp into Cargo.toml and assert in `--version` output,
#            without a leading 'v' (e.g. 1.2.3).
#   tag      Release tag used only in the archive's filename stem, e.g.
#            v1.2.3 or a preflight/<sha> placeholder — never parsed or
#            validated as SemVer here.
#   archive  tar.gz or zip.
#   out-dir  Directory the .{tar.gz,zip} and its .sha256 are written into;
#            created if missing.

set -euo pipefail

if [ "$#" -ne 5 ]; then
  echo "usage: $0 <target> <version> <tag> <archive:tar.gz|zip> <out-dir>" >&2
  exit 2
fi

target="$1"
version="$2"
tag="$3"
archive="$4"
out_dir="$5"

case "$archive" in
tar.gz | zip) ;;
*)
  echo "::error::unsupported archive type '$archive' (want tar.gz or zip)" >&2
  exit 2
  ;;
esac

# Resolved once, up front: every later step but this one may `cd` (the
# packaging step below cds into a staging directory), so out-dir must be
# absolute before that happens or a relative path would resolve against the
# wrong directory.
mkdir -p "$out_dir"
out_dir="$(cd "$out_dir" && pwd)"

# Guards against a mismatched toolchain/runner silently producing a binary
# whose filename claims an architecture it doesn't contain — the same check
# release.yml ran inline before this extraction, now shared instead of
# duplicated per matrix leg.
host="$(rustc -vV | sed -n 's/^host: //p')"
if [ "$host" != "$target" ]; then
  echo "::error::host toolchain ($host) does not match requested target ($target); this script does not cross-compile" >&2
  exit 1
fi

# Cargo.toml's `version` is a permanent development placeholder — the release
# process never commits a bump back to `main` (ADR-0015). This rewrite must
# only ever touch a disposable checkout: release.yml's ephemeral runner, or
# (locally) the throwaway git-worktree release-preflight.sh builds in. Never
# run this script against your own working tree's Cargo.toml directly.
test "$(grep -c '^version = ' Cargo.toml)" -eq 1
sed -e "s/^version = .*/version = \"$version\"/" Cargo.toml >Cargo.toml.stamped
mv Cargo.toml.stamped Cargo.toml
grep -qx "version = \"$version\"" Cargo.toml

# Not --locked: the stamp above makes Cargo.lock's own `imgdedup` entry stale
# by design; cargo rewrites just that one entry, leaving every dependency pin
# untouched.
cargo build --release

bin_suffix=""
case "$target" in
*windows*) bin_suffix=".exe" ;;
esac
bin="target/release/imgdedup${bin_suffix}"

got="$("$bin" --version)"
want="imgdedup $version"
if [ "$got" != "$want" ]; then
  echo "::error::binary reports '$got', expected '$want'" >&2
  exit 1
fi

stem="imgdedup-${tag}-${target}"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/$stem"
cp "$bin" "$stage/$stem/"
cp LICENSE README.md "$stage/$stem/"

(
  cd "$stage"
  if [ "$archive" = "zip" ]; then
    file="$stem.zip"
    7z a -tzip "$file" "$stem" >/dev/null
  else
    file="$stem.tar.gz"
    COPYFILE_DISABLE=1 tar -czf "$file" "$stem" # COPYFILE_DISABLE: no macOS ._ AppleDouble entries
  fi

  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" >"$file.sha256"
  else
    shasum -a 256 "$file" >"$file.sha256" # macOS has no sha256sum
  fi

  mv "$file" "$file.sha256" "$out_dir/"
)

echo "built: $out_dir/$stem.$archive (+ .sha256)"
