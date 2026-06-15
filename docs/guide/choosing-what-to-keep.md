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
single file on disk — removing one frees no space. `imgdedup` recognizes this,
collapses the hard-linked paths to one representative, and never proposes removing
a hard-linked "copy". Only genuinely separate duplicate files are ever candidates
for removal.

Hard-linked siblings are surfaced as an informational `NOTE` line under their
group and are deliberately **left out of that group's file count and the
reclaimable-space figure** — counting them would overstate both, since removing
them frees nothing:

```text
Group 1/1 (2 files, 1024 bytes reclaimable):
  KEEP  /photos/a.jpg
  REMOVE /photos/separate-copy.jpg
  NOTE  hard-link set: /photos/a.jpg → /photos/also-a.jpg
```

Here `/photos/also-a.jpg` is a hard link to the keeper, so it is reported but is
not one of the "2 files" and adds nothing to the reclaimable bytes.

## Next

- [Removing duplicates safely](removing-duplicates-safely.md) — act on the
  non-keepers.
- [Output and exit codes](output-and-exit-codes.md) — see the keeper and the
  files to remove in the report.
