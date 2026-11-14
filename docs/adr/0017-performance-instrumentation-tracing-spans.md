# ADR-0017: Performance instrumentation — `tracing` spans + an opt-in `--timings` consumer

- **Status:** accepted
- **Date:** 2026-08-05
- **Deciders:** architect, human owner
- **Related:** [PRD-0001](../prd/PRD-0001-exact-image-deduplication.md) (NFR-4
  scale; FR-19/AC-21..24 progress, whose stderr contract this must not break),
  [ADR-0007](0007-technology-stack-rust-clap.md) (Rust; dependency-audit
  posture), [ADR-0008](0008-core-deduplication-algorithm-and-semantics.md) (the
  pipeline whose stages these spans mirror),
  [ADR-0010](0010-concurrency-and-memory-model.md) (the throughput questions
  this instrumentation exists to answer, and the ones it leaves open),
  [ADR-0014](0014-project-license-gpl-3.0-only.md) (GPL-3.0-only → dependency
  license compatibility), TASK-0312/TASK-0321/TASK-0322 (the prior
  measurements), TASK-0317/TASK-0328/TASK-0332 (the progress hot-path prior
  art), TASK-0318 (`safe_text.rs`, the control-character escaping rule),
  TASK-0338 (the implementation this ADR hands off)

## Context and problem statement

The owner wants to be able to **assess `imgdedup`'s performance on large, real
image sets** — motivated by session 29's informal `/usr/bin/time -l` numbers on a
2,021-file / 5.72 GB real photo corpus (recorded in
`docs/planning/status.md` (removed from `main` by ADR-0021; in the git history)). Those numbers are a single
wall-clock total: **0.50 s for the whole run**, with no way to say how much of it
was the tree walk, magic-byte confirmation, hashing, byte-verification, or
report rendering. `tests/scale.rs` (`make scale`, NFR-4) is and remains the
project's controlled regression benchmark, but it measures the same
end-to-end totals.

That missing breakdown is not a hypothetical gap — it is the *exact* thing this
project has repeatedly had to reconstruct by building throwaway control
fixtures:

- **TASK-0312** could only show that the (then-serial) `enumerate` + `sniff` pair
  dominated small-file runs by constructing a fixture in which *no file is
  hashed at all* and observing that wall time did not change.
- **TASK-0321** could only isolate `sniff`'s cost by building a 60k-file fixture
  in which *every candidate fails confirmation*, then comparing two release
  binaries.
- **TASK-0322** measured `update_rayon` in a standalone, not-checked-in
  benchmark harness because the real binary could not report per-stage time.
- ADR-0010's most recent consequence note ends on an **explicitly unresolved**
  question — "with sniff's cost cut ~3-7x, `enumerate` (walkdir, still fully
  serial) is a plausible new dominant serial stage... **not re-measured here**".

Every one of those would have been a single run of an instrumented binary. The
decision here is therefore: **what instrumentation to build in, at what
granularity, and what — if anything — makes it observable on day one without
standing up any external infrastructure.**

**Assumptions, confirmed with the owner in the originating conversation:**

1. Instrumentation must be **always compiled in and inert by default** — not
   behind a Cargo feature that release binaries would ship without.
2. It must be usable **with no collector and no OpenTelemetry backend
   connected**; bridging to OTLP later (via `tracing-opentelemetry`) must remain
   possible but is explicitly **not** in scope.
3. `tracing`, not the OpenTelemetry SDK, is the instrumentation layer (see the
   first decision below — recorded here properly, but already settled).

## Decision drivers

- **Answer the open ADR-0010 question**: which pipeline stage dominates, on a
  real corpus, on the user's own hardware — without a bespoke fixture.
- **Zero cost when unused.** A tool whose steady state is "no subscriber
  installed" must pay nothing measurable for carrying the instrumentation.
- **Bounded output and bounded overhead at scale.** NFR-4 targets 100k+ files;
  anything proportional to file count is a design error, in overhead *and* in
  lines printed.
- **Must not corrupt the existing stderr surface.** `progress.rs` redraws a
  single un-terminated line in place; its module doc states outright that
  *nothing else may write to stderr while that line is live*.
- **Dependency weight and license hygiene.** GPL-3.0-only (ADR-0014); every
  dependency passes `deny.toml`; this project checks the graph *before* adding
  (ADR-0010's blake3-`rayon` precedent).
- **Instrumentation is strictly an output concern.** Like progress (AC-24), it
  must never influence what is scanned, grouped, or removed.
- **Security:** paths and filenames are untrusted input (TASK-0318).

## Considered options

**A. Instrumentation layer**

1. **`tracing` spans, no SDK.** (chosen)
2. **OpenTelemetry Rust SDK + OTLP exporter.**
3. **A hand-rolled in-house timing module** (`Instant` + a `Stage` enum), zero
   dependencies.
4. **Nothing built in** — keep writing throwaway benchmark harnesses per
   question.

**B. Span granularity**

1. **Coarse: one span per pipeline stage, with aggregate fields.** (chosen)
2. **Fine: per-file / per-directory-entry spans.**
3. Coarse spans that **mirror `progress::Phase` exactly**.

**C. Day-one consumer**

1. **Ship a minimal `tracing-subscriber` `fmt` consumer behind a new
   `--timings` flag, in the same unit.** (chosen)
2. **Instrumentation only**; defer every consumer to a follow-up.
3. `RUST_LOG`-driven `env-filter` consumer.

## Decision outcome

### 1. `tracing`, not OpenTelemetry (recorded; already settled with the owner)

**Chosen: `tracing` (+ `tracing-core`), with no OpenTelemetry SDK.**

`imgdedup` is a **fully synchronous CLI**. Its nine dependencies (`blake3`,
`clap`, `infer`, `rayon`, `same-file`, `serde`, `serde_json`, `trash`,
`walkdir`) include no async runtime anywhere. OpenTelemetry's OTLP exporters
pull in `tonic`/`hyper`, which require `tokio` — adopting them means adding an
async runtime to a synchronous tool **solely to support an optional telemetry
backend that essentially no run will ever connect to**. That is a genuine
architectural addition (a second execution model alongside rayon, ADR-0010) for
zero benefit to the stated goal.

`tracing` is the Rust ecosystem's actual zero-cost instrumentation layer: a span
macro compiles down to a check of whether anything is subscribed, and does no
work when nothing is. **Measured** (this session, release build, Apple M2 Pro;
harness in the scratchpad, not checked in): 3,000,000 span create+enter+exit
cycles with **no subscriber installed** cost **3.73 ms** against a **3.29 ms**
baseline for the identical loop without spans — i.e. **~0.15 ns per span**, at
or below measurement noise. The ~11 spans this ADR adds per run are therefore
free, exactly, in the default configuration.

Crucially this forecloses nothing: `tracing-opentelemetry` is a *layer* that
bridges the same spans into an OTLP pipeline. If a collector is ever wanted, it
is an additive change at the subscriber, and **not one span annotation moves**.
That optionality is the reason to prefer `tracing` over option A3 (an in-house
timing module), which would be smaller and dependency-free but would be a
dead end the day anything external wants to consume it — and would re-implement
nesting, thread-safety, and formatting that `tracing` already provides,
audited, for free. A4 (build nothing) is rejected on the evidence above: four
separate tasks have already paid the cost of not having this.

### 2. Span boundaries: the ADR-0008 **pipeline**, not the display `Phase`

**Chosen: one span per pipeline stage, carrying aggregate fields — 11 span
sites, a bounded ~11 span instances per run regardless of file count.**

| Span (INFO) | Opened by | Fields recorded at close |
|---|---|---|
| `run` | `main.rs::run` | `roots`, `apply`, `json` |
| `enumerate` | `enumerate::enumerate_with_progress` | `roots`, `candidates`, `warnings`, `root_errors` |
| `symlink_alias_dedupe` *(conditional)* | `enumerate::enumerate_with_progress` | `candidates_in`, `collapsed` |
| `sniff` | `sniff::filter_candidates_with_progress` | `candidates`, `confirmed`, `rejected` |
| `dedup` | `engine::dedup_core_with_sink` | `files`, `groups` |
| `group_by_size` | `engine::dedup_core_with_sink` | `files`, `cohorts`, `size_singletons` |
| `hash` | `engine::dedup_core_with_sink` | `files_hashed`, `bytes_hashed`, `errors` |
| `verify` | `engine::dedup_core_with_sink` | `groups`, `files_verified` |
| `sort_canonical` | `engine::dedup_core_with_sink` | `groups` |
| `apply` | `apply::apply` | `mode`, `groups`, `removed`, `skipped` |
| `render` | `main.rs::run` | `format`, `bytes` |

**Ownership rule: each stage's span is opened by the function that owns the
stage**, not centrally in `main.rs`. This keeps the instrumentation next to the
code it measures, makes it visible to that module's own unit tests, and means
any future caller (not just `main`) gets it. `main.rs` owns only the two spans
that are genuinely its own (`run`, `render`) plus the subscriber's lifetime.

**Why these boundaries deliberately do *not* mirror `progress::Phase`.** This is
the central design point, and the answer is emphatically "differ":

- `Phase::Scanning` **merges `enumerate` and `sniff` on purpose** — a display
  decision (AC-21/AC-21a, TASK-0328's product ruling: the label must not flip
  more than necessary). But separating those two is *precisely* the measurement
  this project keeps needing, and the one ADR-0010 leaves open in writing.
  Instrumentation mirroring `Phase` would be structurally incapable of
  answering the question it exists for.
- `Phase` **stops at `Verifying`**. Everything after it — canonical sorting, the
  destructive apply pass, report/JSON rendering — is completely unmeasured
  today. `apply` in particular does per-file re-validation I/O plus a trash
  operation per file and has never been timed.
- `Phase` is a **product surface with acceptance criteria**; changing it needs a
  product-owner ruling. Spans are an engineering surface; they must be free to
  be added, split, or renamed by an implementer chasing a number, without
  touching PRD-0001.

So: **`Phase` and spans are deliberately decoupled — neither derives from the
other, and neither may be refactored to.** The mapping is documented (above)
rather than encoded. One shared invariant does bind them, though, and it is the
one that matters: like progress (AC-24), **instrumentation never influences what
is scanned, grouped, or removed**.

**Why not finer than this (the per-file / per-entry question).** This project has
direct prior art on exactly this tension. TASK-0332 added `sink.advance(0)` —
a "tick" in the hottest loop of the walk, called once per *directory entry
visited* — and it is affordable only because it was engineered to be: a
`Relaxed` atomic add, a `Relaxed` load comparison, and a `try_lock` that is
skipped entirely unless a repaint is due (TASK-0317/TASK-0328's throttling
work). A span in that same position is a different animal. **Measured**, same
harness, same machine:

| Span position | No subscriber | Subscriber installed, span at its own level | Subscriber installed, span level filtered out |
|---|---|---|---|
| per iteration | ~0.15 ns | **~1.75 µs** | ~0–1 ns |

At 30,000 entries a per-entry span costs **~52 ms and prints 30,000 lines** the
moment anyone turns instrumentation on — on a run whose *entire* current cost at
2,021 files is 500 ms. It would swamp the very measurement it is supposed to
produce, and bury the stage totals in noise. Rejected.

The information a per-file span would carry is instead captured as **aggregate
fields on the stage span** (`files_hashed`, `bytes_hashed`, …), declared
`tracing::field::Empty` at span creation and filled with `Span::record` before
close. That yields throughput (bytes/s, files/s per stage) at **O(1) events per
run** instead of O(files). Same reasoning rejects per-root spans inside
`enumerate` (`dedupe_roots` is explicitly built for tens of thousands of roots)
and per-group spans inside `verify` (group count is data-dependent and
unbounded); both become count fields.

**No span is ever created inside a rayon closure.** Every span above is opened
and closed on the single orchestrating thread; the `hash`/`sniff` spans simply
*wrap* the `par_iter` call, so their `time.busy` is the wall time of the whole
parallel section — which is the number wanted. This also means there is no
cross-thread span-context propagation problem to solve, and no shared-registry
contention on the parallel hot path (the concern that shaped `progress.rs`'s
lock-free numerator).

**Reading the output:** with a guard held for the span's whole extent,
`time.busy` ≈ wall time and `time.idle` ≈ 0. `time.busy` is the number to read.

**Zero-workload stages.** `verify` (and, on the `--no-recurse`/empty-tree paths,
`hash`) can have nothing to do. Unlike `progress::Phase` — which *skips* a
zero-workload phase outright rather than render a degenerate `0 of 0` — a
measurement surface benefits from the opposite: emit the span anyway, with its
count fields at zero, so a reader can tell "this stage cost nothing" apart from
"this stage was never instrumented". The stage line set is then identical run to
run, which also makes it assertable in a test.

### 3. Day-one consumer: yes — an opt-in `--timings` flag, in the same unit

**Chosen: ship a minimal `tracing-subscriber` `fmt` consumer, off by default,
enabled by a new `--timings` boolean flag, writing to stderr.**

Instrumentation with no subscriber attached produces *literally nothing
observable*. Shipping "spans only" would deliver zero of the stated goal, and
would produce a PR with no behavior to test — which sits badly with mandatory
TDD (ADR-0005): the only honest assertions would be "it still compiles". The
consumer is ~15 lines. It ships together.

Configuration, decided concretely:

- **`--timings`**, a plain boolean, default off. Name chosen over `--trace`
  (implies tracing-the-tool, and collides conceptually with a future OTLP
  bridge), `--profile` (implies a sampling profiler), and `--verbose`/`-v`
  (deliberately left free for a future general logging verbosity flag).
- Writer: **stderr** — stdout stays the one report/JSON document (the S2 purity
  contract; `--timings --json` is therefore the ideal scripted measurement mode
  and must stay so).
- `FmtSpan::CLOSE` only — one line per stage, at stage end. No enter events, no
  per-event lines.
- `with_max_level(INFO)`, `with_target(false)`, **`with_ansi` off** (the
  `ansi` feature is not even compiled in — see below), timestamps kept (they
  make interleaving with the apply-time `removed:`/`warning:` lines analyzable).
- No ANSI escape/control-code machinery (the `ansi` feature is not compiled
  in), unlike `progress.rs`'s stricter *pure*-ASCII rule: `tracing-subscriber`'s
  own `Duration` formatting renders the microsecond unit as `µ` (U+00B5) —
  confirmed against a real binary (`time.busy=207µs`) — so this output is
  UTF-8, not ASCII-only. A single non-ASCII unit glyph inside a numeric field
  does not carry the injection/corruption risk that rule exists to prevent
  (§6: only untrusted path/filename bytes do), so this is accepted rather than
  worked around.
- Installed with **`try_init()`**, and a failure to install is **non-fatal** —
  instrumentation must never be the reason the tool fails (the AC-24 principle
  again).

Representative output (verified against a real binary in the design harness):

```text
2026-08-05T19:11:59.732293Z  INFO run{roots=1 apply=false json=false}:enumerate{...}: close time.busy=118ms time.idle=1.4µs
```

**A hard implementation constraint, recorded because it is easy to get wrong:**
`main()` ends in `process::exit(code)`, which **does not run destructors**. The
root `run` span's guard must therefore be created *and dropped inside `run()`*
— exactly like `SinkGuard` already is — or its close event never fires and the
run-total line silently disappears. This is a testable property, not a comment.

**Rejected: `RUST_LOG` / `env-filter` (option C3).** `tracing-subscriber`'s
`env-filter` feature pulls in `regex` (plus `matchers`, `regex-automata`,
`regex-syntax`, `aho-corasick`) — a large, wholly disproportionate addition for
parsing one boolean's worth of intent. A flag is also more discoverable
(`--help`) and consistent with every other knob on this CLI. If per-module
filtering is ever genuinely wanted, `--timings` can grow a value
(`--timings=debug`) without adopting `env-filter`.

**Deliberately deferred, and cheap when wanted:** a machine-readable form.
`tracing-subscriber`'s `json` feature needs `serde`/`serde_json` — **both
already direct dependencies** — plus `tracing-serde`. Not now (nothing consumes
it yet); noted so the follow-up is a known-small step.

### 4. Coexistence with `progress.rs`: `--timings` turns progress off

**Chosen: `--timings` implies progress-off.** Effective rule, one place in
`main.rs`: `progress_enabled = !args.no_progress && !args.timings`.

`progress.rs`'s module doc is unambiguous: on a TTY the progress line is redrawn
in place and left **un-terminated** between repaints, so any other write to
stderr while it is live lands *in the middle of it*
(`scanning...2026-08-05T19:11:59Z INFO ...`). Stage-close lines land squarely
inside that window, by construction — a stage closing is exactly when progress
is live.

Alternatives weighed and rejected:

- **Usage error on `--timings` without `--no-progress`** — the precedent exists
  (`--delete` without `--apply`, exit 2), but that one guards a *safety*
  mismatch about a destructive action. Here the user's intent is unambiguous and
  there is exactly one correct interpretation; making them type two flags for
  one intent is friction with no safety payoff.
- **Buffer timing lines and flush after `finish()`** — requires a custom
  `MakeWriter` plus flush-ordering that must not disturb AC-20's
  progress-before-stdout guarantee, and it destroys the main practical value of
  live stage lines on a long run ("it is still in `enumerate`, 4 minutes in").
  More code, more ways to break an existing acceptance criterion, less useful.
- **Only enable timings on a non-TTY stderr** — silent, surprising, and makes
  the flag a no-op in an interactive terminal, which is where people will first
  try it.
- **Route timing output through `Progress`'s writer/mutex** — turns a display
  renderer into a log multiplexer. Rejected as scope creep into the most
  carefully-reasoned module in the tree.
- **Timings to stdout** — violates the stdout-purity contract (`--json`).

Consequence to document in `--help` and the guide: **`--timings` and the live
progress indicator are mutually exclusive**, timings wins. Timing lines *do*
freely interleave with the apply-time `removed:`/`warning:` lines and with
`error:` lines — all of those are newline-terminated, so that is benign.

### 5. Dependency footprint — measured, and `deny.toml` needs no change

Added to `[dependencies]`:

```toml
tracing = "0.1"
tracing-subscriber = { version = "0.3", default-features = false, features = ["fmt"] }
```

**Measured on a scratch copy of this exact crate** (`cargo tree -e normal`,
before/after diff, this session):

- **Normal (non-dev) build graph: 51 → 60 crates.** The nine additions are
  `tracing`, `tracing-core`, `tracing-attributes` (proc-macro, build-time only),
  `tracing-subscriber`, `sharded-slab`, `lazy_static`, `thread_local`,
  `once_cell`, `pin-project-lite`.
- **`Cargo.lock`: 123 → 130, i.e. only seven of those nine are new to the lock
  at all**: `once_cell` and `pin-project-lite` are already in it via
  dev-dependencies, and `proc-macro2`/`quote`/`syn`/`unicode-ident` (the
  proc-macro chain) are *already in the normal graph* for clap's and serde's
  derives — so the proc-macro cost is genuinely marginal. This is the same
  "check what is already present transitively before adding" discipline ADR-0010
  applied to blake3's `rayon` feature.
- **`default-features = false, features = ["fmt"]` is doing real work**: it drops
  `ansi`/`nu-ansi-term`, `tracing-log`/`log`, and `smallvec` versus the default
  feature set — measured **63 graph crates / 134 lock entries with defaults vs.
  60 / 130 without** — and, more importantly, keeps ANSI escape machinery out of
  a tool that already hand-writes its own terminal control. `fmt` implies
  `std`/`registry`/`alloc`, so nothing further needs listing.
- `tracing`'s **default features are kept** (`std` + `attributes`). Turning
  `attributes` off would remove one proc-macro crate whose own three
  dependencies are already in the graph, saving nothing at runtime while making
  a future `#[instrument]` fail confusingly. Checked, then kept — not assumed.
- **Binary size:** ~+228 KB in a rough probe (release, unstripped: 407 KB
  hello-world → 635 KB with `tracing` + `fmt`). To be confirmed on the real
  binary by the implementer.

**Licenses — all already covered:** `tracing`, `tracing-core`,
`tracing-attributes`, `tracing-subscriber`, `sharded-slab` are `MIT`;
`lazy_static`, `thread_local`, `once_cell`, `pin-project-lite`, `cfg-if` are
`MIT OR Apache-2.0`. Both are on `deny.toml`'s allow-list already, and all are
GPL-3.0-compatible (ADR-0014). **`cargo deny check` was run against a scratch
copy of this crate with both dependencies added: `advisories ok, bans ok,
licenses ok, sources ok`** — the only warnings are the pre-existing
`windows-*`/`chrono` duplicate-version warnings that `trash` already produces.
**`deny.toml` requires no edit.** All added crates are `tokio-rs`-org,
first-tier-maintained, and among the most widely deployed crates in the
ecosystem.

### 6. Security

- **Never record a path or filename in a span field by default.** Every field in
  the table above is a count, a size, or an enum — deliberately. Filenames are
  untrusted input; `tracing`'s `fmt` layer does **not** sanitize them.
  Demonstrated this session against a real binary with a filename containing
  `ESC [ 2 J` (clear-screen) and `BEL`:

  ```text
  stage{dbg="evil\u{1b}[2J\u{7}name.jpg" disp=evil^[[2J^Gname.jpg}
  ```

  The `%` (Display) form writes the raw escape sequence straight to the
  terminal — precisely the injection TASK-0318 and `safe_text.rs` exist to
  prevent. **Rule:** if a path ever must be recorded, use `?` (Debug, which
  escapes via `escape_debug` — shown above), or route it through
  `safe_text::escape_control_chars`; **never** `%`/Display. This belongs in a
  test, not only in a comment.
- Instrumentation reads nothing, opens nothing, and writes only to stderr. It
  adds no network, no filesystem, and no IPC surface — a direct consequence of
  rejecting the OTLP exporter.
- It must never change results (AC-24's principle, extended): the acceptance
  test is that stdout is **byte-identical** with and without `--timings`.

## Consequences

- **Positive:** the ADR-0010 open question ("is `enumerate` now the dominant
  serial stage?") becomes answerable by one run on any real corpus, on the
  user's own hardware, with no fixture, no second binary, and no collector. Zero
  measured cost when unused (~0.15 ns/span). Bounded output (~11 lines/run) at
  any file count. Future OTLP export is additive and touches no annotation.
  `deny.toml` unchanged; all new licenses already allowed.
- **Negative / trade-offs:** +7 crates in `Cargo.lock` (+9 in the normal graph)
  and ~+228 KB of binary for a diagnostic feature most users never enable;
  `--timings` and the live progress indicator are mutually exclusive, which is a
  small piece of user-facing surface area to document; stage-level granularity
  cannot attribute time *within* a stage (e.g. which file was slow) — by design,
  and a targeted throwaway harness remains the right tool for that.
- **Explicitly not delivered here:** any OTLP/collector export; a
  `tracing-opentelemetry` bridge; `RUST_LOG`/`env-filter`; JSON-formatted
  timings; per-file or per-group spans; any change to `tests/scale.rs`, which
  remains the project's formal NFR-4 benchmark (this is a *diagnostic*, not a
  regression gate).
- **Follow-ups / new tasks created:** **TASK-0338** implements this (TDD).
  Re-running ADR-0010's unanswered `enumerate`-dominance question with the
  resulting binary, and recording the answer as a consequence note there, is
  called out in that task.

## Product scope and unit classification

**No PRD-0001 acceptance criterion is needed, and none is added.** PRD-0001's
FRs govern deduplication behavior and its output/safety contracts; `--timings`
changes no result, emits nothing on stdout, and cannot alter which files are
grouped or removed. It is operational/engineering tooling in service of NFR-4
measurement, not product capability.

Two honest caveats on that, rather than a rubber stamp: (1) `--timings` *is* a
new user-visible CLI flag, so its **name, `--help` text, and the documented
"timings replaces progress" behavior** should get a lightweight product-owner
sign-off, and its user-facing documentation is product-owner-owned under
ADR-0012 (a short section in `docs/guide/output-and-exit-codes.md` plus a README
flag-table entry). (2) If the product-owner would rather this be a documented
product capability, the escalation is an amendment adding one FR — but the
architect's recommendation is **no**, on the grounds above.

**Unit classification: `chore`** (per [`../process/branching.md`](../process/branching.md)
and [ADR-0011](0011-unit-of-work-cadence-and-planning-gate.md)) — tooling and
dependency work that adds no product capability. Branch:
`chore/instrument-with-tracing`. It carries production-code changes and a new
dependency, so it takes a **full architect code review**; the ADR-0011 skip
exception does not apply and is not claimed.

## Pros and cons of the options

### A1 — `tracing`, no SDK (chosen)

- Good: ecosystem-standard; measured ~0.15 ns/span when unsubscribed; no async
  runtime; `tracing-opentelemetry` keeps the OTLP door open with no annotation
  churn; nesting/thread-safety/formatting come free and audited.
- Bad: two new direct dependencies and 7 new lock entries for something most
  runs never enable.

### A2 — OpenTelemetry SDK + OTLP exporter

- Good: speaks directly to a real observability backend; industry-standard wire
  format.
- Bad: OTLP exporters need `tonic`/`hyper` → **`tokio`**, i.e. an async runtime
  in a synchronous, rayon-parallel CLI (ADR-0010) — a second execution model
  adopted for a backend that will almost never be connected; large dependency
  and attack surface (network client) in a *destructive* tool; the same spans
  are reachable later via a bridge anyway.

### A3 — Hand-rolled in-house timing module

- Good: zero dependencies; total control; smallest possible binary delta.
- Bad: re-implements nesting, thread-safety, field recording and formatting;
  a dead end for any external consumer (no OTLP path at all — the owner's
  stated optionality is lost); every future need is bespoke work, in a project
  whose stated posture is to prefer proven primitives over hand-rolled ones
  (ADR-0010's rayon reasoning applies verbatim).

### A4 — Build nothing, keep writing throwaway harnesses

- Good: no dependency, no code.
- Bad: demonstrably the status quo that cost four tasks bespoke fixtures, and
  left ADR-0010's dominance question open in writing.

### B1 — Coarse per-stage spans with aggregate fields (chosen)

- Good: O(1) spans and lines per run at any file count; answers the actual open
  question; no spans on the parallel hot path; cost is free when unsubscribed
  and negligible when on.
- Bad: cannot attribute time within a stage (which file, which directory).

### B2 — Per-file / per-entry spans

- Good: maximum resolution; would show outlier files.
- Bad: measured ~1.75 µs/span with a subscriber attached and one output line
  *per file* — ~52 ms and 30,000 lines on a 30k-entry walk, swamping the
  measurement and burying the totals; puts shared subscriber state on the exact
  rayon hot path that `progress.rs` went to lengths to keep lock-free.

### B3 — Spans mirroring `progress::Phase`

- Good: one mental model for readers; trivially consistent with the UI.
- Bad: `Scanning` merges `enumerate` and `sniff` **by product decision**, which
  is exactly the split that needs measuring; `Phase` covers nothing after
  verification (no `apply`, no `render`); couples an engineering knob to a
  product surface with acceptance criteria.

### C1 — Ship the `--timings` `fmt` consumer now (chosen)

- Good: delivers the owner's actual goal on day one, with no collector; ~15
  lines; gives the PR real behavior to test under TDD; one dependency review
  instead of two.
- Bad: adds `tracing-subscriber` (5 of the 7 new lock entries) and a small
  user-facing surface to document.

### C2 — Instrumentation only, consumer deferred

- Good: smallest possible diff; `tracing` alone is 3 lock entries.
- Bad: produces nothing observable — the unit would deliver none of its stated
  purpose, and would be near-untestable behaviorally.

### C3 — `RUST_LOG` / `env-filter`

- Good: familiar to Rust developers; per-module filtering for free.
- Bad: pulls `regex` + `matchers` + friends for one boolean's worth of intent;
  invisible in `--help`; inconsistent with every other knob on this CLI.

## Links

- [ADR-0010](0010-concurrency-and-memory-model.md) — the consequence notes
  (TASK-0312/0321/0322) that motivate this, and the `enumerate`-dominance
  question left open there.
- [ADR-0008](0008-core-deduplication-algorithm-and-semantics.md) — the pipeline
  whose stages the spans mirror.
- `docs/planning/status.md` (removed from `main` by ADR-0021; in the git history), session 29 — the informal
  real-corpus numbers that prompted this, and the breakdown they cannot give.
- [`../planning/reviews/e2e-scale.md`](../planning/reviews/e2e-scale.md) — the
  NFR-4 method `tests/scale.rs` still owns.
- `tracing`, `tracing-subscriber`, and `tracing-opentelemetry` (the deferred
  bridge) crates.
