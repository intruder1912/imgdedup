#!/usr/bin/env bash
# Local pre-flight rehearsal of release.yml's `assets` job (TASK-0334).
#
# Builds two of the four release legs on this machine, using the exact same
# scripts/build-release-artifact.sh the real release job calls:
#   - a glibc Linux container via rootless podman
#     (scripts/release-preflight/Containerfile mirrors the ubuntu-24.04
#     runner closely enough for a sanity check — see that file's own caveat)
#   - aarch64-apple-darwin, natively, when run on Apple Silicon macOS
#
# The podman leg deliberately does NOT force --platform linux/amd64: measured
# on this project's own Apple Silicon dev machine, qemu-user emulation of the
# x86_64 Rust toolchain is not just slow but unreliable — rustc itself
# segfaulted under emulation before any of this project's code was even
# compiled (TASK-0334). The container instead builds for
# podman's native platform, i.e. whatever the podman machine's host
# architecture actually is: on an x86_64 host (or an x86_64 CI runner) that IS
# release.yml's x86_64-unknown-linux-gnu leg, byte-for-byte; on an Apple
# Silicon host it is aarch64-unknown-linux-gnu instead — not one of today's
# four shipped release targets, but a real, reliable native-container rehearsal
# of the exact same stamp/build/verify/package mechanics. The script reports
# which target it actually built and says plainly when it isn't the CI leg.
#
# Windows (x86_64-pc-windows-msvc) and Intel macOS (x86_64-apple-darwin) stay
# CI-only: neither has a local host here, and TASK-0334's own scope explicitly
# limits this tool to the two legs above.
#
# Never touches this repository's actual working tree: everything happens in
# a disposable `git worktree`, because build-release-artifact.sh stamps
# Cargo.toml's version in place. The worktree is removed on exit regardless
# of success or failure.
#
# This is a rehearsal, not a release: no tag is created, nothing is uploaded
# anywhere, and the version stamped is a clearly-marked placeholder unless
# overridden.
#
# Usage: scripts/release-preflight.sh [--ref <git-ref>] [--version <semver>] [--keep]
#   --ref <git-ref>   Ref to build from (default: HEAD).
#   --version <semver> Version to stamp and assert in the built binary's `-V`
#                      output (and in `--version`'s first line)
#                      (default: 0.0.0-preflight+<12-char short SHA of --ref>).
#   --keep            Don't remove the worktree on exit; print its path
#                      instead, for inspecting a failed build.

set -euo pipefail

ref="HEAD"
version=""
tag=""
keep=0

require_value() {
  # $1: flag name (for the error message), $#: remaining args including the
  # flag itself. Without this, `--ref` as the last argument fails on `set -u`
  # with an opaque "$2: unbound variable" instead of naming the actual mistake.
  if [ "$#" -lt 2 ]; then
    echo "error: $1 requires a value" >&2
    exit 2
  fi
}

while [ "$#" -gt 0 ]; do
  case "$1" in
  --ref)
    require_value "$@"
    ref="$2"
    shift 2
    ;;
  --version)
    require_value "$@"
    version="$2"
    shift 2
    ;;
  --tag)
    require_value "$@"
    tag="$2"
    shift 2
    ;;
  --keep)
    keep=1
    shift
    ;;
  -h | --help)
    cat <<'EOF'
usage: scripts/release-preflight.sh [--ref <git-ref>] [--version <semver>] [--tag <tag>] [--keep]

Rehearses release.yml's assets job locally: a glibc Linux target in a podman
container (podman's own native platform — x86_64-unknown-linux-gnu only on an
x86_64 host; see the file header re: qemu emulation) and aarch64-apple-darwin
natively (Apple Silicon macOS only). Builds in a disposable git worktree from a
committed ref; never touches the real working tree, creates no tag, uploads
nothing. See the file header for details.

  --ref <git-ref>      Ref to build from (default: HEAD). Committed state only —
                       uncommitted working-tree changes are not rehearsed.
  --version <semver>   Version to stamp/assert (default: a preflight placeholder).
  --tag <tag>          Value used only in the archive's filename stem (default:
                       preflight-<sha>). Pass a real vX.Y.Z-shaped tag to
                       rehearse the exact filename a real release would produce.
  --keep               Keep the worktree on exit instead of removing it.
EOF
    exit 0
    ;;
  *)
    echo "unknown argument: $1" >&2
    exit 2
    ;;
  esac
done

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

# ^{commit}: today's v* tags are lightweight, so this is a no-op on them, but
# a hand-created annotated tag resolves to a *tag object* SHA without it —
# harmless here (only used for the worktree checkout and the placeholder
# version/tag strings below), but worth being exact about.
resolved_sha="$(git rev-parse "$ref^{commit}")"
short_sha="${resolved_sha:0:12}"
: "${version:=0.0.0-preflight+$short_sha}"
: "${tag:=preflight-$short_sha}"

# Under target/, not a system tmp dir: podman's default macOS machine only
# shares a fixed set of host paths (home directory tree, /tmp, /var/folders),
# and this keeps the rehearsal inside a path already known to be shared and
# already gitignored. Suffixed with $$ so two concurrent runs on this machine
# don't `rm -rf` each other's worktree out from under themselves — only this
# tool's own throwaway output is ever at risk, never a developer's real work.
worktree_dir="$repo_root/target/release-preflight-worktree-$$"
out_root="$repo_root/target/release-preflight"

cleanup() {
  if [ "$keep" -eq 1 ]; then
    echo "kept worktree for inspection: $worktree_dir" >&2
    return
  fi
  git -C "$repo_root" worktree remove --force "$worktree_dir" >/dev/null 2>&1 || rm -rf "$worktree_dir"
}
trap cleanup EXIT INT TERM

# Cleared, not appended to: a stale artifact from a previous --version/--tag
# would otherwise sit alongside this run's and both get listed as "artifacts"
# at the end, undermining the whole point of a tool built around not
# mislabeling what it produced. Only ever this tool's own output directory.
rm -rf "$out_root"

git worktree add --detach "$worktree_dir" "$resolved_sha" >/dev/null

echo "== rehearsing release build: ref=$ref ($resolved_sha) version=$version tag=$tag =="

echo
echo "-- glibc Linux (podman, native platform — see file header re: x86_64 emulation) --"
# Build context is this repo's real working tree (the Containerfile itself
# never varies by --ref), but everything it compiles comes from the mounted
# worktree at --ref below — the two are deliberately different checkouts.
podman build \
  --quiet \
  -t imgdedup-release-preflight \
  -f "$repo_root/scripts/release-preflight/Containerfile" \
  "$repo_root/scripts/release-preflight" >/dev/null
# version/tag reach the container as environment variables, not interpolated
# into the bash -c string: a --version or --tag containing a quote or a
# command substitution would otherwise be evaluated by the inner shell
# (verified: `1.0.0"; echo pwned; :"` broke out of the string entirely). Only
# this tool's own local invocation is exposed either way — nothing here
# reaches release.yml or any credentialed context — but this is exactly the
# unquoted-interpolation shape the rest of this repo's workflows avoid on
# purpose, so the fix applies here too.
podman run --rm \
  -e PREFLIGHT_VERSION="$version" \
  -e PREFLIGHT_TAG="$tag" \
  -v "$worktree_dir:/work" \
  -w /work \
  imgdedup-release-preflight \
  bash -c '
    set -euo pipefail
    rustup toolchain install --no-self-update # the toolchain rust-toolchain.toml pins
    linux_target="$(rustc -vV | sed -n "s/^host: //p")"
    echo -n "$linux_target" > /work/out-linux-target
    if [ "$linux_target" != "x86_64-unknown-linux-gnu" ]; then
      echo "note: this container native-built $linux_target, not release.yml'\''s x86_64-unknown-linux-gnu leg — the mechanics (stamp/build/verify/package) are still exercised for real, but this is not that specific leg. See this script'\''s header." >&2
    fi
    scripts/build-release-artifact.sh \
      "$linux_target" "$PREFLIGHT_VERSION" "$PREFLIGHT_TAG" tar.gz "/work/out-linux"
  '
linux_target="$(cat "$worktree_dir/out-linux-target")"
mkdir -p "$out_root/$linux_target"
cp "$worktree_dir"/out-linux/* "$out_root/$linux_target/"

echo
echo "-- aarch64-apple-darwin (native) --"
host="$(rustc -vV | sed -n 's/^host: //p')"
if [ "$host" = "aarch64-apple-darwin" ]; then
  (
    cd "$worktree_dir"
    rustup toolchain install --no-self-update # the toolchain rust-toolchain.toml pins
    ./scripts/build-release-artifact.sh \
      aarch64-apple-darwin "$version" "$tag" tar.gz "$worktree_dir/out-macos"
  )
  mkdir -p "$out_root/aarch64-apple-darwin"
  cp "$worktree_dir"/out-macos/* "$out_root/aarch64-apple-darwin/"
else
  echo "skip: this host ($host) is not aarch64-apple-darwin — run natively on Apple Silicon macOS to rehearse this leg" >&2
fi

echo
echo "artifacts (not a release, not uploaded anywhere): $out_root"
find "$out_root" -type f | sort
