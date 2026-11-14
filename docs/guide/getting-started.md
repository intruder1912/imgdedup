# Getting started

[← Guide index](README.md)

This page gets you from nothing to a first, safe scan.

## Install

### Prebuilt binary (recommended)

Each [GitHub Release](https://github.com/intruder1912/imgdedup/releases)
carries a binary archive for four platforms: `x86_64-unknown-linux-gnu`,
`aarch64-apple-darwin` (Apple Silicon), `x86_64-apple-darwin` (Intel Mac), and
`x86_64-pc-windows-msvc`. The repository is currently **private**, so release
assets are not anonymously downloadable: a plain `curl` of a
`releases/download/...` URL returns `404` even with a token header. Use the
authenticated [`gh`](https://cli.github.com) CLI (`gh auth login` once), which
resolves the asset through the API:

```sh
# Example: Linux — adjust the pattern for your platform. Omitting the tag
# downloads from the latest release, so this never needs a version bump.
gh release download --repo intruder1912/imgdedup \
  --pattern 'imgdedup-*-x86_64-unknown-linux-gnu.tar.gz*'
sha256sum -c imgdedup-*-x86_64-unknown-linux-gnu.tar.gz.sha256  # macOS: shasum -a 256 -c
tar -xzf imgdedup-*-x86_64-unknown-linux-gnu.tar.gz
# the imgdedup binary is now in imgdedup-<version>-x86_64-unknown-linux-gnu/
```

The trailing `*` in the pattern fetches the archive and its `.sha256` together.
Windows artifacts are `.zip` rather than `.tar.gz` (unzip instead of `tar`).
Once the repository is public, the same assets are downloadable with a plain
`curl -LO <asset URL>`.

The archive also contains `LICENSE-MIT`, `LICENSE-APACHE` and `README.md`. Binaries are unsigned:
on macOS, a browser-downloaded archive is quarantined by Gatekeeper — clear
the flag after verifying the checksum (`xattr -d com.apple.quarantine
./imgdedup`; `gh`/`curl` downloads are not quarantined, so the attribute may
simply not be there); on Windows, SmartScreen may warn on first run. Full
detail — the target matrix, checksum format, and why these two caveats exist —
is in [Distribution targets](../process/release.md#distribution-targets).

### Build from source

`imgdedup` is written in Rust ([ADR-0007](../adr/0007-technology-stack-rust-clap.md)).
You need a Rust toolchain — install one from [rustup.rs](https://rustup.rs).
The repository pins the exact toolchain via `rust-toolchain.toml`, so `rustup`
fetches the right version automatically.

```sh
git clone https://github.com/intruder1912/imgdedup.git
cd imgdedup
cargo build --release
# the binary is now at ./target/release/imgdedup
```

Either way, optionally put the binary on your `PATH`, or run it directly
(`./imgdedup` or `./target/release/imgdedup`).

## Your first scan

`imgdedup` is **safe by default**: a plain run is a dry run that only reports what
it would do — it never changes anything on disk unless you ask it to (see
[Removing duplicates safely](removing-duplicates-safely.md)).

```sh
imgdedup ./photos
```

This scans `./photos` (recursively) for images and reports the duplicate groups it
finds. You can pass more than one path:

```sh
imgdedup ~/Pictures /mnt/backup/photos
```

A path can be a directory or a single file.

## Reading the report

A run prints a **dry-run report** and changes nothing on disk. For each group of
byte-for-byte identical images it shows the file it would keep (`KEEP`) and the
duplicates it would remove (`REMOVE`), then a one-line summary:

```text
Group 1/2 (3 files, 2048 bytes reclaimable):
  KEEP  /photos/cat.jpg
  REMOVE /photos/backup/cat.jpg
  REMOVE /photos/old/cat (1).jpg
Group 2/2 (2 files, 1024 bytes reclaimable):
  KEEP  /photos/dog.png
  REMOVE /photos/dog-copy.png

Summary: 2 duplicate groups, 5 files, 3072 bytes reclaimable.
```

Reading it:

- **Group header** — `Group 1/2 (3 files, 2048 bytes reclaimable)` numbers the
  group out of the total, then gives that group's file count (the keeper plus the
  files that would be removed) and the bytes you would reclaim by removing the
  duplicates.
- **`KEEP`** — the one file kept per group. How it is chosen is covered in
  [Choosing what to keep](choosing-what-to-keep.md).
- **`REMOVE`** — each duplicate that *would* be removed. Nothing is removed today;
  this is a report only.
- **Summary** — totals across all groups: number of groups, total files (keepers
  plus removable), and total reclaimable space.

If nothing matches, the report is simply:

```text
No duplicate groups found.
```

### Hard-linked copies

If some of the matching files are **hard links** to the same physical file, they
are already a single file on disk — removing one frees nothing. `imgdedup` keeps
them and labels them `PROTECT` rather than `REMOVE`:

```text
Group 1/1 (2 files, 1024 bytes reclaimable):
  KEEP  /photos/cat.jpg
  REMOVE /photos/backup/cat.jpg
  PROTECT /photos/also-cat.jpg  (hard-linked; removing it frees no space)
  NOTE  hard-link set: /photos/also-cat.jpg → /photos/cat-link.jpg
```

A `PROTECT` file is never proposed for removal and is never counted in the
group's file count or reclaimable bytes. When an entire match is *only*
hard-linked copies — there is nothing genuinely separate to remove — it is not
reported as a duplicate group at all; instead it appears under an informational
block:

```text
Already deduplicated (hard-linked) — nothing to reclaim:
  /photos/beach.jpg
    hard-linked: /photos/beach-hardlink.jpg, /photos/beach.jpg

Summary: no duplicate groups to remove; 1 already-deduplicated hard-link set.
```

See [Choosing what to keep](choosing-what-to-keep.md#hard-links-are-never-collateral)
for the full hard-link behavior.

### Warnings

Files that cannot be read or are not recognized as supported images are **skipped,
not fatal** — the run continues and lists them in a `Warnings:` section at the top
of the report:

```text
Warnings:
  SKIP  /photos/locked.jpg: could not hash file: permission denied
  SKIP  /photos/notes.txt: content not recognized as a supported image format

Group 1/1 (2 files, 1024 bytes reclaimable):
  ...
```

A skipped file never changes the exit code on its own (see
[Output and exit codes](output-and-exit-codes.md)).

## Watching a long run

While the scan works, a live progress line shows the current phase, how far
along it is, and how long the run has been going — for example
`hashing: 1200 of 28450 (4%) (12s elapsed)` — on your terminal, ahead of the
report it eventually prints, and finishes on a `done:` line with the run's
total time. If you redirect the report to a file
(`imgdedup ./photos > report.txt`), the progress line moves to standard error
instead, so it still shows on your terminal while the file stays a clean
report.

A long run is also worth recording. `--output` writes what the run produces to
a file **as it goes** — progress milestones, each duplicate group as it is
confirmed, warnings, and finally the full report — so a scan you interrupt, or
that dies, still leaves what it had found:

```sh
imgdedup --output run.txt ./photos
```

A record that ends with a `===== RESULT` banner and the report is from a run
that finished; one without it is from a run that did not. Groups are confirmed
near the end of a run, after hashing, so a run stopped early may have recorded
progress but no groups yet. Running the same command again replaces the
record and keeps the previous one as `run.txt.bak`; a file at that path that
is not an `imgdedup` record is never replaced unless you pass
`--overwrite-output` — the run stops instead, before scanning anything.
See [`--output`: a durable record of the run](output-and-exit-codes.md#--output-file-a-durable-record-of-the-run)
for everything that lands in the file and what survives an interrupted run.

## Where to go next

- Control what gets scanned → [Scanning and scope](scanning-and-scope.md)
- Decide which copy is kept → [Choosing what to keep](choosing-what-to-keep.md)
- Actually reclaim space → [Removing duplicates safely](removing-duplicates-safely.md)
- Script against the output → [Output and exit codes](output-and-exit-codes.md)
- Keep a durable record of a long run → [`--output`](output-and-exit-codes.md#--output-file-a-durable-record-of-the-run)
