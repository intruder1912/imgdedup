# Choosing what to keep

[← Guide index](README.md)

When `imgdedup` finds a group of byte-for-byte identical images, exactly one is
designated the **keeper** and the rest are the removable duplicates. This page
explains how the keeper is chosen and how to change the rule.

> **In development.** The keeper-selection rule is being implemented; the `--keep`
> flag is already part of the CLI surface. The behavior below is the intended
> design (PRD-0001, [ADR-0008](../adr/0008-core-deduplication-algorithm-and-semantics.md)).

## The `--keep` rule

```sh
imgdedup --keep <RULE> ./photos
```

| Rule | Keeps the file with… |
|---|---|
| `shortest` (default) | the shortest path — fewest path segments, then fewest characters; ties broken by oldest modification time, then lexicographically. |
| `oldest` | the oldest modification time first, then shortest path, then lexicographically. |
| `newest` | the newest modification time first, then shortest path, then lexicographically. |

The rule is **deterministic**: the same inputs always select the same keeper, so
runs are reproducible.

```sh
# Keep the newest copy in each duplicate group.
imgdedup --keep newest ./inbox
```

## Hard links are never collateral

If two paths are hard links to the same physical file, they are already a single
file on disk. `imgdedup` recognizes this and never proposes removing a hard-linked
"copy" — only genuinely separate duplicate files are ever candidates for removal.

## Next

- [Removing duplicates safely](removing-duplicates-safely.md) — act on the
  non-keepers.
- [Output and exit codes](output-and-exit-codes.md) — see the keeper and the
  files to remove in the report.
