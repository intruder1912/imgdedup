# imgdedup

A fast, cross-platform command-line tool for finding and safely removing
duplicate images.

> **Status: under active development (Phase 3 — implementation).** The
> **read-only detect-and-report** workflow is shipped: `imgdedup` scans your
> paths, finds exact duplicate groups, and prints a human-readable dry-run report
> showing the keeper and the files that would be removed — or a machine-readable
> document with `--json`. **The trash/delete actions (`--apply` / `--delete`) are
> part of the command-line surface but are not functional yet** — they are coming
> in a later release. So today the tool reports duplicates but never modifies any
> files. See
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
  Nothing on disk changes. (Today the tool is dry-run only; the acting flags
  below are reserved for a later release.)
- **Recoverable by default when acting.** `--apply` will move non-keepers to the
  **OS trash / recycle bin**, not oblivion. *(Not functional yet.)*
- **Permanent deletion is opt-in and guarded.** `--delete` will permanently
  remove non-keepers and is only valid *together with* `--apply`; using
  `--delete` alone is refused. *(Not functional yet.)*
- **Never byte-blind.** A hash match alone never authorizes removal; files are
  byte-for-byte verified first. Hard-linked copies (already deduplicated on disk)
  are never proposed for removal: they are shown as `PROTECT` and excluded from
  the reclaimable-space figure, and a set that is *only* hard links is reported as
  already-deduplicated rather than as a duplicate group.

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

`<PATH>...` is one or more files or directories to scan. A run is a **dry run** —
it reports duplicate groups and changes nothing.

```sh
# Report duplicate groups under ./photos (nothing changes).
imgdedup ./photos

# Scan multiple locations at once.
imgdedup ~/Pictures /mnt/backup/photos
```

The report lists each duplicate group with the file to keep (`KEEP`) and the
duplicates that would be removed (`REMOVE`), then a summary with the group/file
counts and the space you could reclaim:

```text
Group 1/1 (2 files, 5242880 bytes reclaimable):
  KEEP  /photos/beach.jpg
  REMOVE /photos/backup/beach (copy).jpg

Summary: 1 duplicate group, 2 files, 5242880 bytes reclaimable.
```

Choose which copy is kept with `--keep`, and make the run signal "duplicates
found" with `--exit-code`:

```sh
# Keep the oldest copy in each group instead of the shortest path.
imgdedup --keep oldest ./photos

# Exit 3 (instead of 0) when any duplicates are found — handy in CI.
imgdedup --exit-code ./photos
```

For scripting, `--json` emits the same results as a machine-readable JSON
document on standard output (groups, keeper, the files that would be removed, and
totals) instead of the human-readable report:

```sh
# Machine-readable output for a script or CI pipeline.
imgdedup --json ./photos > result.json
```

The full JSON schema is documented in
[Output and exit codes](docs/guide/output-and-exit-codes.md#--json-machine-readable-output).

### Saving results to a file

To keep a result for later, **redirect standard output** — there is no separate
output flag. It works for both the human report and `--json` because stdout
carries only the report/JSON while run-level errors go to standard error, so the
saved file stays a clean report or valid JSON document:

```sh
imgdedup ./photos > report.txt         # human-readable report
imgdedup --json ./photos > result.json # machine-readable results
```

See [Saving results to a file](docs/guide/output-and-exit-codes.md#saving-results-to-a-file)
for details.

Common options at a glance: `--no-recurse`, `--hidden`, `--follow-symlinks`,
`--ext <LIST>` (scope); `--keep <shortest|oldest|newest>` (which copy to keep);
`--json` (machine-readable output); `--exit-code` (signal duplicates via exit
status). The `--apply` and `--delete` flags exist but are **not functional yet**
(coming in a later release). Run `imgdedup --help` for the full list.

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
