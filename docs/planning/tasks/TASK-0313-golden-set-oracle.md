# TASK-0313 — Test-data generator & golden-set oracle harness

- **Phase:** 3 — Implementation (TDD)
- **Role:** tester
- **Status:** done (batch 4, merged in PR #41; architect review
  approve-with-nits → nits applied → pre-merge confirmation; finding
  promoted as TASK-0316)
- **Depends on:** TASK-0308 (`--json` is the diffed contract)
- **Parallel:** no

## Goal

Correctness tests need known ground truth (owner decision, 2026-07-19: the test
oracle is a **generated golden set, not real photos**). This task delivers:

1. a **deterministic test-data generator** producing tiny valid images for every
   magic-byte-supported format,
2. a **tree builder** that materializes duplicate/near-miss directory trees with
   fully controlled content and mtimes, and
3. an **oracle harness** that runs the real `imgdedup --json` binary on a
   generated tree and structurally diffs the output against a **hand-derived
   expected manifest**.

## Context & links

- Batch 4 of the [batch plan](../batch-plan.md); owner decisions recorded there.
- **PRD-0001** — the golden set exercises FR-1..FR-5, FR-9/FR-10 (keeper
  rules), FR-14/NFR-3 (`--json`, determinism), FR-17/AC-18 (hard links).
- **ADR-0008** — keeper total order and the hard-link anchor rule the expected
  manifests encode.
- **`docs/guide/output-and-exit-codes.md`** — the schema-version-1 `--json`
  contract the harness diffs against.
- Downstream: **TASK-0312/S4** reuses the generator for AC/scale coverage;
  **TASK-0314** (field validation) complements this with read-only real-photo
  runs.

## Resolved design

- **Location: test support code only.** `tests/support/` (shared module) plus a
  new `tests/golden_set.rs` integration test. **No production code changes**;
  the unit ships on a `test/` branch with a `test:` conventional title.
- **Generator** (`tests/support/images.rs`): pure function
  `image_bytes(format, seed, len) -> Vec<u8>` for JPEG, PNG, GIF87a, GIF89a,
  WebP, BMP, TIFF-LE, TIFF-BE — a minimal valid header followed by
  deterministic seed-derived payload bytes. No RNG, no clock: identical inputs
  give identical bytes on every run and platform. Distinct seeds give distinct
  bytes (exact-duplicate vs near-miss control); `len` controls size grouping.
- **Validity bar:** generated files must be accepted end-to-end by the real
  pipeline (enumerate → sniff → engine). The golden scenarios assert
  `warnings == []`, which proves acceptance without depending on the binary
  crate's internals.
- **Tree builder** (`tests/support/tree.rs`): materializes
  `FileSpec { rel_path, bytes, mtime }` lists (plus hard links) into a
  tempdir; mtimes are set explicitly via `File::set_modified`, so JSON `mtime`
  fields are exactly predictable.
- **Oracle harness** (`tests/support/oracle.rs`): runs the built binary
  (`assert_cmd`) with `--json`, parses stdout, **relativizes all `path` fields
  against the scan root** (platform separators normalized to `/`), and
  compares the whole document to an expected `serde_json::Value` with a
  readable diff on mismatch.
- **True-oracle property:** expected manifests are **derived by hand per
  scenario** — keepers are made unambiguous *by construction* (depth, name
  length, mtime). The harness never re-implements engine/keeper logic.
- Hard-link scenarios run on all platforms (`std::fs::hard_link`; the existing
  engine tests already do this on the 3-OS matrix).

## Acceptance criteria

Test-first (ADR-0005). All scenarios below are golden-manifest diffs of the
full JSON document (groups, keeper, removable/protected, hard-link sets,
totals, warnings, root_errors) unless noted.

- **Per-format coverage:** for each of the 8 supported signatures, an exact
  duplicate pair forms exactly one group with the expected keeper; no
  warnings. (Proves every supported format flows the whole pipeline.)
- **Near-misses never group:** same format+size with different content, same
  format with different size, and different formats — zero groups, zero
  reclaimable bytes.
- **Mislabeled file:** non-image bytes named `*.jpg` produce a `sniff` warning
  and are never grouped (FR-3/NFR-2), while a genuine pair in the same tree
  still groups.
- **Keeper rules:** one tree, three runs — default (`shortest`),
  `--keep oldest`, `--keep newest` — each selecting a *different*,
  by-construction-unambiguous keeper.
- **Hard links (AC-18/FR-17):** an anchor set with an in-scope sibling copy is
  `protected` (never removable, 0 reclaimable); an all-anchor content group
  (two hard-link sets sharing one content, no standalone copy) forms no group
  and surfaces both sets under `already_deduplicated`; a *lone* fully
  hard-linked set is pinned at its current behavior (absent — see Findings).
- **Determinism (NFR-3):** two runs over the same tree produce byte-identical
  stdout; the generator itself is byte-deterministic and seed-distinct
  (meta-tests).
- `make validate` green (fmt, clippy `-D warnings`, tests, coverage).

## Findings

- **TASK-0316 (doc-vs-behavior gap, found by the oracle's first run):** a
  *lone* fully-hard-linked set — one inode, ≥2 in-scope paths, no separate
  copy of that content — appears **nowhere** in the output. The guide
  describes `already_deduplicated` as "identity sets where *every* copy is a
  hard link to the same inode", which reads as if such sets are listed; the
  engine, however, discards the hard-link note when a hash group collapses to
  a single representative (`process_hash_group`, "fewer than two distinct
  physical files"). `already_deduplicated` is only populated by all-anchor
  groups with ≥2 inodes. FR-17 safety is unaffected (nothing is ever proposed
  for removal). The golden manifest **pins current behavior** with a comment
  marking the line that flips once the product-owner decides between
  surfacing lone sets and narrowing the guide wording (backlog TASK-0316).

## Out of scope

- Perceptual/near-duplicate similarity (PRD-0001 is exact-content only —
  "near-miss" here means *looks like a duplicate candidate but is not
  byte-identical*).
- Scale/performance benchmarks and AC-19 binary E2E (TASK-0312, S4).
- Real-photo field validation (TASK-0314).
- `--apply`/`--delete` action manifests (S3; the harness is extended there).
