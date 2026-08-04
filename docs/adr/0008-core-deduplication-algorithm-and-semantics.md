# ADR-0008: Core deduplication algorithm & semantics

- **Status:** accepted
- **Date:** 2026-06-14 (amended 2026-06-15: hard-link **anchor rule**)
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

#### Consequence note — a fourth check, and the residual window (2026-08-01, S3)

Implementing the apply stage (TASK-0309) added a **fourth** check the list above
does not mention, and the architect review of the slice asked for it to be
recorded here rather than living only in the code:

**Check 4 —** the non-keeper and the keeper are **not the same physical file**. A
hard link created between the scan and the apply would pass check 3 while meaning
the candidate *is* the keeper under another name. FR-17 promises hard-linked
siblings are never removal candidates, and that promise has to hold at apply
time, not just at scan time. In practice this is also the check that makes
overlapping or repeated scan roots (`imgdedup ./d ./d/sub`) safe.

**The residual window.** The claim above that a file changing between scan and
apply "cannot cause data loss" needs one qualification. All four checks are
driven from handles the apply stage opens itself, so they are mutually consistent
— but the removal call is necessarily **by path**, because that is the only
interface the OS trash offers. Between the last check and that call the path can
still be re-pointed. The window is a single system-call gap rather than the span of a
byte-for-byte comparison, and nothing in it can remove the keeper (its path is
never handed to the remover), but it is not zero and should not be described as
though it were. What re-validation buys is protection against the realistic
threat — a tree that drifted during a long scan — not a guarantee against a
deliberate adversary racing the process.

### Hard-link / file-identity detection (FR-17, AC-18)

Files that are hard links to the same underlying content share storage, so
removing one frees nothing and may discard a path the user considers distinct.
Such files are **already deduplicated**:

- **Identity key:** on **POSIX**, the `(device, inode)` pair (`st_dev`, `st_ino`)
  via `std::os::unix` metadata. On **Windows**, the volume serial number plus file
  ID (file index) via `GetFileInformationByHandle` (the
  `BY_HANDLE_FILE_INFORMATION` `dwVolumeSerialNumber` + `nFileIndex{High,Low}`),
  surfaced through Rust's Windows file-metadata APIs.
- Within a verified-duplicate group, files sharing an identity key are collapsed
  to a single representative `FileRec`; the collapsed siblings are recorded as an
  informational note (`HardLinkNote`) and **identical-identity files are never
  proposed for removal, never trashed or deleted on that basis** (FR-13/FR-17).
  AC-18 tests this.

#### Anchors: a collapsed representative with surviving siblings is never removable

The collapse alone is **not sufficient**. Collapsing an identity set to one
representative leaves that representative free to be chosen as a *removable
non-keeper* of its group — but removing it frees **nothing**, because its
surviving in-scope siblings still pin the underlying storage. (Concretely:
`a.jpg` ↔ `a-link.jpg` are one inode; `a-copy.jpg` is a byte-identical separate
inode. Collapsing `{a.jpg, a-link.jpg}` to one representative and then letting
`--keep shortest` make `a-copy.jpg` the keeper would propose removing the
representative of the hard-linked inode — yet `a-link.jpg` still pins it, so 0
bytes are freed, contradicting the intent above.) This ADR resolves that
intent-vs-mechanism gap with the **anchor rule**: anchors are included in keeper
selection so that `--keep` ranks all representatives, but if a standalone wins,
anchors are protected to preserve the hard-link set.

- **Definition.** A collapsed representative whose identity set had **≥2 scanned
  members** — i.e. it absorbed at least one surviving in-scope sibling that is
  *never* removed — is an **anchor**. Equivalently: the representative carries a
  non-empty `HardLinkNote.siblings` list. A representative that is the **only
  scanned link** to its inode (no surviving sibling) is **not** an anchor and is
  an ordinary removal candidate.
- **Rule.** An anchor is **never a removal candidate**. The user's `--keep` rule
  is applied **only among the genuinely-removable** members of the group (the
  non-anchored, single-in-scope-link `FileRec`s) to choose the **one keeper**.
  Anchors are reported as **protected** members of the group — neither the keeper
  (KEEP) nor removable (REMOVE) — and are excluded from the reclaimable figure.
  This forces every anchor to the "kept" side without making it *the* keeper, so
  the group always has **exactly one keeper, plus zero or more protected
  anchors, plus zero or more removable copies**, and no surviving link is ever
  proposed for removal.
- **Reclaimable accounting.** The reclaimable-space figure counts **only the
  sizes of files in the removable set** — bytes that actually free storage when
  trashed/deleted. An anchor's size is **never** counted: removing one of its
  paths leaves the inode (and its bytes) alive via the surviving sibling.

##### Edge-case rulings

1. **Group of only hard-link sets (every distinct inode is multiply-linked
   in-scope).** Every representative is an anchor, so there is **no
   genuinely-removable member** and therefore **no keeper to choose** and
   nothing to remove. The group emits **no `KEEP`/`REMOVE` rows and 0
   reclaimable bytes**; it is surfaced **only** as an informational
   "already-deduplicated" note listing each inode and its links. Rationale: an
   inode can only be reclaimed by removing **all** its in-scope links, and we
   never remove a surviving sibling, so no inode in such a group is ever
   reclaimable. This case must not be reported as a removal opportunity.

2. **Standalone copies plus one (or more) hard-link set(s).** The `--keep` rule
   ranks **all representatives** (both anchors and standalones) to find the minimum.

   - If the minimum is an **anchor**, that anchor is the single keeper (KEEP), other
     anchors are protected (PROTECT), and all standalone copies are removable and
     verified against the anchor keeper. Rationale: an anchor inode always survives
     (by definition, its sibling pins it), so every standalone copy in the same
     content group is genuinely redundant and safely removable.

   - If the minimum is a **standalone**, that standalone is the single keeper (KEEP),
     other standalones are removable, and every anchor is protected (PROTECT). In
     this case, reclaimable bytes may legitimately be 0 (the user chose to keep
     the standalone while the anchor cannot be removed), but the report must
     clearly explain this rather than saying "nothing to reclaim" flatly.

3. **Determinism (NFR-3).** The anchor rule is a **total, deterministic
   partition** computed before keeper selection: "is this representative an
   anchor?" is a pure function of the (canonically-ordered) identity sets, with
   no dependence on thread or hash-map order. Keeper selection then runs the
   existing total `--keep` order over the removable subset (`min_by`), so the
   keeper is total and reproducible. mtime is still compared at whole-second
   resolution; the lexicographic fall-through still guarantees no "two equally
   valid keepers" case.

4. **Link count vs "had ≥2 in-scope members".** The engine uses the **in-scope
   member count it already tracks** (a representative is an anchor iff it
   absorbed ≥1 collapsed sibling — the existing `HardLinkNote.siblings`
   non-emptiness). It does **not** stat the OS link count: links **outside the
   scanned scope** are intentionally **out of scope** for this figure. This is an
   honest, documented limitation — if an inode has additional links the scan
   never saw, removing every *in-scope* link of a non-anchored representative may
   still free 0 bytes on disk. We do not chase out-of-scope links (it would
   require a second stat per file and still be racy); the reclaimable figure is
   defined as **"bytes freed assuming the scanned scope is the whole world for
   that inode"**, and this assumption is stated in the user guide. Within the
   scanned scope the rule is exact.

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

Exactly one keeper per group, chosen by a **total, deterministic order** among
the group's **genuinely-removable** members (anchors are excluded from the
candidate set per the anchor rule above). The `--keep` flag picks the
**primary** criterion; the remaining criteria always break ties **in a fixed
order**, so the outcome is total and reproducible:

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
- **`1`** — **run-level error**: an unreadable/nonexistent scan root, or any
  filesystem-state failure that prevents the run from completing meaningfully.
  Per-file errors are *skipped and reported*, not run-level (NFR-6), and do not
  by themselves change the exit code from `0`. **Amended 2026-08-03 — see
  below:** a refused flag combination (e.g. `--delete` without `--apply`) moved
  to `2`; it is no longer an example of `1`.
- **`2`** — **usage error**: clap's own argument-parsing errors (its
  convention), and, as of the 2026-08-03 amendment below, this binary's own
  post-parse semantic validation failures (a syntactically valid but refused
  flag combination) — both mean "the invocation itself was wrong."
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

## Amendment 2026-06-15 — hard-link anchor rule

An end-to-end test of slice S1 (`detect-report`) exposed an unspecified
interaction between hard-link collapse and the keeper rule: a collapsed
representative of a multiply-linked inode could be chosen as a *removable*
non-keeper, contradicting this ADR's own stated intent ("removing one frees
nothing"; identity files "never proposed for removal"; excluded from
reclaimable). The mechanism ("at most one representative is ever a removal
candidate") permitted exactly that.

This amendment (approved by the human owner) adds the **anchor rule** to the
"Hard-link / file-identity detection" section above: a collapsed representative
with surviving in-scope siblings is an **anchor**, never a removal candidate;
`--keep` chooses the single keeper among the **genuinely-removable** members
only; anchors are reported as protected (neither KEEP nor REMOVE) and excluded
from reclaimable space. The four edge-case rulings (all-anchor groups,
standalone-plus-anchor groups, determinism, in-scope-only link counting) are
recorded there. Implementation lands under TDD as a follow-up to TASK-0304 /
TASK-0305 / TASK-0307; the in-scope-only limitation is documented in the user
guide.

## Amendment 2026-08-03 — a refused flag combination is a usage error (TASK-0323)

The S4 acceptance suite found the code and the user guide disagreed:
`imgdedup <path> --delete` (without `--apply`) exited `1` in the binary, but
`docs/guide/output-and-exit-codes.md` had always documented this case as `2`
("usage error — invalid arguments"). PRD-0001's AC-12 only requires
"non-zero," so neither side was a spec violation — but the two disagreed with
each other, and this ADR's original exit-code scheme above sided with the
code, listing `--delete` without `--apply` as an example of `1`.

**Owner ruling: `2`.** A refused flag *combination* is a usage error, the same
bucket as clap's own parse-time argument errors — not a run-level failure like
an unreadable scan root. The invocation itself was wrong in both cases; only
one of them happens to be caught by clap's own parser rather than this
binary's post-parse `cli::validate()`, and that implementation detail should
not put the two cases in different exit-code buckets.

**Consequence:** `1` narrows to genuine run-level/filesystem-state failures
only (currently: an unreadable or nonexistent scan root). `2` now has two
sources — clap's own errors and this binary's semantic validation — sharing
one meaning ("usage error") rather than one mechanism. `exit_code::CLAP` was
renamed to `exit_code::USAGE` to reflect that it is no longer clap-exclusive.
No other current validation rule exists to exercise the second source beyond
`--delete` without `--apply`, but any future one added to `cli::validate()`
follows the same bucket by construction.

## Amendment 2026-08-03 — overlapping/repeated scan roots are no longer double-enumerated (TASK-0319)

The S3 review (Consequence note above, "Check 4") established that overlapping
or repeated scan roots (`imgdedup ./d ./d`, `imgdedup ./d ./d/sub`) are **safe**
— the apply-time identity check never removes a file just because it was
walked twice. It did not make them **accurate**: before this fix, each
occurrence of the same physical file became its own `Candidate` from
`enumerate.rs`, so the engine's identity collapse (this document's hard-link
handling) correctly-but-misleadingly reported a single file as a hard-link set
of one file to itself — `hard-linked: a.png, a.png` — and the dry-run summary
read "no duplicate groups to remove; 1 already-deduplicated hard-link set"
even when the tree had never actually been compared against anything. Fail-safe
(nothing was removed or at risk), but a user reading the dry-run report could
reasonably conclude their tree was clean when it had not been checked.

**Fix: deduplicate at the root list, before enumeration walks anything.**
`enumerate::dedupe_roots` canonicalizes the (normally small) list of
user-supplied scan roots and drops any root that is an exact duplicate of, or
nested inside, another given root — regardless of argument order. This is
enumeration-stage, not engine-stage: it operates on the handful of root
arguments, never on individual candidate files, so it adds no per-file
traversal cost and does not touch `collapse_identity`'s NFR-4 handle-scoping
invariant. A root that cannot be canonicalized (e.g. it does not exist) is
left untouched, so the existing root-error path is unaffected. Check 4 at
apply time is now a second, independent safety net rather than the sole
guard against this scenario — both are correct and neither depends on the
other.

**Nesting is only redundant if the ancestor's own walk actually reaches it**
(architect review, same day, `covers()`). Two of this document's own scope
rules can make that false, and treating a nested root as covered anyway would
silently drop files the user explicitly named — the same class of harm this
fix exists to prevent:

- **`--no-recurse`** caps every root's walk at its own immediate children
  (depth 1, "Scope rules" above). A depth-1 walk still reaches an ancestor's
  direct-child *files*, so a nested root that is a real, existing file
  directly inside its ancestor still collapses — but a nested *directory*
  root, or a file two or more levels down, does not: the ancestor's capped
  walk never reaches either, so both are kept and scanned independently.
- **Hidden entries** are skipped by default (FR-6) — except a hidden path
  passed *explicitly* as a scan root, which is always included
  (`docs/guide/scanning-and-scope.md`). A nested root that is hidden relative
  to its ancestor is kept rather than collapsed, unless `--hidden` is also
  given — otherwise the ancestor's own walk would silently un-opt-in it.

Both exceptions are keyed off the *shared* `ScopeOptions` already threaded
through enumeration (`no_recurse`/`include_hidden` apply uniformly to every
root in one invocation), so `dedupe_roots` takes `&ScopeOptions` alongside the
root list rather than deciding blind.

**A single "last kept ancestor" is not enough** (second architect review
round, same day). A nested root that `covers()` correctly refuses to
collapse — e.g. an explicit hidden root — must still be kept in mind as a
potential ancestor for *its own* descendants, without shadowing the original,
shallower ancestor from an unrelated later sibling. `dedupe_roots` tracks a
**stack** of still-relevant kept ancestors (popped down to the ones that are
still a prefix of the current path in sorted order) rather than a single
slot, checking only the top entry — provably sufficient, since in
`--no-recurse` mode only an exact parent can cover a path (no shallower stack
entry could either), and in recurse mode the hidden-component search that
`is_hidden_relative_to` performs only grows as the candidate ancestor gets
shallower, so if the closest one fails the shallower ones would too.

**Performance: sort, don't compare all pairs.** An initial all-pairs
`starts_with` comparison across the root list is O(n²) — for the common case
of a handful of roots this is free, but a shell glob like `imgdedup */` over a
directory with thousands of subdirectories is a realistic invocation, and
review measured a 25× slowdown at 20,000 roots (73 s vs. 2.97 s on `main`) from
a purely mechanical pre-pass sitting behind a frozen `scanning...` indicator.
`dedupe_roots` instead sorts the canonicalized roots once — `Path`'s `Ord` is
lexicographic over components, so a path's descendants occupy a contiguous
range immediately after it in sorted order, the standard prefix-dedup
property — and does a single linear pass over the sorted list maintaining the
ancestor stack above. The sort is stable, so exact duplicates keep their
original relative order (first occurrence wins) for free.

**Known residual gaps, both third-round review, neither a regression from
this fix:**

- An ancestor directory unreadable at scan time (e.g. `--x` permissions)
  drops an explicitly-named nested root as redundant, then fails to open the
  ancestor itself — the named file is never scanned, but the run exits `1`
  with a clear stderr error, not a silent miss, so the class of harm this
  fix targets does not occur. Pre-verifying ancestor readability before
  dedup would be over-engineering for a case that already fails loudly.
- `--follow-symlinks` can still reproduce the *symptom* this task fixes
  (`hard-linked: <path>, <path>` for a real duplicate) via filesystem
  aliasing rather than root-list redundancy — e.g. one scan root contains a
  symlinked directory pointing at content also reachable another way. This
  is reachable with a **single** root and no repeated/nested root arguments
  at all, so no change to `dedupe_roots` can fix it; it belongs with
  TASK-0320's `--follow-symlinks` candidacy ruling.

## Amendment 2026-08-04 — `--follow-symlinks` alias collisions are no longer misreported as hard links (TASK-0320)

The residual gap TASK-0319 left open (immediately above) — aliasing **within
a single scan root's own walk** — is closed here, at the same enumeration
stage and for the same reason: **one physical file, one
candidate.** (Aliasing *between* two different scan roots is a distinct case
that remains open; see residual gap (1) at the end of this amendment.)
Under `--follow-symlinks`, a symlinked ancestor directory or a
leaf symlink can give a physical file a second route within a single scan
root's own walk — reproduced with a real binary before any fix: `A/real_dir/
photo.png` and `A/link_dir -> real_dir` made `A/link_dir/photo.png` and `A/
real_dir/photo.png` two separate `Candidate`s for what is, on disk, **one**
file with a single directory entry. This is categorically different from a true hard link (two
independent directory entries naming one inode) — the identity check this
document's hard-link handling uses (`same-file::Handle`, which follows
symlinks) cannot tell the two apart, so it labeled the alias a hard-link set
and reported it as `already_deduplicated`.

**Worse than a labeling error: it could mask a genuinely reclaimable
duplicate.** With a *second*, truly separate physical copy of the same
content elsewhere in the tree, the anchor rule (Amendment 2026-06-15) forces
**every** representative sharing that identity onto the protected side —
correct for a true hard link (removing one of its names reclaims zero bytes,
the data stays allocated while any name remains), but wrong here, since
`A/real_dir/photo.png` is not a hard-link name at all, just the sole real
directory entry reached twice. The group reported `removable: []`,
`reclaimable_bytes: 0`, when the genuine second copy was real, removable
space. Product-owner ruling (2026-08-04): a defect (AC-13/AC-18/AC-17), not a
labeling nit, and not fixable by special-casing the anchor rule in
`collapse_identity` — that would leave the phantom candidate in the set,
still sniffed, still hashed, still a group member, with every downstream
consumer needing to learn to ignore it. The phantom must not exist;
enumeration is the stage that owns this invariant.

**Fix: canonicalize and collapse aliasing candidates within one root's walk,
never across roots** (the backlog's own framing: this is filesystem-identity
aliasing within a single root, not root-list redundancy, so it does not
belong in `dedupe_roots`). `enumerate::dedupe_symlink_aliases` runs once per
deduped root, only when `--follow-symlinks` is set **and** that root's own
walk actually visited a symbolic link (the scenario is structurally
impossible otherwise, so neither the default path nor a symlink-free tree
pays anything — architect review measured the pass at ~0.7 s on 30k files at
depth 4, warm cache, i.e. 100% of the `--follow-symlinks` wall-clock delta,
which the guard returns to zero when no symlink is present): every candidate's
canonical form is computed; candidates that canonicalize to the same real path
name **one** physical file reached by more than one route — there is no second
name for the data, so removing the "duplicate" would remove the file itself —
and are collapsed to one survivor. Candidates whose canonical
form cannot be determined (a broken link, an unreadable ancestor) are kept
**unchanged** — fail-open, mirroring `dedupe_roots`: only ever drop what is
*proven* redundant. A true hard link's two paths each canonicalize to
themselves (no symlink hop is needed for either), so they are never grouped
here and continue through the existing, unmodified anchor-protection path.

**Rejected alternative (the wholesale-drop reading of "architect prefers
dropping them"): dropping symlink-derived candidates from candidacy
entirely.** That would contradict AC-14 as already specified and tested
(`follow_symlinks_flag_includes_symlink_targets` — a symlink whose target
lives *outside* every scan root and is not independently a candidate must
still be included), and would make `--follow-symlinks ./library` report
nothing at all for an ordinary link farm. The fix scope is narrower and only
fires on a **proven alias collision** — two candidates resolving to one
canonical identity — never on a lone symlink whose target is not otherwise in
scope.

**Which alias survives is never decided by walk order (NFR-3/AC-16).** In
order: (1) a candidate reached with no symlink hop anywhere below the root —
its real, non-aliased location — if exactly that kind exists among the
colliding candidates; (2) failing that (every route passes through a
symlink), the existing FR-7 `shortest` order (segments → chars → oldest
mtime → lexicographic path), reused via `keeper::cmp_shortest_by` rather than
inventing a second ordering. "Direct" is tested **relative to the root's own
canonical form**, not the process's absolute view of the filesystem — a
symlink several levels *above* the scan root (e.g. a `/tmp` mount that is
itself a symlink, as on macOS) must never make an otherwise-ordinary file
look "routed"; comparing everything against the root's own canonicalization
neutralizes that irrelevant prefix.

**Accepted residual gaps, matching TASK-0319's own precedent of naming what
is left open rather than silently narrowing scope.**

*(1) Aliasing across two different scan roots is unchanged* — the ruling
scoped this fix to one root's own walk, and the same symptom is still
reachable when one root contains a symlink into another root, e.g.
`imgdedup --follow-symlinks A B` with `A/l -> B`. Reproduced with the real
binary during architect review: `A/g.png` (a genuine copy) plus
`B/deep/photo.png` reached again as `A/l/deep/photo.png` reports
`hard_link_sets: [B/deep/photo.png, A/l/deep/photo.png]` and
`reclaimable_bytes: 0` while 498 bytes are genuinely reclaimable — i.e. the
*same severity* as the single-root defect fixed here, not merely the labeling
half of it. It is not fixed here because widening the pass across roots is a
scope change beyond the recorded product ruling, and because the survivor
preference ("direct" = no symlink hop below the root) is defined relative to
*a* root and would have to be evaluated per candidate against its own root.
Tracked as **TASK-0329** for a product-owner call; the single-root fix does
not make it worse, and the behavior is unchanged from before this amendment.

*(2) A leaf symlink whose target is not independently in scope.* A leaf
symlink whose
target is **not** independently enumerated in this run (outside every scan
root, or filtered out by `--ext`/hidden rules) is unchanged from prior
behavior — still a candidate at the link path, still declined at apply time
with the existing `IsASymlink`/`KeeperIsASymlink` skip reasons
(`docs/guide/removing-duplicates-safely.md`). This is the one case in which
the pre-existing dry-run/apply parity exception still applies; it is safe
(nothing is ever removed for it), truthfully reported at apply time, and
reachable only via an opt-in flag — accepted rather than fixed here, per the
product-owner ruling. New acceptance criterion: **AC-14a** (PRD-0001).

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
