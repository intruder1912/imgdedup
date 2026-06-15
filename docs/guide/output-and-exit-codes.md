# Output and exit codes

[← Guide index](README.md)

This page covers how `imgdedup` reports results and how it signals outcomes
through its exit code.

## Human-readable report (default)

`imgdedup` prints a human-readable report to standard output. It has three parts,
in this order:

1. An optional `Warnings:` section listing each file that was skipped and why
   (only present when something was skipped).
2. One block per duplicate group: a header with the group number, its file count,
   and its reclaimable space, then the `KEEP` line and a `REMOVE` line per
   duplicate (plus a `PROTECT` line and a `NOTE` line for any hard-linked
   siblings, which are kept and excluded from the counts).
3. An optional `Already deduplicated (hard-linked)` block listing hard-link sets
   that have nothing to reclaim (only present when such sets exist).
4. A `Summary:` line with the totals — or `No duplicate groups found.` when there
   is nothing to report.

```text
Warnings:
  SKIP  /photos/locked.jpg: could not hash file: permission denied

Group 1/1 (2 files, 1024 bytes reclaimable):
  KEEP  /photos/a.jpg
  REMOVE /photos/b.jpg

Summary: 1 duplicate group, 2 files, 1024 bytes reclaimable.
```

For a fuller walkthrough of each line, see
[Getting started](getting-started.md#reading-the-report). How the `KEEP` file is
chosen — and why hard-linked siblings are excluded from the counts — is in
[Choosing what to keep](choosing-what-to-keep.md).

## `--json`: machine-readable output

> **Not available yet.** The `--json` flag is present on the command line but does
> not produce output in this release; it is coming in a later release. For now,
> use the human-readable report above. This section will document the JSON schema
> once it ships.

## `--exit-code`: signal "duplicates found"

Normally a successful run exits `0` whether or not duplicates were found. Pass
`--exit-code` to make a *clean run that found duplicates* exit `3` instead — handy
in CI or scripts that want to fail when duplicates exist:

```sh
imgdedup --exit-code ./photos
```

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Success. With `--exit-code`, also means no duplicates were found. |
| `1` | Run-level error — e.g. an input path could not be scanned. |
| `2` | Usage error — invalid arguments. |
| `3` | Duplicates were found. Only emitted when `--exit-code` is passed. |

## Next

- Back to the [guide index](README.md).
