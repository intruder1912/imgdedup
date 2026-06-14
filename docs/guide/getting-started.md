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

## Where to go next

- Control what gets scanned → [Scanning and scope](scanning-and-scope.md)
- Decide which copy is kept → [Choosing what to keep](choosing-what-to-keep.md)
- Actually reclaim space → [Removing duplicates safely](removing-duplicates-safely.md)
- Script against the output → [Output and exit codes](output-and-exit-codes.md)
