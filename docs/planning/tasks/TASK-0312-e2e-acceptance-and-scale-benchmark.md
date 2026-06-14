# TASK-0312 — End-to-end acceptance & NFR-4 scale/benchmark

- **Phase:** 3 — Implementation (TDD)
- **Role:** tester
- **Status:** todo
- **Depends on:** TASK-0307, TASK-0308, TASK-0309, TASK-0310, TASK-0311
  (the fan-out surfaces; transitively the core + keeper)

## Goal

Prove the product end-to-end: every PRD acceptance criterion (AC-1..AC-19) verified
through the real CLI, and the NFR-4 scale targets validated (100k+ files, bounded
memory, parallel throughput), using the benchmark to set the empirical concurrency
default (ADR-0010).

## Context & links

- **PRD-0001** AC-1..AC-19 (full acceptance set), NFR-3 (determinism), NFR-4 (scale:
  100k+ images, hundreds of GB; streaming reads, parallel hashing), NFR-5
  (cross-platform).
- **ADR-0010** — concurrency width is empirical and **set here** against the NFR-4
  targets; results are independent of width (canonical sort).
- **ADR-0008/ADR-0009** — the safety semantics and cross-platform trash the E2E suite
  exercises.
- Exercises the assembled CLI from all prior Phase 3 tasks.

## Acceptance criteria

Tester-led; test-first where the suite drives missing behavior back to the
implementer.

- An end-to-end suite drives the **real CLI binary** over temp-dir fixtures and
  asserts **each of AC-1..AC-19** (dry-run, `--apply` trash, `--apply --delete`,
  `--keep` variants, `--json`, `--exit-code`, hidden/symlink/recurse scope,
  hard-links, TOCTOU re-validation, idempotence, dry-run↔apply parity).
- **NFR-4 scale:** a benchmark over a 100k+-file fixture confirms the run completes
  within target, **peak memory stays bounded** (scales with file count, not data
  volume), and hashing uses multiple cores.
- The empirical concurrency default (or read-concurrency bound) is **chosen from
  benchmark data** and recorded; determinism is shown to hold across concurrency
  widths (NFR-3, AC-16).
- **NFR-5:** the acceptance suite runs in CI on Linux, macOS, and Windows.

## Notes & constraints

- This is the integration/acceptance layer — prefer driving the built binary over
  poking internals, so the suite reflects real user behavior.
- Large-scale fixtures may be generated/synthetic; keep the heavy benchmark
  gated/optional in CI if runtime demands, but keep AC coverage always-on.
- Setting the concurrency default is a **tuning** decision (never affects results,
  ADR-0010); record the chosen value and the data behind it.
- If a gap is found, file it back to the responsible Phase 3 task rather than fixing
  product code under the tester role.

## Out of scope

- Implementing product features (owned by TASK-0301..TASK-0311).
- A user-facing `--jobs`/`--threads` flag (deferred, ADR-0010) unless a product
  decision adds it.
