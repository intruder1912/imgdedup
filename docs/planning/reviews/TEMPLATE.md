# Review log — <slice name> (`<branch>`)

- **PRD:** <PRD-XXXX>
- **Slice scope:** <tasks / requirements this slice delivers>
- **Status:** in progress | ready for PR

In-progress review log for this slice ([ADR-0013](../../adr/0013-prd-slice-delivery-cadence.md)).
One entry per review; summarize into the PR description when the slice opens.

## Reviews

### Review N — YYYY-MM-DD

- **Commit range:** `<from>..<to>`
- **Trigger:** implementer-requested | product-owner-demanded | orchestrator-demanded
- **Scope/reason:** <what + why this was reviewed (e.g. risky concurrency change)>

| # | Finding | Severity | Disposition |
|---|---|---|---|
| 1 | <finding> | blocking / should-fix / nit | fixed-now (commit `<sha>`) |
| 2 | <finding> | should-fix | **deferred** → #NNN (PO decision: defer) |

Notes: <anything the final reviewer should know>

## Deferrals summary

- #NNN — <deferred finding> (from Review N).
