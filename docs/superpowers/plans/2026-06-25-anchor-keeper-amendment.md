# ADR-0008 Anchor Keeper Amendment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the anchor-keeper rule to include anchors in `--keep` ranking, so the tool honors user intent and reclaims safe space in anchor+standalone groups.

**Architecture:** The change is localized: rewrite ADR-0008 edge-2, refactor keeper selection in `process_hash_group` to rank all reps (not just standalones), flip the beach test assertions, add a regression guard test, update the guide, and validate.

**Tech Stack:** Rust (TDD with cargo test), markdown (ADR and guide docs)

---

## File Structure

**Files to modify:**
- `docs/adr/0008-core-deduplication-algorithm-and-semantics.md` — ADR edge-2 ruling rewrite
- `src/engine.rs` — keeper selection logic in `process_hash_group`
- `src/engine.rs` (tests) — flip beach test, add new tests, remove old test
- `docs/guide/choosing-what-to-keep.md` — hard-link `--keep` semantics

---

## Implementation Tasks

### Task 1: Update ADR-0008 edge-2 Ruling

**Files:**
- Modify: `docs/adr/0008-core-deduplication-algorithm-and-semantics.md:202-215` (edge-2 ruling)
- Modify: `docs/adr/0008-core-deduplication-algorithm-and-semantics.md:157-169` (intro paragraph)

- [ ] **Step 1: Read the current edge-2 ruling**

Open `docs/adr/0008-core-deduplication-algorithm-and-semantics.md` and locate lines 202–215. Current text starts with "Standalone copies plus one (or more) hard-link set(s)."

- [ ] **Step 2: Rewrite edge-2 ruling**

Replace lines 202–215 with:

```markdown
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
```

- [ ] **Step 3: Update intro paragraph of anchor section**

Find the paragraph starting with "The collapse alone is **not sufficient**" (line 159). Update it to reflect that anchors are now *candidates* for keeper selection, not barred from it. Add a sentence like:

```markdown
This ADR resolves that intent-vs-mechanism gap with the **anchor rule**: anchors
are included in keeper selection so that `--keep` ranks all representatives, but
if a standalone wins, anchors are protected to preserve the hard-link set.
```

- [ ] **Step 4: Run markdownlint**

```bash
npx markdownlint docs/adr/0008-core-deduplication-algorithm-and-semantics.md
```

Expected: No errors (the markdown format is unchanged, only content).

- [ ] **Step 5: Commit**

```bash
git add docs/adr/0008-core-deduplication-algorithm-and-semantics.md
git commit -m "docs(adr): amend ADR-0008 edge-2: include anchors in keeper ranking"
```

---

### Task 2: Write Failing Test — Anchor Wins `--keep shortest`

**Files:**
- Modify: `src/engine.rs` (tests section, add new test)

- [ ] **Step 1: Add the failing test**

Open `src/engine.rs` and scroll to the test section (around line 1200+). Add this new test after the existing hard-link tests:

```rust
#[test]
fn anchor_wins_keep_shortest_removes_standalones() {
    // Scenario: beach.jpg ↔ beach-hardlink.jpg (anchor) + beach-copy.jpg (standalone)
    // Files are byte-identical. With --keep shortest, beach.jpg (shortest path) is an anchor.
    // Expected: keeper = beach.jpg (anchor), removable = [beach-copy.jpg], protected = [beach-hardlink.jpg]
    
    let mut candidates = vec![];
    
    // beach.jpg (anchor, shortest path, 1024 bytes)
    candidates.push(Candidate {
        path: PathBuf::from("beach.jpg"),
        size: 1024,
        mtime: Some(1000),
    });
    
    // beach-hardlink.jpg (same inode as beach.jpg, hard-link, 1024 bytes)
    candidates.push(Candidate {
        path: PathBuf::from("beach-hardlink.jpg"),
        size: 1024,
        mtime: Some(1000),
    });
    
    // beach-copy.jpg (separate inode, standalone, longer path, 1024 bytes)
    candidates.push(Candidate {
        path: PathBuf::from("beach-copy.jpg"),
        size: 1024,
        mtime: Some(1000),
    });
    
    // Create a temp dir with these files (hard-linked beach.jpg and beach-hardlink.jpg,
    // separate beach-copy.jpg with identical content)
    let temp = tempfile::TempDir::new().unwrap();
    let beach_path = temp.path().join("beach.jpg");
    let hardlink_path = temp.path().join("beach-hardlink.jpg");
    let copy_path = temp.path().join("beach-copy.jpg");
    
    std::fs::write(&beach_path, b"identical content").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::hard_link(&beach_path, &hardlink_path).unwrap();
    #[cfg(windows)]
    {
        // Windows doesn't have hard_link in std, use copy for this test
        std::fs::copy(&beach_path, &hardlink_path).unwrap();
    }
    std::fs::write(&copy_path, b"identical content").unwrap();
    
    // Convert candidates to actual FileRec entries
    let mut files = vec![];
    for candidate in candidates {
        let path = temp.path().join(&candidate.path);
        let hash = crate::sniff::hash_file(&path).unwrap();
        files.push((candidate, hash));
    }
    
    // Run dedup with --keep shortest
    let keeper_order = crate::keeper::build_comparator(crate::cli::KeepRule::Shortest);
    let outcome = find_duplicates(files, keeper_order);
    
    // Assertions
    assert_eq!(outcome.groups.len(), 1, "Should have 1 duplicate group");
    let group = &outcome.groups[0];
    
    // Keeper should be beach.jpg (shortest, anchor)
    assert_eq!(group.keeper.path.file_name().unwrap(), "beach.jpg");
    
    // Removable should be [beach-copy.jpg]
    assert_eq!(group.removable.len(), 1);
    assert_eq!(group.removable[0].path.file_name().unwrap(), "beach-copy.jpg");
    
    // Protected should be [beach-hardlink.jpg]
    assert_eq!(group.protected.len(), 1);
    assert_eq!(group.protected[0].path.file_name().unwrap(), "beach-hardlink.jpg");
    
    // Reclaimable should equal beach-copy.jpg's size (1024 bytes)
    let reclaimable: u64 = group.removable.iter().map(|r| r.size).sum();
    assert_eq!(reclaimable, 1024);
}
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cargo test --lib engine::tests::anchor_wins_keep_shortest_removes_standalones -- --nocapture
```

Expected: FAIL with something like "keeper.path doesn't match" or "assertion failed" because the code doesn't yet implement the new logic.

- [ ] **Step 3: Commit the failing test**

```bash
git add src/engine.rs
git commit -m "test(engine): add failing test for anchor-wins keeper scenario (beach shortest)"
```

---

### Task 3: Implement Engine Change — Anchor Wins Keeper Selection

**Files:**
- Modify: `src/engine.rs:process_hash_group` function

- [ ] **Step 1: Locate process_hash_group function**

Open `src/engine.rs` and find `fn process_hash_group(...)` (around line 400–450). You'll see the current keeper-selection logic that partitions `anchors` vs `candidates` (non-anchors) and selects keeper from candidates only.

- [ ] **Step 2: Understand the current structure**

The function currently:
1. Partitions reps into `anchors` and `candidates`
2. Returns early if `candidates.is_empty()` (all-anchor case)
3. Selects keeper from `candidates` only
4. Partitions remaining candidates into removable (all) and empty protected

- [ ] **Step 3: Rewrite keeper selection logic**

Find this section:

```rust
// Partition anchors vs candidates (non-anchors)
let (anchors, candidates): (Vec<_>, Vec<_>) = reps.into_iter()
    .partition(|(_, is_anchor)| *is_anchor);

// Early return for all-anchor groups
if candidates.is_empty() {
    // ... all-anchor handling
}

// Keeper from candidates only
let keeper = ...min_by(keeper_order)... candidates ...
let keeper_rec = ...
let removable = candidates.into_iter()
    .filter(|r| r.path != keeper_rec.path)
    .collect();
let protected = vec![];
```

Replace it with:

```rust
// Partition anchors vs standalones
let (anchors, standalones): (Vec<_>, Vec<_>) = reps.into_iter()
    .partition(|(_, is_anchor)| *is_anchor);

// Early return for all-anchor groups (no keeper to choose)
if standalones.is_empty() {
    if anchors.is_empty() {
        // Should not reach here; < 2 reps handled earlier
        return None;
    }
    // All anchors: return no group, notes only
    return Some(outcome_from_notes_only(notes)); // Existing pattern
}

// Keeper from all representatives (anchors + standalones)
// Build iterator over all reps with their (rec, is_anchor) tuples
let all_reps = anchors.iter().chain(standalones.iter()).collect::<Vec<_>>();
let keeper = all_reps.into_iter()
    .min_by(|a, b| keeper_order(&a.0, &b.0))?;
let (keeper_rec, keeper_is_anchor) = keeper;

// Partition removable and protected based on keeper type
let (protected, removable) = if keeper_is_anchor {
    // Keeper is an anchor: protect other anchors, remove all standalones
    let protected: Vec<FileRec> = anchors.into_iter()
        .filter(|a| a.0.path != keeper_rec.path)
        .map(|(rec, _)| rec)
        .collect();
    (protected, standalones.into_iter().map(|(rec, _)| rec).collect())
} else {
    // Keeper is a standalone: protect all anchors, remove other standalones
    let protected: Vec<FileRec> = anchors.into_iter()
        .map(|(rec, _)| rec)
        .collect();
    let removable: Vec<FileRec> = standalones.into_iter()
        .filter(|s| s.0.path != keeper_rec.path)
        .map(|(rec, _)| rec)
        .collect();
    (protected, removable)
};
```

- [ ] **Step 4: Byte-verify removable (existing pattern)**

Locate the byte-verify section after keeper selection. It currently filters `removable` for those that match the keeper byte-for-byte. Ensure it still applies to the new `removable` list (it should, since the structure is the same).

Verify the code looks like:
```rust
let verified_removable = removable.into_iter()
    .filter(|r| files_equal(&keeper_rec.path, &r.path).unwrap_or(false))
    .collect::<Vec<_>>();
```

No change needed here — it applies to whatever is in `removable`.

- [ ] **Step 5: Ensure protected is included in outcomes**

Find where `DuplicateGroup` is constructed. It should now include the `protected` field (it already does, since the anchor fix landed in S1). Verify:

```rust
DuplicateGroup {
    keeper: keeper_rec,
    removable: verified_removable,
    protected: protected,
}
```

- [ ] **Step 6: Run the test to verify it passes**

```bash
cargo test --lib engine::tests::anchor_wins_keep_shortest_removes_standalones -- --nocapture
```

Expected: PASS. The keeper is now beach.jpg (anchor, shortest), removable is beach-copy.jpg (standalone), protected is beach-hardlink.jpg (other anchor), and reclaimable > 0.

- [ ] **Step 7: Verify all other engine tests still pass**

```bash
cargo test --lib engine::tests
```

Expected: All pass. If any fail, debug the changes (likely the all-anchor or existing beach test).

- [ ] **Step 8: Commit**

```bash
git add src/engine.rs
git commit -m "feat(engine): include anchors in keeper ranking for edge-2 (anchor+standalone)"
```

---

### Task 4: Flip Beach Test Assertions

**Files:**
- Modify: `src/engine.rs:tests::hard_links_are_collapsed_and_never_removed` function

- [ ] **Step 1: Locate the beach test**

Open `src/engine.rs` and find `#[test] fn hard_links_are_collapsed_and_never_removed()`. This test creates a beach scenario with one hard-link set and one standalone.

- [ ] **Step 2: Read the current assertions**

You'll see assertions like:
```rust
// Currently expects standalone to be keeper
assert_eq!(group.keeper.path, ...; // some standalone path
assert!(group.removable.iter().any(|r| ...), "standalone is removable");
assert_eq!(group.protected.len(), 1, "anchor is protected");
```

- [ ] **Step 3: Understand the old behavior**

The test currently asserts that the standalone is the keeper and the anchor (hard-link representative) is protected. With the amendment, this flips: the anchor (shortest) becomes the keeper and the standalone becomes removable.

- [ ] **Step 4: Update the test to assert the new behavior**

Find the assertions section and update it:

**Old:**
```rust
assert_eq!(group.keeper.path, /* standalone path */);
assert_eq!(group.removable.len(), 0, "no removable (standalone is keeper)");
assert_eq!(group.protected.len(), 1, "hard-link rep is protected");
```

**New:**
```rust
// With the amendment: anchor (hard-link rep) is keeper, standalone is removable
assert_eq!(group.keeper.path, /* hard-link representative path */);
assert_eq!(group.removable.len(), 1, "standalone is removable");
assert_eq!(group.removable[0].path, /* standalone path */);
assert_eq!(group.protected.len(), 1, "other hard-link is protected");
assert_eq!(group.protected[0].path, /* other hard-link path */);
```

**Note:** You may need to check the actual path setup in this test to get the exact assertions. The key flip is: anchor→keeper, standalone→removable, and the protected count stays 1 (the other hard-link).

- [ ] **Step 5: Run the test**

```bash
cargo test --lib engine::tests::hard_links_are_collapsed_and_never_removed -- --nocapture
```

Expected: PASS with the new assertions.

- [ ] **Step 6: Commit**

```bash
git add src/engine.rs
git commit -m "test(engine): flip beach test assertions (anchor is keeper, standalone removable)"
```

---

### Task 5: Remove the Old beach_scenario_anchor_protected_reclaimable_zero Test

**Files:**
- Modify: `src/engine.rs` (remove test function)

- [ ] **Step 1: Locate the test to remove**

Find `#[test] fn beach_scenario_anchor_protected_reclaimable_zero()` in `src/engine.rs`. This test asserts that with the old edge-2 rule, reclaimable=0 when the standalone is forced to be keeper.

- [ ] **Step 2: Delete the entire test function**

Remove the function definition and all its code (from `#[test]` to the closing `}` of the function body).

- [ ] **Step 3: Run the test suite**

```bash
cargo test --lib engine::tests
```

Expected: PASS. The test no longer exists, so no failure. All other tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/engine.rs
git commit -m "test(engine): remove beach_scenario_anchor_protected_reclaimable_zero (replaced by new logic)"
```

---

### Task 6: Add Regression Test — Standalone Wins When Shorter

**Files:**
- Modify: `src/engine.rs` (tests section, add new test)

- [ ] **Step 1: Add the regression test**

This test ensures that when the *standalone* is shortest (not the anchor), the old behavior still holds: standalone is keeper, anchors are protected, removable is empty, reclaimable is 0.

Add this test to `src/engine.rs` tests section:

```rust
#[test]
fn standalone_wins_keep_shortest_when_shorter_than_anchors() {
    // Scenario: short.jpg (standalone, shortest) + longer.jpg ↔ longer-link.jpg (anchor, longer path)
    // All byte-identical. With --keep shortest, short.jpg (shortest) is a standalone.
    // Expected: keeper = short.jpg (standalone), removable = [], protected = [longer.jpg, longer-link.jpg]
    
    let temp = tempfile::TempDir::new().unwrap();
    
    // short.jpg (standalone, shortest, 1024 bytes)
    let short_path = temp.path().join("short.jpg");
    std::fs::write(&short_path, b"identical content").unwrap();
    
    // longer.jpg and longer-link.jpg (hard-linked, 1024 bytes)
    let longer_path = temp.path().join("longer.jpg");
    let longer_link_path = temp.path().join("longer-link.jpg");
    std::fs::write(&longer_path, b"identical content").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::hard_link(&longer_path, &longer_link_path).unwrap();
    #[cfg(windows)]
    std::fs::copy(&longer_path, &longer_link_path).unwrap();
    
    // Build file entries
    let files = vec![
        (Candidate {
            path: PathBuf::from("short.jpg"),
            size: 1024,
            mtime: Some(1000),
        }, crate::sniff::hash_file(&short_path).unwrap()),
        (Candidate {
            path: PathBuf::from("longer.jpg"),
            size: 1024,
            mtime: Some(1000),
        }, crate::sniff::hash_file(&longer_path).unwrap()),
        (Candidate {
            path: PathBuf::from("longer-link.jpg"),
            size: 1024,
            mtime: Some(1000),
        }, crate::sniff::hash_file(&longer_link_path).unwrap()),
    ];
    
    // Run dedup with --keep shortest
    let keeper_order = crate::keeper::build_comparator(crate::cli::KeepRule::Shortest);
    let outcome = find_duplicates(files, keeper_order);
    
    // Assertions
    assert_eq!(outcome.groups.len(), 1, "Should have 1 duplicate group");
    let group = &outcome.groups[0];
    
    // Keeper should be short.jpg (shortest, standalone)
    assert_eq!(group.keeper.path.file_name().unwrap(), "short.jpg");
    
    // Removable should be empty (only other standalones, and there are none)
    assert_eq!(group.removable.len(), 0, "No other standalones to remove");
    
    // Protected should be [longer.jpg, longer-link.jpg] (anchors)
    assert_eq!(group.protected.len(), 2, "Both hard-links are protected anchors");
    
    // Reclaimable should be 0
    let reclaimable: u64 = group.removable.iter().map(|r| r.size).sum();
    assert_eq!(reclaimable, 0, "No standalones to remove, so reclaimable is 0");
}
```

- [ ] **Step 2: Run the test**

```bash
cargo test --lib engine::tests::standalone_wins_keep_shortest_when_shorter_than_anchors -- --nocapture
```

Expected: PASS. This test validates the fallback case: when a standalone wins `--keep`, anchors are protected and nothing is removable.

- [ ] **Step 3: Run all engine tests**

```bash
cargo test --lib engine::tests
```

Expected: All pass.

- [ ] **Step 4: Commit**

```bash
git add src/engine.rs
git commit -m "test(engine): add regression test for standalone-wins keeper scenario"
```

---

### Task 7: Update Guide — Hard-link and `--keep` Semantics

**Files:**
- Modify: `docs/guide/choosing-what-to-keep.md:85-105` (hard-links section)

- [ ] **Step 1: Locate the hard-links section**

Open `docs/guide/choosing-what-to-keep.md` and find the section explaining hard-links (around lines 85–105). Current text incorrectly frames the scenario as all-hard-links.

- [ ] **Step 2: Read the current text**

You'll see something like "a match consists *only* of hard-linked copies" with the beach example. This is incorrect — the beach example has both hard-links and a standalone.

- [ ] **Step 3: Replace the hard-links section**

Replace the entire section (roughly lines 85–105, but adjust based on actual file) with:

```markdown
### Hard-links and the `--keep` rule

When a duplicate group contains hard-linked copies (paths to the same underlying file), 
the `--keep` rule ranks **all copies**, including those in the hard-link set.

- If `--keep` selects a path in the hard-link set (e.g., `--keep shortest` picks 
  the shortest path and it belongs to a hard-linked set), the **entire hard-link 
  set is kept**, and any standalone copies become removable.

- If `--keep` selects a standalone copy, that standalone is kept, and hard-linked 
  sets are protected (not removable, because removing them frees no space).

**Example:** `beach.jpg` and `beach-hardlink.jpg` are hard-linked (the same file on disk). 
`beach-copy.jpg` is a separate, byte-identical copy. With `--keep shortest`:

- `beach.jpg` (9 chars) is the shortest path
- Since it's in the hard-link set, the tool keeps the entire hard-link set
- `beach-copy.jpg` is marked for removal (freeing its space)

If instead `beach-copy.jpg` were renamed to `b.jpg` (1 char, making it the shortest), 
the result would be:

- `b.jpg` is kept (standalone, shortest)
- The hard-link set (`beach.jpg`, `beach-hardlink.jpg`) is protected (not removed)
- Reclaimable space is 0 (the hard-link set cannot be reduced further without breaking one of its paths)
```

- [ ] **Step 4: Run markdownlint and cspell**

```bash
npx markdownlint docs/guide/choosing-what-to-keep.md
npx cspell docs/guide/choosing-what-to-keep.md
```

Expected: No errors.

- [ ] **Step 5: Commit**

```bash
git add docs/guide/choosing-what-to-keep.md
git commit -m "docs(guide): clarify hard-link and --keep rule semantics (anchor+standalone case)"
```

---

### Task 8: Run Full Validation and Manual E2E

**Files:**
- No file changes, validation only

- [ ] **Step 1: Run cargo test**

```bash
cargo test
```

Expected: All tests pass (unit + integration). If any fail, the prior tasks have a bug; debug and fix before proceeding.

- [ ] **Step 2: Run make validate**

```bash
make validate
```

Expected: PASS. This runs rustfmt, clippy, tests, cspell, markdownlint.

- [ ] **Step 3: Manual E2E — Beach scenario (anchor wins)**

In a temp directory:

```bash
# Create beach.jpg and beach-hardlink.jpg (hard-linked)
touch beach.jpg
ln beach.jpg beach-hardlink.jpg

# Create beach-copy.jpg (separate, identical content)
cp beach.jpg beach-copy.jpg

# Run imgdedup with --keep shortest
cd /tmp && imgdedup --keep shortest beach.jpg beach-hardlink.jpg beach-copy.jpg
```

**Expected output:**
```
Group 1/1 (2 files, <size> bytes reclaimable):
  KEEP beach.jpg
  PROTECT beach-hardlink.jpg  (hard-linked; removing it frees no space)
  REMOVE beach-copy.jpg

Summary: 1 duplicate group, 2 files, <size> bytes reclaimable.
```

Where `<size>` is the byte size of beach-copy.jpg (>0, not 0).

- [ ] **Step 4: Manual E2E — Beach scenario (standalone wins, renamed)**

In the same temp directory:

```bash
# Rename beach-copy.jpg to b.jpg (making it shortest)
mv beach-copy.jpg b.jpg

# Run imgdedup with --keep shortest
imgdedup --keep shortest beach.jpg beach-hardlink.jpg b.jpg
```

**Expected output:**
```
Group 1/1 (2 files, 0 bytes reclaimable):
  KEEP b.jpg
  PROTECT beach.jpg  (hard-linked; removing it frees no space)
  NOTE hard-linked: beach.jpg, beach-hardlink.jpg

Summary: 1 duplicate group, 2 files, 0 bytes reclaimable.
```

Note: reclaimable is 0 because b.jpg (standalone) is the keeper, and the hard-link set is protected.

- [ ] **Step 5: Verify exit codes**

With the first scenario (anchor wins, removable):

```bash
imgdedup --keep shortest --exit-code beach.jpg beach-hardlink.jpg beach-copy.jpg
echo $?
```

Expected: Exit code 3 (duplicates found, `--exit-code` flag set).

---

### Task 9: Update PR Description

**Files:**
- No file changes, PR metadata update

- [ ] **Step 1: Navigate to PR #25**

On GitHub or via `gh`, open PR #25 (feature/detect-report).

- [ ] **Step 2: Update the PR description**

The current description mentions "Architect re-review pending (deferred to next session)". Update it to reflect:

```markdown
## Review Status Update (Review 3 — Architect Re-review Complete)

Architect Review 3 uncovered a critical issue with edge-2 of ADR-0008: the anchor-keeper 
rule under-reclaims and violates `--keep` when the user asks for the shortest path in a 
hard-link set.

### Amendment Applied

ADR-0008 edge-2 (standalone + hard-link group) has been amended:
- `--keep` now ranks **all** representatives (anchors + standalones), not just standalones
- If `--keep` picks an anchor → anchor is keeper, standalones removable
- If `--keep` picks a standalone → standalone is keeper, anchors protected (unchanged behavior)

**Result:** Beach scenario now honors `--keep shortest` (keeps the short hard-link path, 
removes the standalone), reclaims the standalone's size, and is honest about what's removable.

Committed 5 follow-up changes (ADR amendment, engine logic, test flips, guide update, regression tests).
All tests pass; `make validate` green.

This slice is now **ready for merge** (original CONDITIONAL APPROVE → APPROVE after amendment).
```

- [ ] **Step 3: Add a comment to the PR**

Via GitHub or `gh pr comment 25`:

```markdown
Review 3 complete. Architect identified and approved the amendment to ADR-0008 edge-2.
The issue (anchor rule under-reclaiming) has been fixed. Tests flipped, guide updated, 
all validation passing. Ready to merge when you approve.
```

---

### Task 10: Final Validation and Commit Summary

**Files:**
- No file changes, summary only

- [ ] **Step 1: List commits in this amendment**

Run:

```bash
git log --oneline HEAD~5..HEAD
```

You should see ~5 commits:
1. docs(adr): amend ADR-0008 edge-2
2. test(engine): add failing test for anchor-wins
3. feat(engine): include anchors in keeper ranking
4. test(engine): flip beach test assertions
5. test(engine): add regression test for standalone-wins
6. docs(guide): clarify hard-link and --keep rule semantics

- [ ] **Step 2: Verify branch status**

```bash
git status
```

Expected: Clean (no uncommitted changes).

- [ ] **Step 3: Verify all tests pass**

```bash
cargo test && make validate
```

Expected: All green.

- [ ] **Step 4: View the full diff**

```bash
git diff main..HEAD --stat
```

Expected: Changes to ADR, engine.rs (logic + tests), guide. No unintended changes.

- [ ] **Step 5: Final note**

The amendment is complete and integrated into S1. The PR is ready for merge.

---

## Acceptance Criteria Checklist

- ✅ ADR-0008 edge-2 rewritten (anchors now ranked in `--keep` selection)
- ✅ Engine `process_hash_group` implements new keeper selection (branch on keeper type)
- ✅ Test: `anchor_wins_keep_shortest_removes_standalones` passes (new beach scenario)
- ✅ Test: `hard_links_are_collapsed_and_never_removed` flipped and passes
- ✅ Test: `standalone_wins_keep_shortest_when_shorter_than_anchors` passes (regression)
- ✅ Test: `beach_scenario_anchor_protected_reclaimable_zero` removed (replaced by new logic)
- ✅ All existing engine tests pass (determinism, all-anchor, byte-verify safety untouched)
- ✅ Guide updated: hard-link and `--keep` semantics clarified
- ✅ `make validate` passes (cargo fmt/clippy/test, markdownlint, cspell)
- ✅ Manual e2e: Beach scenario (anchor wins) honors `--keep shortest` and reclaims space
- ✅ Manual e2e: Beach scenario (standalone wins) keeps standalone, protects anchors, 0 reclaimable
- ✅ PR description updated; ready for merge
