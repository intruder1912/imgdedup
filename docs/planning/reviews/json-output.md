# Review log — Slice S2 `json-output` (`feature/json-output`)

- **PRD:** [PRD-0001](../../prd/PRD-0001-exact-image-deduplication.md)
- **Slice scope:** TASK-0308 — `--json` machine-readable output (FR-14 / AC-8,
  NFR-3), a second formatter over the same canonical model as the human report.
- **Status:** in progress

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

<!-- Architect whole-slice review to be appended here. -->
