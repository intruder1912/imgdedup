# Getting started

[← Guide index](README.md)

This page gets you from nothing to a first, safe scan.

## Install

`imgdedup` is written in Rust ([ADR-0007](../adr/0007-technology-stack-rust-clap.md)).
There are no prebuilt binaries yet, so build from source. You need a Rust
toolchain — install one from [rustup.rs](https://rustup.rs). The repository pins
the exact toolchain via `rust-toolchain.toml`, so `rustup` fetches the right
version automatically.

```sh
git clone https://github.com/intruder1912/imgdedup.git
cd imgdedup
cargo build --release
# the binary is now at ./target/release/imgdedup
```

Optionally put it on your `PATH`, or run it directly as `./target/release/imgdedup`.

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
    hard-linked: /photos/beach.jpg, /photos/beach-hardlink.jpg

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

## Where to go next

- Control what gets scanned → [Scanning and scope](scanning-and-scope.md)
- Decide which copy is kept → [Choosing what to keep](choosing-what-to-keep.md)
- Actually reclaim space → [Removing duplicates safely](removing-duplicates-safely.md)
- Script against the output → [Output and exit codes](output-and-exit-codes.md)
