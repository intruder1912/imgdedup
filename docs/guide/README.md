# imgdedup user guide

Detailed, task-oriented documentation for using `imgdedup`. This is the in-depth
companion to the [top-level README](../../README.md): the README gives you the
overview and a quick start, and the pages here go deep on each part of using the
tool.

> **Status: under active development (Phase 3).** Scanning, scope filtering,
> magic-byte image confirmation, and the core content-hash deduplication engine
> (duplicate detection) are implemented; the results report, JSON output, and the
> trash/delete actions are not yet wired — so the tool currently scans and detects
> duplicates internally but does not yet print duplicate groups or modify files.
> Pages below describe the **intended** behavior and are filled out with real
> sample output as each stage lands. See
> [`docs/planning/status.md`](../planning/status.md) for the live state.

## Guide pages

Each page is self-contained; read the one you need, or read them in order for a
full tour.

1. [Getting started](getting-started.md) — install from source and run your first
   (safe, read-only) scan.
2. [Scanning and scope](scanning-and-scope.md) — choose what gets scanned:
   recursion, hidden files, symlinks, and the image-extension allow-list.
3. [Choosing what to keep](choosing-what-to-keep.md) — how the keeper of each
   duplicate group is selected, and the `--keep` rule.
4. [Removing duplicates safely](removing-duplicates-safely.md) — the dry-run
   default, moving duplicates to the trash with `--apply`, and the guarded
   permanent `--delete`.
5. [Output and exit codes](output-and-exit-codes.md) — the human-readable report,
   `--json` for scripting, and what each exit code means.

## How this guide is maintained

User-facing documentation is owned by the **product-owner** and rides the same PR
as the feature it documents (see
[ADR-0012](../adr/0012-documentation-ownership-and-placement.md) and
[ADR-0011](../adr/0011-unit-of-work-cadence-and-planning-gate.md)). General
orientation lives in the [README](../../README.md); detailed, per-topic pages live
here. When a user-visible behavior changes, its guide page changes in the same
unit of work.
