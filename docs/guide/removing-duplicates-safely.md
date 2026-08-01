# Removing duplicates safely

[← Guide index](README.md)

Safety is `imgdedup`'s first principle. Nothing is removed unless you explicitly
ask for it, and the default way of removing is recoverable.

## Dry run by default

A plain run **changes nothing**. It scans and reports the duplicate groups it
would act on, so you can review before committing:

```sh
imgdedup ./photos
```

`--apply` and `--apply --delete` (below) run the exact same scan and grouping
logic as a dry run. As long as the tree does not change in between, a dry run
is a reliable preview of exactly what an apply will do (see
[dry-run/apply parity](#dry-runapply-parity) below) — so the safe habit is
**dry-run first, read the report, then re-run with `--apply`** once you are
happy with what it would remove.

## `--apply`: move duplicates to the trash

`--apply` moves the non-keeper of each group to your operating system's **trash /
recycle bin** — recoverable if you change your mind:

```sh
imgdedup --apply ./photos
```

The report now describes what actually happened, not just what was found:

```text
Group 1/1 (2 files, 1024 bytes reclaimable):
  KEEP  /photos/a.png
  REMOVE /photos/b.png

Summary: 1 duplicate group, 2 files, 1024 bytes reclaimable.

Trashed 1 file:
  /photos/b.png
```

## `--delete`: permanent removal (guarded)

`--delete` permanently removes non-keepers instead of trashing them — no trip
through the trash, so recovery afterward depends entirely on your own backups.
It is opt-in and **only valid together with `--apply`** — using `--delete` on
its own is refused with an error, so you can't permanently delete by accident:

```sh
# Permanently delete duplicates.
imgdedup --apply --delete ./photos
```

```text
Group 1/1 (2 files, 1024 bytes reclaimable):
  KEEP  /photos/a.png
  REMOVE /photos/b.png

Summary: 1 duplicate group, 2 files, 1024 bytes reclaimable.

Deleted 1 file:
  /photos/b.png
```

Both `--apply` and `--apply --delete` share the same executor and the same
safety checks below — the only difference is the removal mechanism (trash vs.
permanent). Choosing the irreversible option never skips a safety check that
the recoverable one gets.

## Why removal is safe

- **Content, not names.** Duplicates are identified by file content. A file's name
  or extension never causes it to be grouped or removed.
- **Byte-for-byte verified, twice.** A fast content hash only *pre-filters* likely
  matches; before anything is grouped for removal during the scan, the files are
  compared **byte for byte**. A hash match alone never authorizes removal — and
  that comparison is repeated immediately before each file is actually removed
  (see [re-validated at apply time](#re-validated-at-apply-time) below).
- **The keeper is never touched.** Whatever else happens, the file chosen as a
  group's keeper is never passed to the trash or delete mechanism, and a group is
  never left with zero surviving copies.
- **Non-images are never touched.** Each candidate's content is confirmed to be a
  real image first; mislabeled or unrecognized files are skipped.
- **Hard links are respected.** Files that are already the same physical file
  (hard links) are never proposed for removal — they are shown as `PROTECT` and
  excluded from reclaimable space, and a match that is only hard links is reported
  as already-deduplicated. See
  [Choosing what to keep](choosing-what-to-keep.md#hard-links-are-never-collateral).
- **A removal failure never escalates.** If trashing a file fails (see
  [when trash is not available](#when-trash-is-not-available) below), the file is
  left in place and reported. `--apply` never falls back to permanently deleting a
  file just because trashing it did not work — permanent deletion only ever
  happens because you asked for it with `--delete`.

## Re-validated at apply time

Scanning and acting happen as two separate passes, so time passes between them —
the tree you scanned is not necessarily the tree you are about to modify. Before
`imgdedup` trashes or deletes any file, it re-checks that file one more time, right
before touching it:

- The file still exists, and is still a regular file — not, for example, a
  symlink someone put in its place since the scan.
- Its keeper still exists and is still a regular file.
- The file is still byte-for-byte identical to its keeper.
- The file and its keeper are still two different files on disk — not the same
  physical file (which would mean a hard link to the keeper was created since the
  scan; removing it would be removing the keeper by another name).

If any of that cannot be proven — including if it cannot even be checked, for
example because of a read error — the file is **left untouched** rather than
removed. This is what makes the tool safe to re-run even if something else is
touching the same files concurrently, and it is why `--apply --delete` gets
exactly the same protection as `--apply`: both go through this check before every
single removal, no exceptions.

### What a skip looks like

A file that is left in place is reported, never silently dropped. In the human
report it appears after the summary:

```text
Deleted 1 file:
  /photos/b.png
Skipped (left in place):
  /photos/c.png: content changed since the scan
```

This section only appears on a run that acted (`--apply` or `--apply --delete`),
and only when at least one file was skipped; a dry run never shows it, because a
dry run never removes anything to skip. Under `--json` the same information is in
`actions.skipped`, an array of `{"path", "reason"}` objects — see
[Output and exit codes](output-and-exit-codes.md#mode-and-actions-what-actually-happened)
for the full field reference.

The reasons you may see:

| Reason | Meaning |
|---|---|
| `no longer exists` | The file was already gone by the time it was re-checked — most often because a previous run already removed it (see [idempotence](#idempotence-re-running-is-a-no-op) below). |
| `no longer a regular file` | The file is no longer a plain file — for example, it was replaced by a symlink after the scan. |
| `the file to keep is missing or unusable` | The group's keeper is gone or no longer a regular file. Removing the candidate could destroy the last remaining copy, so it is refused. |
| `content changed since the scan` | The file no longer matches its keeper byte for byte. Something modified it after it was scanned. |
| `now hard-linked to the file being kept` | The file became a hard link to its keeper since the scan. It and the keeper are the same physical file now, so removing it would remove the keeper too. |
| `could not be re-verified: <detail>` | The safety check itself could not complete (an I/O error while re-checking). Fails closed: no removal. |
| `removal failed: <detail>` | The check passed, but the actual trash/delete call failed (see [when trash is not available](#when-trash-is-not-available) below). |

## Idempotence: re-running is a no-op

Running `--apply` again on a tree that is already deduplicated finds nothing left
to do:

```text
No duplicate groups found.

Deleted 0 files.
```

The removed files are simply gone from the scan (or, for `--apply` without
`--delete`, gone from their original location — they are in the trash now, not
where the scan would find them), so there is nothing left to group. This holds
whether the previous run trashed or permanently deleted its files, and it holds
even after a run that skipped some files: those files are still exactly where
they were, so a later run will consider them again from scratch.

## When trash is not available

`--apply`'s trash step depends on your operating system providing a usable trash
— a desktop environment's trash on Linux, the Finder trash on macOS, the Recycle
Bin on Windows. Not every environment has one: a headless Linux server or a
minimal container, for instance, may have no trash directory configured at all.

When trashing a file fails for any reason — no trash available, a permission
problem, or anything else — that file is **skipped and reported**, exactly like
the other re-validation skips above (reason: `removal failed: <detail>`). It is
never deleted instead. If you hit this, you can either delete permanently and
knowingly with `--delete`, or move the files to a location that does have a
usable trash.

On macOS specifically, a trashed file is always recoverable from the Trash by
hand even if Finder's "Put Back" menu entry does not appear for it on some
systems — the file is genuinely there, just without that one convenience.

## Recovering what was removed

- **`--apply` (trash):** every non-keeper is recoverable through your OS's normal
  trash / recycle bin UI until you empty it. The report lists every file it
  removed, by full path, so you know exactly what to look for — and lists
  separately any it left in place, with the reason. The same information is in
  `--json` under `actions.removed` and `actions.skipped`; see
  [Output and exit codes](output-and-exit-codes.md#mode-and-actions-what-actually-happened).
  To know the list *before* you act, run a dry run first and keep it —
  [dry-run/apply parity](#dry-runapply-parity) guarantees it matches.
- **`--apply --delete`:** there is no trash step to recover from. The only way
  back is your own backups. Treat `--delete` as final.

## Dry-run/apply parity

For a tree that does not change between the two runs, a dry run's `removable`
list and what `--apply` (or `--apply --delete`) actually removes are exactly the
same set of files, in the same order. This is why "dry-run first" is a genuinely
reliable way to preview an apply — not an approximation of it.

## Next

- [Output and exit codes](output-and-exit-codes.md) — review what would be (or
  was) removed, and use exit codes in scripts.
