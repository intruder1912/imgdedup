# ADR-0008: Core deduplication algorithm & semantics

- **Status:** accepted
- **Date:** 2026-06-14
- **Deciders:** architect, human owner
- **Related:** [PRD-0001](../prd/PRD-0001-exact-image-deduplication.md)
  (FR-3, FR-4, FR-5, FR-7, FR-8, FR-15, FR-16, FR-17; NFR-1, NFR-2, NFR-3;
  AC-2, AC-9, AC-12, AC-16, AC-17, AC-18), [ADR-0007](0007-technology-stack-rust-clap.md)
  (Rust + clap), [ADR-0009](0009-cross-platform-trash-and-recycle.md)
  (apply-time removal), [ADR-0010](0010-concurrency-and-memory-model.md)
  (parallel/streaming execution of this pipeline)

## Context and problem statement

This ADR is the **heart of the safety model**. It resolves the algorithmic §10
open questions of PRD-0001 — hash choice, byte-verification strategy, the size
pre-grouping optimization, hard-link detection, magic-byte coverage, the keeper
rule and its case-insensitive tie-break, and exit-code semantics — into concrete,
documented decisions.

The overriding force is the prime directive: **never destroy the wrong file**
(NFR-1) and **never touch a non-image** (NFR-2). Every choice below is made so
that a hash collision, a case-insensitive filesystem, a hard link, or a file
changing between scan and apply (TOCTOU, FR-16) **cannot** cause data loss. A
secondary force is **determinism** (NFR-3): identical filesystem state and flags
must yield identical groups and keepers across runs and platforms.

These decisions were worked through and confirmed with the human owner. Where a
question is genuinely a *product* decision (the duplicates-found exit code), it is
flagged back to the product-owner rather than locked here.

## Decision drivers

- **Safety invariant (NFR-1):** no removal without a byte-for-byte match against
  the surviving keeper, re-validated at apply time.
- **No collateral damage (NFR-2):** content-confirmed images only.
- **Determinism (NFR-3):** stable groups and keeper choice across runs/platforms,
  including on case-insensitive filesystems.
- **Scale (NFR-4):** the pipeline must avoid needless reads and whole-file
  buffering (the *how* of execution is in [ADR-0010](0010-concurrency-and-memory-model.md)).
- **Correctness equivalence (AC-9):** optimizations must not change results.

## Considered options

The cross-cutting decision is the overall **pipeline shape**:

1. **Size pre-group → hash within size groups → group by hash → byte-verify
   against keeper → re-validate at apply.** (chosen)
2. **Hash every candidate, group by hash, trust the hash for removal.** Simplest,
   but trusts a hash to authorize deletion — violates FR-4/NFR-1.
3. **Pairwise byte-compare every candidate.** Maximal safety, O(n²) reads — fails
   the scale driver (NFR-4).

Per-decision options are weighed in "Pros and cons" below.

## Decision outcome

**Chosen: option 1** — a layered pipeline where each cheaper stage is only a
**pre-filter** for the next, and the final byte comparison is the sole authority
for removal. The pipeline:

```text
enumerate candidates (FR-1/FR-2/FR-6)
  → magic-byte confirm each is an image (FR-3, NFR-2)
  → group by exact byte size (FR-5)
  → within each size group: hash contents (FR-4 pre-filter)
  → group by hash
  → collapse hard-link/identity sets (FR-17)
  → choose keeper per group (FR-7/FR-8)
  → byte-for-byte verify each non-keeper against the keeper (FR-4, the authority)
  → report (dry-run default) / apply with re-validation (FR-16)
```

### Hash algorithm — BLAKE3

**Chosen: BLAKE3**, used strictly as a **pre-filter** (FR-4), not as deletion
authority.

- BLAKE3 is fast, **internally parallelizable** (its tree structure lets a single
  large file be hashed across cores — see [ADR-0010](0010-concurrency-and-memory-model.md)),
  and has a well-maintained native Rust implementation (`blake3`), fitting the
  Rust stack (ADR-0007) and the NFR-4 scale target.
- Because FR-4 mandates a **byte-for-byte verification** before any removal, the
  hash never needs to be *cryptographically collision-proof*: a collision can at
  most create a false *candidate* pair, which the byte comparison then rejects. A
  collision can therefore **never** cause data loss (NFR-1). BLAKE3 is in fact
  cryptographically strong, but we rely only on its speed and good distribution
  here. (SHA-256 would also be correct but is slower and not tree-parallel; a
  non-cryptographic hash like xxHash was rejected only on principle-of-least-
  surprise, since BLAKE3 is both fast *and* strong.)

### Pipeline & size pre-grouping (FR-5) — and why it is equivalence-safe (AC-9)

Candidates are first grouped by **exact byte size**; files are hashed and compared
**only within a same-size group**. This is a pure optimization that **does not
change the resulting groups** versus hashing every candidate (AC-9), and the proof
is simple:

- Exact duplicates are byte-for-byte identical, so duplicates **necessarily have
  identical byte size**. Two files of different sizes can never be exact
  duplicates.
- Therefore partitioning by size never separates two files that would otherwise
  group together: every true duplicate pair already shares a size bucket. Size
  pre-grouping only **removes comparisons that were guaranteed to fail**, leaving
  the final groups identical to the naive approach. AC-9 is an explicit test.

A singleton size group (only one file of that size) can never contain a duplicate
and is dropped without hashing — the dominant saving on real libraries.

### Byte-verification strategy (FR-4, FR-16) — the authority

A hash match makes two files *candidates*; **byte-for-byte comparison is what
authorizes removal**. Strategy:

- Within a hash group, verify **each non-keeper against the group keeper** (a star
  comparison, not all-pairs): if every non-keeper equals the keeper, the whole
  group is verified-equal by transitivity, in O(n) comparisons rather than O(n²).
- Comparison is **streamed** with a **bounded buffer** (chunk-by-chunk reads of
  the keeper and the candidate in lock-step), **short-circuiting on the first
  differing byte** or on a length mismatch. No file is ever buffered whole in
  memory (NFR-4; memory model in [ADR-0010](0010-concurrency-and-memory-model.md)).
- If any candidate fails verification (a genuine hash collision, or a mid-scan
  change), it is **excluded from the removal set** for that group and reported; it
  is never removed on a hash match alone.

**Re-validation at apply time (FR-16, AC-17).** The scan and `--apply` are
separate passes, so immediately before trashing/deleting each non-keeper the tool
**re-validates**:

1. the file **still exists** and is **still a regular file** — explicitly **not a
   symlink** that was swapped in since the scan (a TOCTOU defense, FR-6/FR-16);
2. the **keeper still exists** and is unchanged;
3. the non-keeper **still byte-for-byte matches** the keeper.

If any check fails, that file is **skipped with a warning** and **not removed**;
the rest of the run proceeds (NFR-6, AC-17). This re-validation, combined with the
fact that the keeper is *never* in the removal set, guarantees NFR-1: no run can
leave a group with zero surviving copies.

### Hard-link / file-identity detection (FR-17, AC-18)

Files that are hard links to the same underlying content share storage, so
removing one frees nothing and may discard a path the user considers distinct.
Such files are **already deduplicated**:

- **Identity key:** on **POSIX**, the `(device, inode)` pair (`st_dev`, `st_ino`)
  via `std::os::unix` metadata. On **Windows**, the volume serial number plus file
  ID (file index) via `GetFileInformationByHandle` (the
  `BY_HANDLE_FILE_INFORMATION` `dwVolumeSerialNumber` + `nFileIndex{High,Low}`),
  surfaced through Rust's Windows file-metadata APIs.
- Within a verified-duplicate group, files sharing an identity key are collapsed:
  **at most one** representative of each identity set is ever a removal candidate,
  and **identical-identity files are never proposed for removal, never trashed or
  deleted on that basis, and are excluded from the reclaimable-space figure**
  (FR-13/FR-17). They may appear as an informational note. AC-18 tests this.

### Magic-byte coverage (FR-3, NFR-2) — use the `infer` crate

Every default allow-list extension must be confirmable by content signature before
the file is grouped or acted on. We use the maintained **`infer`** crate rather
than a hand-rolled signature table:

- **Coverage required** (default allow-list, FR-2): JPEG (`jpg`/`jpeg`), PNG,
  GIF, WebP, BMP, TIFF (`tif`/`tiff`) — `infer` recognizes all of these via their
  documented magic bytes (e.g. JPEG `FF D8 FF`, PNG `89 50 4E 47 0D 0A 1A 0A`, GIF
  `GIF87a`/`GIF89a`, WebP `RIFF....WEBP`, BMP `42 4D`, TIFF `49 49 2A 00` /
  `4D 4D 00 2A`).
- **Why a crate over hand-rolling:** the signatures (especially container formats
  like WebP/RIFF and TIFF's two endiannesses) are exactly the kind of fiddly,
  security-sensitive parsing where a maintained, widely-used, well-tested library
  reduces the risk of a wrong-positive (treating a non-image as an image — NFR-2)
  or a missed format. `infer` only reads a small header prefix, fitting the
  streaming model. A focused test asserts each allow-listed format is detected and
  that a mislabeled non-image (e.g. text named `photo.jpg`, AC-4) is rejected.
- Files that **fail** content confirmation are **ignored and never removed**
  (FR-3, NFR-2) — they are dropped before grouping, so they cannot enter a group.

### Keeper rule + case-insensitive tie-break (FR-7/FR-8, NFR-3, AC-2/AC-6/AC-16)

Exactly one keeper per group, chosen by a **total, deterministic order**. The
`--keep` flag picks the **primary** criterion; the remaining criteria always break
ties **in a fixed order**, so the outcome is total and reproducible:

- **`--keep shortest` (default):** **fewest path segments** (shallowest), then
  **fewest characters** in the full path → then **oldest mtime** → then
  **lexicographic path order**.
- **`--keep oldest`:** **oldest mtime** → then shortest (segments, chars) → then
  lexicographic path.
- **`--keep newest`:** **newest mtime** → then shortest (segments, chars) → then
  lexicographic path.

The **lexicographic / case tie-break** (PRD §10) is defined unambiguously so the
result is deterministic even on case-insensitive filesystems and across platforms
(NFR-3):

- Paths are first **normalized to a canonical absolute form** (resolved against
  the scan root) and compared as **Unicode scalar sequences** over their **raw
  bytes/codepoints — a case-sensitive, byte-wise ordering** (`'A'` < `'a'`). We do
  **not** fold case for the tie-break: case-folding is locale-dependent and would
  reintroduce non-determinism across platforms, the opposite of NFR-3.
- On a case-insensitive filesystem two paths differing only in case denote the
  **same file**, so they collapse via the hard-link/identity check (same identity
  key) before the tie-break is ever reached — the case-only ambiguity cannot
  produce two distinct keepers. Where case-differing paths denote genuinely
  distinct files (case-sensitive filesystem), the byte-wise order gives a single,
  platform-independent winner.
- mtime is compared at a **fixed resolution** (whole seconds) to avoid
  sub-second/filesystem-granularity nondeterminism; an mtime tie falls through to
  the lexicographic rule, which is total, so the overall order is **always
  total** — there is never a "two equally-valid keepers" case (AC-2, AC-16).

### Exit-code semantics (FR-15, FR-18, AC-12, AC-19)

A concrete, documented scheme:

- **`0`** — success: a clean run, whether or not duplicates were found, including
  a successful `--apply`/`--delete` and a successful dry run. This is the default
  for any successful run regardless of findings.
- **`1`** — **run-level error**: an invalid invocation (e.g. `--delete` without
  `--apply`, AC-12), an unreadable scan root, or any failure that prevents the run
  from completing meaningfully. Per-file errors are *skipped and reported*, not
  run-level (NFR-6), and do not by themselves change the exit code from `0`.
- **`2`** — reserved for clap's argument-parsing errors (its convention), kept
  distinct from our `1`.
- **`3`** — **duplicates found**, emitted **only** when the opt-in `--exit-code`
  flag is set and the run completed successfully with **at least one duplicate
  group** (`0` if none were found). Without `--exit-code`, a successful run is
  always `0`.

**Decided with the owner (resolves PRD §10).** The duplicates-found signal is
**opt-in via `--exit-code`**, modelled on `git diff --exit-code` / `black
--check`: it lets a script or CI job gate on "duplicates present" without parsing
`--json`, while the **default stays `0`** so the common case never surprises a
`set -e` shell. The flag affects **only the exit code** — never what is scanned,
grouped, or removed — so it cannot interact with the safety invariant (NFR-1) or
determinism (NFR-3). It is meaningful in any mode; its primary use is the dry-run
CI check (AC-19).

### Consequences

- **Positive:** layered pre-filters make the common case cheap (size groups, then
  hash) while the byte comparison keeps the destructive path provably safe (NFR-1);
  hard-link and TOCTOU handling close the two subtlest data-loss/space-misreport
  risks; the keeper order is total and platform-stable (NFR-3); magic-byte
  confirmation via `infer` keeps non-images out of every group (NFR-2).
- **Negative / trade-offs:** byte verification re-reads duplicate content (bounded
  by *duplicate* volume, not total volume — only same-size, same-hash files are
  ever compared); apply-time re-validation costs a second stat/compare per
  non-keeper, accepted as the price of TOCTOU safety.
- **Follow-ups / new tasks created:** Phase 3 implementation tasks (enumerate,
  sniff, size-group, hash, byte-verify, identity, keeper, report/apply) are seeded
  from this pipeline after this PR. **Product-owner decision needed** on the
  duplicates-found exit code (code `3` reserved).

## Pros and cons of the options

### Option 1 — layered pre-filter pipeline (chosen)

- Good: cheap common case; byte comparison is the sole removal authority (NFR-1);
  size pre-grouping is provably result-preserving (AC-9); each stage is a small,
  independently testable unit.
- Bad: more moving parts than naive hashing; verification re-reads duplicate bytes.

### Option 2 — hash-only, trust the hash

- Good: simplest; one read per file.
- Bad: **authorizes deletion on a hash match** — directly violates FR-4/NFR-1; a
  collision (however unlikely) becomes data loss. Rejected outright.

### Option 3 — all-pairs byte comparison

- Good: maximal certainty without any hashing.
- Bad: O(n²) reads — fails the NFR-4 scale target on large libraries.

## Links

- [PRD-0001](../prd/PRD-0001-exact-image-deduplication.md) §6/§7/§9/§10 — the
  requirements and open questions resolved here.
- [ADR-0007](0007-technology-stack-rust-clap.md) — `blake3` and `infer` crate
  choices follow from the Rust stack.
- [ADR-0009](0009-cross-platform-trash-and-recycle.md) — how verified non-keepers
  are removed (trash vs. delete).
- [ADR-0010](0010-concurrency-and-memory-model.md) — how this pipeline executes in
  parallel with bounded memory and a canonical sort for determinism.
- BLAKE3 specification; the `infer` crate; POSIX `stat`
  `st_dev`/`st_ino`; Win32 `GetFileInformationByHandle`.
