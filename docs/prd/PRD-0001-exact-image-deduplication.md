# PRD-0001: Exact image deduplication

- **Status:** approved
- **Owner:** product-owner
- **Created:** 2026-06-13
- **Related:** [roadmap Phase 1](../planning/roadmap.md), [backlog TASK-0101](../planning/backlog.md), [TEMPLATE](TEMPLATE.md); hands open questions to the architect (TASK-0201 technology-stack ADR and design ADRs). FR-20/FR-21 are designed in [ADR-0018](../adr/0018-durable-run-record-and-output-stream-routing.md).

## 1. Summary

`imgdedup` is a cross-platform CLI that finds and removes **exact duplicate**
images — files whose contents are byte-for-byte identical — across one or more
directory trees. This first product increment is deliberately scoped to exact
matches: it is fast, has zero false positives, and lets us ship a trustworthy,
safe tool before tackling perceptual / near-duplicate detection in a later phase.
The defining concern is **safety**: the tool must never destroy a file the user
wanted to keep.

## 2. Background & motivation

People accumulate the same images many times over — repeated imports from phones
and cameras, backups merged into backups, "copy" folders, downloads saved twice.
Exact duplicates waste storage and clutter libraries, and existing options are
either GUI-bound, platform-specific, or general-purpose file dedupers with no
image awareness and a large blast radius.

Why exact-only first (confirmed with the owner): exact-content matching is the
high-confidence core of any deduper. It has **no false positives**, so the
destructive path can be made safe and predictable from day one, and it provides
the scanning/grouping/action machinery that perceptual detection later builds on.
Perceptual similarity introduces tunable thresholds and ambiguous "which to keep"
decisions that are out of scope here (see §4).

Confirmed assumptions (with the owner):

- Exact (content-hash) detection only for this PRD.
- Dry-run is the default; destructive actions are explicit and, by default,
  recoverable (OS trash).
- The keeper within a group is chosen by a deterministic, documented rule.
- Candidate files are selected by extension first, then confirmed to be images by
  content before any destructive action.

## 3. Goals

- Find all sets of byte-identical images under given paths, reliably and quickly.
- Make removal **safe by default**: nothing is destroyed unless explicitly
  requested, and the default removal is recoverable.
- Make results **deterministic and scriptable**: same filesystem state in → same
  decisions out, with machine-readable output.
- Work the same way on Linux, macOS, and Windows.

## 4. Non-goals

Explicitly out of scope for this PRD (YAGNI; some are future phases):

- Perceptual / near-duplicate detection (resized, recompressed, cropped, rotated)
  — a later phase.
- Interactive per-group "choose which to keep" prompting — possible later
  enhancement; the deterministic rule plus flags covers v1.
- Detecting images with a wrong or missing extension — documented limitation,
  reserved for a future `--sniff-all` flag (see §10).
- Moving duplicates to a user-specified quarantine folder — the chosen removal
  model is the OS trash.
- GUI, image preview/thumbnails, EXIF-based logic, config files, and watch/daemon
  modes.

**No longer a non-goal (Amendment 2026-08-06):** a dedicated `--output <file>`
flag. It was listed here as deferred by the Amendment 2026-07-19 (decision 2),
on the reasoning that shell redirection already covered saving results. That
reasoning held for a flag that merely duplicates `> file`; it does not hold for
the flag now specified in **FR-20**, which writes its file **continuously as
the run progresses**, so an interrupted run still leaves everything it had
produced — something redirection cannot do. See the
[Amendment 2026-08-06](#amendment-2026-08-06--durable-run-record-stream-model-and-report-paging-product-owner-ruling-task-0340--task-0341).

## 5. Users & use cases

Primary user: anyone with a cluttered local image collection who is comfortable
on the command line — photographers consolidating libraries, developers/sysadmins
cleaning storage, archivists merging folders.

User stories:

- As a user merging several photo folders, I want to **see** every set of
  identical images before anything changes, so that I trust the tool.
- As a user reclaiming space, I want to remove duplicates with **one keeper per
  set** kept by a predictable rule, so that I do not have to decide file by file.
- As a cautious user, I want removed files to go to the **trash** so that a
  mistake is recoverable.
- As a power user, I want **JSON output** and **deterministic** behavior so that I
  can drive `imgdedup` from a script.
- As a user running a scan that takes hours over a large or networked library, I
  want a **file that is written as the run goes**, so that if the run is
  interrupted — or the report scrolls out of my terminal — I still have
  everything it found (Amendment 2026-08-06, FR-20).
- As a user watching a long run, I want to see **how long it has been going**
  and to find the run's own output on **standard output**, with standard error
  reserved for actual problems (Amendment 2026-08-06, FR-21).

## 6. Functional requirements

- **FR-1:** Scan one or more input paths, recursing into subdirectories by
  default; `--no-recurse` limits the scan to the top level of each path.
- **FR-2:** Select candidate files by a configurable extension allow-list
  (default: `jpg`/`jpeg`, `png`, `gif`, `webp`, `bmp`, `tif`/`tiff`).
- **FR-3:** Before a file is grouped or acted on, confirm it is an image by its
  magic-byte signature. Files that fail confirmation are ignored and never
  removed.
- **FR-4:** Group files that are exact duplicates — byte-for-byte identical
  content — using a collision-resistant content hash as the matching primitive.
  Because the tool deletes files, a hash match alone must not authorize removal:
  before any non-keeper is trashed or deleted, its content is **byte-for-byte
  verified** against the keeper of its group. The hash is a fast pre-filter; the
  byte comparison is the authority. (Cost is bounded by duplicate volume, since
  only same-size, same-hash candidates are ever compared — see FR-5.)
- **FR-5:** Apply a size pre-grouping optimization: only files that share an exact
  byte size are hashed against each other. This must not change the resulting
  groups versus naively hashing every candidate.
- **FR-6:** Do not follow symbolic links and skip hidden files/directories by
  default; provide opt-in flags to include each.
- **FR-7:** Select exactly one **keeper** per duplicate group by a deterministic,
  documented rule. The default (`--keep shortest`) primary criterion is the
  **shortest path**, defined as fewest path segments (shallowest) and then fewest
  characters; ties are broken by oldest modification time, then by lexicographic
  path order. Results are reproducible across runs and platforms.
- **FR-8:** `--keep {shortest|oldest|newest}` selects the primary keeper
  criterion (`shortest` is the default; `oldest`/`newest` use modification time).
  The remaining criteria from FR-7 still break ties in a fixed order, so the
  outcome is always deterministic.
- **FR-9:** Default to a **dry run**: report duplicate groups and the actions that
  *would* be taken, modifying nothing on disk.
- **FR-10:** `--apply` moves every non-keeper in each group to the operating
  system trash / recycle bin (recoverable).
- **FR-11:** `--delete`, only valid together with `--apply`, permanently deletes
  non-keepers instead of trashing them. This is opt-in.
- **FR-12:** Be idempotent: re-running with `--apply` on an already-deduplicated
  tree finds nothing to do and changes nothing.
- **FR-13:** Produce a human-readable summary by default: the duplicate groups,
  the keeper and removed files per group, group/file counts, and reclaimable
  space.
- **FR-14:** `--json` emits machine-readable output (groups, keeper, intended or
  performed actions, and totals) suitable for scripting.
- **FR-15:** Exit `0` on success and a distinct non-zero code on error. A
  failure to write the FR-20 record file mid-run is such an error (exit `1`)
  even when the deduplication work itself succeeded — see FR-20 and AC-28a.
- **FR-16:** Re-validate at apply time. Because the scan (which selects groups and
  keepers) and `--apply` are separate passes, immediately before trashing or
  deleting a non-keeper the tool re-confirms the file still exists, is still a
  regular file (not newly a symlink), and still matches its keeper per FR-4. If a
  file changed since the scan, it is skipped with a warning rather than removed.
- **FR-17:** Treat hard links as already-deduplicated. Files that are hard links to
  the same underlying content (same device and inode / file identity) share
  storage, so removing one frees nothing. Such files are not proposed for removal,
  are never trashed or deleted on that basis, and are excluded from the
  reclaimable-space figure (FR-13). They may be reported as an informational note.
- **FR-18:** Provide an opt-in `--exit-code` flag: when set, the tool exits `3` if
  at least one duplicate group was found and `0` if none were, so scripts and CI
  can detect duplicates without parsing output. Without the flag, a successful run
  exits `0` regardless of findings (see FR-15). The flag changes only the exit
  status — never what is scanned, grouped, or removed.
- **FR-19 (Amendment 2026-07-19; refined 2026-08-01; amended 2026-08-06):**
  While a scan runs, show
  **live progress** so the user can see the tool is working and
  how far along it is. The indicator conveys both (a) the **overall problem
  size** — the number of candidate files to be checked, known once enumeration
  completes — and (b) **ongoing progress** as `x of y` files done plus a
  **percentage**, labeled with the current phase (e.g. scanning → hashing →
  verifying; user-facing phase names, not an implementation contract). Progress
  is **output-only**: it never changes what is scanned, grouped, or removed
  (like `--json`). **Which stream carries it is governed by FR-21 (Amendment
  2026-08-06)**: stdout in the default report mode when stdout is an
  interactive terminal, stderr in every other case. FR-19 originally specified
  stderr only, in every mode; that clause is superseded, while the property it
  existed to protect — a redirected or piped stdout carrying nothing but the
  report/JSON document — is preserved intact by FR-21. Progress is **enabled by
  default** and **auto-adapts to the terminal**:
  - When the stream carrying progress is an **interactive terminal (TTY)**, it
    renders as a **single line
    updated in place** (redrawn as counts advance).
  - When that stream is **not a TTY** (piped, redirected, or captured by
    CI/logs), the
    in-place updater is **disabled automatically**; instead it emits **occasional
    plain, newline-terminated progress lines** — no carriage returns or ANSI
    control codes — at coarse milestones (phase transitions and bounded percentage
    steps), so logs stay readable and bounded.
  - `--no-progress` **disables progress output entirely** (both the TTY and
    non-TTY paths); nothing progress-related is written to either stream.

  **Elapsed time (Amendment 2026-08-06).** Every progress render carries the
  time elapsed since the run started — in **every** phase and state, not only
  scanning's indeterminate walk state (AC-21b, which this generalizes) — and
  the terminal completion state additionally reports the run's **total**
  elapsed time. No remaining-time / ETA estimate is shown: this pipeline's
  phases have very different per-file costs (hashing dominated a real corpus
  at ~86%, TASK-0338), so an extrapolated estimate would be confidently wrong
  in exactly the long runs it would be wanted for.

  Before the candidate total `y` is known the indicator may show a
  counting/indeterminate state and switch to `x of y` + percentage once the
  denominator is fixed.

  **Scanning's two states (Amendment 2026-08-05).** Scanning does two things in
  sequence: it **walks** the tree to find files worth examining, then
  **examines** each file it found to confirm the content really is an image.
  Only the walk has a genuinely unknown total, and it is the *smaller* part of
  the wait; examination — which knows exactly how many files it must look at
  the moment it starts — is the larger one. So scanning shows an indeterminate
  count of files found while it walks, and, once the walk finishes, a
  determinate `x of <files found>` + percentage while it examines them. That
  denominator is the number of files **being examined**, not the run-wide
  candidate total `y` (which is only fixed once examination has rejected the
  non-images, and is therefore never larger).

  **Walk liveness cue (Amendment 2026-08-05, TASK-0332).** On a tree where
  very few entries match the extension scope, the walk's found-so-far count
  can itself sit static for a long stretch, indistinguishable on screen from
  a hung process. The indeterminate walk state therefore also shows **elapsed
  time since the scan started**, ticking on the same throttled cadence as
  every other repaint (AC-21b).

  **Denominator across phases (Amendment 2026-08-01; refined 2026-08-05).**
  Scanning runs before the candidate total `y` is known, so it renders an
  indeterminate counting state while walking and then a determinate count
  against the number of files it found (see "Scanning's two states" above);
  hashing is the first phase measured against `y` itself and reports `x of y`
  against the full candidate count. A later phase whose
  workload is a **strict subset** of the candidates (verifying, since only
  candidates that collided in a duplicate group are byte-verified) reports its
  own subset progress while **keeping the run's candidate total `y` visible in
  the same line**, e.g. `verifying: 1 of 2 (50%) (302 candidates)`. A single
  run-wide denominator shared by every phase is not used: a candidate file
  legitimately passes through more than one phase (hashing, then — only if it
  collided — verifying), so forcing one denominator across phases would either
  double-count a file's progress past `y` or reintroduce an indeterminate
  0%-then-jump stall. A phase with **zero workload** (nothing collided, so there
  is nothing to verify) is **skipped entirely** rather than rendered as a
  degenerate `0 of 0`. Every run **ends on a terminal completion state rendered
  against the full candidate total**, e.g. `done: 302 of 302 (100%)` — on a TTY
  this is the state left on screen once the run finishes, so the overall problem
  size (a) stays visible at the point the run completes even when an
  intermediate phase's own denominator was smaller.

  Progress output is **excluded from the determinism/parity guarantees** (NFR-3
  governs the result document; see §7). This is unaffected by the 2026-08-06
  stream change: progress reaches stdout only when stdout is an interactive
  terminal, which is exactly the case NFR-3 does not govern.

- **FR-20 (Amendment 2026-08-06):** Provide **`--output <file>`**: a durable
  record of the run, written **continuously as the run progresses** rather than
  at the end, so that a run which is interrupted, killed, or throttled still
  leaves everything it had produced by that point.
  - The file contains, in order of occurrence: a header (tool version,
    invocation, start time); coarse phase/elapsed milestones at the same
    granularity as the non-TTY progress fallback (AC-22), never one line per
    file; each duplicate group as soon as it is known to be final, with its
    keeper and removable files; every warning and error as it occurs;
    apply-time removals and skips as they happen; and, **on completion only**,
    the complete canonical result document — the **same document** the run
    writes to stdout. In the human-readable record that is a byte-for-byte
    match; under `--json` it is the same document in the record's own
    serialization (one line, compact) rather than the same bytes as stdout's
    pretty-printed form. Same content and same schema, not necessarily the
    same bytes.
  - **Only the completion document is covered by NFR-3.** The preceding
    transcript follows the order work finished, which is not guaranteed
    reproducible, and its running counts are provisional. The file must make
    that distinction legible to a reader. A file with no completion document is
    the signal that the run did not finish.
  - **Format follows the output mode:** the human-readable rendering by
    default; **NDJSON** (one JSON object per line, the completion document on
    the final line) under `--json`, so that every complete line of a
    partially-written file still parses.
  - Content is **flushed as it is written**, so a `SIGKILL` loses at most the
    record currently being written.
  - The flag is **opt-in**; no file is created without it. It is **additive**,
    not a redirection: the terminal still shows progress and still receives the
    report exactly as it would without the flag. Without `--output` there is no
    incremental trace — see FR-21's closing note, where that is recorded as a
    deliberate decision rather than a gap.
  - An unwritable path fails the run **before any scanning begins**, with a
    usage-level error, rather than failing hours in.
  - **A write failure *during* the run does not stop the run** (decided
    2026-08-06, ADR-0018): the scan or apply proceeds and completes entirely on
    its own terms, unaffected. The failure is reported **once** — not once per
    subsequent failed write — and the run **exits `1`**, FR-15's run-level
    failure code, even though the deduplication work itself succeeded. The
    rationale is that a run whose durable record is incomplete has failed at
    something the user explicitly asked for, and a script must be able to see
    that; aborting the scan instead would throw away completed work to punish a
    reporting problem.
  - `--output` is **output-only**: it never changes what is scanned, grouped,
    or removed, and a run with and without it produces the same stdout bytes.
    Its only effect outside the file is the exit code in the write-failure case
    above.

- **FR-21 (Amendment 2026-08-06; supersedes FR-19's "stderr only, in any
  mode"):** Route the run's **normal output to stdout and reserve stderr for
  genuine failures**.
  - In the default (non-`--json`) mode **with stdout an interactive terminal**,
    progress (FR-19), phase and elapsed-time output, and apply-time status
    lines are written to **stdout**.
  - In the default mode **with stdout redirected or piped**, none of that is
    written to stdout: stdout carries the report and nothing else, and progress
    moves to **stderr**, rendered according to **stderr's own** TTY-ness
    (AC-21/AC-22, unchanged) — a live in-place line when stderr is still a
    terminal, plain milestone lines when it is not. Redirecting stdout must not
    downgrade a live indicator the user can still see.
  - Under **`--json`**, stdout is **exactly one JSON document and nothing
    else**, terminal or not; progress goes to stderr, as before, again rendered
    per stderr's own TTY-ness.
  - **stderr carries only genuine failures** — run-level errors and per-file
    failures — plus `--timings` (ADR-0017's engineering diagnostic, an explicit
    carve-out). Routine status no longer appears there. The report's own
    `Warnings:` section remains part of the report document on stdout.
    **Apply-time destructive-action lines (`trashed:`/`deleted:`) get no stderr
    carve-out** (decided 2026-08-06, ADR-0018). They are routine status, not
    failures, and they are already durably recorded twice over: the result
    document's `actions` object (FR-14) is authoritative once a run completes,
    and with `--output` each one is captured incrementally as it happens
    (FR-20). Under `--json` they therefore appear on neither stream — which is
    correct, not an omission.
  - `--no-progress` continues to suppress progress entirely, on whichever
    stream it would have used (AC-23 unchanged).

  **`--json` without `--output` has no incremental or durable trace — accepted
  deliberately (2026-08-06), not an oversight.** In that combination the only
  output is the single JSON document, and only if the run completes, exactly as
  `--json` behaves today. This was considered and kept for two reasons.
  **Chainability:** one parseable document piped straight into `jq` or another
  consumer is the contract TASK-0308's schema and ADR-0016's plugin are built
  on, and making stdout incremental would break it for every existing consumer.
  **It would not actually fix the problem it appears to fix:** a single-document
  pipe is already comparably fragile on a crash — nothing downstream of a
  crashed pipe survives either — so streaming stdout relocates the failure mode
  rather than removing it. The durable answer for a `--json` run that must
  survive interruption is `--output` (FR-20), which is why the two compose.
  A future reader should treat this paragraph as the reason **not** to reopen
  streaming stdout without new evidence.

## 7. Non-functional requirements

- **NFR-1 (safety invariant):** The keeper of every group is never trashed or
  deleted, and no run may leave a group with zero surviving copies. No file is
  ever removed without a byte-for-byte content match against its surviving keeper
  (FR-4) re-validated at apply time (FR-16).
- **NFR-2 (no collateral damage):** A non-image is never trashed or deleted;
  magic-byte confirmation (FR-3) precedes every destructive action.
- **NFR-3 (determinism; narrowed 2026-08-06):** For a given filesystem state and
  flags, results and keeper choices are identical across runs and platforms.
  This guarantee governs the **result document**, which means concretely:
  **stdout whenever stdout is not an interactive terminal, and stdout always
  under `--json`** — every case a script, a redirect, a pipe, or a diff
  depends on — plus the completion document that ends an FR-20 record file.
  Explicitly **excluded**: the FR-19 progress stream, on whichever stream
  FR-21 puts it (Amendment 2026-07-19, re-scoped by the Amendment 2026-08-06);
  anything written to an interactive terminal alongside progress; and the
  transcript portion of an FR-20 record file, whose order follows the order
  work finished.
- **NFR-4 (scale):** Comfortably process large libraries — on the order of 100k+
  images and hundreds of gigabytes — by streaming file reads (no whole-file
  buffering in memory) and hashing in parallel across CPU cores. Concrete
  throughput/memory targets to be firmed up with the architect.
- **NFR-5 (portability):** Run on Linux, macOS, and Windows, honoring each
  platform's trash/recycle semantics.
- **NFR-6 (robust errors):** Unreadable files, permission errors, and similar
  per-file failures are reported and skipped; the run continues rather than
  aborting.

## 8. UX / CLI surface

Illustrative surface (final names/framework decided in a CLI design ADR):

```text
imgdedup <path> [<path>...] [flags]

Scope:
  --no-recurse        Do not descend into subdirectories
  --hidden            Include hidden files/directories
  --follow-symlinks   Follow symbolic links
  --ext <list>        Override the image extension allow-list

Keeper selection:
  --keep <rule>       shortest (default) | oldest | newest

Actions (default is dry-run / report only):
  --apply             Move non-keepers to the OS trash (recoverable)
  --delete            With --apply: permanently delete instead of trashing

Output:
  --json              Machine-readable output
  --output <file>     Write a continuous, durable record of the run to <file>
  --exit-code         Exit 3 if any duplicate groups were found (else 0)
  --no-progress       Disable the live progress indicator
```

Behavior:

- **Default (no action flag):** dry run — print each duplicate group with its
  keeper and the files that would be removed, plus totals and reclaimable space.
  Nothing on disk changes.
- **`--apply`:** perform the trash moves; report what was moved and the space
  reclaimed.
- **`--apply --delete`:** permanently delete non-keepers; report likewise.
- **`--delete` without `--apply`:** rejected with a clear error (guards against
  accidental permanent deletion).
- **`--exit-code`:** opt-in; the tool exits `3` when duplicate groups were found
  and `0` when none were, so scripts/CI can detect duplicates without parsing
  output. Without it, a successful run exits `0` regardless of findings.
- **Progress (Amendment 2026-07-19; streams amended 2026-08-06):** by default a
  live progress indicator shows the phase, `x of y` files, a percentage, and the
  elapsed time while the run works (a single line redrawn in place on a TTY;
  occasional plain lines otherwise). In the default report mode on an
  interactive terminal it is written to **stdout**; when stdout is redirected or
  piped, and always under `--json`, it is written to **stderr** instead — so
  `imgdedup … > out` and `imgdedup … --json > out.json` still capture only the
  report/JSON. `--no-progress` turns it off.
- **`--output <file>` (Amendment 2026-08-06):** writes a durable record of the
  run to `<file>` **as the run progresses** — groups as they are determined,
  warnings and errors as they occur, elapsed milestones, and the complete result
  document once the run finishes — so an interrupted run still leaves what it
  had produced. Human-readable by default, NDJSON under `--json`. It adds a
  file; it changes nothing about what the terminal or stdout receives.
- **Errors:** per-file problems are listed and skipped; the overall run still
  completes and summarizes. Exit non-zero only on a run-level failure. Standard
  error carries **only** genuine failures (Amendment 2026-08-06) — plus
  `--timings`, ADR-0017's diagnostic.

## 9. Acceptance criteria

- **AC-1 (FR-9, NFR-1):** Given a directory containing duplicate images, when run
  with no action flag, then every duplicate group is listed and no file on disk is
  modified, moved, or deleted.
- **AC-2 (FR-4, FR-7):** Given three byte-identical copies of an image, when
  scanned, then they form exactly one group with exactly one keeper chosen per the
  deterministic rule.
- **AC-3 (FR-10, NFR-1):** Given a duplicate group, when run with `--apply`, then
  the non-keepers are in the OS trash and recoverable, and the keeper remains in
  place.
- **AC-4 (FR-3, NFR-2):** Given a file named `photo.jpg` that is actually plain
  text, when scanned, then it is never grouped with images and never trashed or
  deleted.
- **AC-5 (FR-12):** Given a tree already deduplicated, when run again with
  `--apply`, then zero actions are taken and nothing changes.
- **AC-6 (FR-8):** Given `--keep newest`, when a group is resolved, then the file
  with the newest modification time is the keeper.
- **AC-7 (FR-11):** Given `--apply --delete`, when run on a duplicate group, then
  the non-keepers are permanently removed and are not present in the trash.
- **AC-8 (FR-14):** Given `--json`, when run, then output is valid JSON containing
  each group, its keeper, the intended or performed actions, and totals.
- **AC-9 (FR-5):** Given a candidate set, when scanned, then the groups produced
  with the size pre-grouping optimization are identical to those produced by
  hashing every candidate.
- **AC-10 (NFR-6):** Given an unreadable file in the scan path, when run, then the
  file is reported and skipped and the run completes successfully.
- **AC-11 (FR-9 → FR-10 parity):** Given a tree **left unchanged between runs**,
  the groups and intended removals reported by a dry run match what a subsequent
  `--apply` performs.
- **AC-12 (FR-15, §8):** Given `--delete` without `--apply`, when run, then the
  tool exits non-zero with a clear error and removes nothing; given a successful
  run it exits `0`, and given a run-level failure it exits with a distinct
  non-zero code.
- **AC-13 (FR-13):** Given duplicate groups, when run, then the summary reports
  each group's keeper and removed files, the group and file counts, and the
  reclaimable space.
- **AC-14 (FR-6, NFR-2):** Given a directory containing a symlink and a hidden
  file, when run with defaults, then neither is followed/included; when run with
  `--follow-symlinks` / `--hidden`, then each is included accordingly.
- **AC-14a (FR-6, FR-13, FR-17; added 2026-08-04, TASK-0320):** Given
  `--follow-symlinks` and **a single scan root** under which one physical file
  is reachable by two paths (one through a symlinked ancestor directory or a
  leaf symlink, one direct), when scanned, then the file appears exactly once,
  at its
  non-symlink-routed path — it is not reported as a hard-link set, and a
  genuine byte-identical copy elsewhere in the tree is still reported as
  removable with its true reclaimable size. This closes a defect, not just a
  labeling nit: before this fix, the alias was mislabeled `hard-linked`
  (ADR-0008 FR-17's label is reserved for two independent directory entries
  naming one inode, not one entry revisited via a symlinked route) and, when a
  genuine duplicate existed alongside it, the anchor rule protected it from
  removal — reporting `0` reclaimable bytes when real space was reclaimable. A
  true hard link (two independent directory entries) is unaffected and keeps
  going through the existing anchor-protection path (AC-18/AC-18a). Two cases
  were explicitly **out of** this criterion's scope when it was written: (a)
  the same aliasing **between two different scan roots**, now covered by
  AC-14b (product-owner ruling 2026-08-04, TASK-0329); and (b) a leaf symlink
  whose target is **not** independently in scope (outside every scan root, or
  filtered out by `--ext`/hidden rules) — still a candidate at the link path,
  still declined at apply with the existing skip reasons, safe (nothing is
  ever removed for it), and still accepted rather than fixed.
- **AC-14b (FR-6, FR-13, FR-17; added 2026-08-04, TASK-0329):** Given
  `--follow-symlinks` and **two or more scan roots**, where a symlink under one
  root (a symlinked ancestor directory or a leaf symlink) gives a physical file
  under another root a second route, when scanned, then that file appears
  exactly once, at its non-symlink-routed path — it is not reported as a
  hard-link set, and a genuine byte-identical copy anywhere in the scan is
  still reported as removable with its true reclaimable size. The result,
  including *which* path survives, is identical regardless of the order the
  roots are given on the command line and regardless of how many roots are
  passed (three or more behave as two) — NFR-3/AC-16. True hard links are
  unaffected and keep going through the anchor-protection path (AC-18/AC-18a);
  a candidate whose canonical path cannot be determined is never dropped. This
  closes the residual named in AC-14a: before the fix, `imgdedup
  --follow-symlinks A B` with `A/l -> B/deep` reported the one physical file as
  a two-member hard-link set and `reclaimable_bytes: 0` while a genuine copy
  under `A` was truly reclaimable.
- **AC-15 (FR-1):** Given duplicates nested in subdirectories, when run with
  defaults they are found; when run with `--no-recurse` only top-level files of
  each path are considered.
- **AC-16 (NFR-3):** Given the same tree and flags, two separate runs produce
  identical groups and keeper choices.
- **AC-17 (FR-16, NFR-6):** Given a non-keeper that is modified or replaced
  between the scan and `--apply`, when applied, then that file is skipped with a
  warning and not removed, and the remaining groups are still applied normally.
- **AC-18 (FR-17):** Given two hard links to the same content, when scanned, then
  neither is proposed for removal and they do not contribute to reclaimable space.
- **AC-18a (FR-17; added 2026-08-01, TASK-0316):** Given two or more scanned
  hard links to the same content and **no other file in scope with that
  content**, when scanned, then the set is reported as already deduplicated —
  the human report shows it under `Already deduplicated (hard-linked)` and
  `--json` lists it in `already_deduplicated` with
  `already_deduplicated_sets` counting it — while `duplicate_groups`,
  `removable_files` and `reclaimable_bytes` all stay `0`. A single file with no
  hard-linked sibling is **not** such a set and is reported as before. As
  always, a file excluded by an identity-lookup warning is out of scope for
  this determination — it is reported as a warning and does not count as
  "another file in scope with that content". This
  tightens AC-18 rather than extending scope: ADR-0008 edge case 1 already
  required it (one inode trivially satisfies "every distinct inode is
  multiply-linked in-scope"), and both output formatters already handled it —
  only one engine guard dropped the note before it could be surfaced.
- **AC-19 (FR-18):** Given `--exit-code`, when a scan finds duplicate groups the
  tool exits `3`; when it finds none it exits `0`; and without `--exit-code` a
  successful run exits `0` even when duplicates are present.
- **AC-20 (FR-19/FR-21, stdout purity; re-scoped 2026-08-06):** Given progress
  is enabled (default), when a run completes with **stdout redirected or piped**
  — in either the default report mode or with `--json` — or with **`--json`** at
  all, then **stdout contains exactly the report/JSON document and no progress
  text**; the progress output appears on stderr instead. Redirecting stdout to a
  file therefore yields a clean report/JSON. On an interactive terminal in the
  default report mode, progress deliberately *does* share stdout with the report
  (FR-21) — no script or redirect is reading in that case.
- **AC-21 (FR-19, TTY; amended 2026-08-01, 2026-08-05, 2026-08-06):** Given the
  stream FR-21 assigns to progress (stdout in the default report mode, stderr
  under `--json`) is an
  interactive terminal, when a scan runs, then a **live single-line** progress
  indicator is shown on that stream reporting the current phase, its progress,
  and the elapsed time (AC-32),
  where: scanning renders an indeterminate counting state while it walks the
  tree and the total is genuinely unknown, and then a **determinate** state
  measured against the number of files that walk found (AC-21a),
  and hashing — the first phase measured against `y` — reports `x of y` and a
  percentage against `y` = the count of
  candidate files to check (known once scanning has confirmed which of the
  files it found are images); a phase whose workload is
  a strict subset of the candidates (verifying) reports its own subset `x of
  <subset-total>` and percentage while keeping the candidate total `y` visible in
  the same line, e.g. `verifying: 1 of 2 (50%) (302 candidates)`; a phase with
  zero workload is skipped entirely rather than shown as `0 of 0`; and the run
  always ends on a terminal completion line rendered against the full candidate
  total, e.g. `done: 302 of 302 (100%)` — the state left on screen once the run
  finishes, so the overall problem size stays visible at completion regardless of
  a smaller intermediate-phase denominator.
- **AC-21a (FR-19, scanning's determinate second state; added 2026-08-05,
  TASK-0328):** Given a scan whose tree walk found `N > 0` files to examine,
  when the tool starts examining those files (confirming each one's content is
  really an image), then the indicator leaves the indeterminate
  `scanning... N files found` state and reports **`scanning: x of N (p%)`**,
  where:
  - `N` is exactly the number of files the walk found — the same number the
    indeterminate state last displayed — so the count the user just watched
    grow becomes the target it is counted against;
  - `x` advances by exactly one **per file examined, whether that file is
    confirmed an image or rejected as a non-image**, so the indicator keeps
    moving even on a tree where nothing is confirmed;
  - the first render of this state is `scanning: 0 of N (0%)` (it repaints
    immediately on the switch, like any other phase/denominator change, rather
    than waiting for the next throttled repaint) and the state ends at
    `scanning: N of N (100%)`;
  - the phase **label stays `scanning`** — this is a second state of the same
    user-facing phase, not a new phase name;
  - `N` is **not** the run-wide candidate total `y` and carries **no
    `(<n> candidates)` suffix**: `y` is not yet known while scanning runs, and
    `N ≥ y` because rejected files are examined but never become candidates.
    Hashing therefore still begins at `0 of y` with a denominator that may be
    smaller than `N` (e.g. `scanning: 30000 of 30000 (100%)` then
    `hashing: 0 of 28450 (0%)`, or `done: 0 of 0 (100%)` when nothing was
    confirmed), regardless of where scanning's counter ended.

  When the walk found nothing (`N == 0`), this state is **skipped entirely** —
  never `scanning: 0 of 0` — consistent with AC-21's zero-workload rule. On a
  non-TTY the same state appears only as bounded coarse-milestone plain lines
  per AC-22 (no in-place redraw, no per-file line).
- **AC-21b (FR-19, walk liveness cue; added 2026-08-05, TASK-0332):** Given
  stderr is an interactive terminal, when the tree walk (AC-21's indeterminate
  scanning state) is running, then the indicator shows **elapsed time since
  the scan started** (the process's own progress-indicator construction,
  fractions of a second before the walk's first entry — not distinguishable
  in practice, since the indeterminate state is the very first thing shown)
  alongside the found-so-far count on every repaint, e.g.
  `scanning... 3 files found (14s elapsed)`, updating on the same throttled
  cadence as every other repaint (AC-21) — unconditionally, from the very
  first render, not only once some threshold of time or non-matching entries
  is crossed. This is deliberately not framed as a response to the walk
  finding nothing: the suffix is part of every indeterminate repaint, not a
  special case that switches on when a tree happens to be slow to walk, even
  though the case it matters for is a tree where very few entries match the
  extension scope, where the found-so-far count alone would otherwise sit
  static for a long stretch (indistinguishable on screen from a hung
  process). Non-TTY is unaffected — AC-22's existing "found-so-far count
  stays TTY-only" reasoning applies identically to elapsed time, since a
  non-redrawn line could only ever record a stale value.
- **AC-22 (FR-19, non-TTY fallback; amended 2026-08-01, 2026-08-05,
  2026-08-06):** Given the stream FR-21 assigns to progress is
  **not** a TTY (redirected or piped) — which, in the default report mode with
  stdout redirected, means progress falls back to plain lines on **stderr**
  rather than joining the report on stdout — when a scan runs, then **no in-place
  redraw / carriage-return / ANSI control sequences** are emitted; progress
  instead appears as occasional plain, newline-terminated lines (log-friendly and
  bounded), following the same per-phase-subset-with-total-visible convention as
  AC-21 (and skipping any zero-workload phase) — at minimum a problem-size line
  once enumeration completes and a completion line rendered against the full
  candidate total (e.g. `done: 302 of 302 (100%)`), so a captured log always
  evidences the run's scale and finish. Scanning's determinate second state
  (AC-21a) also appears here, at the same coarse milestone granularity as every
  other phase, but its denominator is the examined-file count, not `y` — it
  therefore does **not** discharge the problem-size minimum above, which is
  about `y`. Scanning's indeterminate found-so-far count stays TTY-only: a
  non-redrawn line could only ever record a stale count. The AC-21b elapsed-
  time liveness cue is TTY-only for the identical reason. **Scope note
  (2026-08-06):** that TTY-only reasoning is about the *indeterminate walk
  state*, whose count and clock are only meaningful while being redrawn. It
  does **not** constrain elapsed time elsewhere: AC-32's per-render elapsed time
  applies wherever progress renders, and FR-20's record-file milestone lines
  always carry elapsed time regardless of any stream's TTY-ness — a written
  milestone is a timestamped fact, not a stale redraw.
- **AC-23 (FR-19, opt-out; re-scoped 2026-08-06):** Given `--no-progress`, when
  a scan runs, then **no progress output is written at all** — to stderr or to
  stdout, whichever FR-21 would have used — and the report/JSON document is
  unchanged.
- **AC-24 (FR-19, output-only; re-scoped 2026-08-06):** Given identical inputs
  and flags, the groups, keeper, intended removals, and the **result document
  are identical** whether progress is enabled, disabled (`--no-progress`), or on
  the non-TTY fallback path. Where the result document is the whole of stdout —
  stdout redirected or piped, and always under `--json` — those stdout bytes are
  therefore identical too (NFR-3 unaffected). On an interactive terminal in the
  default report mode, progress shares stdout by design (FR-21), so the
  invariant is on the report itself, not on every byte of the stream.
- **AC-25 (FR-20, durability):** Given `--output run.txt` on a run that finds
  duplicate groups, when the process is killed partway through, then the file
  exists and contains the header, the milestones reached so far, and every
  group that had already been determined — and it does **not** contain the
  completion document, so an interrupted run is distinguishable from a
  finished one.
- **AC-26 (FR-20, completion document):** Given `--output run.txt` on a run
  that completes, when the run finishes, then the file ends with the complete
  canonical result document — **byte-identical** to what the same run writes to
  stdout in the human-readable case — and two runs over an unchanged tree
  produce byte-identical completion documents (NFR-3, AC-16) even if their
  transcripts differ.
- **AC-27 (FR-20, NDJSON under `--json`):** Given `--output run.ndjson --json`,
  when the process is killed partway through, then every complete line of the
  file parses as an independent JSON object; and when the run completes
  instead, the final line is the complete result document — **the same document
  and schema that stdout received, though not the same bytes**: the record
  serializes it compactly on one line, stdout pretty-prints it. Parsing both and
  comparing the resulting values must show them equal.
- **AC-28 (FR-20, output-only + fail fast):** Given identical inputs and flags,
  the groups, keeper, intended removals, and **stdout bytes** are identical with
  and without `--output`, and the terminal shows the same progress and report
  either way; and given an unwritable `--output` path, the run fails with a
  usage-level error **before scanning starts** and writes no report.
- **AC-28a (FR-20, mid-run write failure; added 2026-08-06):** Given a valid
  `--output` path that becomes unwritable **while the run is in progress** (disk
  full, network path dropped), when the run continues, then the scan or apply
  completes normally and its groups, keeper choices, removals, and stdout
  report/JSON are exactly what they would have been without `--output`; the
  failure is reported **exactly once**, not once per subsequent failed write;
  and the process exits **`1`** (FR-15's run-level failure code) despite the
  deduplication work itself having succeeded.
- **AC-29 (FR-21, interactive stdout):** Given the default (non-`--json`) mode
  with stdout an interactive terminal, when a scan runs, then progress —
  including elapsed time — and apply-time status lines appear on **stdout**,
  and **stderr receives nothing** unless a genuine failure occurred.
- **AC-30 (FR-21, redirected stdout unchanged; corrected 2026-08-06):** Given
  the default mode with stdout redirected or piped, when a run finishes, then
  stdout is byte-identical to the report the same run produced before the
  Amendment 2026-08-06 — no progress, no elapsed-time lines, no status lines —
  and progress appears on **stderr** instead, rendered according to **stderr's
  own** TTY-ness per AC-21/AC-22: a live single line redrawn in place when
  stderr is still a terminal (the common `imgdedup ./photos > report.txt` case,
  whose live indicator must not regress), plain milestone lines when stderr is
  redirected too.
- **AC-31 (FR-21, `--json` purity):** Given `--json`, when a run finishes,
  whether or not stdout is a terminal, then stdout is exactly one JSON document
  and nothing else, and progress appears on stderr only.
- **AC-32 (FR-19 amended 2026-08-06, elapsed time):** Given progress is
  enabled, when a scan runs on a terminal, then **every** progress render
  carries the elapsed time since the run started — in every phase and state,
  generalizing AC-21b beyond scanning's indeterminate walk state — and the
  terminal completion state reports the run's total elapsed time. No
  remaining-time estimate is shown.

## 10. Open questions

Handed to the architect (Phase 2) unless noted:

- **Hash algorithm:** which collision-resistant content hash (e.g. SHA-256 vs
  BLAKE3) best balances collision resistance and throughput? Note the byte-verify
  requirement (FR-4) means the hash need not be cryptographically collision-proof
  on its own. (design ADR)
- **Byte-verification strategy:** how to verify content equality efficiently
  within a group (streamed pairwise vs. against the keeper) without re-reading
  data needlessly, given the NFR-4 scale targets. (design ADR)
- **Hard-link / file-identity detection:** how to determine same-content links
  (device + inode on POSIX; file ID on Windows) portably, for FR-17. (design ADR)
- **Cross-platform trash:** how to move to the trash/recycle bin reliably on
  Linux, macOS, and Windows (native APIs vs a maintained library)? (design ADR)
- **Concurrency & memory model:** parallel hashing strategy and memory bounds for
  the NFR-4 scale targets. (design ADR)
- **Magic-byte coverage:** the image-signature set must cover every extension in
  the default allow-list (FR-2/FR-3); confirm detection for each format. (design)
- **Case-insensitive filesystems:** the lexicographic tie-break (FR-7) must be
  defined unambiguously where paths differ only by case. (design)
- **Exit-code semantics (resolved):** a "duplicates found" signal is provided as
  the **opt-in `--exit-code`** flag (exit `3` when duplicates are found, `0`
  otherwise); the default exit behavior is unchanged. Decided with the owner — see
  FR-18, AC-19, and [ADR-0008](../adr/0008-core-deduplication-algorithm-and-semantics.md).
- **Technology stack:** language and CLI framework — this PRD is stack-agnostic
  and feeds TASK-0201.

## 11. Risks & dependencies

- **Risk — accidental data loss:** highest-stakes risk. Mitigated by the dry-run
  default (FR-9), recoverable trash default (FR-10), opt-in permanent delete
  (FR-11), the keeper safety invariant (NFR-1), byte-verification before removal
  (FR-4), and apply-time re-validation (FR-16).
- **Risk — trashing a non-image:** mitigated by magic-byte confirmation before any
  action (FR-3, NFR-2).
- **Risk — missed images with wrong/missing extensions:** accepted, documented
  limitation; future `--sniff-all` flag (§4, §10).
- **Risk — hash collision causing a false "duplicate":** mitigated by byte-for-byte
  verification before any removal (FR-4), so a hash collision can never cause data
  loss; hash choice is still an open architecture question.
- **Risk — TOCTOU between scan and apply:** a file may change between the dry-run
  scan and `--apply` (including being swapped for a symlink). Mitigated by
  re-validation at apply time (FR-16); AC-11 parity is scoped to an unchanged tree.
- **Risk — a second machine-readable surface drifting (Amendment 2026-08-06):**
  FR-20's NDJSON record file is machine-readable output that is *not* the
  `--json` document, so the two can diverge in field names, meanings, or
  coverage. Mitigated by making the record file's completion line the
  **same** result document `--json` emits, and by giving the event vocabulary an
  explicit stability posture (handed to the architect) rather than letting one
  accrete by accident.
- **Risk — a partial record read as a complete one (Amendment 2026-08-06):** a
  transcript from an interrupted run reports provisional counts in
  work-completion order. Mitigated by the completion document's presence being
  the only "this run finished" signal, and by requiring the file to make the
  transcript/result distinction legible (FR-20).
- **Risk — hard links misreported as reclaimable:** removing one link frees no
  space and may discard a path the user considers distinct. Mitigated by FR-17
  (hard links are not proposed for removal and excluded from reclaimable space).
- **Dependencies:** technology-stack ADR (TASK-0201) and design ADRs (hashing,
  cross-platform trash, concurrency) depend on this PRD; implementation tasks are
  seeded into the backlog once those ADRs land.

## Amendment 2026-07-19 — progress indication & stored results

The human owner ran the shipped tool manually on a real folder and gave direct
product feedback: while the program runs there is **no terminal output at all**,
so there is no sense of (a) how big the job is, or (b) how far along it is; and
they wanted a way to **keep the results in a file** to view or process later. The
orchestrator brainstormed the intent with the owner, who made three decisions
(2026-07-19), recorded here and reflected in FR-19, AC-20..AC-24, §4, §7 (NFR-3),
and §8 above.

**Owner decision 1 — progress UX.** A live progress indicator is added, on
**stderr**, default-on, showing the phase, `x of y` files, and a percentage. It
**auto-disables its in-place rendering when stderr is not a TTY**, falling back to
occasional plain, newline-terminated progress lines (log-friendly; no carriage
returns / ANSI control codes), and `--no-progress` turns it off entirely.
Formalized as **FR-19** with acceptance criteria **AC-20..AC-24**. Because stdout
purity is a shipped contract (slice S2, `docs/guide/output-and-exit-codes.md`),
progress must go to stderr **only** and must never interleave into stdout in any
mode — including `--json`. Progress is output-only (it changes nothing about what
is scanned, grouped, or removed) and is excluded from the NFR-3 determinism/parity
guarantees, which cover stdout content only.

**Owner decision 2 — stored results via redirection, no new flag.** Saving results
to a file is supported through **shell redirection of stdout**, not a dedicated
flag:

```sh
# Human-readable report saved to a file.
imgdedup ./photos > report.txt

# Machine-readable results saved for a script or later processing.
imgdedup --json ./photos > result.json
```

This works precisely because stdout carries **only** the report/JSON while all
diagnostics (warnings, run-level errors, and the new FR-19 progress) go to stderr,
so the redirected file stays clean. A dedicated **`--output <file>` flag was
considered and explicitly deferred** (YAGNI): redirection already covers the need
with zero new surface. Recorded as a non-goal in §4; it is intentionally **not** a
backlog task. If a future need arises (e.g. writing a file while also printing a
human summary to the terminal), it can be revisited in a new PRD/ADR.

> **Superseded 2026-08-06 — decision 2 only.** The future need this decision
> anticipated arrived. A real ~69 000-file run showed that redirection cannot
> help a run interrupted before it ends, because the report is written in one
> shot at the very end. `--output <file>` now exists as **FR-20**, carrying the
> property that distinguishes it from the flag deferred here: it writes
> **continuously as the run progresses**. The §4 non-goal is removed. Decisions
> 1 and 3 stand, except that decision 1's "stderr only, in any mode" routing is
> separately superseded by **FR-21**. See the
> [Amendment 2026-08-06](#amendment-2026-08-06--durable-run-record-stream-model-and-report-paging-product-owner-ruling-task-0340--task-0341).

**Owner decision 3 — scheduling.** FR-19 is specified now but **built in a fresh
usage window as its own unit** (implementer, TDD) — see
[TASK-0315](../planning/tasks/TASK-0315-progress-indication.md) and batch 4b in
the [batch plan](../planning/batch-plan.md). Implementation must not start ahead of
that window. The stored-results guidance documents **existing** behavior and ships
immediately in the README and user guide.

## Amendment 2026-08-01 — progress denominator refinement (architect review, TASK-0315)

During the architect's whole-unit review of the TASK-0315 progress-indication
build, a genuine deviation from AC-21 as originally written surfaced: AC-21
pinned `y` to "the count of candidate files to check (known after
enumeration)" for the entire run, but the shipped build gives the verifying
phase its own, smaller denominator (e.g. `verifying: 2 of 2 (100%)` on a
302-candidate run). The architect's verdict, accepted by the product owner, is
two-fold:

1. This is a **genuine spec deviation**, not a defensible reading of the
   original AC-21 text.
2. The **strict original reading is unimplementable as written**. A candidate
   file legitimately passes through more than one phase — hashing always, then
   verifying if (and only if) it collided with another file. A single
   run-wide denominator shared across every phase therefore forces one of two
   bad outcomes: either a file's second pass (through verifying) pushes the
   numerator **past `y`** — double-counting — or the display must hold `x` at
   whatever hashing left it at while verifying runs, producing the
   0%-then-jump stall that an earlier iteration of this same build had already
   been fixed to avoid. AC-21 was therefore **under-specified**, not merely
   under-implemented.

The deviation also had a real, owner-relevant consequence, verified under a
pty: on a TTY the progress line is redrawn **in place**, so once the run
enters verifying, the earlier candidate total is erased from the screen. On a
302-file library with two duplicates, the run would rest at `verifying: 2 of 2
(100%)`, no longer conveying the **overall problem size** (FR-19(a)) — the
owner's original 2026-07-19 complaint (no sense of how big the job is)
reappearing at the very moment the run ends, when the resting line matters
most. On a duplicate-free tree the closing line was the degenerate `verifying:
0 of 0 (100%)`, evidencing a scale of zero rather than the library actually
scanned.

**Ratified resolution**, reflected in FR-19 and AC-21/AC-22 above:

- Scanning stays indeterminate (the total is not known until it finishes) and
  hashing continues to report `x of y` against `y` = the candidate count,
  unchanged from the original amendment.
- A phase whose workload is a strict subset of the candidates (verifying)
  reports its own subset progress **while keeping the run's candidate total
  visible in the same line**: `verifying: 1 of 2 (50%) (302 candidates)`.
- A phase with zero workload is **skipped entirely** — no more degenerate
  `verifying: 0 of 0 (100%)` lines.
- Every run **ends on a terminal completion state rendered against the full
  candidate total**: `done: 302 of 302 (100%)`. On a TTY this is the resting
  state left on screen, so the overall problem size is visible whenever the run
  finishes, regardless of which phase's own denominator was smaller along the
  way.

This satisfies FR-19(a) (overall problem size) at the point it matters most —
the state left on screen at completion — without forcing a shared denominator
across phases whose workloads are genuinely different in kind. It does not
reintroduce a single run-wide `x of y` for the whole run; readers of FR-19
should not expect the verifying phase's own counter to match `y` — only the
closing `done:` line and the `— <n> candidates` suffix carry that total once
verifying is in progress.

## Amendment 2026-08-02 — AI-agent automation as a supported capability

The owner confirmed (session 9, TASK-0218) that AI-agent-driven usage is
expected to become an increasingly common way people operate `imgdedup`, so it
is recorded here as a supported product capability even though its design was
architect-owned rather than run as a full PRD slice — see
[ADR-0016](../adr/0016-ai-agent-automation-plugin.md) for the complete design
rationale.

`imgdedup` ships an official **Claude Code plugin** (`claude-plugin/` in this
repository): a skill defining a dry-run-first safety protocol, a
`/imgdedup:dedupe` slash command, and a read-only `imgdedup-operator` subagent
for large scans. The plugin is a pure wrapper over the CLI contract already
defined in this PRD — FR-9 (dry-run default), FR-10 (`--apply`), FR-11
(`--delete`), and FR-14 (`--json`) — and introduces **no new CLI behavior**:
every destructive action it takes is gated behind an explicit,
in-conversation human confirmation, and permanent deletion (`--delete`)
requires its own separate explicit confirmation beyond the one that
authorizes `--apply`. See
[Using imgdedup with an AI agent](../guide/using-with-claude.md) for the
user-facing walkthrough.

## Amendment 2026-08-05 — scanning's two states (product-owner ruling, TASK-0328)

TASK-0317 animated the scanning phase's indeterminate counter, but its own
architect review then **measured** what that animation actually covers (release
build, 30k files / 60 dirs / ~117 MB, warm cache, 10 cores): of the whole
scanning window, walking the tree is ≈10% (0.03 s) and examining each found
file's magic bytes is ≈90% (0.88 s). The shipped indicator therefore animates
the small part and sits on a live-but-static `scanning... 30000 files found`
for the large part — the same "is it hung?" complaint FR-19 exists to answer,
one stage further in. On a library an order of magnitude larger this static
window is tens of seconds, and it grows with the library while the walk does
not dominate it. That is enough user-visible value to close, not to defer.

The blocker was never technical (the examination stage is already
rayon-parallel and the sink is documented safe under concurrent `advance`); it
was that AC-21 specified scanning as a **single** indeterminate state, which is
only true of the walk. The moment examination starts, its total is known
exactly — it is the walk's own output — so this stage *can* be determinate.

**Ratified ruling** (reflected in FR-19 and AC-21/AC-21a above):

1. **Animate it, determinately.** Scanning becomes two successive states of one
   phase: the existing indeterminate `scanning... N files found` while walking,
   then `scanning: x of N (p%)` while examining, with one advance per file
   examined.
2. **No new phase name.** The label stays `scanning`. Splitting scanning into
   two named phases (`scanning` → `confirming`/`checking`) was considered and
   rejected: it buys the user nothing the numbers do not already say, and every
   candidate word for the second stage is a near-synonym of the existing
   `verifying` phase (which does something entirely different — byte-for-byte
   comparison), so the vocabulary would actively mislead. The state change is
   self-evident on screen instead: the count the user just watched grow becomes
   the denominator it is counted against (`scanning... 30000 files found` →
   `scanning: 0 of 30000 (0%)`).
3. **A shared, still-indeterminate counter running through both stages
   (`scanning... N files found` continuing to climb during examination) was
   rejected** as the cheapest option that is also dishonest: the meaning of
   `N` would silently change from "files discovered" to "files examined"
   mid-count with nothing on screen distinguishing them, and it would throw
   away a denominator that is known exactly — leaving the user without the one
   thing they asked for during the longest part of the wait, "how far along".
4. **The examination denominator is not `y`.** It is the number of files
   examined; `y` (the candidate total used by hashing and the closing `done:`
   line) counts only those confirmed as images, so `N ≥ y`. This is the same
   per-phase-own-workload principle ratified on 2026-08-01, applied one stage
   earlier. Unlike verifying, no `(<n> candidates)` suffix is shown, because
   `y` genuinely is not known yet at that point — showing the run total there
   is impossible, not merely omitted.

**Residual, explicitly accepted and named** (the framing TASK-0328's backlog
row asked to be ruled on): the counters count *files*, not directory entries
visited. On a tree whose files mostly carry image extensions but not image
content, this ruling is a strict improvement — every such file is examined, so
the counter advances on all of them and the run truthfully ends up with a small
or zero `y`. But on a tree where very few entries match the extension scope at
all (a source checkout, a backup volume), the **walk** can grind through a long
stretch of entries while `scanning... 3 files found` barely moves. That is
truthful — three files really is all it has found — but it does not evidence
liveness, and this ruling does not fix it, because the fix is a different
mechanism (a liveness cue, not a counter) and the walk is the ≈10% part.
Tracked as TASK-0332 rather than folded in here.

## Amendment 2026-08-05 — walk liveness cue (product-owner ruling, TASK-0332)

Closes the residual TASK-0328 named and explicitly declined to fix: on a tree
where very few entries match the extension scope (a source checkout, a backup
volume), the walk can grind through a long stretch of directory entries while
`scanning... 3 files found` sits static — truthful, but indistinguishable on
screen from a hung process, the exact complaint FR-19 exists to answer.

**Ratified ruling** (reflected in FR-19 and AC-21b above): show **elapsed time
since the scan started**, appended to the existing indeterminate line —
`scanning... 3 files found (14s elapsed)` — updating on the same throttled
cadence as every other repaint, from the first render of the state (no
threshold before it appears).

Two alternatives were on the table, both named in TASK-0332's own backlog
row, both rejected:

1. **An entries-visited tick** (`scanning... 3 files found, 48213 entries
   scanned`) — rejected as the more informative-*looking* option that is
   actually the more misleading one. There is no denominator for "entries
   visited" (the walk does not know how many entries a tree has until it
   finishes), so a growing number next to a percentage-shaped display
   invites exactly the reading TASK-0332's backlog row warned against:
   that it measures progress *toward* something, when it measures nothing
   more than "the walk has not stopped". Elapsed time carries no such
   implication — a clock is legible as "still running", full stop, to
   every reader, technical or not.
2. **A spinner** — rejected as strictly weaker than elapsed time for equal
   implementation cost. Both are liveness-only signals that assert nothing
   about quantified progress, so neither has an honesty edge over the
   other; but a spinner communicates only "alive", while a ticking clock
   communicates "alive" *and* "for this long" — genuinely useful
   information a spinner discards for no offsetting benefit here (this
   tool's other progress text is plain counts and percentages throughout,
   not iconography, so a spinner would also be the one glyph-based element
   in an otherwise plain-text display).

**Mechanism, not just UX.** The elapsed clock can only *tick* if something
keeps triggering repaints while the walk finds nothing — today, a repaint is
driven exclusively by `advance()` calls, and on a low-match-density tree those
are exactly the calls that go quiet for a long stretch. The walk now checks in
on **every** directory entry it visits, accepted or not, through the same
throttled repaint path `advance()` already uses (a zero-count "tick" that
updates no counter but is still subject to the existing ≤1-repaint-per-100ms
throttle). This adds no *extra repaint*, since the throttle already bounds
how often a repaint can happen; the per-entry cost of the tick call itself
(an atomic increment-by-zero plus a clock read) is negligible next to the
syscalls the walk already pays per entry, but is not literally free. This is
what makes the elapsed suffix actually move during the slow case, rather than
freezing alongside the count it sits next to.

**Residual, named rather than silently narrowed:** a single directory read
that itself blocks for longer than one repaint interval (an unresponsive
network mount, a very large single directory) still cannot be ticked through
mid-read — the clock resumes the moment the next entry is visited, same as
the count it accompanies. The same gap exists, less prominently, in two
places outside `enumerate_root`'s walk itself: `dedupe_roots`'s per-root
canonicalization (before any walk starts) and, under `--follow-symlinks`
only, `dedupe_symlink_aliases_across_roots`'s post-walk canonicalization pass
(I/O proportional to the candidate count) — neither is ticked, both are
normally short, and both are the same class of gap as the mid-read case
rather than a new one. This is the same "truthful, not exhaustive" posture
AC-21/AC-22 already take everywhere else (a repaint can only reflect work
already completed, never work in flight), not a new gap this ruling
introduces.

## Amendment 2026-08-05 — AC-23 covers `--timings` too (product-owner clarification, TASK-0338)

TASK-0338 (ADR-0017) added `--timings`, a diagnostic flag that — as a
necessary side effect, not its main purpose — also disables the live
progress indicator (a stage-close timing line written while the progress
line is mid-redraw would corrupt it). AC-23 as written names only
`--no-progress` as the way progress is suppressed; a literal reading could
leave a future tester wondering whether `--timings` is in or out of scope of
that acceptance criterion.

**Clarification, no behavior change:** AC-23's guarantee — no progress output
reaches stderr, stdout unaffected — applies identically whenever progress is
disabled, regardless of which flag caused it. AC-24's output-only invariant
(results and stdout identical regardless of progress state) already covers
`--timings` on the same reasoning, and needed no wording change since it is
phrased in terms of "progress enabled/disabled," not by flag name.

No new FR or AC: `--timings` remains ADR-0017's diagnostic/engineering
surface, not a PRD-0001 product capability (see that ADR's "Product scope and
unit classification" section).

## Amendment 2026-08-06 — durable run record, stream model, and report paging (product-owner ruling, TASK-0340 + TASK-0341)

The owner ran the shipped binary unattended against a real ~69 000-file corpus
on an SMB-mounted NAS volume (session 32). The run completed and the result was
correct; the two complaints came from *watching* it, not from what it produced:

1. **The report outran the terminal's scrollback.** Enough duplicate groups
   were found that, with stdout not redirected, the earlier part of the report
   could no longer be scrolled back to once the run finished. FR-19 bounds
   *progress* output, but the report body — one block per duplicate group
   (FR-13) — has no size bound at all and is written in one shot at the end
   (TASK-0340).
2. **The stream split "reads backwards" at this scale, and an interrupted run
   leaves nothing.** For the whole run stdout is silent and every visible sign
   of life is on stderr; and because the report is buffered to the very end, a
   run killed, interrupted, or throttled (TASK-0339) before completion leaves
   **zero** stdout output — with the in-flight progress gone from scrollback
   too, unless stderr happened to be redirected as well (TASK-0341).

The two tasks were deliberately routed into one ruling: they are the same
question — what a user actually gets to *see* and *keep* from a long run — and
each one's leading candidate direction was the other's alternative.

**Owner direction, 2026-08-06.** The human product owner gave direct product
direction on this amendment before it was finalized. It is the governing input
for rulings 1–4 and it is quoted verbatim so later readers can check the
rulings against it rather than against a paraphrase:

> "i would like to see a switch/flag that let's one define a file that is
> continuously written to and with that contains all information. i would also
> like to see some form of tracking of elapsed time in the terminal output.
> this normal output should run on stdout; stderr should only carry actual
> errors"

This supersedes three positions an earlier draft of this amendment had taken:
deferring `--output <file>` a second time, deferring incremental emission, and
leaving the stdout/stderr split untouched. Ruling 6 records what was
superseded and why, so the overridden reasoning stays legible instead of
vanishing.

**Ruling 1 — `--output <file>`: a durable run record, written continuously.**
A new flag names a file that `imgdedup` writes **as the run progresses**, not
at the end, and that ends up containing **everything the run produced** — the
duplicate groups as they become known, every warning and error, phase and
elapsed-time milestones, apply-time removals and skips, and finally the
complete canonical result. This closes both field complaints at once: a long
run's results survive the terminal's scrollback (TASK-0340) *and* survive the
run itself being killed, interrupted, or throttled (TASK-0341's durability
gap), which no end-of-run write and no shell redirection can do. It reverses
the 2026-07-19 deferral of `--output <file>` (§4, Amendment 2026-07-19
decision 2) on the owner's direction, and on the substantive ground that
distinguishes it from that decision: the flag deferred then was a convenience
duplicate of `> file`, whereas this one does something redirection
provably cannot. Specified as **FR-20** below; the name `--output <file>` is
owner-confirmed (2026-08-06).

The flag is **opt-in**. Writing a record file into the user's working
directory unasked (the `rdfind`/`rmlint` shape) is still rejected: FR-9's
posture is that a default run changes nothing on disk, and that includes
files the user did not ask for.

**The record file is additional, not a replacement** (owner-confirmed
2026-08-06). Passing `--output` changes nothing about what the terminal shows
or what stdout receives: progress still renders, the report is still printed.
The file is an independent, parallel record — so a user who passes it does not
have to choose between watching the run and keeping it.

**Ruling 2 — what "all information" means, and what the file's format is.**
The record file is a **transcript followed by the authoritative result**, in
one file:

1. A header identifying the tool version, the invocation (roots and flags),
   and the start time.
2. Coarse phase/progress milestones carrying elapsed time — bounded, at the
   same milestone granularity as the non-TTY progress fallback (AC-22), never
   one line per file.
3. Each duplicate group **as soon as it is known to be final**, with its
   keeper and its removable files.
4. Every warning and every error, at the moment it occurs.
5. Apply-time removals and skips, as they happen.
6. On completion, the **complete canonical result document** — byte-identical
   to what stdout receives for the same run.

Only item 6 is covered by **NFR-3**. Items 1–5 are a transcript of a run in
progress: their order follows the order work actually finished, which under
parallel hashing is not guaranteed to be reproducible, and they may report
counts that are still provisional. Saying so plainly is what makes the file
honest — a reader can trust the tail absolutely and read the body as "what had
happened by then". A partial file left by a killed run therefore has items 1–5
and no item 6, which is exactly the signal that the run did not finish.

**Format follows the output mode** (owner-confirmed 2026-08-06). Without
`--json` the file is the human-readable rendering. With `--json` it is
**NDJSON** — one JSON object per line, items 1–5 as event objects and item 6 as
the complete result document on the final line. NDJSON is chosen precisely
because it is valid at every line boundary, so a process killed mid-write
leaves a file whose every complete line still parses; that is the direct answer
to "what is a valid partial JSON document", and it costs stdout's
single-document contract nothing. The cost is accepted with eyes open: this is
a **second machine-readable surface** alongside `--json`'s single document,
with its own event vocabulary to design, document and version (handed to the
architect below).

**Ruling 3 — stream model: normal output on stdout, stderr for genuine errors
only.** Per the owner's direction, the FR-19 rule that progress goes to stderr
in every mode is reversed for the interactive default case, and the S2
stdout-purity contract is **narrowed rather than abolished**. The
**TTY-conditional** reading below is owner-confirmed (2026-08-06):

- **Default (non-`--json`) mode with stdout an interactive terminal:**
  progress, phase and elapsed-time output, and apply-time status lines are
  written to **stdout**. This is the case the owner is describing — the run's
  visible activity is now on the stream a user expects it on.
- **Default mode with stdout redirected or piped:** progress is **not** written
  to stdout. It moves to **stderr**, rendered per stderr's own TTY-ness
  (AC-21/AC-22, unchanged) — still a live in-place line when stderr is a
  terminal, plain milestone lines when it is not. Interleaving progress
  into `imgdedup ./photos > report.txt` would corrupt the documented
  save-a-report workflow and break AC-24 for no benefit — and the durable copy
  a user wants from a redirected run is now ruling 1's job anyway.
- **`--json`, always:** stdout stays **exactly one JSON document and nothing
  else**, on a terminal or not. Progress under `--json` goes to stderr as
  today. This is not deference to the old contract for its own sake: writing
  progress into stdout under `--json` would literally corrupt the document, so
  no reading of the owner's direction can intend it. TASK-0308's schema and
  ADR-0016's plugin consumer are unaffected.
- **stderr carries only genuine failures:** run-level errors (`root_errors`)
  and per-file failures (unreadable file, permission denied, a skip that
  represents an actual problem). Routine status — progress, phase changes,
  elapsed time, "removed X" lines — no longer belongs there. The report's own
  `Warnings:` section is part of the report document and stays on stdout,
  unchanged; live per-file error lines on stderr are the existing pattern
  (`docs/guide/output-and-exit-codes.md`), not a new one.
- **`--timings` is an explicit carve-out** and stays on stderr. It is
  ADR-0017's engineering diagnostic, not "normal output", and it already owns
  stderr to the exclusion of the progress indicator.

So stdout purity now means: **stdout is exactly the result document whenever
stdout is not an interactive terminal, and always under `--json`** — which is
every case NFR-3, AC-20, AC-24, `--json > result.json`, and every scripted
consumer actually depend on. The interactive terminal case, where no script is
reading, is the one that changes. Specified as **FR-21** below, which
supersedes FR-19's "stderr only, in any mode" clause.

**Ruling 4 — elapsed time everywhere in the terminal output, not just during
the walk.** AC-21b already shows elapsed time, but only in scanning's
indeterminate walk state — precisely the ≈10% of the scanning window that is
*not* the long wait. Per the owner's direction this generalizes: **every**
progress render carries elapsed time since the run started, in every phase and
state, and the terminal completion state carries the run's **total** elapsed
time. A remaining-time / ETA estimate is deliberately **not** adopted: this
pipeline's phases have very different per-file costs (hashing dominated a real
corpus at ~86%, TASK-0338), so an extrapolated ETA would be confidently wrong
in exactly the long runs it is wanted for — the same honesty test that
rejected an entries-visited tick on 2026-08-05. Recorded as an amendment to
FR-19 with **AC-32**.

**Ruling 5 — page the human-readable report on an interactive terminal, as a
second, lower-priority slice.** The `git log` pattern still earns its place:
it is the only mechanism that requires **no foresight**, and foresight is what
actually failed in the field (redirection has always existed and is
documented; the owner simply had not decided to use it before starting a
multi-hour run). Ruling 1's record file also requires the user to have decided
in advance, so it does not make paging redundant — but it *does* make it less
urgent, since results are no longer lost either way. Sequencing therefore:
ruling 1 + ruling 3 + ruling 4 ship first as one slice; paging is a separate,
independently shippable slice after it. Specified as **FR-22**, and
deliberately **never applied to `--json`**: routing machine-readable output
through an interactive program is never right, and an agent harness that
allocates a pty (ADR-0016's plugin) would otherwise hang on a keypress it can
never send. Named residual: `imgdedup --json` eyeballed on a terminal can
still overrun scrollback — accepted, since redirection and `--output` both
cover it and `| less` remains available.

**Ruling 6 — what an earlier draft of this amendment ruled, now superseded.**
Recorded rather than deleted, per this project's practice of keeping
overridden reasoning legible:

- *`--output <file>` stays deferred* — **superseded by ruling 1.** The draft's
  argument was that the flag needs the same foresight redirection needs, so it
  would not have prevented what happened. That argument survives for a
  *final-write* output flag and is why paging is still worth having (ruling
  5); it does not survive for a *continuously written* one, which buys
  durability nothing else can.
- *Incremental emission deferred with a trigger* — **superseded by rulings 1
  and 2.** The draft deferred it partly on a suspected NFR-3 conflict. Ruling
  2 dissolves that conflict rather than resolving it: the incremental content
  goes to a file that is explicitly not covered by NFR-3, while stdout keeps
  the canonical, deterministic, complete document. The draft's other objection
  — that a partial prefix misstates end-of-run totals — is answered by
  labelling items 1–5 as provisional and by the absence of item 6 being the
  signal that the run did not finish.
- *The S2 contract stands unchanged* — **superseded by ruling 3.** The draft
  argued the split matches convention (`curl`, `grep`, `git`) and that the
  complaint was about *when* stdout arrives rather than *what* is on it. The
  owner's call is that the interactive experience matters more than the
  convention argument; ruling 3 keeps the part of the contract that scripts
  depend on and gives up only the interactive-terminal case.

**Rejected alternatives**, named so they are not re-proposed:

- **Truncating the report** (show the first *N* groups plus "…and 3 412 more")
  — rejected outright. §5's first user story is seeing *every* set before
  anything changes; silently withholding findings from a safety-critical dry
  run is the one thing this report must not do.
- **Docs/discoverability only** (TASK-0340 direction (c)) — rejected as the
  answer by itself: "remember to redirect" is precisely the instruction that
  was available and did not help.
- **Writing a record file by default, without a flag** — see ruling 1.
- **Streaming the incremental result to stdout** (TASK-0341 direction (b) in
  its original form) — not adopted: it would put a nondeterministic, partial
  document on the one stream that must stay canonical. Ruling 1 gets the same
  durability with none of that cost.

### Requirement text

Rulings 1–4 are confirmed and **live in the PRD body**, not here, so there is
one source of truth: **FR-20** (`--output <file>`) and **FR-21** (stream model)
in §6, the elapsed-time amendment inside **FR-19** in §6, NFR-3's narrowing in
§7, the flag and behavior entries in §8, and **AC-25..AC-32** in §9 (with
AC-20/AC-21/AC-22/AC-23/AC-24 re-scoped in place). §4's `--output <file>`
non-goal is deleted and the Amendment 2026-07-19's decision 2 is marked
superseded there.

Only **FR-22 (paging)** is still tentative and therefore still stated here in
full, together with its acceptance criteria. It is **not** in the body and no
work on it starts until the open question below is answered. This capability is
**two slices** (ADR-0013): FR-20 + FR-21 + the FR-19 amendment first, FR-22
second.

- **FR-22 (Amendment 2026-08-06; TENTATIVE — not folded into §6, pending the
  open question below):** When the human-readable report is written
  to an **interactive terminal**, display it through a **pager**, so that a
  report longer than the screen can be read from its first line and cannot
  overrun the terminal's scrollback. Specifics:
  - Paging engages **only** when stdout is an interactive TTY, and **only** for
    the human-readable report. It never engages under `--json` (ruling 5), and
    never when stdout is redirected or piped.
  - The byte stream handed to the pager is **exactly** the byte stream that
    would otherwise have gone to stdout. Nothing is truncated, summarized,
    reordered, or re-styled.
  - The pager is the one named by **`$PAGER`** when set, otherwise a sensible
    platform default. If no pager is available, or launching it fails, the
    report is written directly to stdout and the run still succeeds.
  - **`--no-pager`** disables paging. `--timings` disables it too, for the same
    reason it disables progress (ADR-0017): both would write to a screen the
    pager owns.
  - A report that **fits on one screen** must not require a keypress to get
    back to the prompt.
  - **Exit codes are the tool's own.** The pager's exit status never replaces
    FR-15's or FR-18's. Quitting the pager before the end of the report is a
    normal outcome: the exit code is unchanged and no broken-pipe/`EPIPE`
    error is reported.
  - Progress (which under FR-21 is on stdout in this case) and every other
    in-flight output are emitted **before** the pager takes the screen.
  - Paging is **output-only**: it never changes what is scanned, grouped, or
    removed. NFR-3 is unaffected — as narrowed by ruling 3 it governs stdout
    whenever stdout is not an interactive terminal, which is exactly the case
    in which no pager runs; the paged bytes are identical either way.

Acceptance criteria for FR-22 (the confirmed criteria AC-25..AC-32 are in §9):

- **AC-33 (FR-22, paging on a TTY):** Given stdout is an interactive terminal
  and a run produces a human-readable report longer than the screen, when the
  run finishes, then the report is displayed through a pager and can be
  scrolled back to its first line, and the bytes displayed are identical to
  those the same run writes when stdout is redirected.
- **AC-34 (FR-22, when it never engages):** Given stdout is redirected or
  piped, or `--json`, or `--no-pager`, or `--timings`, when a run finishes,
  then no pager is launched and the report/JSON is written directly to stdout.
- **AC-35 (FR-22, exit codes):** Given a paged report, when the user quits the
  pager before reaching the end, then the tool exits with its own status — `0`
  for a successful run, `3` for a run with `--exit-code` that found duplicates,
  `1` for a run-level failure — and no broken-pipe/`EPIPE` error is printed.
- **AC-36 (FR-22, no pager available):** Given no usable pager (`$PAGER` unset
  with no platform default present, or the pager fails to launch), when a run
  finishes on an interactive terminal, then the report is written directly to
  stdout in full and the run completes with its normal exit code.
- **AC-37 (FR-22, short report):** Given a report that fits within the
  terminal, when the run finishes, then the user is not required to press a key
  to return to the prompt.

**Fold status.** Done for the confirmed part: §4 **deletes** the
`--output <file>` non-goal (ruling 1) and Amendment 2026-07-19's decision 2 is
cross-noted as superseded there; §5 gains two user stories; §6 gains FR-20 and
FR-21, and FR-19 is amended in place; §7 records NFR-3's narrowing (ruling 3);
§8's Output block gains `--output <file>` and its Behavior list is restated; §9
gains AC-25..AC-32 plus **AC-28a** (mid-run record-write failure) and re-scopes
AC-20/AC-21/AC-22/AC-23/AC-24; §11 gains two risks. The ADR-0018 design pass's
resolutions (2026-08-06) are folded into FR-20, FR-21, AC-22, AC-26, AC-27,
AC-28a and AC-30 — see the handoff status below. **Not folded:** FR-22 and
AC-33..AC-37 (paging), which stay in this amendment until the open question
below is answered.

### Open question — for the human owner, and only for the FR-22 slice

Questions on the flag name, the TTY-conditional stdout rule, the NDJSON record
format, and whether the record file duplicates or replaces terminal output were
**all answered by the owner on 2026-08-06** and are recorded as decided in
rulings 1–3 above. One remains, and it blocks only the second slice:

1. **Paging: default-on or opt-in, and is Windows a blocker?** FR-22 specifies
   paging **on by default** (matching `git`), which means an interactive run
   would end inside a pager waiting for `q` — a visible first-run change
   landing right before the repository goes public (TASK-0337). And if no
   satisfactory default pager exists on Windows, the fallback is "no paging
   unless `$PAGER` is set" — a documented gap against NFR-5.
   **Recommendation: default-on, and accept the Windows gap rather than
   blocking the slice.** Until this is answered, FR-22 and AC-33..AC-37 stay
   out of the PRD body and no paging work starts.

### Handed to the architect

The owner decided *that* the incrementally-written record file happens, so these
were **design** questions, not a feasibility gate.

**Status: the FR-20 and FR-21 questions are closed** by the architect's design
pass ([ADR-0018](../adr/0018-durable-run-record-and-output-stream-routing.md)) and the
owner's rulings on what it surfaced (2026-08-06). What was decided, kept short
so a later reader need not reconstruct it:

1. **Apply-time `trashed:`/`deleted:` lines need no stderr carve-out.** The
   architect flagged that FR-21's failures-only stderr rule leaves them
   "nowhere to go" under `--json`. Ruled: correct as-is — they are routine
   status, already authoritative in the result document's `actions` object
   (FR-14) and captured incrementally by `--output` when it is set. FR-21's
   taxonomy stands unchanged; the reasoning is now written into FR-21 so the
   silence reads as intentional.
2. **`--json` without `--output` has no incremental or durable trace —
   accepted, not a gap.** Recorded in full in FR-21's closing note, including
   why streaming stdout would relocate rather than remove the fragility. This
   is the paragraph that should stop a future reader from "fixing" it.
3. **Mid-run record-write failure: continue, report once, exit `1`.** Folded
   into FR-20 and pinned by the new **AC-28a**, which closes the gap the
   architect named (AC-28 covered only the startup check).
4. Three wording corrections the architect raised as technical, not product,
   changes, all applied: **AC-30** now keys the redirected-stdout fallback off
   **stderr's own** TTY-ness (as written it would have regressed the live
   in-place indicator on `… > report.txt`); **FR-20/AC-27** now distinguish
   document-equivalence from byte-equivalence for the NDJSON completion line
   (compact vs. stdout's pretty-printed form); and **AC-22**'s "TTY-only"
   reasoning is scoped to the indeterminate walk state so it no longer reads as
   contradicting AC-32 or FR-20's always-timestamped record milestones.

The FR-22 questions below remain open, with that slice.

For FR-20 (the record file) — **closed, retained for the record**:

- Can a duplicate group ever be **final** before the whole run completes? Does
  the FR-5 size-cohort partition mean a cohort's groups are settled once that
  cohort's candidates are hashed and byte-verified, or can a
  discovered/hashed-later candidate still join a group already written to the
  file? If the latter, what *can* be honestly written early — and how should a
  later correction be represented?
- What does incremental emission cost the pipeline as it stands — is it local
  to the `sort_canonical`/`render` stages (ADR-0017's span table), or does it
  require restructuring the engine's result model? Note that ruling 2 keeps
  stdout's canonical document intact, so nothing needs to become deterministic
  that is not deterministic today.
- Durability mechanics: what flushing/syncing discipline makes AC-25 true, and
  what exactly survives `SIGINT` vs `SIGKILL` vs a hard power loss? State the
  guarantee the docs can safely make.
- NDJSON design (the format is owner-confirmed, so this is a *how*): the event
  vocabulary, how it versions relative to `schema_version: 1`, whether the
  completion line is literally the existing document or a wrapped form, and
  whether it carries its own stability contract or is explicitly declared
  unstable for now.
- The record file is written **while the tool is scanning** — what happens if
  the user points `--output` at a path inside a scan root? (It is not an image
  so it should be filtered, but the interaction should be reasoned about, not
  assumed.) Likewise: pre-existing file — truncate, refuse, or append?
- Mid-run write failure (disk full, network path lost): abort the run, or
  continue and report? **Resolved** — continue, report once, exit `1`
  (FR-20, AC-28a).

For FR-21 (the stream model) — **closed, retained for the record**:

- Which stream a component writes to now depends on `--json` and on stdout's
  TTY-ness. Where should that decision live so it is made once, and how is it
  tested without a pty for every case?
- Progress currently renders in place using stderr TTY-ness (FR-19). With
  progress on stdout, in-place rendering must key off **stdout**'s TTY-ness —
  confirm there is no case where the two disagree in a way that produces
  control codes in a captured file.
- Re-classify each existing stderr writer as "genuine failure" or "routine
  status": run-level `root_errors`, per-file warnings, apply-time
  `removed:`/skip lines. Flag anything where the classification is genuinely
  arguable rather than deciding it silently. **Resolved** — the apply-time
  lines were the one arguable case; see item 1 above.

For FR-22 (paging) — **still open**, and needed before that second slice:

- Whether `$PAGER` is executed **through a shell** (needed for the common
  `PAGER="less -R"` form, as `git` does) or exec'd directly, and what that
  choice means under ADR-0005's security gate — the pager is a program named
  by user-controlled environment, so the trust model and any injection surface
  should be stated explicitly rather than inherited by analogy to `git`.
- The platform default pager, including the Windows story (feeds the open
  owner question above).
- Mechanics of AC-35 and AC-37: propagating the tool's own exit code around
  the pager, suppressing `SIGPIPE`/`EPIPE` on an early quit, and achieving
  "no keypress for a short report" without depending on one specific pager's
  flags.
- Ordering guarantee for FR-22's "in-flight output is emitted before the pager
  takes the screen", including run-level errors and the apply-stage live
  stream — now that most of that stream is stdout, the same stream the pager
  is about to take.
