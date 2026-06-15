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

## Product-owner re-acceptance under the anchor model — 2026-06-15

The implementer landed the anchor rule under TDD (165 tests green). I re-verified
the functional behavior against the rendered output (`src/report.rs` tests),
`DuplicateGroup.protected` (`src/engine.rs`), and ADR-0008 Amendment 2026-06-15.

### Acceptance verdict

- **FR-17 (hard links already-deduplicated; not proposed for removal; excluded
  from reclaimable; may be an informational note) — ACCEPTED.** A representative
  with a surviving in-scope sibling is now an **anchor**: never in `removable`,
  never counted in reclaimable, surfaced as a `PROTECT` row plus the `NOTE`
  hard-link-set line. Verified by `report.rs::hard_link_reclaimable_excludes_hard_link_sizes`,
  `beach_scenario_anchor_protected_reclaimable_zero`,
  `total_files_and_reclaimable_exclude_protected_anchors`, and
  `report_shows_hard_link_note`. The earlier defect (anchor leaking into
  `removable` with its size counted) is gone.
- **AC-18 (two hard links to the same content: neither proposed for removal; do
  not contribute to reclaimable) — ACCEPTED.** Both links share one inode → one
  anchor representative, never removable, size never in reclaimable
  (`beach_scenario_…` asserts `removable` contains neither link and reclaimable
  is 0). The pure all-hard-link case emits no removal opportunity at all
  (`all_anchor_group_notes_surface_no_group_in_report`).
- **FR-7 / FR-8 (keeper rule + `--keep`) — UNAFFECTED, still accepted.** The
  `--keep` order now runs over the *genuinely-removable* (non-anchor) members
  only; the order itself is unchanged. With one non-anchor candidate it is the
  keeper trivially; with several, the documented tie-break order still applies.
  Architect Review 1/2 verified the orders match ADR-0008 and are total.
- **FR-9 (dry-run, mutates nothing) — UNAFFECTED.** `dry_run_does_not_modify_any_file`
  still green; the report remains a pure formatter.
- **FR-13 (human-readable summary: groups, keeper, removed files, counts,
  reclaimable) — UNAFFECTED in substance.** Counts remain keeper+removable per
  the N1 ruling. (Two cosmetic refinements are specified below — singular/plural
  and removable-less framing — neither changes what FR-13 mandates; both improve
  clarity.)
- **FR-15 / FR-18 (exit codes / `--exit-code`) — UNAFFECTED.** No exit-code logic
  touched by the anchor fix.
- **NFR-3 (determinism) — UNAFFECTED, still accepted.** The anchor partition is a
  total deterministic function computed before keeper selection; `protected` is
  canonically sorted like `removable` (ADR-0008 edge case 3). Determinism tests
  pass across thread widths and input orders.
- **NFR-4 (scale / no whole-file buffering) — UNAFFECTED.** Anchors are skipped,
  not byte-verified; no new reads or buffering introduced.
- **NFR-6 (robust per-file errors) — UNAFFECTED.** Warning surfacing is unchanged;
  the orphaned-note (S1) fix is preserved and extended to anchors.

**Overall: the slice's functional requirements remain accepted under the anchor
model.** Two of the presentation refinements below ((b), (c)) require small
`report.rs` string/format changes; per ADR-0013 I am the PR-open gate, so the PR
opens only after those land under TDD and I have re-confirmed the docs.

### Presentation decisions (architect-deferred — authoritative)

**(a) PROTECT-row wording — KEEP AS-IS.** The row `PROTECT <path>  (hard-linked;
removing it frees no space)` (rendered with the same two-space indent as
`KEEP`/`REMOVE`) is accurate, parallel to those rows, and states the *why* in
plain terms. No change.

**(b) Removable-less group framing — REFRAME (requires `report.rs` change).** A
group with a keeper + protected anchor(s) but **nothing removable** must **not**
be presented as a "duplicate group" nor counted in the "duplicate groups" /
"files" / "reclaimable" totals. Showing `Group 1/1 (1 file, 0 bytes reclaimable)`
with no `REMOVE` row reads as "you have a duplicate" when there is in fact nothing
to do — alarming and wrong. Such groups are *already deduplicated*; surface them
under a dedicated informational block (shared with edge case (d)). See the exact
spec under "Presentation decisions — report-string spec".

**(c) Pluralization glitch — FIX (requires `report.rs` change).** The group header
must use `1 file` / `N files` consistently with the summary line. See spec below.

**(d) All-anchor groups — SURFACE as informational (requires `report.rs` change).**
"No duplicate groups found." is misleading when already-deduplicated hard-link
sets exist. Fold these into the same "Already deduplicated (hard-linked)" block as
(b). They remain excluded from the duplicate-group/file/reclaimable counts (they
free nothing), but the user is told they exist and why nothing is proposed.

### Presentation decisions — report-string spec (for the implementer, TDD)

These are the exact desired output strings/format. (a) is unchanged; (b), (c),
(d) are the changes.

**(c) Group-header pluralization.** In the group header, pluralize `file` exactly
as the summary already does:

```text
Group {i}/{N} ({M} file{s}, {X} bytes reclaimable):
```

where `{s}` is empty when `M == 1`, else `"s"`. (`M` = keeper + removable, the
existing count.) Nothing else in the header changes.

**(b)+(d) "Already deduplicated (hard-linked)" informational block.** Define a
*removal opportunity* as a group with **≥1 removable** file. Only removal
opportunities are "duplicate groups": they keep the existing `Group i/N …`
rendering, the `Group`/`file`/`reclaimable` counts, and the `Summary:` line —
**unchanged**.

Everything that is already-deduplicated and offers nothing to remove is collected
into a separate, clearly non-alarming block that does **not** use the word
"duplicate" and is **not** counted in the duplicate-group/file/reclaimable totals.
This block covers two cases the engine produces:

1. An emitted `DuplicateGroup` whose `removable` is **empty** (keeper + ≥1
   protected anchor, nothing to remove) — case (b).
2. All-anchor identity sets that emit **no** `DuplicateGroup` and surface via
   `hard_link_notes` — case (d).

Desired format (the block appears **after** the duplicate groups, **before** the
`Summary:` line; omit the whole block when there is nothing already-deduplicated):

```text
Already deduplicated (hard-linked) — nothing to reclaim:
  /photos/beach.jpg
    hard-linked: /photos/beach.jpg, /photos/beach-hardlink.jpg
  /photos/holiday.png
    hard-linked: /photos/holiday.png, /photos/holiday-2.png
```

Format rules:

- Heading line exactly: `Already deduplicated (hard-linked) — nothing to reclaim:`
  (em dash `—`, matching the existing report's punctuation style; keep
  markdownlint/cspell-clean — it is code-fenced output, not prose).
- One entry per already-deduplicated **inode/identity set**. The first indented
  line (2 spaces) is the representative path. The second line (4 spaces) is the
  literal `hard-linked:` label followed by the **full link set** for that inode —
  i.e. the representative plus its siblings — joined by a comma and a space, in
  canonical (byte-wise) order, so the user sees every path that shares the storage.
  - For case (b) (a removable-less emitted group), the entry's representative is
    the group's keeper-or-protected representative that carries the
    `HardLinkNote`; list the note's representative + siblings.
  - For case (d) (all-anchor, no group), emit one entry per `HardLinkNote`
    (one per inode), each listing representative + siblings.
- The block is **purely informational**: its entries are **not** added to the
  `total_groups`, `total_files`, or `total_reclaimable` accumulators, and not to
  the `Group i/N` numbering.
- **Summary line interaction.** The `Summary:` line counts only the real
  duplicate groups (removal opportunities). When the *only* findings are
  already-deduplicated sets (no removal opportunities at all), do **not** print
  `No duplicate groups found.` followed by nothing; instead print the
  already-deduplicated block and then a summary that reflects zero reclaimable
  groups, e.g.:

  ```text
  Already deduplicated (hard-linked) — nothing to reclaim:
    /photos/beach.jpg
      hard-linked: /photos/beach.jpg, /photos/beach-hardlink.jpg

  Summary: no duplicate groups to remove; 1 already-deduplicated hard-link set.
  ```

  Pluralize `set` (`1 … set` / `N … sets`). When there are **no** findings of any
  kind (no groups, no already-deduplicated sets), keep the existing
  `No duplicate groups found.` exactly as today.
- When there are **both** real duplicate groups **and** already-deduplicated
  sets, render the groups (with the normal `Summary:` line counting only the
  groups), the already-deduplicated block, and a single `Summary:` line. Append a
  trailing clause to that summary noting the already-deduplicated sets, e.g.:
  `Summary: 2 duplicate groups, 5 files, 3072 bytes reclaimable; 1
  already-deduplicated hard-link set (nothing to reclaim).` Pluralize `set`.

Net effect: the words "duplicate group" and the reclaimable total now mean
exactly "things you can act on to free space", and hard-linked already-dedup sets
are reported honestly without ever masquerading as a removal opportunity. The
`PROTECT`/`NOTE` rows inside a *removal opportunity* group (a group that still has
≥1 removable file alongside a protected anchor — e.g. two standalone copies + one
hard-link set) are **unchanged** by this spec.

### Acceptance gate note (ADR-0013)

(b), (c), (d) are cosmetic/clarity refinements, not safety or correctness
defects — the anchor safety behavior (FR-17/AC-18) is **already correct and
accepted now**. I am **not deferring** them: they ship inside this S1 slice under
TDD before the PR opens, because they change user-visible report text this slice
introduces and the docs below describe the reframed output. The implementer
should add report tests for: header pluralization (`1 file`), the
already-deduplicated block for a removable-less group (case b), the all-anchor
block (case d), and the combined summary clause.

## OPEN — Review 3 pending (architect re-review deferred to next session)

The architect re-review of the anchor fix + presentation changes is **deferred by
the owner** to the next session; it runs on the open PR **before merge**. The PR
is opened with the items below recorded so the reviewer has them up front.

### OPEN CONCERN (orchestrator-found, escalate at re-review) — anchor-keeper rule under-reclaims and can mislead

Found during end-to-end verification of the presentation changes. In a group made
of **one** standalone copy **plus** a hard-link set (all byte-identical), the
approved anchor rule ("anchor is never the keeper; the keeper is chosen among the
non-anchor candidates only") makes the lone standalone the keeper, leaves
`removable` empty, and the report renders the group in the **"Already deduplicated
(hard-linked) — nothing to reclaim"** block.

Concrete repro (`--keep shortest`): `beach.jpg` ↔ `beach-hardlink.jpg` (hard
links) + `beach-copy.jpg` (separate identical copy). Actual output:

```text
Already deduplicated (hard-linked) — nothing to reclaim:
  /…/beach-hardlink.jpg
    hard-linked: /…/beach-hardlink.jpg, /…/beach.jpg

Summary: no duplicate groups to remove; 1 already-deduplicated hard-link set.
```

Three problems with this outcome:

1. **It misleads.** `beach-copy.jpg` is a genuine standalone duplicate whose
   removal **would** free real space, yet the report says "nothing to reclaim" and
   does not list it at all.
2. **It under-reclaims.** The tool reclaims 0 bytes when 1 copy's worth is safely
   reclaimable (remove the standalone, keep the hard-link set).
3. **It violates `--keep`.** The user asked to keep the *shortest* path
   (`beach.jpg`); `beach.jpg` is in the protected set, so the tool kept
   `beach-copy.jpg` instead.

Root cause: the edge-2 ruling ("anchor never keeper") is the wrong tradeoff. The
likely correct model: a hard-link set (already deduplicated, with multiple wanted
references) is the **natural keeper** — when `--keep` selects a file that belongs
to a hard-link set, keep that whole set (protected) and make the standalone copies
**removable**. Then the repro keeps the hard-link set (honoring `--keep shortest` →
`beach.jpg`) and removes `beach-copy.jpg` (reclaim = its size). This needs an
architect ruling (likely an ADR-0008 follow-up amendment) and a follow-on engine
fix; the case-(b) "already-deduplicated" reframing then only applies to genuinely
all-hard-link sets (case d). The current behavior is **read-only and safe** (it
removes nothing), so it is not a data-loss risk — but it should be resolved before
the destructive slice (S3) and before this slice merges if the reviewer agrees.
