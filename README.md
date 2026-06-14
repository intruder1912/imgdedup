# imgdedup

A fast, cross-platform command-line tool for finding and safely removing
duplicate images.

> **Status: under active development (Phase 3 — implementation).** Scanning, scope
> filtering, magic-byte image confirmation, and the core content-hash
> deduplication engine (duplicate detection) are implemented; the results
> **report**, **JSON output**, and the **trash/delete actions** are not yet wired.
> **So today the tool scans and detects duplicates internally, but does not yet
> print duplicate groups or modify any files.** See
> [`docs/planning/status.md`](docs/planning/status.md) for the live state and
> [`docs/planning/roadmap.md`](docs/planning/roadmap.md) for the path to v1.0.

## What it does

`imgdedup` scans one or more directories for images, identifies **exact
(byte-for-byte) duplicates** by content — not by filename — and helps you remove
the redundant copies safely. Detection is layered for safety: an extension
allow-list selects candidates, magic-byte sniffing confirms each is really an
image, a content hash pre-filters likely matches, and a **byte-for-byte
comparison is the final authority** before any file is ever considered for
removal.

Scope for the first release (exact duplicates only) is defined in
[`docs/prd/PRD-0001-exact-image-deduplication.md`](docs/prd/PRD-0001-exact-image-deduplication.md);
the algorithm and safety model are in
[`docs/adr/0008-core-deduplication-algorithm-and-semantics.md`](docs/adr/0008-core-deduplication-algorithm-and-semantics.md).

## Safety model

Safety is the product's first principle:

- **Dry run by default.** A normal run only *reports* what it would remove.
  Nothing on disk changes unless you pass `--apply`.
- **Recoverable by default when acting.** `--apply` moves non-keepers to the
  **OS trash / recycle bin**, not oblivion.
- **Permanent deletion is opt-in and guarded.** `--delete` permanently removes
  non-keepers and is only valid *together with* `--apply`; using `--delete`
  alone is refused.
- **Never byte-blind.** A hash match alone never authorizes removal; files are
  byte-for-byte verified first. Hard-linked copies (already deduplicated on disk)
  are never proposed for removal.

## Installing

The technology stack is **Rust + clap** (chosen in
[ADR-0007](docs/adr/0007-technology-stack-rust-clap.md)). No prebuilt binaries are
published yet, so build from source:

```sh
# Requires a Rust toolchain (https://rustup.rs). The repo pins the toolchain
# via rust-toolchain.toml, so rustup will fetch the right version automatically.
git clone https://github.com/intruder1912/imgdedup.git
cd imgdedup
cargo build --release
# the binary is then at ./target/release/imgdedup
```

## Usage

```text
imgdedup [OPTIONS] <PATH>...
```

`<PATH>...` is one or more files or directories to scan. A plain run is a **dry
run** — it reports duplicate groups and changes nothing.

```sh
# Dry run: report duplicate groups under ./photos (default; nothing changes).
imgdedup ./photos

# Scan multiple locations at once.
imgdedup ~/Pictures /mnt/backup/photos

# Reclaim space by moving duplicates to the OS trash (recoverable).
imgdedup --apply ./photos
```

Common options at a glance: `--no-recurse`, `--hidden`, `--follow-symlinks`,
`--ext <LIST>` (scope); `--keep <shortest|oldest|newest>` (which copy to keep);
`--apply` / `--delete` (act on duplicates); `--json`, `--exit-code` (output).
Run `imgdedup --help` for the full list.

## Detailed guide

For in-depth, per-topic documentation — the full flag reference, the keeper rule,
the safety model, output formats, and exit codes — see the **[user
guide](docs/guide/README.md)**:

- [Getting started](docs/guide/getting-started.md)
- [Scanning and scope](docs/guide/scanning-and-scope.md)
- [Choosing what to keep](docs/guide/choosing-what-to-keep.md)
- [Removing duplicates safely](docs/guide/removing-duplicates-safely.md)
- [Output and exit codes](docs/guide/output-and-exit-codes.md)

## How this repository works

This project is built and maintained by a crew of AI agents and a human owner.
Everything needed to pick up the work lives **in the repository itself** — not in
any chat session — so any contributor (human or agent) can resume from the repo
content alone.

| If you want to… | Read |
|---|---|
| Understand the AI agent setup | [`CLAUDE.md`](CLAUDE.md) and [`docs/process/crew.md`](docs/process/crew.md) |
| Contribute (humans or agents) | [`CONTRIBUTING.md`](CONTRIBUTING.md) |
| Follow the development cycle | [`docs/process/sdlc.md`](docs/process/sdlc.md) |
| See what's planned / in progress | [`docs/planning/`](docs/planning/) |
| Read product requirements | [`docs/prd/`](docs/prd/) |
| Read architecture decisions | [`docs/adr/`](docs/adr/) |
| Report a security issue | [`SECURITY.md`](SECURITY.md) |

## Conventions at a glance

- **Conventional Commits** for all commit/PR titles.
- **Semantic Versioning**, with releases automated from commit history.
- **Trunk-based**: short-lived `feature/`, `fix/`, `chore/`, `docs/` branches →
  squash-merge into protected `main` via reviewed PRs.
- **Test-Driven Development** is mandatory for product code, with a security gate
  before every merge.

## License

No license is published yet. The repository is **private**; all rights reserved.
A licensing decision will be recorded in an ADR before any public release.
