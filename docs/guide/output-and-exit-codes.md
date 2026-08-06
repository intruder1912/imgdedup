# Output and exit codes

[← Guide index](README.md)

This page covers how `imgdedup` reports results, which stream each kind of
output goes to, how to keep a durable record of a run, and how the tool signals
outcomes through its exit code.

> **Not in a release yet.** Three things described below — the `--output` flag,
> progress appearing on standard output on an interactive terminal, and elapsed
> time on every progress render — are accepted product behavior
> ([PRD-0001 Amendment 2026-08-06](../prd/PRD-0001-exact-image-deduplication.md#amendment-2026-08-06--durable-run-record-stream-model-and-report-paging-product-owner-ruling-task-0340--task-0341),
> [ADR-0018](../adr/0018-durable-run-record-and-output-stream-routing.md)) that
> is still being implemented. A released binary today has no `--output` flag and
> writes progress to standard error in every mode. This note goes away with the
> implementation.

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

- **Per-file `warnings`** (skipped files) appear **only** inside the JSON
  document, in the `warnings` array. They are not echoed to stderr.
- **Run-level `root_errors`** appear in the JSON `root_errors` array **and** are
  additionally written to standard error, so an unreadable input path is visible
  even when you pipe stdout straight into a JSON parser. A non-empty `root_errors`
  also drives the exit code to `1`.
- **Apply-stage skips** (`--apply`, with or without `--delete`) are written to
  standard error **as they happen**, in addition to appearing in the `actions`
  object at the end: a file you approved for removal that was *not* removed is a
  genuine problem, and standard error is where genuine problems go. In the
  default report mode the same skips therefore appear twice — once live on
  stderr, once in the report's `Skipped (left in place):` list.
- **Apply-stage removals** — the live `trashed:` / `deleted:` lines — are
  routine status rather than diagnostics, so under `--json` they are written to
  **neither** stream while the run works. Nothing is lost: `actions.removed` in
  the final document is the authoritative list once a run completes, and for a
  run that might not get there, an
  [`--output`](#--output-file-a-durable-record-of-the-run) record captures each
  removal the instant it happens.

So a script can rely on stdout being pure JSON, while a human watching the
terminal still sees anything that actually went wrong, on stderr.

## Where output goes: stdout and stderr

`imgdedup` writes to two streams. Which one carries what depends on the output
mode, and on whether standard output is a live terminal or something is
capturing it:

| Situation | Standard output carries | Standard error carries |
|---|---|---|
| Default report mode, stdout is an **interactive terminal** | The report, **plus** the run's live status: the progress indicator with its elapsed time, and the apply-time `trashed:` / `deleted:` lines | **Only genuine failures** — nothing at all on a clean run |
| Default report mode, stdout **redirected or piped** | **Exactly the report**, nothing else | The progress indicator, plus genuine failures |
| **`--json`**, terminal or not | **Exactly one JSON document**, nothing else | The progress indicator, plus genuine failures |

Two rules follow from that table, and they are the ones worth remembering:

- **Whenever something is reading your standard output — a redirect, a pipe, or
  `--json` — stdout is exactly the result document and nothing else.** That is
  the property scripts and redirects depend on, and it is unchanged. An
  interactive terminal in the default report mode is the only case where stdout
  carries anything more, and no script is reading in that case.
- **Standard error is for what actually went wrong**: run-level errors (an input
  path that could not be scanned), per-file failures, apply-time skips, and a
  failure to write an [`--output`](#--output-file-a-durable-record-of-the-run)
  record. Routine status — progress, phase changes, elapsed time, "removed X" —
  is not a failure and no longer appears there.

Two deliberate exceptions to "routine status follows the table":

- **[`--timings`](#--timings-where-the-time-went)** always writes to standard
  error, in every mode. It is an engineering diagnostic rather than normal
  output, so it is an explicit carve-out from the rule above.
- **Apply-time `trashed:` / `deleted:` lines are simply not written** when
  standard output is not an interactive terminal (redirected, piped, or
  `--json`). They are not moved to stderr, because they are status rather than
  failure — and nothing is lost, because
  [`actions.removed`](#mode-and-actions-what-actually-happened) in the result
  document is authoritative once a run completes, and an `--output` record
  captures each removal as it happens for a run that may not complete.

When progress falls back to standard error, it is rendered according to
**stderr's own** terminal-ness: `imgdedup ./photos > report.txt` typed at a
terminal still shows a live, redrawn progress line, because stderr is still
attached to that terminal. Redirecting stdout never downgrades an indicator you
can still see.

## Progress while it runs

While a scan runs, `imgdedup` shows a live progress indicator, enabled by
default, so you can see it is working and how far along it is. It goes to
whichever stream the table in
[Where output goes](#where-output-goes-stdout-and-stderr) assigns it: standard
output when you are watching a default-mode run on a terminal, standard error
whenever stdout is redirected, piped, or carrying a `--json` document — so a
captured stdout (see [Saving results to a file](#saving-results-to-a-file)
below) always holds a clean report or JSON document, with nothing
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
- The **elapsed time** since the run started, on every render, in every phase
  and state — the same `(14s elapsed)` suffix scanning has always shown, now
  everywhere — and, on the completion line, the run's total elapsed time.
  There is deliberately **no** estimate of the time remaining: this pipeline's
  phases cost very different amounts per file (hashing dominates a real run), so
  an extrapolated ETA would be confidently wrong in exactly the long runs you
  would want one for.

### Scanning happens in two steps

Scanning does two things in sequence, and the indicator shows both:

1. **Walking the tree** to find files worth examining. There is no total to
   count against yet, so on a terminal it shows a live count of what it has
   found so far, alongside the elapsed time every render carries, for example
   `scanning... 30000 files found (14s elapsed)`. The clock earns its keep most
   here, on a tree where almost nothing matches (a source checkout, a backup
   volume of documents) and the count alone can sit still for a long stretch
   even though the walk is actively working; the ticking clock is evidence the
   tool has not hung when the count itself gives none.
2. **Examining each file it found**, reading a few header bytes from each to
   confirm the content really is an image. The total is known exactly here —
   it is what step 1 just finished counting — so the indicator switches to
   `scanning: 12345 of 30000 (41%)`. Every file counts, whether it turns out
   to be an image or not, so the number keeps moving even on a folder full of
   misnamed files.

The count you watched grow in step 1 becomes the number counted against in
step 2. Step 2 is usually the longer of the two.

Note that `30000` there is how many files were **examined**, not how many
turned out to be images. The candidate total `y` used by the later phases only
counts the confirmed images, so it can be smaller — a folder of 30 000 files
where none is really an image shows `scanning: 30000 of 30000 (100%)` and then
`done: 0 of 0 (100%)`. If the walk found nothing at all, step 2 is skipped
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
  total**, for example `done: 302 of 302 (100%)`, carrying the run's **total**
  elapsed time. On a TTY this is the state left on screen once the run
  finishes, so the overall size of the job you just scanned — and what it cost
  in wall-clock time — stays visible at the end, even though the verifying
  phase along the way was reporting progress against a smaller number.

### On a terminal vs. in a log

The indicator adapts to the terminal-ness of **whichever stream is carrying
it** — the two travel together, so in-place redraw codes can never end up in a
captured file:

- When that stream is an interactive terminal, progress renders as a
  **single line, redrawn in place** as counts advance. Repaints are
  throttled so a fast run does not flood the terminal.
- When it is **not** a terminal — the `imgdedup ./photos > report.txt 2> log.txt`
  or CI case, where progress has fallen back to a standard error that is itself
  captured — the in-place redraw is disabled automatically. Instead `imgdedup`
  prints occasional **plain, newline-terminated lines** at coarse milestones
  (phase changes and roughly every 10% of progress), with no carriage returns or
  ANSI control codes, so a captured log stays readable and bounded rather than
  growing one line per file. Each such milestone line carries its elapsed time:
  a written milestone is a timestamped fact, not a value waiting to be redrawn.
- The live *found-so-far* count from scanning's first step
  (`scanning... 30000 files found (14s elapsed)`) stays a **terminal-only**
  display, because a single non-redrawn line could only ever record a stale
  count. A log gets a plain `scanning...` line instead. Scanning's second step
  does appear in the log, at the same coarse milestones as every other phase.

### `--no-progress`

Pass `--no-progress` to disable the indicator entirely: nothing
progress-related is written at all — to standard error or standard output,
whichever it would have used, terminal or not. The report and the `--json`
document are unaffected either way, as is an
[`--output`](#--output-file-a-durable-record-of-the-run) record, which keeps its
milestones regardless.

```sh
imgdedup --no-progress ./photos
```

The diagnostic [`--timings`](#--timings-where-the-time-went) flag also turns
the indicator off, because the two would collide on screen — see below.

### Progress is output-only

Progress never changes what is scanned, grouped, or removed, and it is
excluded from the [determinism](#determinism) guarantee above: that guarantee
covers the **result document** — which is the whole of standard output whenever
stdout is redirected, piped, or carrying `--json`, plus the completion document
that ends an [`--output`](#--output-file-a-durable-record-of-the-run) record.
The progress stream — its exact timing and milestone lines, and whatever it
puts on an interactive terminal alongside the report — can legitimately vary
between otherwise-identical runs without affecting the result.

## `--timings`: where the time went

Pass `--timings` to see how long each part of a run took. As each stage of the
scan finishes, `imgdedup` prints one line describing it to **standard error** —
in every mode, an explicit carve-out from the
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

Turning on timings turns the live progress indicator **off**, in every mode. You
do not need to pass `--no-progress` as well, and passing both is not an error —
`--timings` always wins.

The reason is that the two would collide on screen. The progress indicator keeps
a single line open and redraws it in place, so a stage line arriving mid-redraw
lands in the middle of it — directly, whenever progress is on standard error
too (under `--json`, or with stdout redirected), and just as visibly when
progress is on standard output, since both streams normally end up on the same
terminal. Rather than garble the display, or make you type two flags to express
one intention, `--timings` takes the screen for itself. The apply-time and
run-level `error:` lines are unaffected: they are ordinary whole lines and
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
document, not the diagnostic streams — and, also like progress, timings never
change what is scanned, grouped, or removed. The design and its rationale are recorded in
[ADR-0017](../adr/0017-performance-instrumentation-tracing-spans.md).

## Saving results to a file

There are two ways to keep what a run produced, and they answer different
questions:

- **Shell redirection** saves the finished result — the report, or the `--json`
  document — exactly as the run wrote it. Nothing reaches the file until the run
  completes.
- **[`--output <file>`](#--output-file-a-durable-record-of-the-run)** writes a
  durable record *while the run works*, so a run that is interrupted, killed, or
  simply too slow to wait for still leaves everything it had produced by that
  point.

They compose: `--output` adds a file and changes nothing about what standard
output receives, so a long run can do both at once.

### Shell redirection

```sh
imgdedup ./photos > report.txt          # save the human-readable report
imgdedup --json ./photos > result.json  # save the machine-readable JSON
```

Redirection is reliable because **the moment standard output is redirected or
piped it carries exactly the result and nothing else** — the human report, or
exactly one JSON document with `--json`. That is precisely the case in which
progress and status lines go to standard error instead (see
[Where output goes](#where-output-goes-stdout-and-stderr)), so the captured file
stays clean: a `> result.json` capture is a valid JSON document you can pipe
straight into a parser, and a `> report.txt` capture is just the report.

Standard error then carries the progress indicator — still a live, redrawn line
if your terminal is still attached to it — plus genuine failures: run-level
`root_errors`, apply-time skips, and, with `--timings`, the per-stage timing
lines. Under the default report the per-file `Warnings:` section prints on
stdout as part of the report; under `--json` those per-file problems move into
the JSON `warnings` array. To capture the stderr diagnostics too, redirect that
stream as well — for example `imgdedup ./photos > report.txt 2> log.txt`, or
`2>&1` to merge them.

## `--output <file>`: a durable record of the run

Pass `--output <file>` to have `imgdedup` write a record of the run to `<file>`
**continuously, as the run progresses** — not once at the end. The point is
durability: a run that is interrupted, killed, or throttled by slow storage
still leaves behind everything it had produced up to that moment, which no
end-of-run write and no shell redirection can do.

```sh
imgdedup --output run.txt ./photos
```

The flag is **opt-in** — no file is ever created without it — and **additive**:
it changes nothing about what the terminal shows or what standard output
receives. Progress still renders, the report is still printed, and the stdout
bytes are identical with and without the flag. You never have to choose between
watching a run and keeping it.

### What the record contains

In the order things actually happened:

1. A **header** — the tool version, the invocation (roots and flags), and the
   start time.
2. **Phase and elapsed-time milestones**, at the same coarse granularity as the
   plain-log progress fallback — never one line per file.
3. Each **duplicate group** as soon as it is final, with its keeper, the files
   it would remove, and any hard-linked siblings.
4. Every **warning and error**, at the moment it occurs.
5. Apply-time **removals and skips**, as they happen.
6. On completion only, the **complete result document** — the same document the
   same run wrote to standard output.

Items 1–5 are a transcript of a run *in progress*: their order follows the order
work actually finished, which under parallel hashing is not reproducible from
run to run, and any running counts in them are provisional. Item 6 is the
authoritative, [deterministic](#determinism) result. Read the tail as fact and
the body as "what had happened by then".

**A record with no result document is the signal that the run did not finish.**
In the human-readable format the result is introduced by an unmistakable
`===== RESULT` banner; its absence is the whole tell.

### When groups appear

A group is written the instant it is known to be final, and *final* is a real
guarantee: nothing later in the run can add a file to a group, remove one from
it, or split it.

But the earliest such moment is not early in the run. `imgdedup` hashes every
candidate before it byte-verifies any of them, and hashing is usually the long
part of a scan — so **groups stream during the last stretch of a run, not
steadily throughout it**. A run killed while it is still hashing leaves a record
with a header, milestones and warnings, and no groups. That is the honest shape
of it.

Group entries in the record also carry **no `Group i/N` number** and no running
totals. What is final when a group is written is its *membership* — the keeper,
the removable files, the protected hard links. Its position in the final
canonical ordering, and every run-wide total, are still unknown at that point;
they appear in the completion document.

### NDJSON under `--json`

The record's format follows the output mode. Without `--json` it is the
human-readable rendering, reusing the report's `KEEP` / `REMOVE` / `PROTECT`
vocabulary so you are not reading two languages. With `--json` it is
**NDJSON** — one JSON object per line:

```sh
imgdedup --json --output run.ndjson ./photos > result.json
```

NDJSON is chosen precisely because it is valid at every line boundary: a process
killed mid-write leaves a file whose every *complete* line still parses, so a
partial record is still machine-readable. Every line carries a `type`
(`header`, `milestone`, `group`, `already_deduplicated`, `warning`, `error`,
`action`, `result`) and an `elapsed_ms`, and the final `result` line wraps the
complete `--json` document:

```sh
jq -c 'select(.type=="result").result' run.ndjson
```

That recovers the same document, with the same `schema_version: 1` schema and
the same guarantees, that standard output received — serialized compactly on one
line rather than pretty-printed, so it is the same *document*, not the same
bytes. (In the human-readable format the completion document is a byte-for-byte
copy of the report.)

**The record's own event vocabulary is not yet a stable interface.** It carries
its own `record_version` (`1` today), independent of the result document's
`schema_version`, on both the `header` and the `result` line — so a consumer
reading either end of the file learns it. New event types and new fields may be
added *without* a version bump, so a consumer must ignore `type` values and
fields it does not recognize; `record_version` moves only for a change that
breaks a consumer doing that. The nested result document is the exception: it is
the existing `schema_version: 1` contract with its existing
[stability guarantees](#stability-contract).

### What survives, and what does not

Records go straight to the operating system as they are produced — nothing is
held in a buffer waiting for a tidy shutdown. So:

- The record **survives the process dying**: `Ctrl-C`, a `kill`, a `SIGKILL`, a
  panic, a crash. Every record the tool finished writing is in the file; at most
  the single record that was mid-write is truncated — under `--json` that is a
  partial final line, which a line-wise NDJSON reader discards.
- The record **does not survive a host crash or a power cut.** Written records
  live in the operating system's cache until it flushes them; `imgdedup` does
  not force each event to stable storage, because on network storage that would
  mean a round-trip per event. On a network mount the record inherits that
  mount's semantics.

### Practical notes

- **An existing file is overwritten**, exactly as `> file` overwrites it.
  Appending was rejected deliberately: a previous run's result document followed
  by a new partial transcript would read as a finished run. A mistyped
  `--output` path clobbers a file the same way a mistyped `>` does.
- **An unusable path fails immediately.** If `<file>` cannot be created — it is
  a directory, a read-only location, a path that does not resolve — the run
  stops with a usage error (exit `2`) **before any scanning starts**, rather
  than hours in.
- **A write failure mid-run does not stop the run.** If the file becomes
  unwritable while the run is working — a full disk, a dropped network mount —
  the scan, and any `--apply` already in flight, continues and completes
  entirely on its own terms: the same groups, the same removals, the same stdout
  bytes it would have produced anyway. The failure is reported **once** on
  standard error, not once per subsequent failed write, and the run **exits
  `1`** even though the deduplication itself succeeded — you asked for a durable
  record and did not get one, and the exit code is the channel a script can see
  that on. The partial file is left exactly as it is.
- **The record contains every path the run touched**, including the full paths of
  your files. On a shared machine, choose the destination accordingly — the file
  is created with your usual default permissions, just as `>` would create it.
- **Pointing `--output` inside a directory being scanned is safe.** The record
  is never a valid image, so it can never be grouped, trashed, or deleted. With
  a permissive `--ext` it may earn itself one "not a recognized image" warning —
  which then shows up in its own transcript.

### `--json` without `--output` keeps nothing

Worth knowing **before** you start a long unattended run: `imgdedup --json` on
its own produces output only if it reaches the end. Its entire result is one
JSON document written at completion, and there is no incremental trace anywhere.
Kill it at 95% and you have nothing.

That is deliberate, not a gap. Keeping stdout to exactly one document is what
makes `imgdedup --json ./photos | jq …` work at all, and making it incremental
would break every existing consumer — for a durability gain a broken pipe would
largely negate anyway. `--output` is the answer instead, and a better one, since
NDJSON is valid at every line boundary:

```sh
imgdedup --json --output run.ndjson ./photos > result.json
```

For a long `--apply` run this is the shape to reach for: each removal lands in
the record the moment it happens, so even a run that never finishes leaves a
complete ledger of what it did.

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
| `1` | Run-level error — e.g. an input path could not be scanned, or an [`--output`](#--output-file-a-durable-record-of-the-run) record could not be written while the run was in progress (the scan itself still completed). |
| `2` | Usage error — invalid arguments, or an `--output` path that cannot be created (checked before scanning starts). |
| `3` | Duplicates were found. Only emitted when `--exit-code` is passed. |

## Next

- Back to the [guide index](README.md).
