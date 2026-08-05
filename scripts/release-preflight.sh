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
#   --version <semver> Version to stamp and assert in `--version` output
#                      (default: 0.0.0-preflight+<12-char short SHA of --ref>).
#   --keep            Don't remove the worktree on exit; print its path
#                      instead, for inspecting a failed build.

set -euo pipefail

ref="HEAD"
version=""
keep=0

while [ "$#" -gt 0 ]; do
  case "$1" in
  --ref)
    ref="$2"
    shift 2
    ;;
  --version)
    version="$2"
    shift 2
    ;;
  --keep)
    keep=1
    shift
    ;;
  -h | --help)
    cat <<'EOF'
usage: scripts/release-preflight.sh [--ref <git-ref>] [--version <semver>] [--keep]

Rehearses release.yml's assets job locally: x86_64-unknown-linux-gnu in a
podman container, aarch64-apple-darwin natively (Apple Silicon macOS only).
Builds in a disposable git worktree; never touches the real working tree,
creates no tag, uploads nothing. See the file header for details.

  --ref <git-ref>      Ref to build from (default: HEAD).
  --version <semver>   Version to stamp/assert (default: a preflight placeholder).
  --keep                Keep the worktree on exit instead of removing it.
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

resolved_sha="$(git rev-parse "$ref")"
short_sha="${resolved_sha:0:12}"
: "${version:=0.0.0-preflight+$short_sha}"
tag="preflight-$short_sha"

# Under target/, not a system tmp dir: podman's default macOS machine only
# shares a fixed set of host paths (home directory tree, /tmp, /var/folders),
# and this keeps the rehearsal inside a path already known to be shared and
# already gitignored.
worktree_dir="$repo_root/target/release-preflight-worktree"
out_root="$repo_root/target/release-preflight"

cleanup() {
  if [ "$keep" -eq 1 ]; then
    echo "kept worktree for inspection: $worktree_dir" >&2
    return
  fi
  git -C "$repo_root" worktree remove --force "$worktree_dir" >/dev/null 2>&1 || rm -rf "$worktree_dir"
}
trap cleanup EXIT

git worktree remove --force "$worktree_dir" >/dev/null 2>&1 || true
rm -rf "$worktree_dir"
git worktree add --detach "$worktree_dir" "$resolved_sha" >/dev/null

echo "== rehearsing release build: ref=$ref ($resolved_sha) version=$version tag=$tag =="

echo
echo "-- glibc Linux (podman, native platform — see file header re: x86_64 emulation) --"
podman build \
  --quiet \
  -t imgdedup-release-preflight \
  -f "$repo_root/scripts/release-preflight/Containerfile" \
  "$repo_root/scripts/release-preflight" >/dev/null
podman run --rm \
  -v "$worktree_dir:/work" \
  -w /work \
  imgdedup-release-preflight \
  bash -c '
    set -euo pipefail
    rustup show
    linux_target="$(rustc -vV | sed -n "s/^host: //p")"
    echo -n "$linux_target" > /work/out-linux-target
    if [ "$linux_target" != "x86_64-unknown-linux-gnu" ]; then
      echo "note: this container native-built $linux_target, not release.yml'\''s x86_64-unknown-linux-gnu leg — the mechanics (stamp/build/verify/package) are still exercised for real, but this is not that specific leg. See this script'\''s header." >&2
    fi
    scripts/build-release-artifact.sh \
      "$linux_target" "'"$version"'" "'"$tag"'" tar.gz "/work/out-linux"
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
    rustup show
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
