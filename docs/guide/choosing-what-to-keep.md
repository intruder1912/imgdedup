# Choosing what to keep

[← Guide index](README.md)

When `imgdedup` finds a group of byte-for-byte identical images, exactly one is
designated the **keeper** (printed as `KEEP` in the report) and the rest are the
removable duplicates (`REMOVE`). This page explains how the keeper is chosen and
how to change the rule.

## The `--keep` rule

```sh
imgdedup --keep <RULE> ./photos
```

Each rule is a strict, ordered list of criteria: the first criterion that
distinguishes two files decides the keeper, and the next criterion is consulted
only on a tie. Every rule ends in the same final tie-break, so the choice is
always unambiguous.

| Rule | Order of criteria (first wins; later ones break ties) |
|---|---|
| `shortest` (default) | 1. fewest path segments (shallowest) → 2. fewest characters in the full path → 3. oldest modification time → 4. path order |
| `oldest` | 1. oldest modification time → 2. fewest path segments → 3. fewest characters → 4. path order |
| `newest` | 1. newest modification time → 2. fewest path segments → 3. fewest characters → 4. path order |

```sh
# Keep the newest copy in each duplicate group.
imgdedup --keep newest ./inbox
```

### The final tie-break and case

The last criterion ("path order") is a **byte-wise, case-sensitive** comparison of
the full path. There is no locale-aware case folding, so an uppercase `A` (byte
`0x41`) sorts before a lowercase `a` (byte `0x61`). This makes the choice
identical on every platform.

### Files with no modification time

If a file's modification time cannot be read, it is treated as the **oldest
possible** time. So under `shortest`/`oldest` such a file is *preferred* as the
keeper when the time criterion is reached, and under `newest` it is the *least*
preferred. Modification times are compared in whole seconds.

### Determinism

The rule is **deterministic**: the same files and the same `--keep` value always
select the same keeper and produce the same report, regardless of the order the
files happened to be scanned in or how many CPU cores did the work. The full rule
and its rationale are in
[ADR-0008](../adr/0008-core-deduplication-algorithm-and-semantics.md).

## Hard links are never collateral

If several paths are hard links to the same physical file, they are already a
single file on disk — removing one frees no space while another link still points
at the same content. `imgdedup` treats such a file as **already deduplicated**: it
is never proposed for removal, never counted in reclaimable space, and never the
removable side of a group.

### Anchors and the `PROTECT` row

A hard-linked file whose inode has **another scanned link** is called an
**anchor**. An anchor is shown with a `PROTECT` label, distinct from both `KEEP`
and `REMOVE`:

```text
Group 1/1 (2 files, 1024 bytes reclaimable):
  KEEP  /photos/separate-copy.jpg
  REMOVE /photos/another-separate-copy.jpg
  PROTECT /photos/a.jpg  (hard-linked; removing it frees no space)
  NOTE  hard-link set: /photos/a.jpg → /photos/also-a.jpg
```

- `PROTECT` files are kept but are **not** the keeper — they are simply files the
  tool will never delete.
- The `NOTE  hard-link set:` line lists every path that shares the same physical
  storage.
- Anchors are deliberately **left out of the group's file count and the
  reclaimable-space figure** (here "2 files" = the keeper plus the one removable
  copy; the anchor adds nothing) — counting them would overstate both, since
  removing them frees nothing.

### How `--keep` interacts with hard links

The `--keep` rule chooses the keeper **only among the genuinely-removable copies**
— the separate, non-hard-linked files. Anchors are excluded from that choice up
front: they are forced to the kept side as `PROTECT` entries, never considered for
the `KEEP` slot. So the keeper is always one of the standalone copies, and your
`--keep shortest` / `oldest` / `newest` preference ranks only those.

### Already-deduplicated sets

When a match consists *only* of hard-linked copies — there is no genuinely
separate file to remove — there is nothing to reclaim, so it is **not** reported
as a duplicate group. It is surfaced under an informational block instead:

```text
Already deduplicated (hard-linked) — nothing to reclaim:
  /photos/beach-hardlink.jpg
    hard-linked: /photos/beach-hardlink.jpg, /photos/beach.jpg
```

These entries are excluded from the duplicate-group, file, and reclaimable totals.

### A note on links outside the scan

Anchor detection only sees the links that fall **within the paths you scanned**.
If an inode has additional hard links elsewhere on disk that the scan never
visited, the reclaimable figure is computed as if the scanned paths were the only
links to that file. Within the scanned scope the accounting is exact; this
in-scope-only limitation is by design (see
[ADR-0008](../adr/0008-core-deduplication-algorithm-and-semantics.md), anchor
rule).

## Next

- [Removing duplicates safely](removing-duplicates-safely.md) — act on the
  non-keepers.
- [Output and exit codes](output-and-exit-codes.md) — see the keeper and the
  files to remove in the report.
