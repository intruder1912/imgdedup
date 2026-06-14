# In-progress review logs

This directory holds the **per-slice review logs** used during PRD-slice feature
delivery ([ADR-0013](../../adr/0013-prd-slice-delivery-cadence.md)).

Because a slice is developed on a branch that stays **PR-less until it is
complete**, the architect's **in-progress (mid-stream) reviews** have no PR to live
on. They are recorded here instead: one log file per slice, named for the slice's
branch/slug (e.g. `slice-prd-0001-detection-core.md`), created from
[`TEMPLATE.md`](TEMPLATE.md).

The log is a **working artifact that travels with the slice branch**. It captures,
for each review:

- what was reviewed (commit range), when, and why (implementer-requested, or
  product-owner/orchestrator-demanded);
- findings, each marked **fixed-now** or **deferred**;
- for deferrals: the product-owner's now-vs-defer decision and the **backlog task
  the deferral became** (deferred findings are always promoted to tracked tasks —
  nothing is lost).

When the slice's PR opens, the log is **summarized into the PR description** so the
final whole-slice review can build on it (already-reviewed chunks are re-confirmed,
not re-reviewed wholesale). By default the log file is **kept in-repo** for
traceability (it is small); the PR description is the authoritative summary the
final review builds on.

These logs are **advisory/in-progress**; the authoritative review of record is the
architect's final whole-slice review on the opened PR, under the usual
resolve-all-threads gate ([ADR-0006](../../adr/0006-post-pr-review-iteration-loop.md)).
