# TASK-0301 — Crate scaffold, CLI surface & toolchain

- **Phase:** 3 — Implementation (TDD)
- **Role:** implementer
- **Status:** todo
- **Depends on:** — (Phase 3 root)
- **Blocks:** all other Phase 3 tasks (TASK-0302..TASK-0312)

## Goal

Stand up the Rust crate, pin the toolchain, and build the **complete CLI surface**
from PRD §8 with clap (derive) — parsing, validation, help, and exit-code
constants only. No deduplication logic yet: a binary that **parses arguments and
errors correctly**, and a `make validate` / CI pipeline that gates every later
Phase 3 PR. This also **fulfils TASK-0207** (Rust toolchain wiring).

## Context & links

- **PRD-0001** §8 (CLI surface), FR-15 (exit `0`/non-zero), FR-18 (`--exit-code`),
  AC-12 (`--delete` without `--apply` rejected; clean run exits `0`), AC-19.
- **ADR-0007** — Rust + clap (derive API); `--delete`-requires-`--apply` guard as a
  typed clap constraint; toolchain (cargo, rustfmt, clippy `-D warnings`,
  cargo-llvm-cov, cargo-audit, cargo-deny). This task implements that toolchain
  (i.e. TASK-0207).
- **ADR-0008** — exit-code scheme: `0` success, `1` run-level error, `2` clap
  parse error (clap convention), `3` opt-in `--exit-code` duplicates-found.
- Reframed security gate (ADR-0007): clippy + cargo-audit + cargo-deny replace
  CodeQL (see TASK-0208).

## Acceptance criteria

Test-first (ADR-0005): write failing CLI/argument tests, then implement.

- A Cargo binary crate builds; `rust-toolchain.toml` pins the toolchain version.
- `imgdedup --help` lists every PRD §8 flag: positional `<path>...`,
  `--no-recurse`, `--hidden`, `--follow-symlinks`, `--ext <list>`,
  `--keep <shortest|oldest|newest>` (default `shortest`), `--apply`, `--delete`,
  `--json`, `--exit-code`. Each parses to a typed value.
- **AC-12 / FR-15:** `--delete` without `--apply` is rejected with a clear,
  non-zero error (exit `1`) and performs no work; a successful invocation returns
  exit `0`. A clap parse error (unknown flag / bad `--keep` value) exits `2`. These
  three exit codes are defined as named constants per ADR-0008 (`0`/`1`/`2`; `3`
  reserved for `--exit-code`, wired in TASK-0311).
- `--keep` rejects values other than `shortest|oldest|newest`.
- At least one path argument is required; invoking with none errors clearly.
- `make validate` runs and gates on: `cargo fmt --check`, `cargo clippy --
  -D warnings`, `cargo test`, and `cargo llvm-cov` (coverage measured; floor per
  ADR-0005). CI runs the same as **required checks**.

## Notes & constraints

- Use clap's **derive** API; express the `--delete`/`--apply` dependency as a typed
  clap constraint (`requires`), not a hand-rolled post-parse check, where possible.
- Exit-code constants live in one module and are the single source the rest of
  Phase 3 reuses — do not scatter literals.
- No traversal, hashing, or filesystem mutation in this task; subcommands/flags may
  be wired to stub handlers that the dependent tasks fill in.
- Keep CI cross-platform (Linux/macOS/Windows) in mind from the first workflow.

## Out of scope

- Any business logic (enumeration, sniffing, hashing, keeper, report, apply) — owned
  by later Phase 3 tasks.
- cargo-audit / cargo-deny **policy** authoring beyond wiring them to run — that is
  TASK-0208's reframed scope; this task only ensures the toolchain hooks exist.

## Deviation

**Coverage floor deferred.** The acceptance criteria mention gating `cargo llvm-cov`
in `validate`/CI. Coverage tooling is wired as `make coverage` (a non-blocking
convenience target), but an *enforced floor* is deferred to TASK-0312 (or when real
deduplication logic exists). Rationale: setting a coverage floor on stub code provides
no signal and adds CI cost for zero benefit — ADR-0005 explicitly states the floor
should be "set once meaningful code exists". The `make coverage` target remains
available for local measurement at any time.
