# Output and exit codes

[← Guide index](README.md)

This page covers how `imgdedup` reports results, which stream each kind of
output goes to, how to keep a durable record of a run with `--output`, and how
the tool signals outcomes through its exit code.

## Human-readable report (default)

`imgdedup` prints a human-readable report to standard output. It has up to five
parts, in this order:

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
5. On a run that acted (`--apply` or `--apply --delete`), a **performed-actions**
   section: how many files were trashed or deleted, and — when at least one file
   was skipped — a `Skipped (left in place):` list of each skipped file and why.
   This section never appears on a dry run (there is nothing performed to
   report), but it always appears on an acted run, even when zero files were
   removed, so "did the apply run?" never has to be inferred from silence. See
   [Removing duplicates safely](removing-duplicates-safely.md#what-a-skip-looks-like)
   for what each skip reason means.

```text
Warnings:
  SKIP  /photos/locked.jpg: could not hash file: permission denied

Group 1/1 (2 files, 1024 bytes reclaimable):
  KEEP  /photos/a.jpg
  REMOVE /photos/b.jpg

Summary: 1 duplicate group, 2 files, 1024 bytes reclaimable.
```

The same run with `--apply` adds the performed-actions section:

```text
Group 1/1 (2 files, 1024 bytes reclaimable):
  KEEP  /photos/a.jpg
  REMOVE /photos/b.jpg

Summary: 1 duplicate group, 2 files, 1024 bytes reclaimable.

Trashed 1 file:
  /photos/b.jpg
```

And if a file could not be removed:

```text
Deleted 1 file:
  /photos/b.png
Skipped (left in place):
  /photos/c.png: content changed since the scan
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
  "actions": {
    "removed": [],
    "skipped": []
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
| `mode` | string | What the run actually did: `"dry-run"`, `"trash"`, or `"delete"` (see [`mode` and `actions`](#mode-and-actions-what-actually-happened)). |
| `groups` | array of Group | The duplicate groups. Each has a `keeper` and may list `removable` and/or `protected` files. A group whose only extra members are hard-linked anchors has an empty `removable` (nothing to reclaim) but still appears here, mirroring the human report's `Group i/N`. |
| `already_deduplicated` | array of HardLinkSet | Identity sets where *every* copy is a hard link to the same inode, so no duplicate group is formed and nothing can be reclaimed. Excluded from `totals` counts. |
| `totals` | Totals object | Run-wide counts. |
| `actions` | Actions object | What the run actually removed and skipped (see [`mode` and `actions`](#mode-and-actions-what-actually-happened)). Always present; both arrays are empty on a dry run. |
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

**Actions object** (the top-level `actions` field):

| Field | Type | Meaning |
|---|---|---|
| `removed` | array of string | Paths actually trashed (`mode: "trash"`) or permanently deleted (`mode: "delete"`). Empty on a dry run. |
| `skipped` | array of Skip | Candidates that were **not** removed, and why. Empty on a dry run. |

**Skip object** (entries of `actions.skipped`):

| Field | Type | Meaning |
|---|---|---|
| `path` | string | The candidate that was left in place. |
| `reason` | string | Why it was skipped — the same wording the human report uses (see [Removing duplicates safely](removing-duplicates-safely.md#what-a-skip-looks-like) for the full list of reasons). |

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

### `mode` and `actions`: what actually happened

`mode` reports what the run *did* to the files, not what flags were passed — a
run that refused to act (for example `--delete` without `--apply`, which exits
before anything runs) is never reported as having acted:

| `mode` | When | What `removable` means |
|---|---|---|
| `"dry-run"` | No action flag. | The files that *would* be removed. Nothing on disk changed. |
| `"trash"` | `--apply`. | The files that *were* moved to the OS trash — see `actions.removed` for exactly which ones. |
| `"delete"` | `--apply --delete`. | The files that *were* permanently deleted — see `actions.removed`. |

The `groups[].removable` lists are unchanged by mode — they always reflect the
scan's grouping decision. What actually happened to those files is the separate
top-level `actions` object:

```json
"actions": {
  "removed": ["/photos/b.png"],
  "skipped": [
    { "path": "/photos/c.png", "reason": "content changed since the scan" }
  ]
}
```

`actions` is always present, with both arrays empty on a dry run — the document
shape does not change with `mode`, only its contents. `actions.removed` is the
authoritative list of paths that were actually trashed or deleted; a group's
`removable` array may list files that ended up in `actions.skipped` instead, if
[re-validation](removing-duplicates-safely.md#re-validated-at-apply-time) at
apply time found a reason not to remove them. See
[Removing duplicates safely](removing-duplicates-safely.md) for the full safety
model and what each skip reason means.

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
written there, whether or not it is a terminal. Diagnostics appear as follows:

- **Per-file `warnings`** (files skipped during the scan) appear **only** inside
  the JSON document, in the `warnings` array. They are not echoed to stderr.
- **Run-level `root_errors`** appear in the JSON `root_errors` array **and** are
  additionally written to standard error, so an unreadable input path is visible
  even when you pipe stdout straight into a JSON parser. A non-empty `root_errors`
  also drives the exit code to `1`.
- **Apply-time skips** (`--apply`, with or without `--delete`) are written to
  standard error **as they happen**, as `warning: <path>: <reason>` lines, in
  addition to appearing in `actions.skipped` at the end. In the default report
  mode the same skips likewise appear twice — once live on stderr, once in the
  report's `Skipped (left in place):` list.
- **Apply-time removals** — the `trashed:` / `deleted:` lines you see when you
  watch an `--apply` run on a terminal — are routine status, not diagnostics, so
  under `--json` they are written to **neither** stream. Nothing is lost:
  `actions.removed` in the final document is the authoritative list once a run
  completes, and for a run that might not get that far, an
  [`--output`](#--output-file-a-durable-record-of-the-run) record captures each
  removal the moment it happens.

So a script can rely on stdout being pure JSON, while a human watching the
terminal still sees, on stderr, the scan roots that could not be read and the
removals that did not happen.

## Where output goes: stdout and stderr

`imgdedup` writes to two streams. What each one carries depends on the output
mode and on whether standard output is an interactive terminal or something is
capturing it (a redirect to a file, or a pipe):

| Situation | Standard output carries | Standard error carries |
|---|---|---|
| Default report mode, stdout is an **interactive terminal** | The run as you watch it — the live progress line with its elapsed time, then, on an `--apply` run, a `trashed:` / `deleted:` line for each file as it is removed — followed by the report | **Failures only** — nothing at all on a run where nothing went wrong |
| Default report mode, stdout **redirected or piped** | **Exactly the report**, nothing else | The progress indicator, plus failures |
| **`--json`**, terminal or not | **Exactly one JSON document**, nothing else | The progress indicator, plus failures |

Two rules follow from that table:

- **Whenever something other than you is reading standard output — a redirect,
  a pipe, or `--json` — stdout is exactly the result and nothing else.** That
  is what makes `imgdedup ./photos > report.txt` and
  `imgdedup --json ./photos | jq …` safe. The interactive terminal in the
  default mode is the only case where stdout carries more than the report, and
  in that case you are the one reading it.
- **Standard error is for failures.** Concretely, apart from progress (when it
  falls back there) and `--timings`, standard error carries:
  - usage errors, such as an unknown flag, `--delete` without `--apply`, or an
    `--output` file that cannot be created or that the tool refuses to replace
    (exit code `2`);
  - a scan root that could not be read (`error: <path>: <reason>`), and the
    `error: not applying: …` refusal when `--apply` is combined with one;
  - apply-time skips (`warning: <path>: <reason>`), one per file you approved
    for removal that was left in place;
  - a failure to keep writing an [`--output`](#--output-file-a-durable-record-of-the-run)
    record (`error: could not write to <path>: <reason>`).

  Files skipped *during the scan* (unreadable, or not really an image) are not
  on that list: they belong to the result, so they appear in the report's
  `Warnings:` section or the JSON `warnings` array, never on stderr.

A note on apply-time skips: every skip means a removal you approved did not
happen, so they are all kept together on stderr, even though some of them are
expected rather than a problem — a file that is already gone (which is what
makes re-running `--apply` a no-op), a symlink (which is never removed), or a
file that has become a hard link to the copy being kept. See
[What a skip looks like](removing-duplicates-safely.md#what-a-skip-looks-like)
for each reason and whether it needs your attention.

Some consequences worth knowing:

- **`--no-progress`** removes the progress indicator from whichever stream would
  have carried it; everything else in the table stays as it is.
- **[`--timings`](#--timings-where-the-time-went)** writes its stage lines to
  standard error in every mode — an engineering diagnostic, and a deliberate
  exception to "stderr is for failures" — and turns the progress indicator off.
- **The `trashed:` / `deleted:` lines are written only to an interactive
  terminal.** With stdout redirected or piped, or under `--json`, they are not
  moved to stderr — they are not failures — they are simply not written. The
  result's performed-actions section (or `actions.removed` under `--json`) lists
  every removal once the run completes, and an
  [`--output`](#--output-file-a-durable-record-of-the-run) record captures each
  one as it happens, for a run that may not complete.
- **When progress falls back to standard error, it follows standard error's own
  terminal-ness.** `imgdedup ./photos > report.txt` typed at a terminal still
  shows a live progress line, redrawn in place, because stderr is still your
  terminal; only when stderr is captured as well (`2> progress.log`, or CI)
  does it switch to plain log lines. Redirecting stdout never takes away an
  indicator you can still see, and the in-place redraw codes never end up in a
  file.

## Progress while it runs

While a scan runs, `imgdedup` shows a live progress indicator, enabled by
default, so you can see it is working, how far along it is, and how long it has
been going. It goes to the stream the table in
[Where output goes](#where-output-goes-stdout-and-stderr) assigns it: standard
output when you are watching a default-mode run on a terminal, standard error
whenever stdout is redirected, piped, or carrying a `--json` document. So
redirecting stdout (see [Saving results to a file](#saving-results-to-a-file)
below) always captures a clean report or JSON document, with nothing
progress-related mixed in.

The indicator shows:

- The current **phase**: `scanning` (enumerating and magic-byte-confirming
  candidate files), `hashing` (content-hash pre-filtering), or `verifying`
  (byte-for-byte confirmation against each group's keeper). These are
  user-facing labels for where the run is, not a contract on the internal
  pipeline.
- Once the candidate count is known, **`x of y` files done and a
  percentage**, where `y` is the number of candidate files confirmed as
  images, fixed once scanning finishes. Before that the candidate total is
  unknown, so scanning shows its own counting states instead (see below).
- The **elapsed time** since the run started, at the end of every line, in
  every phase — for example `hashing: 1200 of 28450 (4%) (12s elapsed)` — and,
  on the completion line, the run's **total** time:
  `done: 302 of 302 (100%) (141s total)`. There is deliberately **no** estimate
  of the time remaining: the phases cost very different amounts per file
  (hashing usually dominates), so an estimate would be confidently wrong in
  exactly the long runs where you would want one.

### Scanning happens in two steps

Scanning does two things in sequence, and the indicator shows both:

1. **Walking the tree** to find files worth examining. There is no total to
   count against yet, so on a terminal it shows a live count of what it has
   found so far, alongside the elapsed time every line carries, for example
   `scanning... 30000 files found (14s elapsed)`. The clock earns its keep
   most here, on a tree where almost nothing matches (a source checkout, a
   backup volume of documents): the count alone can sit still for a long
   stretch even though the walk is actively working, and the ticking clock is
   evidence the tool has not hung when the count itself gives none.
2. **Examining each file it found**, reading a few header bytes from each to
   confirm the content really is an image. The total is known exactly here —
   it is what step 1 just finished counting — so the indicator switches to
   `scanning: 12345 of 30000 (41%) (52s elapsed)`. Every file counts, whether it turns out
   to be an image or not, so the number keeps moving even on a folder full of
   misnamed files.

The count you watched grow in step 1 becomes the number counted against in
step 2. Step 2 is usually the longer of the two.

Note that `30000` there is how many files were **examined**, not how many
turned out to be images. The candidate total `y` used by the later phases only
counts the confirmed images, so it can be smaller — a folder of 30 000 files
where none is really an image shows `scanning: 30000 of 30000 (100%)` and then
`done: 0 of 0 (100%)` (each followed by its elapsed time). If the walk found
nothing at all, step 2 is skipped
rather than shown as a pointless `0 of 0`.

### What the denominator means per phase

Not every phase works on all `y` candidates — only files that turn out to
collide with another file ever reach verifying — so the indicator does not
force one shared denominator across every phase:

- **Scanning** runs before the candidate total is known: it counts up while
  walking, then counts against the number of files it found while examining
  them (above). **Hashing** is the first phase measured against the candidate
  total and reports `x of y` against the full `y`, as above.
- **Verifying** works on a smaller set (only the files involved in a
  collision), so it reports its own subset progress while keeping the run's
  candidate total visible in the same line, for example
  `verifying: 1 of 2 (50%) (302 candidates)`. If nothing collided, verifying
  has zero work to do and is skipped entirely — you will not see a degenerate
  `verifying: 0 of 0 (100%)`.
- Every run **ends on a completion line rendered against the full candidate
  total**, carrying the run's total elapsed time, for example
  `done: 302 of 302 (100%) (141s total)`. On a terminal this is the state left
  on screen once the run finishes, so the overall size of the job you just
  scanned — and how long it took — stays visible at the end, even though the
  verifying phase along the way was reporting progress against a smaller
  number.

### On a terminal vs. in a log

The indicator adapts to **whichever stream is carrying it** (see
[Where output goes](#where-output-goes-stdout-and-stderr)):

- When that stream is an interactive terminal, progress renders as a
  **single line, redrawn in place** as counts advance. Repaints are
  throttled so a fast run does not flood the terminal.
- When it is **not** a terminal — progress has fallen back to standard error
  and standard error is itself redirected to a file, piped, or captured by
  CI/log tooling — the in-place redraw is disabled automatically. Instead
  `imgdedup` prints occasional **plain, newline-terminated lines** at coarse
  milestones (phase changes and roughly every 10% of progress), with no
  carriage returns or ANSI control codes, so a captured log stays readable and
  bounded rather than growing one line per file. Each of these lines carries
  its elapsed time too: it is written at that moment, so the time on it is
  accurate. A complete log of a small run looks like this:

  ```text
  scanning...
  scanning: 0 of 6 (0%) (0s elapsed)
  scanning: 6 of 6 (100%) (0s elapsed)
  hashing: 0 of 6 (0%) (0s elapsed)
  verifying: 0 of 4 (0%) (6 candidates) (0s elapsed)
  done: 6 of 6 (100%) (0s total)
  ```

- The live *found-so-far* count from scanning's first step
  (`scanning... 30000 files found (14s elapsed)`) is a **terminal-only**
  display: a log gets the plain `scanning...` line above instead, since a
  single line that is never redrawn could only ever record a stale count and
  a stale clock. Scanning's second step does appear in the log, at the same
  coarse milestones as every other phase.

### `--no-progress`

Pass `--no-progress` to disable the indicator entirely: nothing
progress-related is written to either stream, terminal or not. The report and
the `--json` document are unaffected either way, and so is an
[`--output`](#--output-file-a-durable-record-of-the-run) record, which keeps
its milestones regardless.

```sh
imgdedup --no-progress ./photos
```

The diagnostic [`--timings`](#--timings-where-the-time-went) flag also turns
the indicator off, because the two would collide on screen — see below.

### Progress is output-only

Progress never changes what is scanned, grouped, or removed, and it is
excluded from the [determinism](#determinism) guarantee above: that guarantee
covers the **result** — the report or `--json` document, which is the whole of
standard output whenever stdout is redirected, piped, or carrying `--json`. The
progress lines — their exact timing and milestones, and whatever they put on an
interactive terminal ahead of the report — can legitimately vary between
otherwise-identical runs without affecting the result.

## `--timings`: where the time went

Pass `--timings` to see how long each part of a run took. As each stage of the
scan finishes, `imgdedup` prints one line describing it to **standard error** —
in every mode, a deliberate exception to the
[stream rules](#where-output-goes-stdout-and-stderr) above, because it is an
engineering diagnostic rather than normal output. Standard output is untouched:
the report — or the `--json` document — is exactly what it would have been
without the flag.

```sh
imgdedup --timings ./photos
```

A run over a 2,021-file photo library prints (abridged):

```text
2026-08-05T21:20:29.195834Z  INFO run{roots=1 apply=false json=false}:enumerate{roots=1 candidates=2021 warnings=0 root_errors=0}: close time.busy=11.3ms time.idle=917ns
2026-08-05T21:20:29.247251Z  INFO run{roots=1 apply=false json=false}:sniff{candidates=2021 confirmed=2021 rejected=0}: close time.busy=51.4ms time.idle=291ns
2026-08-05T21:20:29.247482Z  INFO run{roots=1 apply=false json=false}:dedup{files=2021}:group_by_size{files=2021 cohorts=421 size_singletons=1171}: close time.busy=207µs time.idle=251ns
2026-08-05T21:20:29.664206Z  INFO run{roots=1 apply=false json=false}:dedup{files=2021}:hash{files_hashed=850 bytes_hashed=2168910223 errors=0}: close time.busy=417ms time.idle=1.04µs
2026-08-05T21:20:29.664389Z  INFO run{roots=1 apply=false json=false}: close time.busy=480ms time.idle=1.96µs
```

### How to read it

- **One line per stage, printed when that stage ends.** The nesting —
  `run{...}:dedup{...}:hash{...}` — shows where a stage sits inside the
  pipeline. The outermost `run` line comes last and is the whole-run total
  (480 ms above, of which hashing was 417 ms).
- **`time.busy` is the number to read.** It is how long that stage took.
  `time.idle` is its bookkeeping counterpart and stays near zero here.
- **The `{...}` values are that stage's own counts** — files seen, files
  hashed, bytes hashed, candidates rejected — so a duration can be turned into
  a throughput (above: 2.17 GB hashed in 417 ms).
- **A stage with nothing to do still prints**, with its counts at zero, rather
  than being omitted — so "this stage cost nothing" stays visibly different
  from "this stage never ran". A few stages are genuinely conditional, though:
  the removal stage only appears under `--apply`, for instance.

The stages are `imgdedup`'s internal pipeline, and they are deliberately
finer-grained than the progress indicator's phases: the single `scanning`
phase you watch on screen is two separate stages here (walking the tree, then
confirming each found file's content really is an image), and the work after
verification — removing files under `--apply`, and rendering the report — has
no progress phase at all. Splitting exactly there is the point: it is what
lets a slow run be attributed to a specific stage.

### `--timings` replaces the progress indicator

Turning on timings turns the live progress indicator **off**, in every mode.
You do not need to pass `--no-progress` as well, and passing both is not an
error — `--timings` always wins.

The reason is that the two would collide on screen. The progress indicator
keeps a single line open and redraws it in place, so a timing line arriving
mid-redraw would land in the middle of it — whether progress is on standard
error alongside the timing lines, or on standard output, which normally shows
up on the same terminal. Rather than garble the display, or make you type two
flags to express one intention, `--timings` takes the screen for itself. Every
other line — the apply-time `trashed:` / `deleted:` and `warning:` lines, and
the `error:` lines — is unaffected: they are ordinary whole lines and
interleave with timing lines harmlessly.

### Combining with `--json`

`--timings --json` is the recommended shape for measuring a scripted run: the
JSON document goes to standard output, the timing lines to standard error, and
neither can contaminate the other.

```sh
imgdedup --timings --json ./photos > result.json 2> timings.log
```

### A diagnostic, not a benchmark

`--timings` answers "where did the time go on **this** run, on **this**
machine". It is meant for informal investigation — a scan that felt slow, a
library that behaves differently from another one — not for publishing
numbers. A single run's figures include whatever the operating system happened
to have in its file cache and whatever else the machine was doing, so compare
several runs before believing a difference. (`imgdedup`'s own repeatable
scale benchmark is a separate contributor-facing test, `make scale`, not this
flag.)

Consequently the stage names, their fields, and the line format are **not a
stable interface**: they exist to be read, not parsed, and they may change in
any release. Like progress, the timing stream is excluded from the
[determinism](#determinism) guarantee above — that guarantee covers the result
document only — and, also like progress, timings never change what is scanned,
grouped, or removed. The design and its rationale are recorded in
[ADR-0017](../adr/0017-performance-instrumentation-tracing-spans.md).

## Saving results to a file

There are two ways to keep what a run produced, and they answer different
needs:

- **Shell redirection** saves the finished result — the report, or the `--json`
  document — exactly as the run wrote it. Nothing reaches the file until the
  run completes.
- **[`--output <file>`](#--output-file-a-durable-record-of-the-run)** writes a
  durable record *while the run works*, so a run that is interrupted or killed
  still leaves everything it had produced by that point, and a finished one
  ends with the same result.

They compose: `--output` adds a file and changes nothing about what standard
output receives, so a long run can do both at once.

### Shell redirection

This works for both output formats:

```sh
imgdedup ./photos > report.txt          # save the human-readable report
imgdedup --json ./photos > result.json  # save the machine-readable JSON
```

Redirection is reliable because **the moment standard output is redirected or
piped, it carries exactly the result and nothing else** — the human report, or
exactly one JSON document with `--json`. Progress moves to standard error in
exactly that case, and the apply-time `trashed:` / `deleted:` lines are not
written at all (see [Where output goes](#where-output-goes-stdout-and-stderr)),
so the captured file stays clean: a `> result.json` capture is a valid JSON
document you can pipe straight into a parser, and a `> report.txt` capture is
just the report.

Standard error then carries the progress indicator — still a live line, redrawn
in place, if your terminal is still attached to it — plus failures: scan roots
that could not be read, apply-time skips, and, with `--timings`, the per-stage
timing lines. Under the default report the per-file `Warnings:` section prints
on stdout as part of the report; under `--json` those per-file problems move
into the JSON `warnings` array. To capture standard error too, redirect it as
well — for example `imgdedup ./photos > report.txt 2> log.txt`, or `2>&1` to
merge the two.

What redirection cannot do is survive an interrupted run: the report (or JSON
document) is written once, at the very end, so a run that is killed, crashes,
or is stopped with `Ctrl-C` before it finishes leaves an empty file. For that,
use `--output`.

## `--output <file>`: a durable record of the run

Pass `--output <file>` to have `imgdedup` write a record of the run to `<file>`
**continuously, while the run is working** — not once at the end. The point is
durability: a run that is interrupted, killed, or simply too slow to wait for
still leaves behind everything it had produced up to that moment.

```sh
imgdedup --output run.txt ./photos
```

The flag is **opt-in** — no file is ever created without it — and **additive**:
it changes nothing about what the terminal shows or what standard output
receives. Progress still renders, the report is still printed, and standard
output is byte-for-byte the same with and without the flag, so you never have
to choose between watching a run and keeping it. The two compose with
redirection, too:

```sh
imgdedup --output run.txt ./photos > report.txt
```

Because each entry reaches the file the moment it is written, you can watch a
record grow from another terminal with `tail -f run.txt` (or `tail -F`, which
picks up the new file when you re-run with the same path and the old one
becomes `run.txt.bak`).

### What the record contains

In the order things actually happened:

1. A **header** — the tool version, the start time (UTC), and the invocation:
   the scan roots and the flags the run was started with, including the
   record's own path.
2. **Progress milestones**, each stamped with the time since the run started,
   at the same coarse granularity as the plain-log progress fallback (phase
   changes and roughly every 10%) — never one line per file. They are written
   even with `--no-progress` or `--timings`.
3. Each **duplicate group** as soon as its membership is final: the copy kept,
   the copies that would be removed, and any hard-linked copies.
4. Every **warning and error**, at the moment it occurs: files skipped during
   the scan, scan roots that could not be read, and a refusal to apply.
5. On an `--apply` run, each **removal and skip**, as it happens.
6. **Only if the run finishes**, the **complete result** — exactly the report
   (or JSON document) the same run wrote to standard output.

A quick run over a small `photos` folder holding two pairs of duplicates
(`imgdedup --output run.txt photos`) leaves this file:

```text
# imgdedup 0.1.0 run record
# started:    2026-11-13T17:18:35Z
# invocation: roots=[photos] keep=shortest apply=false delete=false hidden=false
#             recurse=true follow-symlinks=false ext=<default> exit-code=false
#             json=false no-progress=false timings=false
#             overwrite-output=false output=run.txt
[     0.0s] scanning...
[     0.0s] scanning: 0 of 6 (0%)
[     0.0s] scanning: 6 of 6 (100%)
[     0.0s] hashing: 0 of 6 (0%)
[     0.0s] verifying: 0 of 4 (0%) (6 candidates)
[     0.0s] group (2668960 bytes reclaimable):
              KEEP    photos/a/img1.JPG
              REMOVE  photos/b/copy1.jpg
[     0.0s] group (2013563 bytes reclaimable):
              KEEP    photos/a/img2.JPG
              REMOVE  photos/b/img2.JPG
[     0.0s] done: 6 of 6 (100%)

===== RESULT — run completed 2026-11-13T17:18:35Z after 0.0s =====

Group 1/2 (2 files, 2668960 bytes reclaimable):
  KEEP  photos/a/img1.JPG
  REMOVE photos/b/copy1.jpg

Group 2/2 (2 files, 2013563 bytes reclaimable):
  KEEP  photos/a/img2.JPG
  REMOVE photos/b/img2.JPG

Summary: 2 duplicate groups, 4 files, 4682523 bytes reclaimable.
```

Reading it:

- Header lines start with `#`. Every later entry starts at the left margin with
  `[` and the time since the run started, in seconds (`[   131.4s]` on a long
  run); a group's member lines are indented beneath it. Group rows use the
  report's own `KEEP` / `REMOVE` / `PROTECT` / `hard-linked:` words, so there is
  only one vocabulary to learn.
- Other entries you may see: `warning: <stage>: <path>: <reason>` (a file
  skipped during the scan; the stage is `enumerate`, `sniff` or `engine`, as in
  the JSON `warnings` array),
  `error: <path>: <reason>` (a scan root that could not be read),
  `already deduplicated (hard-linked): <path>`, and on an `--apply` run
  `trashed: <path>` / `deleted: <path>` for each removal and
  `warning: <path>: <reason>` for each skip.
- Everything above the `===== RESULT` banner is a transcript of the run **in
  progress**: its order follows the order work actually finished, which is not
  reproducible from run to run, and it carries no totals. Everything below the
  banner is the authoritative, [deterministic](#determinism) result — a
  byte-for-byte copy of the report standard output received.

### Finished or interrupted?

**The `===== RESULT` banner is the tell.** A record that has it is a finished
run; a record without it is a run that did not finish — it was killed or
crashed, or it stopped itself, as when `--apply` refuses to act because a scan
root could not be read (the record then ends with that `error:` line). Under
`--json` the same signal is the final line: a finished record ends with a line
whose `type` is `result`; an unfinished one does not.

One exception: if `imgdedup` reported that it **could not write to** the record
(see [Practical notes](#practical-notes)), do not rely on the banner. If that
failure struck while the result itself was being written, the default-format
record can end with the banner followed by a report that is cut short — so the
banner alone does not prove the record is complete. The exit status `1` and the
message on standard error are the signal then. The `--json` form does not have
this ambiguity: a cut-short `result` line is not valid JSON, and a finished
one ends with a newline.

### When groups appear

A group is written as soon as its **membership** is final — the copy kept, the
copies to remove, the hard-linked copies — and *final* is a real guarantee:
nothing later in the run can add a file to a group, remove one from it, or
split it.

But that moment is not early in the run. `imgdedup` hashes every candidate
before it byte-verifies any of them, and hashing is usually the long part of a
scan, so **groups appear during the verifying phase — the last stretch of the
run — not steadily throughout it.** A run killed while it is still hashing
leaves a record with a header, milestones and warnings, and **no groups**.

What is *not* final when a group is written is its place in the final ordering
and every run-wide total, so group entries carry **no `Group i/N` number** and
no running totals; those appear only in the result.

### NDJSON under `--json`

With `--json`, the record is **NDJSON** — one JSON object per line — instead of
the human-readable form:

```sh
imgdedup --json --output run.ndjson photos > result.json
```

NDJSON is used because it stays valid at every line boundary: if the process
dies mid-write, every *complete* line still parses on its own, so even a partial
record is machine-readable. The same run as above produces:

```text
{"type":"header","elapsed_ms":0,"record_version":1,"tool":"imgdedup","tool_version":"0.1.0","started_at":"2026-11-13T17:18:35Z","invocation":{"roots":["photos"],"keep":"shortest","apply":false,"delete":false,"json":true,"no_recurse":false,"hidden":false,"follow_symlinks":false,"ext":null,"exit_code":false,"no_progress":false,"timings":false,"overwrite_output":false,"output":"run.ndjson"}}
{"type":"milestone","elapsed_ms":0,"phase":"scanning","current":0,"total":null,"percent":null,"run_total":0}
{"type":"milestone","elapsed_ms":0,"phase":"scanning","current":0,"total":6,"percent":0,"run_total":0}
{"type":"milestone","elapsed_ms":1,"phase":"scanning","current":6,"total":6,"percent":100,"run_total":0}
{"type":"milestone","elapsed_ms":1,"phase":"hashing","current":0,"total":6,"percent":0,"run_total":6}
{"type":"milestone","elapsed_ms":4,"phase":"verifying","current":0,"total":4,"percent":0,"run_total":6}
{"type":"group","elapsed_ms":5,"keeper":{"path":"photos/a/img2.JPG","size":2013563,"mtime":1794590314},"removable":[{"path":"photos/b/img2.JPG","size":2013563,"mtime":1794590314}],"protected":[],"hard_link_sets":[],"reclaimable_bytes":2013563}
{"type":"group","elapsed_ms":6,"keeper":{"path":"photos/a/img1.JPG","size":2668960,"mtime":1794590314},"removable":[{"path":"photos/b/copy1.jpg","size":2668960,"mtime":1794590314}],"protected":[],"hard_link_sets":[],"reclaimable_bytes":2668960}
{"type":"milestone","elapsed_ms":6,"phase":"done","current":6,"total":6,"percent":100,"run_total":6}
{"type":"result","elapsed_ms":6,"record_version":1,"result":{"schema_version":1,"mode":"dry-run","groups":[{"keeper":{"path":"photos/a/img1.JPG","size":2668960,"mtime":1794590314},"removable":[{"path":"photos/b/copy1.jpg","size":2668960,"mtime":1794590314}],"protected":[],"hard_link_sets":[],"reclaimable_bytes":2668960},{"keeper":{"path":"photos/a/img2.JPG","size":2013563,"mtime":1794590314},"removable":[{"path":"photos/b/img2.JPG","size":2013563,"mtime":1794590314}],"protected":[],"hard_link_sets":[],"reclaimable_bytes":2013563}],"already_deduplicated":[],"totals":{"duplicate_groups":2,"removable_files":2,"reclaimable_bytes":4682523,"already_deduplicated_sets":0},"actions":{"removed":[],"skipped":[]},"warnings":[],"root_errors":[]}}
```

Note the two `group` lines arrive in the opposite order from the `result`'s
`groups` array: the transcript follows the order verification happened to
finish, and only the result is in canonical order.

Every line starts with `type` and `elapsed_ms` (milliseconds since the run
started). The event types:

| `type` | When | Fields besides `type` and `elapsed_ms` |
|---|---|---|
| `header` | Always the first line. | `record_version`, `tool`, `tool_version`, `started_at` (UTC, RFC 3339), `invocation` (an object: `roots` and every flag) |
| `milestone` | At each phase change and roughly every 10% of progress. | `phase` (`scanning`, `hashing`, `verifying`, `done`), `current`, `total` and `percent` (both `null` while the walk's total is still unknown), `run_total` (the candidate total; `0` until it is known) |
| `group` | When a group's membership is final. | Exactly a [Group object](#field-reference-schema-version-1) from the `--json` document: `keeper`, `removable`, `protected`, `hard_link_sets`, `reclaimable_bytes` |
| `already_deduplicated` | When a hard-link-only set is found. | A HardLinkSet object: `representative`, `siblings` |
| `warning` | When a file is skipped during the scan. | A Warning object: `stage`, `path`, `reason` |
| `error` | When a run-level error occurs. | `scope`: `"root"` for a scan root that could not be read (with `path` and `reason`), or `"run"` for a failure not tied to one path, such as `--apply` refusing to act (with `reason` only) |
| `action` | On an `--apply` run, as each file is removed or skipped. | `action` (`removed` or `skipped`), `mode` (`trash` or `delete`), `path`, and `reason` (skips only) |
| `result` | The last line — **only if the run finished**. | `record_version`, and `result`: the complete `--json` document |

To pull the result out of a finished record:

```sh
jq -c 'select(.type=="result").result' run.ndjson
```

That is the same document, with the same `schema_version: 1` schema, that
standard output received — serialized compactly on one line rather than
pretty-printed, so it is the same *document*, not the same bytes. (In the
human-readable record, by contrast, the result is a byte-for-byte copy.)

A record from a run that was killed mid-write may end in a partial line, which
plain `jq` reports as a parse error once it reaches it. To read whatever *is*
complete — every group found so far, say — have `jq` skip lines that do not
parse:

```sh
jq -cR 'fromjson? | select(.type=="group")' run.ndjson
```

**The record's event vocabulary is not a stable interface yet.** Its version is
`record_version` (`1` today) — separate from the result's `schema_version` — and
it appears on both the first (`header`) and last (`result`) line. New event
types and new fields may be added **without** changing `record_version`, so a
consumer must **ignore `type` values and fields it does not recognize**;
`record_version` changes only for a change that would break a consumer that
does so. The nested `result` document is the exception: it is the existing
`schema_version: 1` document with its existing
[stability contract](#stability-contract), unchanged.

### What survives, and what does not

Each entry is handed to the operating system the moment it is written, with
nothing held back in a buffer waiting for a tidy shutdown. So:

- The record **survives the process dying** — `Ctrl-C`, `kill`, `kill -9`, a
  panic, or any crash. Every entry the tool finished writing is in the file; at
  most the single entry that was being written at that instant is cut short.
  Under `--json` that is a partial final line, which a line-by-line reader
  discards; every complete line still parses.
- The record **does not survive a host crash or power loss.** Written entries
  sit in the operating system's cache until it writes them out to disk;
  `imgdedup` does not force each one to stable storage. On a network mount, the
  record inherits that mount's behavior.

And remember [when groups appear](#when-groups-appear): a run killed while it is
still hashing has not found any groups yet, so its record has none.

### If the file already exists

`imgdedup` never destroys a file you did not ask it to. `--output` replaces
only its **own** earlier records, and keeps the one it replaces:

| What is at the `--output` path | What happens |
|---|---|
| Nothing | The record is created. |
| An empty file | The record is written there. |
| A record from an earlier `imgdedup` run | It is renamed to `<file>.bak` (for example `run.txt.bak`), and the new record is written. No flag needed. |
| Any other file | The run is **refused** before anything is scanned (exit `2`), and the file is left exactly as it was. |

So re-running the same command just works, and the previous run's record is
still there next to the new one:

```sh
imgdedup --output run.txt ./photos   # creates run.txt
imgdedup --output run.txt ./photos   # run.txt is the new record; the old one is run.txt.bak
```

A mistyped path that lands on a photo, or on any file that is not an
`imgdedup` record, stops the run instead:

```text
$ imgdedup --output photos/a/ixus50-1_385.JPG photos
error: output file photos/a/ixus50-1_385.JPG already exists and is not an imgdedup run record; choose another path, or pass --overwrite-output to replace it (it will be kept as photos/a/ixus50-1_385.JPG.bak)
```

If you really do want to replace that file, pass **`--overwrite-output`**. The
file is still not lost: it becomes `<file>.bak`, just like an earlier record.
`--overwrite-output` only works together with `--output`; on its own it is a
usage error.

Only **one** backup is kept. The next run replaces `<file>.bak` with the record
it just moved aside, so the run before last is gone. That only happens when
`<file>.bak` is empty or is itself an `imgdedup` record: if something else
already sits at that name, the run is refused — even with
`--overwrite-output`, which gives permission to replace the file you named,
not one you never mentioned:

```text
error: cannot back up run.txt: run.txt.bak already exists and is not an imgdedup run record; move run.txt.bak away or choose another output path
```

Move that file out of the way, or choose another `--output` path. Making a
backup prints nothing; the `.bak` file is the only sign of it. If `--output`
names a symbolic link, the link stays in place and the file it points to
receives the new record; `<file>.bak` is a plain copy of what was there.

The record is never appended to an existing file: an old run's result
followed by a new, partial transcript would read as a finished run.

### Practical notes

- **A path that cannot be written fails immediately.** If the file cannot be
  created — the path is a directory, its parent folder does not exist, or you
  cannot write there — `imgdedup` prints
  `error: cannot open output file <path>: <reason>` and exits `2` **before scanning
  anything**, with no report, rather than failing hours in.
- **A write failure partway through does not stop the run.** If the file
  becomes unwritable while the run is working — a full disk, a dropped network
  mount — the scan, and any `--apply` already under way, carries on and
  finishes exactly as it would have without `--output`: the same groups, the
  same removals, the same report. The failure is reported **once** on standard
  error (`error: could not write to <path>: <reason>`), not once per later
  write; the file is left as it is, without a complete result (see
  [Finished or interrupted?](#finished-or-interrupted)); and the run
  **exits `1`**,
  even though the deduplication itself succeeded — you asked for a durable
  record and did not get one, and the exit code is how a script finds out.
- **The record holds your files' full paths** — the scan roots, every file in a
  duplicate group or an already-deduplicated hard-link set, and every file
  warned about, removed, or skipped. It is
  created with your usual default file permissions, just as `>` would create
  it, so on a shared machine choose where you put it accordingly. Each run
  writes a fresh file, so permissions you set on an earlier record stay with
  its `.bak`, not with the new one.
- **Putting the record inside a folder you are scanning is safe.** The record is
  never an image, so it can never be grouped, trashed, or deleted. If its name
  has an image extension (or you widen the allow-list with `--ext`), it is
  examined like any other candidate and skipped with a "not recognized as a
  supported image format" warning — which then appears in its own transcript.

### `--json` without `--output` keeps nothing until the end

Worth knowing **before** you start a long unattended run: `imgdedup --json` on
its own produces output only if it reaches the end. Its entire output is one
JSON document written on completion, and nothing about the run is recorded
anywhere along the way — not even the removals of an `--apply` run, which are
not printed under `--json`. Stop it at 95% and you have nothing.

That is by design, not an oversight. Keeping standard output to exactly one
document is what makes `imgdedup --json ./photos | jq …` work at all, and
streaming it piece by piece would break every program that reads it — for
little gain, since a pipeline whose producer dies loses its output anyway.
`--output` is the answer instead, and a better one, because NDJSON is valid at
every line boundary:

```sh
imgdedup --json --output run.ndjson ./photos > result.json
```

For a long `--apply` run this is the shape to reach for: each removal lands in
the record the moment it happens, so even a run that never finishes leaves a
complete account of what it did.

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
| `1` | Run-level error — e.g. an input path could not be scanned, an [`--output`](#--output-file-a-durable-record-of-the-run) record could not be written while the run was in progress, or the report could not be written to standard output because whatever was reading it had gone away (in both of the last two cases the run itself still completed). Takes precedence over `3`. |
| `2` | Usage error — invalid arguments, an `--output` file that cannot be created, or an existing `--output` file the tool [will not replace](#if-the-file-already-exists) (all checked before anything is scanned). |
| `3` | Duplicates were found. Only emitted when `--exit-code` is passed. |

A note on pipes: when the program reading standard output stops before the
report has been written — `imgdedup ./photos | head -5` on a long report, for
example — `imgdedup` does not crash. It finishes the run (including any
`--output` record, which still gets its result), prints nothing more, and
exits `1`, because the report did not reach its reader. A report short enough
to be written in full before the reader stops counts as delivered, so the same
pipe can exit `0` on a small run. If a script runs with `set -o pipefail`,
expect that `1`. A diagnostic that cannot be written to standard error is
simply dropped: it never stops a run, interrupts an `--apply`, or changes the
exit code.

## Next

- Back to the [guide index](README.md).
