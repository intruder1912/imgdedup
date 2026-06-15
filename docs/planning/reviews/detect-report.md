# In-progress reviews — Slice S1 `detect-report`

Per [ADR-0013](../../adr/0013-prd-slice-delivery-cadence.md), this log records the
in-progress architect reviews requested during the `detect-report` slice
(TASK-0305 keeper → TASK-0306 concurrency/determinism → TASK-0307 human report →
TASK-0311 exit codes), plus the final whole-slice review. The implementer
requests a review for significant/risky changes (not per-commit); the
product-owner or orchestrator may demand one. Feedback is worked immediately;
uncertain now-vs-defer calls go to the product-owner, and deferrals are promoted
to tracked backlog tasks.

## Reviews

### Review 1 — in-progress whole-slice code review (architect)

Reviewed the full branch diff vs `main` after the implementer completed all four
tasks (TASK-0305/0306/0307/0311). `cargo test` (137 unit + 14 integration) and
`cargo clippy --all-targets` clean at review time. Verified clean: keeper orders
match ADR-0008 exactly and are total (byte-wise/case-sensitive paths, whole-second
mtime, `None`→oldest applied uniformly); determinism via `min_by` + canonical
sort; no whole-file buffering; reclaimable space excludes collapsed hard-link
members; report is a pure formatter (AC-1/NFR-1 intact); exit-code precedence
(`1` over `3`) and `--exit-code` status-only effect correct.

**Findings:**

- **B1 (Blocker) — hard-link note non-determinism.** `HardLinkNote.representative`
  was "first member in group-input order", but input order derives from
  `HashMap::into_values()` (unstable across runs); `sort_canonical` ordered notes
  by representative path but never normalized *which* member is the representative,
  nor sorted `siblings`. Two runs could emit different note content → violates
  NFR-3/AC-16 for the hard-link-note path. Safety/keeper choice unaffected
  (reporting-determinism defect only). **Resolution:** fixed — representative is
  now the min path of the identity set and `siblings` are sorted; added a
  multi-sibling determinism test across input orders / thread widths.
- **S1 (Should-fix) — orphaned hard-link note when the representative fails
  byte-verify.** If the collapsed representative is a non-keeper later excluded by
  `files_equal` (collision / I/O error), its note had no matching group and
  rendered nowhere. **Resolution:** fixed — engine drops notes whose
  representative is not retained; added a verify-failure test.
- **S2 (now-vs-defer → product-owner) — `--exit-code` → `3` has no binary-level
  E2E test.** `exit_code::compute` is unit-tested thoroughly; AC-19's exit-3 path
  over a real duplicate tree is covered by TASK-0312 (S4) acceptance sweep.
  **Decision: deferred to S4** (tracked; the binary-level acceptance sweep is S4's
  explicit scope). Recorded here per ADR-0013.
- **N1 (Nit → product-owner) — `total_files` excludes collapsed hard-link
  siblings.** Per-group "file count" = keeper + removable files, not group membership.
  Defensible; confirm against FR-13 wording during product-owner acceptance.
  - **Product-owner decision (2026-06-15): confirmed — this is the intended
    semantics; no code change.** FR-13's "group/file counts" is a count of the
    files the report *acts on* — the keeper plus the duplicates that would be
    removed. Hard-link siblings are, per FR-17, deliberately *not* removal
    candidates and free no space, so counting them in the per-group/summary file
    total would overstate "files involved in a removal decision" and double-count
    storage that the reclaimable figure already (correctly) excludes. Keeping the
    count = keeper + removable, with siblings surfaced only on the informational
    `NOTE` line, keeps the count and the reclaimable figure mutually consistent
    (both exclude collapsed siblings). The S1 docs describe this explicitly
    (`docs/guide/getting-started.md` report walkthrough and
    `docs/guide/choosing-what-to-keep.md` hard-link section).
- **N2 (Nit) — `Sync` bound on the keeper comparator is broader than strictly
  needed** (comparator runs only in the sequential phase). Harmless; doc-precision
  only. Left as-is.

### Review 2 — final whole-slice review (architect): **APPROVE**

After the B1/S1 fixes and the product-owner acceptance + docs sync, the architect
re-reviewed the whole slice. `cargo test` (141 unit + 14 integration), `cargo
clippy --all-targets`, and `make validate` all green at review time.

- **B1 — genuinely resolved.** `collapse_identity` makes the note representative
  the byte-wise min path of the identity set with sorted siblings; the retained
  `FileRec.path` is relabeled to that min path. The relabel fires only for a
  representative that absorbed ≥1 same-inode sibling, draws the path solely from
  that identity set (all the same physical file), so it cannot mislabel or drop a
  distinct-inode file, preserves byte-verify safety, and *strengthens* keeper
  determinism (a stable path for the tie-breaks). Verified by the multi-sibling
  tests across input orders and thread widths.
- **S1 — genuinely resolved.** Notes attach only after `removable` is finalized
  and only when the representative is in `{keeper} ∪ removable`; the relabel keeps
  `note.representative` in sync with the forwarded `FileRec.path`, so the report's
  matching predicate can neither orphan nor over-drop a note.
- **No regression** to the TASK-0304 byte-verify safety authority, canonical-sort
  determinism, exit codes, or the read-only guarantee. **Docs** describe only
  shipped S1 behavior; `--apply`/`--delete`/`--json` are marked not-functional;
  the keeper-rule and report samples match `keeper.rs`/`report.rs`.
- **Verdict: APPROVE** — no blockers, no should-fix. Cleared to open the PR
  (final confirmation pass runs on the PR before the human owner squash-merges).

## Hard-link/keeper ruling (anchor model) — 2026-06-15

An E2E test of S1 exposed an unspecified interaction between hard-link collapse
(TASK-0304) and the keeper rule (TASK-0305): a collapsed representative of a
multiply-linked inode could become a *removable* non-keeper while a surviving
sibling still pins the inode (0 bytes freed) — contradicting ADR-0008's intent
and FR-17/AC-18. The owner approved the **anchor model**; ADR-0008 is amended
(see its "Amendment 2026-06-15"). This is the implementer spec for the TDD fix.

### Ruling (authoritative)

- A collapsed representative whose identity set had **≥2 in-scope members**
  (i.e. it absorbed ≥1 sibling → non-empty `HardLinkNote.siblings`) is an
  **anchor**: **never** in `removable`, **never** counted in reclaimable.
- The `--keep` order chooses the **single keeper among the genuinely-removable
  (non-anchor) members only**. Anchors are forced to the kept side as
  **protected** entries (not KEEP, not REMOVE).
- A representative with **no** surviving in-scope sibling is **not** an anchor
  and is an ordinary removal candidate (current behavior).
- Reclaimable = Σ sizes of `removable` only.
- Anchor detection uses the **already-tracked sibling count** — no extra `stat`,
  no OS link count. Links outside the scan are out of scope (documented limit).

### Engine (`src/engine.rs`)

- **Carry anchor-ness forward.** Today `collapse_identity` returns
  `Vec<FileRec>` and discards which reps absorbed siblings. Change the
  representative model so `process_hash_group` knows each rep's anchor status —
  e.g. return `Vec<(FileRec, bool /* is_anchor */)>` alongside the notes, or add
  a private `is_anchor` field to the rep carried internally (do **not** leak it
  into the public `FileRec` unless a test needs it; prefer an internal tuple).
  `is_anchor == !siblings.is_empty()` for that rep.
- **`process_hash_group` rewrite of keeper/removable split:**
  1. Collapse → list of `(rec, is_anchor)`.
  2. If `< 2` reps → return (unchanged).
  3. Partition reps into `anchors` and `candidates` (non-anchors).
  4. **Keeper selection:** if `candidates` is **non-empty**, keeper =
     `min_by(keeper_order)` **over `candidates`**; the other candidates are
     byte-verified against the keeper and become `removable` (unchanged
     verify/collision/I-O-error handling). All `anchors` are protected.
     - If `candidates` is **empty** (edge case 1, all-anchor group): **emit no
       `DuplicateGroup`** (no keeper, nothing removable). Retain the hard-link
       notes for every anchor in the set so the report can show the
       already-deduplicated note. (See report task below — these notes must
       still surface even with no group.)
  5. **Anchors are never byte-verified for removal** and never enter
     `removable`. (They are byte-identical by hash+the group's verified keeper
     transitively; we don't need to verify them because we never remove them.
     Keep it simple: skip them.)
- **Note retention (extends the existing S1 fix):** today notes are kept only
  when `representative ∈ {keeper} ∪ removable`. Extend so an **anchor's** note
  is retained when the anchor is a protected member of an emitted group **or**
  when the group is an all-anchor (no-`DuplicateGroup`) already-deduplicated
  set. The matching predicate in `report.rs` keys on `representative` path, so
  the representative must remain present as keeper, removable, **or a protected
  anchor entry** — extend the group-paths set accordingly.
- **Surfacing protected anchors to the report.** `DuplicateGroup` currently has
  `{ keeper, removable }`. Add a third field, e.g.
  `pub protected: Vec<FileRec>` (the anchors kept but not the keeper), so the
  report can render a PROTECT/ANCHOR row and so `group_paths` includes them.
  For edge case 1 (all-anchor, no keeper) prefer surfacing via the existing
  `hard_link_notes` "already-deduplicated" path rather than a degenerate
  keeper-less `DuplicateGroup` — i.e. do **not** invent a group with no keeper.
- **Canonical sort:** sort `protected` by `keeper_order` (same as `removable`)
  in `sort_canonical` so determinism holds.

### Report (`src/report.rs`)

- **Reclaimable unchanged in principle** (`Σ removable.size`) — already correct;
  the defect was upstream (anchors leaking into `removable`). Add an explicit
  test that an anchor's size is **not** in reclaimable for the 3-file scenario
  below.
- **Render protected anchors:** for each group, after the REMOVE rows, render
  the protected anchors on their own labelled line (e.g.
  `PROTECT <path>  (hard-linked; removing it frees no space)`), distinct from
  KEEP and REMOVE. The existing `NOTE  hard-link set:` line still lists the
  sibling paths. `total_files` per group stays **keeper + removable** (N1
  product-owner ruling: count = files acted on); protected anchors are **not**
  added to the file count, matching the reclaimable figure. Confirm this with
  the product-owner for the new PROTECT row wording, but the *count* rule is
  already decided (N1).
- **All-anchor already-deduplicated group:** render the informational note (no
  KEEP/REMOVE, 0 reclaimable) and exclude it from group/file counts. Decide with
  the product-owner whether it appears under a dedicated "Already deduplicated"
  heading or only as a NOTE; engine must make the data available either way.

### Tests that change

- `engine.rs::hard_links_are_collapsed_and_never_removed` — still valid (single
  sibling is an anchor; separate copy is keeper). Strengthen: assert the
  hard-linked representative appears as **protected**, not keeper and not
  removable, and that the separate copy is the keeper (it is the only candidate).
  NB: previously this test made the representative the keeper (it sorted first);
  under the anchor rule the representative is now an **anchor**, so the
  **separate copy becomes the keeper**. This assertion flips — update it.
- `report.rs::hard_link_reclaimable_excludes_hard_link_sizes` — still passes;
  add assertion for the PROTECT row presence.
- `report.rs::report_shows_hard_link_note` — still passes.

### New tests required (TDD — write failing first)

1. **The reported 3-file scenario** (`beach`): `beach.jpg` ↔ `beach-hardlink.jpg`
   (one inode) + `beach-copy.jpg` (separate inode, byte-identical), `--keep
   shortest`. Assert: keeper = `beach-copy.jpg` (only non-anchor candidate;
   `shortest` is moot with one candidate); the hard-link representative is
   **protected**, never removable; `removable` is **empty**; **reclaimable = 0**
   (the only kept-vs-anchor pair frees nothing — there is no removable copy).
   *Wait:* re-examine — here `beach-copy.jpg` is the lone non-anchor, so it is
   the keeper and there is nothing else to remove → reclaimable 0, removable
   empty, one protected anchor. This is the corrected behavior vs. the defect
   (which proposed removing the anchor and counted its size). Assert exactly this.
2. **3-file scenario, two separate copies + one hard-link set:** hard-link inode
   `{h.jpg, h-link.jpg}` + standalone `c1.jpg`, `c2.jpg` (all byte-identical),
   `--keep shortest`. Assert: keeper = the `shortest` of `{c1, c2}`; the other
   standalone is `removable`; the hard-link rep is `protected`; reclaimable =
   size of the one removable standalone only (anchor and keeper excluded).
3. **Edge case 1 — all-anchor group:** two distinct inode sets, each
   multiply-linked in-scope (`a.jpg`↔`a2.jpg` inode I1; `b.jpg`↔`b2.jpg` inode
   I2; I1 and I2 byte-identical). Assert: **no `DuplicateGroup` emitted**,
   reclaimable 0, and an already-deduplicated note/notes surfaced for each inode
   set. Nothing removable.
4. **Non-anchor unaffected:** representative with **no** surviving sibling (a
   single scanned link) plus a separate copy — keeper/removable behavior is
   unchanged from today (regression guard that the anchor rule does not bar
   ordinary single-link reps).
5. **Determinism:** the anchor partition + keeper choice is byte-identical at 1
   vs 4 rayon threads and across shuffled input order (extend the existing
   determinism tests to a group containing an anchor + ≥2 standalone copies).

### Out of scope for this fix

- `--apply`/`--delete` removal of anchors: already impossible (anchors never
  enter `removable`); apply-time re-validation is unchanged. No change needed,
  but add a note in the apply task (S2/S3) that the removal set never contains
  anchors by construction.
