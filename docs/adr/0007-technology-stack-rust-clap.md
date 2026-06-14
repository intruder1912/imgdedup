# ADR-0007: Technology stack — Rust + clap

- **Status:** accepted
- **Date:** 2026-06-14
- **Deciders:** architect, human owner
- **Related:** [PRD-0001](../prd/PRD-0001-exact-image-deduplication.md)
  (NFR-1, NFR-3, NFR-4, NFR-5), [ADR-0005](0005-mandatory-tdd-and-security-gate.md)
  (TDD & security gate), [ADR-0008](0008-core-deduplication-algorithm-and-semantics.md),
  [ADR-0009](0009-cross-platform-trash-and-recycle.md),
  [ADR-0010](0010-concurrency-and-memory-model.md); fulfils **TASK-0201**;
  drives **TASK-0207** (toolchain), **TASK-0208** (security analysis),
  **TASK-0209** (Dependabot), **TASK-0211** (prebuilt binaries)

## Context and problem statement

PRD-0001 is deliberately **stack-agnostic** and hands the language and CLI
framework choice to the architect via its §10 open questions and TASK-0201. The
product is a **destructive, cross-platform CLI** whose prime directive is to
**never destroy the wrong file** (NFR-1). It must run identically on Linux,
macOS, and Windows (NFR-5), process 100k+ files and hundreds of gigabytes by
streaming and hashing in parallel (NFR-4), and be distributed as something a
non-developer can install and trust.

The choice here is the foundation every other Phase 2 ADR and all of Phase 3
build on, so it is made through a **fully-open comparison** (per the architect's
charter and the PRD's "do not pre-commit a stack" constraint). The decision was
worked through and confirmed with the human owner.

## Decision drivers

- **Safety guarantees (NFR-1, NFR-2):** the language should help the compiler
  prove that error and edge cases are handled before a destructive action runs —
  no silently-ignored error path can reach a `trash`/`delete` call.
- **Cross-platform trash (NFR-5, FR-10):** moving files to the OS trash / recycle
  bin natively on all three platforms is our **riskiest requirement**; the stack
  must support it without fragile shelling-out.
- **Distribution:** a single self-contained binary per platform, installable
  without a runtime, suitable for prebuilt GitHub Release assets (TASK-0211).
- **Performance at scale (NFR-4):** efficient parallelism and streaming I/O,
  though the workload is largely **I/O-bound**, so raw CPU speed is a near-wash.
- **Ecosystem fit:** mature, maintained crates/libraries for hashing, parallelism,
  directory walking, magic-byte sniffing, and trash.
- **Security gate (ADR-0005):** first-class lint, dependency-audit, and
  supply-chain tooling so the mandatory gate has real teeth in this language.
- **Determinism (NFR-3):** predictable, well-defined behavior across platforms.

## Considered options

1. **Rust** (+ clap for the CLI) — compiled, memory-safe, single static binary,
   native cross-platform `trash` crate.
2. **Go** (+ Cobra) — compiled, single binary, excellent cross-compilation;
   runner-up.
3. **Python** — fast to write, rich ecosystem; interpreted/packaged runtime.
4. **C++** — maximal control and speed; manual memory safety.
5. **Zig** — modern systems language, excellent cross-compilation.

## Decision outcome

**Chosen option: Rust**, with **clap** (derive API) for the CLI surface in
PRD §8, because it is the only candidate that serves both deciding drivers at
once: it solves our riskiest requirement (cross-platform trash) natively, and its
compiler-enforced error and exhaustiveness guarantees directly back the
never-destroy-the-wrong-file invariant (NFR-1).

Two factors decided it over the Go runner-up:

1. **Cross-platform trash (NFR-5).** The maintained Rust `trash` crate implements
   recoverable removal natively on all three platforms (XDG trash on Linux,
   Finder/`NSFileManager` on macOS, `IFileOperation` recycle bin on Windows — see
   [ADR-0009](0009-cross-platform-trash-and-recycle.md)). The equivalent in Go
   forces **cgo** (linking platform C/Objective-C/COM APIs) or **shelling out** to
   external binaries — both fragile, both undercutting the single-static-binary
   property, on the one requirement we least want to be fragile.
2. **Safety the compiler enforces (NFR-1, NFR-2).** Rust's `Result`-based error
   handling plus **exhaustive `match`** mean an unhandled error or an unconsidered
   state is a *compile error*, not a runtime surprise. For a tool whose worst
   failure is irreversible data loss, having the type system prove that every
   error path is handled before a destructive call is a structural safety win that
   Go's `if err != nil` convention (easy to forget, not enforced) does not match.

The workload is largely **I/O-bound**, so Go's and Rust's raw throughput are a
near-wash; performance did not decide this. clap (derive API) gives a declarative,
well-tested mapping from the PRD §8 flag surface to typed values, with generated
help, validation, and the mutually-exclusive `--delete`-requires-`--apply` guard
(AC-12) expressible as a typed constraint rather than hand-rolled checks.

### Consequences

- **Positive:**
  - Native, single-binary cross-platform trash and a memory-safe core for the
    destructive paths (NFR-1, NFR-5).
  - Self-contained per-platform binaries cross-compiled to
    `x86_64`/`aarch64` Linux, macOS, and Windows — no runtime to install — feeding
    the Release assets in **TASK-0211** and the SemVer release flow (ADR-0004).
  - Strong, maintained crates for every core need: `blake3` (hashing, ADR-0008),
    `rayon` (parallel hashing, ADR-0010), `walkdir`/`ignore` (traversal, FR-1/FR-6),
    `infer` (magic-byte sniffing, ADR-0008/FR-3), `trash` (ADR-0009),
    `clap` (CLI), `serde`/`serde_json` (`--json`, FR-14).
- **Negative / trade-offs:**
  - Steeper language and slower compile times than Go; higher contributor bar.
  - Heavier dependency tree than Go's larger standard library — managed by the
    dependency-audit gate below.
- **Follow-ups / new tasks created:**
  - **TASK-0207** wires the toolchain (see "Toolchain" below). *No separate
    toolchain ADR* — this ADR is the decision; TASK-0207 is its implementation.
  - **TASK-0208** must be **reframed** (see "Security gate" below): CodeQL has no
    first-class Rust support, so the Rust static-analysis/security gate is
    **clippy + cargo-audit + cargo-deny**, not CodeQL.
  - **TASK-0209** adds the `cargo` ecosystem to Dependabot.
  - **TASK-0211** produces the cross-compiled Release binaries.

### Toolchain (implemented by TASK-0207)

The Rust toolchain that the security gate (ADR-0005) and `make validate` will
wire:

- **cargo** — build, test, and dependency management; pinned via `rust-toolchain.toml`.
- **rustfmt** — canonical formatting, checked in CI (`cargo fmt --check`).
- **clippy** — linting with **warnings-as-errors** (`cargo clippy -- -D warnings`);
  this is the primary static-analysis layer of the security gate.
- **cargo-llvm-cov** — coverage measurement, to set the ADR-0005 coverage floor.
- **cargo-audit** — scans dependencies against the RustSec advisory database.
- **cargo-deny** — license, duplicate, and advisory policy enforcement
  (complements the licensing decision in TASK-0202).

### Security gate consequence — CodeQL is not used for Rust

ADR-0005 and TASK-0208 name **CodeQL** as the planned static-analysis layer.
**CodeQL has no first-class Rust support**, so it would add little here. This ADR
records that for the Rust stack the static-analysis and dependency-security gate
is:

- **clippy** (`-D warnings`) — the static-analysis layer in place of CodeQL.
- **cargo-audit** — vulnerability scanning of the dependency graph.
- **cargo-deny** — supply-chain / license / advisory policy.

These run in CI as required checks (alongside the existing `gitleaks` secret
scan). **TASK-0208 should be reframed** from "CodeQL + dependency audit" to
"clippy + cargo-audit + cargo-deny" when it is picked up.

## Pros and cons of the options

### Rust (chosen)

- Good: native cross-platform `trash` crate (the riskiest requirement, solved
  without cgo); compiler-enforced error/exhaustiveness handling for NFR-1;
  single static binary; memory-safe; mature crates for hashing, parallelism,
  traversal, sniffing; first-class lint and audit tooling for the security gate.
- Bad: steeper learning curve; slower compiles; larger dependency graph than Go.

### Go (runner-up)

- Good: simple language; excellent cross-compilation; single binary; strong
  standard library; fast builds.
- Bad: **cross-platform trash forces cgo or shelling out**, undermining the
  single-binary property on our riskiest requirement; error handling is a
  *convention* (`if err != nil`) the compiler does not enforce, a weaker fit for
  the NFR-1 safety invariant; no sum-type/exhaustiveness guarantees.

### Python (rejected)

- Good: fastest to write; huge ecosystem.
- Bad: **distribution** — needs an interpreter or heavyweight bundling
  (PyInstaller), against the self-contained-binary goal; the **GIL** hampers true
  parallel hashing (NFR-4); dynamic typing offers the weakest compile-time safety
  net for a destructive tool.

### C++ (rejected)

- Good: maximal control and performance.
- Bad: **manual memory safety** — the opposite of what a destructive,
  data-loss-sensitive tool wants; weaker, more fragmented cross-platform tooling
  and dependency management; slowest to deliver safely.

### Zig (rejected)

- Good: excellent cross-compilation; modern systems language; small binaries.
- Bad: **ecosystem immaturity** — pre-1.0 language and a thin package ecosystem;
  no maintained cross-platform trash library, so we would build NFR-5 from
  scratch; too much foundational risk for a safety-critical v1.

## Links

- [PRD-0001](../prd/PRD-0001-exact-image-deduplication.md) — requirements (NFR-1,
  NFR-3, NFR-4, NFR-5) this stack serves.
- [ADR-0005](0005-mandatory-tdd-and-security-gate.md) — the gate this stack's
  tooling implements; CodeQL substitution recorded above.
- [ADR-0009](0009-cross-platform-trash-and-recycle.md) — the `trash` crate that
  decided Rust over Go.
- The `trash`, `blake3`, `rayon`, `walkdir`/`ignore`, `infer`, `clap`, and
  `serde` crates; the RustSec advisory database (cargo-audit).
