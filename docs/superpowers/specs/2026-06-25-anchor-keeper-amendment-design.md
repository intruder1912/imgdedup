# Design: ADR-0008 Amendment — Keeper Selection Across Anchors + Standalones

**Date:** 2026-06-25  
**Related:** [ADR-0008](../adr/0008-core-deduplication-algorithm-and-semantics.md) (edge-2 ruling), [PR #25](https://github.com/intruder1912/imgdedup/pull/25) (S1 detect-report slice), Architect Review 3 (CONDITIONAL APPROVE)

## Context and Problem

Slice S1 (detect-report) implemented the anchor rule per ADR-0008 edge-case rulings. During architect re-review, a critical issue was identified: **edge-2 (standalone copies + hard-link set) under-reclaims and violates `--keep`**.

### The Scenario

A group with one standalone copy + one hard-link set (all byte-identical):
- `beach.jpg` ↔ `beach-hardlink.jpg` (same inode, hard-link set = anchors)
- `beach-copy.jpg` (separate inode, standalone)
- User runs: `imgdedup --keep shortest photos/`

### Current Behavior (edge-2)

The `--keep shortest` rule ranks **only the standalones**. Since there is only one standalone (`beach-copy.jpg`), it becomes the keeper (by default). The hard-links are protected. Result:

```
Group 1/1 (2 files, 0 bytes reclaimable):
  KEEP /photos/beach-copy.jpg
  PROTECT /photos/beach.jpg (hard-linked; removing it frees no space)

Already deduplicated (hard-linked) — nothing to reclaim:
  /photos/beach.jpg
    hard-linked: /photos/beach.jpg, /photos/beach-hardlink.jpg

Summary: no duplicate groups to remove; 1 already-deduplicated hard-link set.
```

### Three Problems

1. **Misleads:** Reports "nothing to reclaim" when `beach-copy.jpg` (the standalone) could be safely removed, freeing its full size.
2. **Under-reclaims:** Reclaims 0 bytes when 1 copy's worth is safely reclaimable.
3. **Violates `--keep`:** User asked for the shortest path (`--keep shortest`). `beach.jpg` is the shortest. But it's in the protected set, so the tool kept `beach-copy.jpg` instead — the opposite of what was asked.

### Root Cause

ADR-0008 edge-2 restricts `--keep` to rank **only standalones**, barring anchors from the keeper race. The stated rationale is label purity: "we don't want to conflate 'the file the user chose to keep' with 'a file we are forbidden to delete'." But this is a cosmetic concern (solvable with clear labeling) that forfeits real, safe reclaim and violates the user's intent.

## Decision: Include Anchors in Keeper Ranking

**The `--keep` rule will rank all representatives** (both anchors and standalones) to find the minimum. The keeper selection then branches:

### Case 1: The `--keep` minimum is an anchor

- That anchor becomes the single **keeper (KEEP)**.
- Other anchors become **protected (PROTECT)**.
- **All standalone copies become removable** (verified against the anchor keeper).

**Rationale:** An anchor inode *always* survives (by definition, its sibling pins it). Therefore every standalone copy in the same content group is genuinely redundant and safely removable. The `--keep` rule now means what the user asked.

**Beach scenario outcome (with this amendment):**
```
Group 1/1 (2 files, 1024 bytes reclaimable):
  KEEP /photos/beach.jpg
  PROTECT /photos/beach-hardlink.jpg  (hard-linked; removing it frees no space)
  REMOVE /photos/beach-copy.jpg

Summary: 1 duplicate group, 2 files, 1024 bytes reclaimable.
```

This honors `--keep shortest`, reclaims the standalone's size, and is honest about what can be removed.

### Case 2: The `--keep` minimum is a standalone

- That standalone becomes the **keeper (KEEP)**.
- Other standalones become **removable (REMOVE)**.
- Anchors become **protected (PROTECT)**.

This is the current behavior (edge-2 when there is only one standalone). Reclaimable can legitimately be 0 here (the user chose to keep the standalone while the anchor is immovable), but the report must be clear about *why*, not say "nothing to reclaim" flatly.

### Case 3: All-anchor groups (edge-1)

No change. These emit no `DuplicateGroup` and surface as "already deduplicated" notes.

## Implementation Changes

### 1. ADR-0008 Amendment (docs/adr/0008-core-deduplication-algorithm-and-semantics.md)

**Rewrite edge-2 ruling (lines 202–215):**

From:
```
2. Standalone copies plus one (or more) hard-link set(s). The `--keep` rule
ranks the genuinely-removable standalone copies only; the minimum under that
order is the single keeper, the remaining standalone copies are removable,
and every anchor is protected (kept, not the keeper).
```

To:
```
2. Standalone copies plus one (or more) hard-link set(s). The `--keep` rule
ranks all representatives (both anchors and standalones) to find the minimum.

- If the minimum is an anchor, that anchor is the single keeper (KEEP), other
  anchors are protected (PROTECT), and all standalone copies are removable and
  verified against the anchor keeper. Rationale: an anchor inode always survives
  (by definition, its sibling pins it), so every standalone copy in the same
  content group is genuinely redundant and safely removable.

- If the minimum is a standalone, that standalone is the single keeper (KEEP),
  other standalones are removable, and every anchor is protected (PROTECT). In
  this case, reclaimable bytes may legitimately be 0 (the user chose to keep
  the standalone while the anchor cannot be removed), but the report must
  clearly explain this rather than saying "nothing to reclaim" flatly.
```

Also update the introductory paragraph of the anchor section (before edge-1) to reflect that anchors are now candidates for keeper selection.

### 2. Engine Changes (src/engine.rs)

**Keeper selection in `process_hash_group`:**

Current logic:
```rust
// Partition anchors vs candidates (non-anchors)
let (anchors, candidates): (Vec<_>, Vec<_>) = reps.into_iter()
    .partition(|(_, is_anchor)| *is_anchor);

// Keeper from candidates only
let keeper = if candidates.is_empty() {
    // All-anchor case
    return ...no group...
};
let keeper = candidates.iter().min_by(|a, b| keeper_order(a, b))?;
```

New logic:
```rust
// Partition anchors vs standalones
let (anchors, standalones): (Vec<_>, Vec<_>) = reps.into_iter()
    .partition(|(_, is_anchor)| *is_anchor);

// All-anchor case: no keeper, surface as already-deduplicated notes
if standalones.is_empty() && !anchors.is_empty() {
    // All-anchor case (edge-1)
    return ...no group...
}

// Genuinely removable reps = anchors + standalones
let all_reps = anchors.iter().chain(standalones.iter());

// Keeper from all representatives
let keeper = all_reps.min_by(|a, b| keeper_order(&a.0, &b.0))?;
let keeper_is_anchor = keeper.1;

// Partition removable based on keeper type
let (protected, removable) = if keeper_is_anchor {
    // Keeper is an anchor: protect other anchors, remove all standalones
    (anchors, standalones)
} else {
    // Keeper is a standalone: remove other standalones, protect anchors
    (anchors, standalones.into_iter().filter(|r| r != keeper).collect())
};

// Byte-verify removable
let verified_removable = ...
let protected_recs = protected.into_iter().map(|(r, _)| r).collect();

DuplicateGroup {
    keeper: keeper.0,
    removable: verified_removable,
    protected: protected_recs,
}
```

### 3. Report Changes (src/report.rs)

**Already-deduplicated block logic:**

Current: A removable-less group (keeper + protected anchors, no standalones to remove) renders in the "already deduplicated" block.

**New:** This scenario no longer occurs when an anchor is keeper (standalones are always removable then). The block now contains **only**:
- All-anchor groups (edge-1, no `DuplicateGroup` emitted)
- **Not** the case where a standalone keeper has 0 reclaimable

The latter stays in the normal "Group" rendering with a PROTECT row explaining the anchors are immovable.

**Detailed rendering (no change to the structure, only the inputs):**

For a group with keeper=standalone and protected anchors and 0 removable:
```
Group 1/1 (2 files, 0 bytes reclaimable):
  KEEP /photos/beach-copy.jpg
  PROTECT /photos/beach.jpg  (hard-linked; removing it frees no space)
  NOTE hard-link: /photos/beach.jpg, /photos/beach-hardlink.jpg

[... Summary line ...]
```

This is clear: the user chose the standalone, anchors are immovable, reclaimable is 0.

### 4. Test Updates

**Flip beach scenario assertions:**

Test: `hard_links_are_collapsed_and_never_removed`
- **Current:** Asserts the standalone is keeper, anchors are protected.
- **New:** Asserts the anchor (shortest) is keeper, the standalone is removable.

Test: `beach_scenario_anchor_protected_reclaimable_zero`
- **Current:** Asserts reclaimable=0 when standalone is keeper.
- **Delete:** This case no longer occurs when an anchor wins the sort order. Will be replaced by the inverse.

**New test: `anchor_wins_keep_shortest_removes_standalones`**

Scenario: `beach.jpg` ↔ `beach-hardlink.jpg` (anchor) + `beach-copy.jpg` (standalone), `--keep shortest`.
- Assert: keeper = `beach.jpg` (anchor, shortest).
- Assert: removable = [`beach-copy.jpg`] (standalone).
- Assert: protected = [`beach-hardlink.jpg`] (other anchor).
- Assert: reclaimable = size of `beach-copy.jpg`.
- Assert: report renders KEEP for `beach.jpg`, REMOVE for `beach-copy.jpg`, PROTECT for `beach-hardlink.jpg`.

**Regression test: `standalone_wins_keep_shortest_when_shorter_than_anchors`**

Scenario: `short.jpg` (standalone, shortest) + `longer.jpg` (anchor), `--keep shortest`.
- Assert: keeper = `short.jpg` (standalone, shortest).
- Assert: removable = [] (no other standalones).
- Assert: protected = [`longer.jpg`] (anchor).
- Assert: reclaimable = 0 (can't remove the anchor).
- Assert: report renders KEEP for `short.jpg`, PROTECT for `longer.jpg`, and explains why reclaimable is 0.

### 5. Guide Update (docs/guide/choosing-what-to-keep.md)

**Section: Understanding hard-links (lines ~85–105)**

Current text frames the beach scenario as "a match consists *only* of hard-linked copies" (all-anchor case). It's actually anchor+standalone.

**New text:**

```markdown
### Hard-links and the `--keep` rule

When a duplicate group contains hard-linked copies (paths to the same underlying file), 
the `--keep` rule ranks **all copies**, including those in the hard-link set.

- If `--keep` selects a path in the hard-link set (e.g., `--keep shortest` picks 
  the shortest path and it belongs to a hard-linked set), the **entire hard-link 
  set is kept**, and any standalone copies become removable.

- If `--keep` selects a standalone copy, that standalone is kept, and hard-linked 
  sets are protected (not removable, because removing them frees no space).

**Example:** `beach.jpg` and `beach-hardlink.jpg` are hard-linked (same file on disk). 
`beach-copy.jpg` is a separate, byte-identical copy. With `--keep shortest`:
- `beach.jpg` is the shortest path
- Since it's in the hard-link set, the tool keeps the entire hard-link set
- `beach-copy.jpg` is removed (freeing its space)

If instead `beach-copy.jpg` were the shortest path, it would be kept, the hard-link 
set would be protected, and reclaimable would be 0 (the hard-link set cannot be reduced 
further without breaking one of its paths).
```

## Verification and Testing

### Manual Testing

1. **Beach scenario (anchor wins):**
   - Create: `beach.jpg` ↔ `beach-hardlink.jpg` (hard-link) + `beach-copy.jpg` (separate, identical)
   - Run: `imgdedup --keep shortest photos/`
   - Verify: Reports keeper=`beach.jpg`, removable=`beach-copy.jpg`, reclaimable>0

2. **Beach scenario (standalone wins):**
   - Same files, but make `beach-copy.jpg` shorter (rename to `b.jpg`)
   - Run: `imgdedup --keep shortest photos/`
   - Verify: Reports keeper=`b.jpg`, protected anchors, reclaimable=0, explains why

3. **All-anchor group (unchanged):**
   - Create: `a.jpg` ↔ `a-link.jpg` (inode I1) + `b.jpg` ↔ `b-link.jpg` (inode I2), byte-identical
   - Run: `imgdedup photos/`
   - Verify: Reports "already deduplicated" block, no removal groups

### Automated Tests

- Flip `hard_links_are_collapsed_and_never_removed` assertions (anchor is keeper, standalone is removable)
- Add `anchor_wins_keep_shortest_removes_standalones` (new beach scenario)
- Add `standalone_wins_keep_shortest_when_shorter_than_anchors` (regression guard)
- Verify all existing tests still pass (determinism, all-anchor, etc.)
- Run `make validate` (fmt, clippy, tests, cspell)

## Implementation Sequence

1. **Update ADR-0008** — rewrite edge-2 ruling, commit
2. **Implement engine change** — keeper selection logic, TDD (red test, green code)
3. **Update tests** — flip beach assertions, add new tests, verify all pass
4. **Update report** — only necessary if the removable-less case no longer occurs (likely minimal)
5. **Update guide** — clarify hard-link `--keep` semantics
6. **Validate** — `make validate` green, manual e2e tests pass
7. **Update PR description** — reflect the amendment and re-review outcome

## Acceptance Criteria

- ✅ ADR-0008 edge-2 amended with new keeper-selection logic
- ✅ Engine `process_hash_group` ranks all reps (anchors + standalones) for keeper
- ✅ Keeper is anchor → standalones become removable (and verified)
- ✅ Keeper is standalone → anchors become protected
- ✅ Beach scenario test flipped: anchor is keeper, standalone is removable, reclaimable>0
- ✅ All existing tests pass (determinism, all-anchor, byte-verify safety)
- ✅ Guide updated to explain new `--keep` ranking across hard-links
- ✅ `make validate` passes (fmt, clippy, tests, markdownlint, cspell)
- ✅ Manual e2e: `--keep shortest` honors user intent in anchor+standalone scenarios
