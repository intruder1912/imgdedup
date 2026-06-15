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
