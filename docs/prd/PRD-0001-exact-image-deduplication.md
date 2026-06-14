# PRD-0001: Exact image deduplication

- **Status:** accepted
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

## 7. Non-functional requirements

- **NFR-1 (safety invariant):** The keeper of every group is never trashed or
  deleted, and no run may leave a group with zero surviving copies. No file is
  ever removed without a byte-for-byte content match against its surviving keeper
  (FR-4) re-validated at apply time (FR-16).
- **NFR-2 (no collateral damage):** A non-image is never trashed or deleted;
  magic-byte confirmation (FR-3) precedes every destructive action.
- **NFR-3 (determinism):** For a given filesystem state and flags, results and
  keeper choices are identical across runs and platforms.
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
- **AC-19 (FR-18):** Given `--exit-code`, when a scan finds duplicate groups the
  tool exits `3`; when it finds none it exits `0`; and without `--exit-code` a
  successful run exits `0` even when duplicates are present.

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
