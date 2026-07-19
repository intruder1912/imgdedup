# Resilience & error-path review — 2026-07 (TASK-0216, second half)

Whole-pipeline review of failure handling across
`enumerate → sniff → engine → report → exit`, per the
[batch plan](../batch-plan.md) Batch 2. Scope: what happens on I/O errors,
missing/unreadable paths, degenerate metadata, and hash edge cases — does the
tool degrade safely and *visibly*? Reviewer: orchestrator (Fable), architect
confirmation on the PR diff.

## Verdict

The safety-critical core degrades **safely** everywhere examined: every I/O
failure funnels into a warning on the conservative side (a file we cannot
read/identify/verify is never proposed for removal). One defect was found in
*visibility* — a run-level failure was silent — fixed in this unit.

## Findings

### F1 — DEFECT (fixed here): silent run-level failure

`CandidateSet.root_errors` (nonexistent/unreadable scan root) correctly drove
exit code 1 (`exit_code::compute`) but was **never rendered**: `report::render`
receives only `warnings` + `sniff_warnings`, and stderr was used only for the
`--delete`-without-`--apply` message. `imgdedup /no/such/path` printed a
normal "no duplicate groups found" report and exited 1 with no explanation.

**Fix:** `main.rs` now prints each root error to **stderr**
(`error: <path>: <reason>`) before the report; the run still continues over
remaining roots and exits 1 (partial results + explicit failure). Stderr keeps
the machine-readable stdout surface clean for `--json` (TASK-0308).
Regression-locked by integration test
`nonexistent_root_prints_error_to_stderr_and_exits_1`.

### F2 — Confirmed safe: engine I/O failure paths

- Hash failure → `EngineWarning` ("could not hash file"), file excluded.
- Identity (`Handle::from_path`) failure → warning, file **dropped from the
  group** — the safe direction (never remove what we cannot identify).
- Byte-verify `Err` → warning, file **not removed**. Hash-collision
  (`Ok(false)`) → warning naming the BLAKE3 pre-filter, not removed.
- FD pressure bounded: identity handles live only within one hash-group.

### F3 — Acceptance note for S2 (TASK-0308): JSON must carry diagnostics

The JSON model must include per-file warnings (enumerate/sniff/engine) **and**
root errors (or JSON consumers re-inherit F1). Added to the TASK-0308
acceptance expectations in the backlog.

### F4 — Confirmed by design: TOCTOU window

Analysis-time byte-verify can go stale before `--apply`. Already covered:
FR-16 apply-time re-validation is a hard S3 requirement (ADR-0008); nothing
in the current read-only surface mutates, so no exposure today.

### F5 — Accepted: degenerate mtimes

Pre-epoch or unavailable mtimes → `None` → deterministically treated as
oldest (keeper tests cover all three rules). Documented behavior; no change.

### F6 — Confirmed: symlink loops

`walkdir` with `follow_links(true)` detects cycles and yields an error entry
→ per-entry warning; traversal continues. Matches the module contract.

### F7 — Noted (no change): warnings render to stdout

Per-file warnings are part of the human report (FR-13/NFR-6) on stdout;
run-level errors now go to stderr (F1). For scripting, `--json` (S2) is the
contract surface; no further split warranted now.

### F8 — Noted for S4: custom `--ext` formats always warn

An extension outside the six sniffable formats (e.g. camera RAW via `--ext`)
can never confirm, so every such file becomes a sniff warning. Documented in
the sniff module; S4's field validation should confirm the UX is acceptable
at realistic volumes (belongs to TASK-0312/0314 scope).

## Outcome

TASK-0216 complete: refresh half (PR #35) + this review + F1 fix. F3 rides
TASK-0308; F8 rides TASK-0312/0314. No other deferrals — nothing else met the
bar for a tracked task.
