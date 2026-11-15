# imgdedup

A fast, cross-platform command-line tool for finding and safely removing
duplicate images.

> **Status: pre-1.0, under active development.** `imgdedup`
> scans your paths, finds exact duplicate groups, and prints a human-readable
> dry-run report showing the keeper and the files that would be removed — or a
> machine-readable document with `--json`. **The trash/delete actions are
> shipped:** `--apply` moves duplicates to the OS trash, and `--apply --delete`
> removes them permanently. A plain run is still always a dry run — nothing
> changes on disk unless you pass `--apply`. See
> [the issues](https://github.com/intruder1912/imgdedup/issues) for open work and
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
  alone is refused with an error.
- **Never byte-blind, twice.** A hash match alone never authorizes removal;
  files are byte-for-byte verified first during the scan, **and re-verified
  again immediately before each file is actually removed** — the keeper, the
  match, and the file identity are all re-checked, so a file that changed (or
  was swapped for a symlink, or became a hard link to its keeper) between the
  scan and the apply is skipped and reported rather than removed.
- **A removal failure never escalates.** If trashing a file fails (for example,
  no trash is available), the file is left in place and reported — `--apply`
  never falls back to permanently deleting it.
- **Hard-linked copies (already deduplicated on disk) are never proposed for
  removal:** they are shown as `PROTECT` and excluded from the reclaimable-space
  figure, and a set that is *only* hard links is reported as already-deduplicated
  rather than as a duplicate group.

See [Removing duplicates safely](docs/guide/removing-duplicates-safely.md) for
the full safety model, what a skipped file looks like, and recovery guidance.

## Installing

Every [GitHub Release](https://github.com/intruder1912/imgdedup/releases)
carries prebuilt binaries for Linux, macOS (Apple Silicon and Intel), and
Windows — download the archive for your platform, verify its checksum, and
extract it. Alternatively, build from source (Rust + clap, chosen in
[ADR-0007](docs/adr/0007-technology-stack-rust-clap.md)):

```sh
# Requires a Rust toolchain (https://rustup.rs). The repo pins the toolchain
# via rust-toolchain.toml, so rustup will fetch the right version automatically.
git clone https://github.com/intruder1912/imgdedup.git
cd imgdedup
cargo build --release
# the binary is then at ./target/release/imgdedup
```

See [Getting started](docs/guide/getting-started.md#install) for the
download/checksum walkthrough and platform-specific notes (the repository is
currently private, so release assets are not anonymously downloadable — fetch
them with `gh release download`, which authenticates).

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

### Removing duplicates

Everything above is read-only. To actually reclaim space, add `--apply`, which
moves every non-keeper to your OS's trash / recycle bin — recoverable if you
change your mind — and reports what it did:

```sh
# Move duplicates to the trash (recoverable). The keeper is never touched.
imgdedup --apply ./photos
```

For permanent removal instead of trashing, add `--delete` (only valid together
with `--apply`; using it alone is refused):

```sh
# Permanently delete duplicates instead of trashing them.
imgdedup --apply --delete ./photos
```

Immediately before removing each file, `imgdedup` re-checks it one more time —
still exists, still a regular file, still byte-for-byte identical to its keeper,
still not a hard link to it — so a file that changed since the scan is skipped
and reported rather than removed, and re-running `--apply` on an already-clean
tree is a no-op. See
[Removing duplicates safely](docs/guide/removing-duplicates-safely.md) for the
full safety model, including what a skipped file looks like and how to recover
what was trashed.

For scripting, `--json` emits the same results as a machine-readable JSON
document on standard output (groups, keeper, the files that would be or were
removed, totals, and — on an acted run — exactly which paths were removed or
skipped) instead of the human-readable report:

```sh
# Machine-readable output for a script or CI pipeline.
imgdedup --json ./photos > result.json
```

The full JSON schema is documented in
[Output and exit codes](docs/guide/output-and-exit-codes.md#--json-machine-readable-output).

### Saving results to a file

To keep the finished result, **redirect standard output**. The moment stdout is
redirected or piped it carries exactly the report or the JSON document and
nothing else — progress moves to standard error — so the saved file stays a
clean report or a valid JSON document:

```sh
imgdedup ./photos > report.txt         # human-readable report
imgdedup --json ./photos > result.json # machine-readable results
```

Redirection only writes the file when the run finishes, though. For a long or
unattended run, use `--output <file>`: it writes a durable record **while the
run works** — the start of the run, progress milestones with elapsed time, each
duplicate group as it is confirmed, every warning and error, each removal on an
`--apply` run, and finally the complete result — so a run that is interrupted
or killed still leaves everything it had found. It is additive: what the
terminal shows and what standard output receives stay exactly the same. The
record is human-readable by default and NDJSON (one JSON object per line) under
`--json`:

```sh
imgdedup --output run.txt ./photos
imgdedup --json --output run.ndjson ./photos > result.json
```

`--output` never destroys a file you did not ask it to. Re-running with the
same path replaces the earlier record and keeps it as `run.txt.bak` (one
backup). Any other existing file — a mistyped path that names a photo, say —
is refused before anything is scanned, and left untouched; pass
`--overwrite-output` if you really mean to replace it (it is kept as
`<file>.bak` too).

A finished record ends with a `===== RESULT` banner followed by the report (or,
under `--json`, a final line whose `type` is `result`); a record without it is
from a run that did not finish. The record survives the process being killed,
but not a power cut or host crash. Note that `--json` on its own keeps nothing
until the run completes — pair it with `--output` for anything long.

See [Saving results to a file](docs/guide/output-and-exit-codes.md#saving-results-to-a-file)
and [`--output`: a durable record of the run](docs/guide/output-and-exit-codes.md#--output-file-a-durable-record-of-the-run)
for details.

### Live progress

While a scan runs, `imgdedup` shows a live progress indicator by default — the
current phase, `x of y` files, a percentage, and how long the run has been
going, for example `hashing: 1200 of 28450 (4%) (12s elapsed)`, ending on a
`done:` line with the total time. On an interactive terminal it appears on
**standard output**, ahead of the report, together with a `trashed:` /
`deleted:` line for each file an `--apply` run removes. As soon as stdout is
redirected, piped, or carrying a `--json` document, progress moves to
**standard error** and the `trashed:` / `deleted:` lines are not printed (the
report's own summary of what was removed still is), so
`imgdedup ./photos > report.txt` and `imgdedup --json ./photos > result.json`
still capture a clean report or JSON document. Standard error is otherwise
kept for things that went wrong. Pass `--no-progress` to turn the indicator
off:

```sh
imgdedup --no-progress ./photos
```

See [Where output goes](docs/guide/output-and-exit-codes.md#where-output-goes-stdout-and-stderr)
for the full stream rules and
[Progress while it runs](docs/guide/output-and-exit-codes.md#progress-while-it-runs)
for what the indicator shows.

If a scan feels slow, `--timings` prints how long each stage of the run took to
standard error — a diagnostic, and one that replaces the progress indicator
(the two would collide on screen):

```sh
imgdedup --timings ./photos
```

See [`--timings`: where the time went](docs/guide/output-and-exit-codes.md#--timings-where-the-time-went)
for how to read the output and how to combine it with `--json`.

Common options at a glance: `--no-recurse`, `--hidden`, `--follow-symlinks`,
`--ext <LIST>` (scope); `--keep <shortest|oldest|newest>` (which copy to keep);
`--apply` (move non-keepers to the OS trash); `--delete` (with `--apply`,
permanently delete instead); `--json` (machine-readable output);
`--output <FILE>` (write a durable, continuously updated record of the run);
`--overwrite-output` (let `--output` replace a file that is not an earlier
record);
`--exit-code` (signal duplicates via exit status); `--no-progress` (disable the
live progress indicator); `--timings` (print per-stage timings to stderr for
diagnosis). Run `imgdedup --help` for the full list.

## Detailed guide

For in-depth, per-topic documentation — the full flag reference, the keeper rule,
the safety model, output formats, and exit codes — see the **[user
guide](docs/guide/README.md)**:

- [Getting started](docs/guide/getting-started.md)
- [Scanning and scope](docs/guide/scanning-and-scope.md)
- [Choosing what to keep](docs/guide/choosing-what-to-keep.md)
- [Removing duplicates safely](docs/guide/removing-duplicates-safely.md)
- [Output and exit codes](docs/guide/output-and-exit-codes.md)

## Using imgdedup with an AI agent

`imgdedup` ships an official Claude Code plugin — a skill, a `/imgdedup:dedupe`
slash command, and a read-only `imgdedup-operator` subagent — so you can find
and clean up duplicates by asking Claude instead of hand-writing commands. It
always scans read-only first, always shows you the plan, and always asks
before it touches disk; permanent deletion needs its own separate, explicit
confirmation. See
[Using imgdedup with an AI agent](docs/guide/using-with-claude.md) for install
steps and a worked example.

(This is a different thing from the AI agent **crew that builds imgdedup**,
described below — that's contributor tooling; this is a product feature for
anyone running `imgdedup`.)

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
| See what's planned / in progress | [Issues](https://github.com/intruder1912/imgdedup/issues); direction in [`docs/planning/roadmap.md`](docs/planning/roadmap.md) |
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

`imgdedup` is licensed under either of

- the MIT license ([`LICENSE-MIT`](LICENSE-MIT)), or
- the Apache License, Version 2.0 ([`LICENSE-APACHE`](LICENSE-APACHE)),

at your option (`MIT OR Apache-2.0`). The decision is recorded in
[ADR-0020](docs/adr/0020-relicense-mit-or-apache-2.0.md).

Copyright © 2026 the `imgdedup` authors.

Both license files ship in every release archive next to the binary, and
`imgdedup --version` prints the license and the URL of the source repository.

The binary is built with third-party Rust libraries under permissive licenses
(MIT, Apache-2.0 and similar). Their license texts are collected in
[`THIRD-PARTY-LICENSES.txt`](THIRD-PARTY-LICENSES.txt), which also ships in
every release archive from the release after `v0.7.0` on.

Releases up to and including `v0.6.0` were published under `GPL-3.0-only` and
stay under that license for anyone who received them.

### Contributions

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in `imgdedup` by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
