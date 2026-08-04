# PRD-0001: Exact image deduplication

- **Status:** approved
- **Owner:** product-owner
- **Created:** 2026-06-13
- **Related:** [roadmap Phase 1](../planning/roadmap.md), [backlog TASK-0101](../planning/backlog.md), [TEMPLATE](TEMPLATE.md); hands open questions to the architect (TASK-0201 technology-stack ADR and design ADRs)

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
- A dedicated `--output <file>` flag for saving results — deferred. Shell
  redirection of stdout (`imgdedup … --json > result.json`) is the supported
  mechanism, because stdout already carries only the report/JSON and all
  diagnostics go to stderr (see the [Amendment 2026-07-19](#amendment-2026-07-19--progress-indication--stored-results)).
- GUI, image preview/thumbnails, EXIF-based logic, config files, and watch/daemon
  modes.

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
- **FR-15:** Exit `0` on success and a distinct non-zero code on error.
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
- **FR-19 (Amendment 2026-07-19; refined 2026-08-01):** While a scan runs, show
  **live progress on standard error** so the user can see the tool is working and
  how far along it is. The indicator conveys both (a) the **overall problem
  size** — the number of candidate files to be checked, known once enumeration
  completes — and (b) **ongoing progress** as `x of y` files done plus a
  **percentage**, labeled with the current phase (e.g. scanning → hashing →
  verifying; user-facing phase names, not an implementation contract). Progress
  is **output-only**: it never changes what is scanned, grouped, or removed
  (like `--json`), and it is written **only to stderr — never to stdout, in any
  mode (including `--json`)**, so redirected stdout stays a pure report/JSON
  document (the S2 stdout-purity contract). It is **enabled by default** and
  **auto-adapts to the terminal**:
  - When stderr is an **interactive terminal (TTY)**, it renders as a **single line
    updated in place** (redrawn as counts advance).
  - When stderr is **not a TTY** (piped, redirected, or captured by CI/logs), the
    in-place updater is **disabled automatically**; instead it emits **occasional
    plain, newline-terminated progress lines** — no carriage returns or ANSI
    control codes — at coarse milestones (phase transitions and bounded percentage
    steps), so logs stay readable and bounded.
  - `--no-progress` **disables progress output entirely** (both the TTY and
    non-TTY paths); nothing progress-related is written to stderr.

  Before enumeration finishes the total `y` is not yet known, so the indicator may
  show a counting/indeterminate state and switch to `x of y` + percentage once the
  denominator is fixed.

  **Denominator across phases (Amendment 2026-08-01).** Scanning runs before the
  total is known, so it renders an indeterminate counting state with no
  denominator; hashing is the first phase with one and reports `x of y` against
  the full candidate count `y`. A later phase whose
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
  governs stdout content only; see §7).

## 7. Non-functional requirements

- **NFR-1 (safety invariant):** The keeper of every group is never trashed or
  deleted, and no run may leave a group with zero surviving copies. No file is
  ever removed without a byte-for-byte content match against its surviving keeper
  (FR-4) re-validated at apply time (FR-16).
- **NFR-2 (no collateral damage):** A non-image is never trashed or deleted;
  magic-byte confirmation (FR-3) precedes every destructive action.
- **NFR-3 (determinism):** For a given filesystem state and flags, results and
  keeper choices are identical across runs and platforms. This guarantee governs
  **stdout content only** (the report / `--json` document); the FR-19 progress
  stream on stderr is explicitly excluded from determinism and parity (Amendment
  2026-07-19).
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
  --exit-code         Exit 3 if any duplicate groups were found (else 0)
  --no-progress       Disable the live progress indicator on stderr
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
- **Progress (Amendment 2026-07-19):** by default a live progress indicator on
  **stderr** shows the phase, `x of y` files, and a percentage while the run works
  (a single line redrawn in place on a TTY; occasional plain lines when stderr is
  redirected). It never writes to stdout, so `imgdedup … > out` and
  `imgdedup … --json > out.json` capture only the report/JSON. `--no-progress`
  turns it off.
- **Errors:** per-file problems are listed and skipped; the overall run still
  completes and summarizes. Exit non-zero only on a run-level failure.

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
- **AC-20 (FR-19, stdout purity):** Given progress is enabled (default), when a
  run completes — in either the default report mode or with `--json` — then
  **stdout contains exactly the report/JSON document and no progress text**; all
  progress output appears on stderr only. Redirecting stdout to a file therefore
  yields a clean report/JSON.
- **AC-21 (FR-19, TTY; amended 2026-08-01):** Given stderr is an interactive
  terminal, when a scan runs, then a **live single-line** progress indicator is
  shown on stderr reporting the current phase and its progress, where: scanning
  renders an indeterminate counting state because the total is not yet known,
  and hashing — the first phase with a denominator — reports `x of y` and a
  percentage against `y` = the count of
  candidate files to check (known after enumeration); a phase whose workload is
  a strict subset of the candidates (verifying) reports its own subset `x of
  <subset-total>` and percentage while keeping the candidate total `y` visible in
  the same line, e.g. `verifying: 1 of 2 (50%) (302 candidates)`; a phase with
  zero workload is skipped entirely rather than shown as `0 of 0`; and the run
  always ends on a terminal completion line rendered against the full candidate
  total, e.g. `done: 302 of 302 (100%)` — the state left on screen once the run
  finishes, so the overall problem size stays visible at completion regardless of
  a smaller intermediate-phase denominator.
- **AC-22 (FR-19, non-TTY fallback; amended 2026-08-01):** Given stderr is
  **not** a TTY (redirected or piped), when a scan runs, then **no in-place
  redraw / carriage-return / ANSI control sequences** are emitted; progress
  instead appears as occasional plain, newline-terminated lines (log-friendly and
  bounded), following the same per-phase-subset-with-total-visible convention as
  AC-21 (and skipping any zero-workload phase) — at minimum a problem-size line
  once enumeration completes and a completion line rendered against the full
  candidate total (e.g. `done: 302 of 302 (100%)`), so a captured log always
  evidences the run's scale and finish.
- **AC-23 (FR-19, opt-out):** Given `--no-progress`, when a scan runs, then **no
  progress output is written to stderr at all**, and stdout is unchanged.
- **AC-24 (FR-19, output-only):** Given identical inputs and flags, the groups,
  keeper, intended removals, and the **stdout report/JSON are identical** whether
  progress is enabled, disabled (`--no-progress`), or on the non-TTY fallback path
  — progress affects neither the results nor stdout (NFR-3 unaffected).

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
