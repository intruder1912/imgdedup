# Removing duplicates safely

[← Guide index](README.md)

Safety is `imgdedup`'s first principle. Nothing is removed unless you explicitly
ask for it, and the default way of removing is recoverable.

> **In development.** The reporting and action stages are being implemented. The
> safety model below is fixed by design (PRD-0001,
> [ADR-0008](../adr/0008-core-deduplication-algorithm-and-semantics.md),
> [ADR-0009](../adr/0009-cross-platform-trash-and-recycle.md)); this page documents
> the intended commands.

## Dry run by default

A plain run **changes nothing**. It scans and reports the duplicate groups it
would act on, so you can review before committing:

```sh
imgdedup ./photos
```

## `--apply`: move duplicates to the trash

`--apply` moves the non-keeper of each group to your operating system's **trash /
recycle bin** — recoverable if you change your mind:

```sh
imgdedup --apply ./photos
```

## `--delete`: permanent removal (guarded)

`--delete` permanently removes non-keepers instead of trashing them. It is opt-in
and **only valid together with `--apply`** — using `--delete` on its own is
refused with an error, so you can't permanently delete by accident:

```sh
# Permanently delete duplicates (no trip through the trash).
imgdedup --apply --delete ./photos
```

## Why removal is safe

- **Content, not names.** Duplicates are identified by file content. A file's name
  or extension never causes it to be grouped or removed.
- **Byte-for-byte verified.** A fast content hash only *pre-filters* likely
  matches; before anything is grouped for removal, the files are compared
  **byte for byte**. A hash match alone never authorizes removal.
- **Non-images are never touched.** Each candidate's content is confirmed to be a
  real image first; mislabeled or unrecognized files are skipped.
- **Hard links are respected.** Files that are already the same physical file
  (hard links) are never proposed for removal — see
  [Choosing what to keep](choosing-what-to-keep.md).

## Next

- [Output and exit codes](output-and-exit-codes.md) — review what would be (or
  was) removed, and use exit codes in scripts.
