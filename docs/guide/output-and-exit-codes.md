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

Pass `--json` to emit the results as a single JSON document on standard output
instead of the human-readable report. It is a formatter over the exact same
result the human report uses — the same groups, keeper, intended actions, and
totals — so scripts can consume `imgdedup` without parsing prose. `--json`
changes only the output; it never changes what is scanned, grouped, or removed.

```sh
imgdedup --json ./photos > result.json
```

A run over a folder with one duplicate group (three copies of one image) and one
mislabeled non-image produces:

```json
{
  "schema_version": 1,
  "mode": "dry-run",
  "groups": [
    {
      "keeper": { "path": "photos/a.png", "size": 24, "mtime": 1784471961 },
      "removable": [
        { "path": "photos/sub/a-copy.png", "size": 24, "mtime": 1784471961 },
        { "path": "photos/sub/a-copy2.png", "size": 24, "mtime": 1784471961 }
      ],
      "protected": [],
      "hard_link_sets": [],
      "reclaimable_bytes": 48
    }
  ],
  "already_deduplicated": [],
  "totals": {
    "duplicate_groups": 1,
    "removable_files": 2,
    "reclaimable_bytes": 48,
    "already_deduplicated_sets": 0
  },
  "warnings": [
    {
      "stage": "sniff",
      "path": "photos/fake.png",
      "reason": "content not recognized as a supported image format"
    }
  ],
  "root_errors": []
}
```

The document is pretty-printed and ends with a trailing newline.

### Field reference (schema version 1)

**Top-level object:**

| Field | Type | Meaning |
|---|---|---|
| `schema_version` | integer | Version of this schema. `1` today. See the stability contract below. |
| `mode` | string | The run mode. Currently always `"dry-run"` (see [`mode` semantics](#mode-semantics)). |
| `groups` | array of Group | The duplicate groups. Each has a `keeper` and may list `removable` and/or `protected` files. A group whose only extra members are hard-linked anchors has an empty `removable` (nothing to reclaim) but still appears here, mirroring the human report's `Group i/N`. |
| `already_deduplicated` | array of HardLinkSet | Identity sets where *every* copy is a hard link to the same inode, so no duplicate group is formed and nothing can be reclaimed. Excluded from `totals` counts. |
| `totals` | Totals object | Run-wide counts. |
| `warnings` | array of Warning | Per-file problems that were reported and skipped (the run continued). |
| `root_errors` | array of RootError | Run-level failures — an input path that could not be scanned. Non-empty means exit code `1`. |

**Group object** (an entry in `groups`):

| Field | Type | Meaning |
|---|---|---|
| `keeper` | File | The one file kept in this group. |
| `removable` | array of File | The non-keepers. In `dry-run` these are the files that *would* be removed. |
| `protected` | array of File | Hard-link anchors kept but not the keeper — removing them frees no space (see below). |
| `hard_link_sets` | array of HardLinkSet | Hard-link identity sets whose representative is part of this group. |
| `reclaimable_bytes` | integer | Bytes reclaimable from this group — the sum of the `removable` sizes. |

**File object** (`keeper` and each entry of `removable` / `protected`):

| Field | Type | Meaning |
|---|---|---|
| `path` | string | The file's path (see the [path caveat](#path-serialization-caveat)). |
| `size` | integer | Size in bytes. |
| `mtime` | integer or null | Last-modified time as Unix epoch seconds, or `null` when the platform did not provide it. |

**HardLinkSet object** (entries of `already_deduplicated` and a group's
`hard_link_sets`):

| Field | Type | Meaning |
|---|---|---|
| `representative` | string | The representative path of the identity set: the shortest path, with lexicographic order as the tie-break. |
| `siblings` | array of string | The *other* paths that share the same inode/file identity. The full link set is `representative` plus `siblings`. |

**Totals object:**

| Field | Type | Meaning |
|---|---|---|
| `duplicate_groups` | integer | Number of entries in `groups`. |
| `removable_files` | integer | Total `removable` files across all groups. |
| `reclaimable_bytes` | integer | Total reclaimable bytes across all groups. |
| `already_deduplicated_sets` | integer | Number of entries in `already_deduplicated`. |

**Warning object** (entries of `warnings`):

| Field | Type | Meaning |
|---|---|---|
| `stage` | string | Where the problem occurred: `"enumerate"` (walking the tree), `"sniff"` (magic-byte image confirmation), or `"engine"` (hashing / byte-verify). |
| `path` | string | The file the warning concerns. |
| `reason` | string | A human-readable explanation. |

**RootError object** (entries of `root_errors`):

| Field | Type | Meaning |
|---|---|---|
| `path` | string | The input path that could not be scanned. |
| `reason` | string | Why it could not be scanned. |

### Stability contract

- `schema_version` is bumped **only on a breaking change** — a field removed or
  renamed, a type changed, or the meaning of an existing field altered.
- **Additive changes do not bump the version.** New fields may be added within
  schema version 1, so a consumer should ignore unknown fields rather than reject
  them.
- Internal representations are deliberately kept out of the schema (for example,
  the content hash is never emitted), so the contract stays stable across
  implementation changes.

### Determinism

For a given filesystem state and set of flags, `--json` emits a **byte-identical**
document across runs and platforms (NFR-3). The groups, keeper choice, and
hard-link sets come from an already canonically sorted result; on top of that the
`warnings` array is **grouped by stage (`enumerate`, `sniff`, `engine`) and
sorted by path within each stage**, the `root_errors` array is **sorted by
path**, and field order is fixed. This makes the output safe to diff or snapshot in CI.

### `mode` semantics

`mode` reports what the run did to the files. Today it is always `"dry-run"`: the
run only reports what it *would* remove and changes nothing on disk, so
`removable` lists intended removals. The acting flags (`--apply`, `--delete`) are
not functional yet; when they ship, `mode` will take additional values to
distinguish a dry run from a run that trashed or deleted files, and the per-group
lists will report the actions actually performed.

### `protected`, `hard_link_sets`, and `already_deduplicated`

These three fields express how `imgdedup` handles hard links — files that are
different paths to the *same* bytes on disk (same inode/file identity). Removing
one such path frees no space, so they are never proposed for removal (the anchor
rule; see [ADR-0008](../adr/0008-core-deduplication-algorithm-and-semantics.md)
and [Choosing what to keep](choosing-what-to-keep.md#hard-links-are-never-collateral)).

- A **`protected`** entry is a path belonging to a hard-link *anchor* set that
  is kept alongside the `keeper`: it is neither the keeper nor removable, and
  its size is excluded from `reclaimable_bytes`. Every path of a non-keeper
  anchor set is listed; if the keeper is itself hard-linked, its sibling paths
  (not the keeper itself) appear here too. A group may consist of a keeper plus only protected
  anchors, in which case `removable` is empty and `reclaimable_bytes` is `0`.
- A group's **`hard_link_sets`** list the identity sets whose representative is
  part of that group, so a script can see exactly which paths share storage.
- **`already_deduplicated`** holds identity sets that offer *nothing* to reclaim
  (every copy is a hard link to the same inode). These emit no duplicate group at
  all and are excluded from the `totals` group/file/reclaimable counts — they are
  reported so you know they exist, not because there is anything to do.

### `mtime` and null values

`mtime` is Unix epoch **seconds**. When the platform or filesystem does not report
a modification time for a file, the field is `null` rather than a guessed value —
consumers should handle `null`.

### Path serialization caveat

Paths are serialized as text. On a path containing bytes that are not valid UTF-8,
the invalid bytes are replaced with the Unicode replacement character (`U+FFFD`)
rather than failing the whole document. A round-tripped path is therefore not
guaranteed to be byte-for-byte identical to the on-disk path in that rare case; do
not assume a `path` string can always be used verbatim to re-open the file.

### Diagnostics: JSON vs. stderr

Under `--json`, standard output is **exactly one JSON document** — nothing else is
written there. Diagnostics appear as follows:

- **Per-file `warnings`** (skipped files) appear **only** inside the JSON
  document, in the `warnings` array. They are not echoed to stderr.
- **Run-level `root_errors`** appear in the JSON `root_errors` array **and** are
  additionally written to standard error, so an unreadable input path is visible
  even when you pipe stdout straight into a JSON parser. A non-empty `root_errors`
  also drives the exit code to `1`.

So a script can rely on stdout being pure JSON, while a human watching the
terminal still sees run-level failures on stderr.

## Progress while it runs

While a scan runs, `imgdedup` shows a live progress indicator on **standard
error**, enabled by default, so you can see it is working and how far along it
is. It never writes to standard output — in the default report or with
`--json` — so redirecting stdout (see
[Saving results to a file](#saving-results-to-a-file) below) always captures a
clean report or JSON document, with nothing progress-related mixed in.

The indicator shows:

- The current **phase**: `scanning` (enumerating and magic-byte-confirming
  candidate files), `hashing` (content-hash pre-filtering), or `verifying`
  (byte-for-byte confirmation against each group's keeper). These are
  user-facing labels for where the run is, not a contract on the internal
  pipeline.
- Once the candidate count is known, **`x of y` files done and a
  percentage**, where `y` is the number of candidate files confirmed as
  images, fixed once enumeration and magic-byte confirmation finish. Before
  that the total is unknown, so the indicator shows a plain counting state
  (for example `scanning...`) instead of `x of y`.

### What the denominator means per phase

Not every phase works on all `y` candidates — only files that turn out to
collide with another file ever reach verifying — so the indicator does not
force one shared denominator across every phase:

- **Scanning** and **hashing** report `x of y` against the full candidate
  total `y`, as above.
- **Verifying** works on a smaller set (only the files involved in a
  collision), so it reports its own subset progress while keeping the run's
  candidate total visible in the same line, for example
  `verifying: 1 of 2 (50%) — 302 candidates`. If nothing collided, verifying
  has zero work to do and is skipped entirely — you will not see a degenerate
  `verifying: 0 of 0 (100%)`.
- Every run **ends on a completion line rendered against the full candidate
  total**, for example `done: 302 of 302 (100%)`. On a TTY this is the state
  left on screen once the run finishes, so the overall size of the job you
  just scanned stays visible at the end — even though the verifying phase
  along the way was reporting progress against a smaller number.

### On a terminal vs. redirected or piped

- When standard error is an interactive terminal, progress renders as a
  **single line, redrawn in place** as counts advance. Repaints are
  throttled so a fast run does not flood the terminal.
- When standard error is **not** a terminal — redirected to a file, piped, or
  captured by CI/log tooling — the in-place redraw is disabled automatically.
  Instead `imgdedup` prints occasional **plain, newline-terminated lines** at
  coarse milestones (phase changes and roughly every 10% of progress), with no
  carriage returns or ANSI control codes, so a captured log stays readable and
  bounded rather than growing one line per file.

### `--no-progress`

Pass `--no-progress` to disable the indicator entirely: nothing
progress-related is written to standard error, whether it is a terminal or
not. Standard output is unaffected either way.

```sh
imgdedup --no-progress ./photos
```

### Progress is output-only

Progress never changes what is scanned, grouped, or removed, and it is
excluded from the [determinism](#determinism) guarantee above: that guarantee
covers standard-output content only (the report or `--json` document). The
stderr progress stream — its exact timing and milestone lines — can
legitimately vary between otherwise-identical runs without affecting the
result.

## Saving results to a file

There is **no dedicated output flag** — to keep a result for later, use your
shell's **output redirection**. This works for both output formats:

```sh
imgdedup ./photos > report.txt          # save the human-readable report
imgdedup --json ./photos > result.json  # save the machine-readable JSON
```

Redirection is reliable here because `imgdedup` keeps a strict separation between
its two output streams:

- **Standard output** carries **only** the result — the human report, or exactly
  one JSON document with `--json`. Nothing else is ever written there.
- **Standard error** carries diagnostics — run-level `root_errors`, and the
  live progress indicator while a scan runs (see
  [Progress while it runs](#progress-while-it-runs) above). Under the default
  report the per-file `Warnings:` section prints on stdout as part of the report;
  under `--json` those per-file problems move into the JSON `warnings` array. To
  redirect only the result and let diagnostics stay on the terminal, capture stdout
  as shown above.

Because stdout carries nothing but the result, the redirected file stays clean: a
`> result.json` capture is a valid JSON document you can pipe straight into a
parser, and a `> report.txt` capture is just the report. To capture the stderr
diagnostics too, redirect that stream as well — for example
`imgdedup ./photos > report.txt 2> log.txt`, or `2>&1` to merge them.

> A dedicated `--output <file>` flag was considered and deliberately **not** added:
> shell redirection already covers saving results with no extra surface. See the
> [PRD-0001 Amendment 2026-07-19](../prd/PRD-0001-exact-image-deduplication.md#amendment-2026-07-19--progress-indication--stored-results).

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
