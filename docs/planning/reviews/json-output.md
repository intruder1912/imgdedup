# Review log — Slice S2 `json-output` (`feature/json-output`)

- **PRD:** [PRD-0001](../../prd/PRD-0001-exact-image-deduplication.md)
- **Slice scope:** TASK-0308 — `--json` machine-readable output (FR-14 / AC-8,
  NFR-3), a second formatter over the same canonical model as the human report.
- **Status:** converged — PO acceptance recorded, Review 1 findings resolved;
  PR open, awaiting owner merge

In-progress review log for this slice
([ADR-0013](../../adr/0013-prd-slice-delivery-cadence.md)). One entry per review;
summarize into the PR description when the slice opens. The product-owner
functional acceptance is recorded first; the architect's whole-slice review is
appended below after it runs.

## Product-owner acceptance — FR-14 / NFR-3 (2026-07-19)

### What I checked

Verified against `src/json.rs` (formatter + its 8 unit tests), the
`json_flag_emits_valid_json_document` integration test in
`tests/cli_integration.rs`, the `--json` wiring in `src/main.rs`, and a live
branch run over a fixture (1 duplicate group of 3 PNGs, 1 unique JPEG, 1
mislabeled `fake.png`) whose stdout, stderr, and exit code I inspected directly.

- **AC-8 — valid JSON with each group, its keeper, the intended actions, and
  totals.** The document parses as valid JSON (integration test parses stdout;
  `output_is_valid_json_with_schema_version_and_mode` and `parse()` in every unit
  test parse the rendered string). Each duplicate group carries `keeper`,
  `removable`, `protected`, `hard_link_sets`, and `reclaimable_bytes`
  (`groups_carry_keeper_removable_and_reclaimable`); the top level carries
  `totals` (`duplicate_groups`, `removable_files`, `reclaimable_bytes`,
  `already_deduplicated_sets`). In the current dry-run scope the **intended
  action** is expressed by `mode: "dry-run"` plus the per-group `removable` list
  (the files that would be removed) and `protected` (anchors kept, freeing
  nothing). FR-14's "intended **or performed** actions" — performed-action
  reporting (`mode: "trash"`/`"delete"`) — is explicitly S3 (TASK-0309/0310), out
  of this slice's scope; its absence is not a defect here.
- **Parity with the human report.** The JSON is a second formatter over the same
  `DedupOutcome` model, mirroring `report::render`'s note-partitioning: a
  hard-link note whose representative is in a group's paths attaches to that group
  as `hard_link_sets`; all-anchor sets surface top-level as
  `already_deduplicated` and are excluded from `totals`
  (`parity_with_model_groups_and_keepers`, `hard_link_set_attaches_to_group`).
  The live JSON and human samples over the same fixture agree exactly: same
  keeper (`a.png`), same two removable copies, same 48 reclaimable bytes, same
  single `sniff` warning for `fake.png`.
- **NFR-3 — determinism / canonical order.** `DedupOutcome` is already
  canonically sorted (ADR-0010); `serde_json` preserves struct field order; and
  the formatter **sorts enumerate/sniff warnings and root errors by path** before
  serializing (engine warnings arrive already canonically sorted). Two runs over
  identical state emit byte-identical documents
  (`output_is_deterministic_across_runs`). The path-sorted warnings/root-errors
  ordering is the piece the formatter itself adds on top of the model's canonical
  order — verified in `collect_sorted` and the `root_errors` sort in
  `src/json.rs`.
- **Resilience-review F3 (diagnostics as first-class output).** `warnings` (per
  file, tagged by `stage`: `enumerate` | `sniff` | `engine`) and `root_errors`
  (run-level) are part of the document, so a `--json` consumer never inherits a
  silent failure (`warnings_and_root_errors_are_first_class_f3`). This satisfies
  the resilience-review **F3 acceptance note on TASK-0308** — JSON does not drop
  the diagnostics the human report shows. Run-level root errors are
  **additionally** echoed to stderr (resilience F1), so stdout stays exactly one
  valid JSON document even when a root cannot be scanned.
- **No internal-representation leak.** The engine's `hash` field is deliberately
  absent from the schema (`no_hash_field_leaks_into_schema`), keeping the public
  contract stable and free of implementation churn.
- **`mtime` null-when-unavailable** (`mtime_serializes_as_null_when_unavailable`)
  and **lossy path serialization** (`to_string_lossy`, `U+FFFD` replacement) behave
  as documented; both are called out in the guide.

### Verdict

**Accepted.** FR-14 / AC-8 and NFR-3 are met for the dry-run scope of this
slice, the JSON reflects the same canonical model as the human report, and the
F3 diagnostics acceptance note on TASK-0308 is satisfied. User-facing docs are
delivered on this branch (README `--json` mention + full schema reference in
`docs/guide/output-and-exit-codes.md`), so the ADR-0013 PR-open gate (my
acceptance + docs) is met from the product side.

### Deferrals

None. No findings promoted to backlog tasks. The performed-action `mode`
values (`trash`/`delete`) and their reporting are already scoped to S3
(TASK-0309/0310), not deferrals from this slice.

## Reviews

### Review 1 — whole-slice code review (architect, 2026-07-19)

Reviewed the full branch diff vs `main` (`git diff main...HEAD`, commits
e409987 and b9457fe): `src/json.rs` (new formatter + 8 unit tests), the `src/main.rs`
output wiring, the `tests/cli_integration.rs` addition, `Cargo.toml`/`Cargo.lock`
(serde, serde_json), and the docs (`docs/guide/output-and-exit-codes.md` schema
reference, `README.md`, `docs/guide/README.md`). `cargo test` clean at review
time (172 unit + 16 integration, 0 failed). Built and exercised the binary on
scratchpad fixtures: a 3-copy duplicate group, an anchor+standalone group, an
all-anchor (already-deduplicated) set, a bad root, and a shortest-vs-lex
hard-link representative case.

**Verified clean:**

- **Output-only / safety (task note, AC).** `--json` changes only the output
  path: `main.rs` swaps `json::render` for `report::render` after
  `find_duplicates`, with no change to scan/group/verify. `json.rs` does no
  filesystem I/O and no recomputation — it serializes the existing
  `DedupOutcome` plus upstream warnings. No behavior change to detection.
- **Parity with the human report (ADR-0008 anchor rule).** JSON mirrors
  `report::render`'s note-partitioning: a `HardLinkNote` whose representative is
  among a group's paths attaches to that group (`hard_link_sets`); the remainder
  are all-anchor sets surfaced top-level as `already_deduplicated` and excluded
  from totals. The per-group `attached`-set approach is equivalent to the human
  report's global `claimed_representatives` partition because a representative
  path lives in at most one group. Confirmed on-fixture: keeper+protected group
  with empty `removable` and `reclaimable_bytes: 0` still emitted (matches
  `Group i/N`); all-anchor sets appear in `already_deduplicated` and are excluded
  from `totals` (2 sets, not counted in `duplicate_groups`/`removable_files`).
- **Determinism (NFR-3).** `DedupOutcome` is canonically sorted (ADR-0010);
  `serde_json` preserves struct field order; enumerate/sniff warnings and
  root_errors are sorted by path here. Two runs over the fixture emit
  byte-identical documents (verified).
- **Schema / API-contract quality.** No internal representation leaks — the
  engine `hash` field is deliberately absent (`no_hash_field_leaks_into_schema`,
  confirmed on-fixture). `schema_version`/`mode` are constants; the stability
  contract (additive-only within v1, bump only on breaking change) is sound and
  documented.
- **Security.** Untrusted filenames are serialized through `serde_json` (proper
  JSON string escaping); paths use `to_string_lossy` (U+FFFD replacement) so
  invalid-UTF-8 bytes never fail the document; no secrets. Root errors go to
  both the JSON `root_errors` array and stderr, exit code `1` (resilience F1+F3)
  — confirmed on-fixture.
- **Tests.** The contract is pinned by parse-back-and-assert-structure across the
  8 unit tests + the integration test (valid JSON, schema_version/mode,
  keeper/removable/reclaimable, model parity, determinism, first-class
  warnings/root_errors with stage ordering, `mtime` null, no `hash` leak,
  hard-link attach). TDD honoured.

**Findings:**

- **F1 (Minor, fix before PR opens) — schema doc misstates how `representative`
  is chosen.** `docs/guide/output-and-exit-codes.md` describes the HardLinkSet
  `representative` (twice: the field-reference row and the `already_deduplicated`
  row) as "the canonical, **byte-wise smallest** path" of the identity set. That
  is not what the engine does since the S2 anchor amendment: `collapse_identity`
  selects the representative by **shortest path length (`OsStr` len), with
  lexicographic path order only as the tie-break** — so it can honour `--keep
  shortest`. Verified on-fixture: a set `{zzz.png, aaaaaaaaaa.png}` yields
  `representative = zzz.png` (shortest), *not* the byte-wise-smaller
  `aaaaaaaaaa.png`. The published schema reference is factually wrong about a
  documented value. Cheap doc-only fix (reword to "the shortest path, with
  lexicographic order as the tie-break", or state the value is
  implementation-chosen and should be treated as opaque). No code change needed.
  (Note: the stale wording originates in the `HardLinkNote` docstring in
  `src/engine.rs` lines ~116–128, which is out of this slice's diff but similarly
  says "byte-wise minimum / lexicographically smallest" — worth a follow-up chore
  to correct there too.)

- **F2 (Nit) — "sorted by path" is imprecise for the `warnings` array.** The
  determinism section says `warnings` is "sorted by path before serialization",
  but the array is concatenated by stage (`enumerate` → `sniff` → `engine`) and
  sorted by path *within* each stage, so it is not globally path-sorted (an
  `enumerate` warning at `/z` precedes a `sniff` warning at `/a`). The output is
  fully deterministic — only the doc wording overstates the ordering. Reword to
  "grouped by stage, then sorted by path within each stage" (root_errors are
  globally path-sorted and correctly described).

- **F3 (Nit) — JSON-side coverage gaps for cases proven only via the shared
  model.** The unit tests assert the anchor-attach case with
  `already_deduplicated == 0`, but there is no positive JSON test that
  populates `already_deduplicated` (all-anchor set) and asserts it is excluded
  from `totals`, nor a JSON test for a keeper+protected-only group (empty
  `removable`, `reclaimable_bytes: 0`), nor an assertion that a `stage: "engine"`
  warning renders. All three behaviors are covered in `report.rs`/`engine.rs`
  tests and were verified here on-fixture, so this is thin-not-wrong; adding the
  three JSON assertions would pin the ADR-0008 edge cases directly at the JSON
  contract. Advisory — not blocking.

**Verdict:** approve-with-nits. No correctness, safety, determinism, or security
blockers; parity with the human report holds. **F1 must be fixed before the PR
opens** (it is a factual error in the published schema contract, doc-only). F2
and F3 are advisory and may be folded into the same doc pass or deferred.

### Review 1 — resolution (orchestrator, 2026-07-19)

All three findings resolved on the branch (no deferrals):

- **F1 fixed.** The `representative` row in
  `docs/guide/output-and-exit-codes.md` now reads "the shortest path, with
  lexicographic order as the tie-break". The stale `HardLinkNote` docstring in
  `src/engine.rs` flagged in the side note is corrected in the same pass (a
  disclosed comment-only ride-along rather than a follow-up chore): both the
  representative-selection claim and the sibling-order claim — post-amendment,
  `siblings` keeps the `(path length, byte-wise path)` ordering that selected
  the representative, not pure byte-wise order. `report.rs` re-sorts its
  displayed link set (`link_set.sort()`), so its "canonical byte-wise order"
  comment remains accurate; no drift there.
- **F2 fixed.** Determinism section now says `warnings` is grouped by stage
  (`enumerate`, `sniff`, `engine`) and sorted by path within each stage;
  `root_errors` globally path-sorted (unchanged).
- **F3 addressed now (tests added, not deferred).** Three JSON-contract tests
  pin the edge cases directly: `engine_stage_warning_renders`,
  `already_deduplicated_sets_are_populated_and_excluded_from_totals`
  (two same-content hard-link pairs → 2 sets, zeroed totals; a *lone*
  identity set is intentionally discarded by the engine — fewer than two
  distinct physical files — and never surfaces, matching the human report), and
  `keeper_plus_protected_only_group_renders_with_empty_removable`
  (`removable: []`, `reclaimable_bytes: 0`, both paths of the anchor's link
  set listed in `protected`). Suite now 191 tests (175 unit + 16 integration).
- **Follow-on doc precision (writing the F3 tests surfaced it):** the guide's
  `protected` bullet now states that *every path* of a non-keeper anchor set is
  listed, and that a hard-linked keeper's sibling paths (not the keeper itself)
  appear in `protected` too — verified against both branches of
  `process_hash_group`.
