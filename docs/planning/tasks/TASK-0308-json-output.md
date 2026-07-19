# TASK-0308 — JSON output

- **Phase:** 3 — Implementation (TDD)
- **Role:** implementer
- **Status:** done (slice S2, `feature/json-output`; PO accepted FR-14/NFR-3 +
  architect whole-slice review resolved — see
  [`../reviews/json-output.md`](../reviews/json-output.md))
- **Depends on:** TASK-0305
- **Parallel:** yes (with TASK-0307)

## Goal

`--json` emits valid, machine-readable output — each group, its keeper, the intended
or performed actions, and totals — suitable for scripting, via serde_json.

## Context & links

- **PRD-0001** FR-14 (`--json` machine-readable: groups, keeper, intended/performed
  actions, totals), AC-8.
- **ADR-0007** — `serde`/`serde_json`.
- **NFR-3** — JSON output is deterministic (canonical order, TASK-0306).
- Consumes the same canonical group model as the human report (TASK-0307).

## Acceptance criteria

Test-first (ADR-0005).

- **AC-8 (FR-14):** with `--json`, output is **valid JSON** containing each group,
  its keeper, the intended (dry-run) or performed (`--apply`/`--delete`) actions, and
  totals — asserted by parsing the output back and checking the structure.
- The JSON reflects the same groups/keepers/actions/totals as the human report for
  identical input (parity with TASK-0307's model).
- Output is deterministic across runs (canonical order, TASK-0306).
- Field schema is documented so scripts can rely on it.

## Notes & constraints

- Share the in-memory result model with the human report (TASK-0307); the two are
  formatters over one model, not parallel computations.
- Use serde derive; keep the schema stable and explicit (avoid leaking internal
  representations that would churn the public contract).
- `--json` does not change what is scanned, grouped, or removed — output only.

## Out of scope

- Human-readable report (TASK-0307).
- Apply/delete execution (TASK-0309/TASK-0310).
- Exit-code wiring (TASK-0311).
