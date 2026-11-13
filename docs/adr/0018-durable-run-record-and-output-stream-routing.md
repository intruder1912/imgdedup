# ADR-0018: Durable run record (`--output`) & output-stream routing

- **Status:** accepted (implemented 2026-11; see
  [Implementation notes](#implementation-notes-2026-11))
- **Date:** 2026-08-06
- **Deciders:** architect, human owner
- **Related:** [PRD-0001](../prd/PRD-0001-exact-image-deduplication.md) —
  Amendment 2026-08-06 (rulings 1–4), **FR-20**, **FR-21**, the FR-19
  elapsed-time amendment, NFR-3 (narrowed), AC-25..AC-32;
  [ADR-0008](0008-core-deduplication-algorithm-and-semantics.md) (pipeline &
  finalization semantics), [ADR-0010](0010-concurrency-and-memory-model.md)
  (parallel hashing, canonical sort),
  [ADR-0017](0017-performance-instrumentation-tracing-spans.md) (`--timings`
  carve-out, stage boundaries), [ADR-0016](0016-ai-agent-automation-plugin.md)
  (`--json` consumer); TASK-0339, TASK-0340, TASK-0341.
- **Scope note:** this ADR covers the **first** slice of the Amendment
  2026-08-06 — FR-20 + FR-21 + the FR-19 elapsed-time amendment. **FR-22
  (paging) is deliberately out of scope** and is not designed here; it is a
  separate, later slice whose own open question is still with the owner.

## Context and problem statement

The owner ran the shipped binary unattended against a ~69 000-file corpus on an
SMB-mounted NAS. The result was correct; *watching* it was the problem. The
product-owner's Amendment 2026-08-06 rules that two capabilities ship together:

- **FR-20 `--output <file>`** — a durable record written **continuously**: a
  transcript (header, phase/elapsed milestones, each duplicate group "as soon as
  it is known to be final", every warning and error, apply-time actions)
  followed, **on completion only**, by the complete result document.
  Human-readable by default, **NDJSON** under `--json`.
- **FR-21 stream model** — in default (non-`--json`) mode, progress / elapsed
  time / apply-time status move to **stdout when stdout is an interactive
  terminal**, falling back to stderr otherwise; `--json` is completely
  unchanged; stderr is reserved for genuine failures.

The owner decided *that* both ship. This ADR answers the mechanism questions the
amendment's "Handed to the architect" section hands over, and — where the code
does not support the PRD's wording — says so rather than designing around the
contradiction.

**The questions, restated:** (1) can a group honestly be called *final* early?
(2) what does incremental emission cost the pipeline? (3) what durability
guarantee can the docs honestly make? (4) what is the NDJSON event vocabulary,
how does it version, and is the completion line the existing document or a
wrapped form? (5) `--output` inside a scan root, a pre-existing target, a
mid-run write failure. (6) where does the stream-routing decision live, does
stdout-TTY ever disagree with stderr-TTY in a harmful way, and how does every
existing stderr writer reclassify?

## Decision drivers

- **Honesty over convenience.** A record file that claims more than the pipeline
  can deliver is worse than one that claims less. This is the same test that
  rejected an ETA (ruling 4) and an entries-visited tick (2026-08-05).
- **The shipped contracts must not move.** NFR-3 as narrowed, AC-16, AC-20,
  AC-24, AC-26, AC-30, AC-31, `--json`'s `schema_version: 1`, and ADR-0016's
  plugin consumer all keep working unchanged.
- **Output-only, always.** `--output` and stream routing never change what is
  scanned, grouped, or removed (FR-20, FR-21, the AC-24 principle).
- **Testability without a terminal.** ADR-0017 and `progress.rs` already
  established the pattern: probe the environment once at the boundary, inject
  booleans, keep every renderer unit-testable. Do not regress it.
- **Cost on the hot path.** `advance` is called once per file from inside a
  rayon closure (ADR-0010). Nothing this ADR adds may put a syscall or a
  contended lock there.
- **Security.** The record file carries untrusted text (paths, error strings)
  and is a new file the tool creates on the user's disk.
- **Don't restructure the engine for a display feature.** The result model
  (`DedupOutcome`, the canonical sort) stays exactly as it is.

## Considered options

Grouped by question, with the full pros/cons at the end.

- **A — group finalization:** A1 emit groups as they are decided; A2 emit
  "provisional" groups with a later correction mechanism; A3 emit no groups
  until the end (transcript = milestones + diagnostics only).
- **B — emission mechanism:** B1 a separate `RunRecorder` observer threaded
  through the stages; B2 extend `ProgressSink` with result-bearing methods;
  B3 restructure the engine to return a stream of groups.
- **C — durability:** C1 unbuffered `write(2)` per record, no `fsync`;
  C2 `fsync`/`sync_data` per record; C3 buffered with a periodic flush.
- **D — NDJSON completion line:** D1 wrapped (`{"type":"result","result":{…}}`);
  D2 the bare `--json` value, compacted; D3 the pretty document appended
  verbatim.
- **E — stream routing:** E1 one pure decision function returning a
  `(stream, is_tty)` pair, computed once in `main.rs`; E2 each writer probes
  its own stream; E3 a global routing singleton.

## Decision outcome

### 1. Group finalization — **yes, groups can honestly be called final early**

**Verdict: a `DuplicateGroup`'s *membership* is final the instant the engine
finishes deciding it, and nothing later in the run can add to it, remove from
it, or split it.** Ruling 2's "as soon as it is known to be final" is
implementable as written. The proof is structural, not empirical:

1. **The candidate set is closed before the engine starts.** `main.rs` runs
   enumeration to completion, then sniffing to completion, and only then calls
   `engine::find_duplicates_with_progress(confirmed, …)`. No candidate is
   discovered after grouping begins — the "discovered/hashed-later candidate
   joins an already-written group" case the amendment worried about **cannot
   occur**.
2. **A group cannot span size cohorts.** Byte-identical files have identical
   sizes, and `group_by_size` partitions the *complete* candidate set by exact
   size. FR-5's cohort partition is therefore not merely an optimization here:
   it is a **closure property**. Whatever happens in one cohort can never affect
   another.
3. **A group cannot span hash groups.** `group_by_hash` runs per cohort and its
   output groups are disjoint subsets of that cohort; `pending_hash_groups`
   accumulates them but never merges across them.
4. **`process_hash_group` is a pure function of one hash group.** It reads only
   that group and the keeper comparator, and it only *appends* to
   `outcome.groups` / `warnings` / `hard_link_notes`. No later call revisits an
   emitted group.
5. **`sort_canonical` only reorders.** It sorts groups by keeper path and each
   group's `removable`/`protected` by the keeper comparator. Membership is
   untouched.

**What is *not* final early — and must therefore not be written as if it were:**

| Property | Final at emit? | Consequence for the record |
|---|---|---|
| keeper, `removable` set, `protected` set | **yes** | write them |
| the group's hard-link notes / already-deduplicated sets | **yes** (a path belongs to exactly one hash group, so the note→group attachment `report.rs`/`json.rs` compute globally equals the local decision) | write them |
| member **order** within the group | no — set by `sort_canonical` later | **sort at emit time** with the same comparator; `sort_by` is stable and the comparator is total, so the emitted order equals the final one |
| the group's **position** and its `Group i/N` index | **no** — canonical order is by keeper path, `N` is unknown | **never emit an index.** The record's group entries carry no number |
| run totals (group count, removable count, reclaimable bytes) | **no** | never emit a total in the transcript; totals appear only in the completion document |

So **ruling 2 does not need to be withdrawn or reframed as "provisional".** One
clarifying half-sentence is recommended (see
[§10](#10-prd-wording-fold-ins--accepted-tracked-outside-this-adr) item 4): *final* means the
group's **membership**, not its position, its index, or the run's totals — which
is what ruling 2's existing "items 1–5 … may report counts that are still
provisional" paragraph already implies but does not say about groups
specifically.

**The honest limitation the docs must state.** The *earliest moment* a group can
be emitted is not the same as "early in the run". The engine deliberately defers
**all** byte-verification to a second pass that begins only after **every**
cohort has been hashed (ADR-0010, consequence note 2026-08-01, TASK-0315 — the
two-pass shape exists so the display phase toggles at most once each way).
Hashing dominated the owner's real corpus at ~86% (TASK-0338). Therefore:

> A run killed **during hashing** — the long part — leaves a record file with a
> header, milestones, and warnings, but **no groups**. Groups stream during the
> verify phase, i.e. the last stretch of the run.

This keeps AC-25 literally true ("every group that had already been
determined"), but it means the user-facing docs must **not** promise that groups
appear steadily throughout a long run. Interleaving verification per cohort so
groups stream during hashing was considered and **rejected**: it would make the
progress phase label flap between `hashing` and `verifying` once per cohort and
would destroy the single-`retotal` verify denominator, breaking AC-21's bounded
phase-transition rule — a real cost on a shipped product surface, for a
speculative benefit. Revisit only if a field report shows the current behaviour
is inadequate.

### 2. Emission mechanism — a separate `RunRecorder`, not an extended `ProgressSink`

**Chosen: B1.** A new `record` module owning one type that is the *single*
writer to the record file.

```text
trait RunRecorder: Sync + Send { fn record(&self, event: RecordEvent<'_>); }
struct NoRecord;              // no-op; used when --output is absent
struct FileRecord { … }       // owns Mutex<File> + a Format (Human | Ndjson) + Instant
```

Why not fold it into `ProgressSink` (B2): `--no-progress` must **not** suppress
the record file (AC-23 governs "stderr or stdout, whichever FR-21 would have
used" — the record file is neither, and FR-20 item 2 *requires* milestones in
it). Making the recorder a `ProgressSink` implementation would put it behind
`NoProgress` and silently lose them. The two concerns must stay orthogonal.

**Milestones reach the recorder by tee, not by inheritance.** `FileRecord`
*additionally* implements `ProgressSink`, and `main.rs` composes:

```text
sink = Tee( terminal_sink (Progress<…> | NoProgress),
            record_sink   (&FileRecord | NoProgress) )
```

so the record gets the milestone stream through the existing, already-plumbed
observer, on the right threads, with the existing thread-safety story — and it
gets it independently of `--no-progress` and `--timings`.

**Milestone cadence must be shared, not re-derived.** `Progress`'s non-TTY
branch already implements exactly the cadence FR-20 item 2 asks for (phase
change, or a `NON_TTY_PERCENT_STEP` bucket change, behind the lock-free
`due()` rate-limit). Extract that decision into a small `MilestoneGate` value
used by *both* `Progress`'s non-TTY branch and `FileRecord`, so there is one
definition of "a milestone" and AC-22's granularity claim cannot drift between
the two surfaces.

**Hot-path cost.** `advance` from a rayon worker keeps the existing lock-free
`due()` pre-check *before* touching the recorder's mutex; a record-file write
therefore happens at most once per repaint interval, never once per file. The
`Tee` adds one non-virtual branch plus one `NoRecord` no-op call per `advance`
when `--output` is absent — the same shape as today's `NoProgress`.

**Where each event originates** (this is the honest cost of "at the moment it
occurs"; it is *not* local to `sort_canonical`/`render`):

| Event | Origin | Change needed |
|---|---|---|
| header | `main.rs`, before the progress sink is built | new |
| milestone | `ProgressSink` calls from `main`/`enumerate`/`sniff`/`engine` | none — arrives via the tee |
| group, already-deduplicated | `engine::dedup_core_with_sink`, in the verify loop, right after `process_hash_group` returns | one `recorder.record(…)` call; engine takes a `&dyn RunRecorder` alongside `&dyn ProgressSink` |
| warning | `enumerate` (metadata failures), `sniff` (rejections), `engine` (hash / identity / verify failures) | one call at each existing `warnings.push` site |
| error (run-level) | `enumerate`, at each `root_errors.push` | one call at the push site. **Unlike stderr, the record has no TTY-interleaving constraint**, so these are written when they happen rather than deferred behind `sink.finish()` |
| apply action | `main.rs`'s existing `apply::Event` closure | one call inside the closure — **zero new plumbing**; `apply.rs` already streams events for exactly this reason |
| result | `main.rs`, from the *same* `rendered` string that is printed to stdout | new |

**What does *not* change:** `DedupOutcome`, the canonical sort, `report.rs`,
`json.rs`'s document, stdout's bytes, the exit-code logic, and the engine's
control flow. The engine gains one observer parameter and one call site inside
the existing verify loop. Nothing becomes deterministic that is not deterministic
today, and nothing that is deterministic today stops being so.

**Byte-identity of the completion document (AC-26)** is free by construction:
`main.rs` renders once into `rendered`, prints it, and hands *the same `String`*
to the recorder.

### 3. Durability — what actually survives, stated without overclaiming

**Chosen: C1 — one unbuffered `write_all` per complete record, no `fsync`.**

Mechanics:

- Open the file **once** at startup with `File::create` and keep it open for the
  run. `std::fs::File` performs no user-space buffering: each `write_all` is a
  `write(2)` that reaches the kernel immediately.
- **Never wrap it in a `BufWriter`.** `main()` ends in `process::exit`, which
  runs **no destructors** — the same hard constraint ADR-0017 called out for the
  tracing guards. A buffered writer would silently lose the tail of every run,
  including successful ones. If buffering is ever added it must come with an
  explicit flush on every exit path, and the ADR-0017 note applies.
- One record = one `write_all` of a **complete, newline-terminated** unit, taken
  under the recorder's mutex, so records never interleave and a reader never
  sees two half-records spliced together.

**The guarantee the docs may make:**

> The record file survives **the process dying** — `SIGINT`, `SIGTERM`,
> `SIGKILL`, a panic, or any crash. Every record the tool finished writing is in
> the file; at most the single record that was mid-write is truncated. Under
> `--json` that truncated tail is a partial final line, which a line-wise NDJSON
> reader discards; the file's every *complete* line still parses (AC-27).

**The guarantee the docs must *not* make:**

> It does **not** survive a host crash or power loss. Written records live in
> the operating system's page cache until the OS writes them out; `imgdedup`
> does not force them to stable storage. On a network mount the record inherits
> that mount's semantics.

`fsync`/`sync_data` per record (C2) was rejected deliberately: on the exact
setup that motivated this work — an SMB-mounted NAS — it is a network round-trip
per event, and slow shared storage is already the suspected cause of TASK-0339.
Paying that on every milestone to defend against power loss, on a file whose
whole purpose is to be re-derivable by re-running the scan, is the wrong trade.
Revisit only if a user reports losing a record to a host crash.

Note the process installs **no signal handler**, and none is needed: with no
user-space buffer there is nothing to flush on the way out. Adding one later
(e.g. to write a "interrupted" marker) is possible but is **not** required by
AC-25 — the *absence* of the completion document is already the signal, and it
is a signal that works for `SIGKILL` too, which no handler can.

**Accepted design boundary: `--json` without `--output` has no incremental or
durable trace — by design, not by gap** (human owner, 2026-08-06). Under
`--json`, stdout is exactly one document that appears only if the run completes;
nothing else about the run is recorded anywhere unless `--output` is passed.
That is the intended shape, and it should be documented as a property rather
than hedged as a limitation:

- Preserving stdout's **single-document chainability** — pipeable to `jq`,
  consumed by TASK-0308's schema and ADR-0016's plugin — outweighs the
  durability that streaming stdout would buy.
- The durability gain would be smaller than it looks anyway: a single-document
  pipe already has comparable fragility on crash regardless of how the producer
  writes it. Streaming stdout would **relocate** the problem into the consumer,
  not fix it.
- `--output` is the answer for anyone who wants a durable `--json` run, and it
  is a strictly better one: NDJSON is valid at every line boundary, whereas a
  streamed stdout document is not.

This is the same boundary read from the other direction in §9: because the
result document and `--output` between them cover the durable-record need, no
stream needs a routine-status carve-out under `--json`.

### 4. The record file's two formats

Both formats obey three rules: (a) every path and every reason passes through
`safe_text::escape_control_chars` (human) or JSON string escaping (NDJSON —
`serde_json` escapes every C0 byte including ESC, so the channel is closed by
construction); (b) one record is one self-delimiting unit; (c) the
transcript/result boundary is unmistakable.

#### 4a. Human-readable (default)

```text
# imgdedup 0.4.1 run record
# started:    2026-08-06T11:22:33Z
# invocation: roots=[/photos] keep=shortest apply=false delete=false hidden=false
#             recurse=true follow-symlinks=false ext=<default> exit-code=false
[     0.0s] scanning...
[    12.0s] scanning: 30000 of 30000 (100%)
[    12.1s] hashing: 0 of 28450 (0%)
[    41.7s] warning: sniff: /photos/notes.txt: not a recognized image
[    98.2s] error: /nope: No such file or directory
[   131.4s] verifying: 12 of 40 (30%) (28450 candidates)
[   131.5s] group (8192 bytes reclaimable):
              KEEP    /photos/a.jpg
              REMOVE  /photos/b.jpg
              REMOVE  /photos/c.jpg
              PROTECT /photos/d.jpg
              hard-linked: /photos/d.jpg, /photos/e.jpg
[   131.9s] already deduplicated (hard-linked): /photos/f.jpg
              hard-linked: /photos/f.jpg, /photos/g.jpg
[   140.0s] trashed: /photos/b.jpg
[   140.1s] warning: /photos/c.jpg: content changed since the scan
[   141.3s] done: 28450 of 28450 (100%)

===== RESULT — run completed 2026-08-06T11:24:55Z after 141.3s =====

<the exact bytes stdout received, verbatim>
```

- A record **starts** at column 0 with `[` (or `#` in the header); continuation
  lines are indented. That single rule makes truncation visible and the file
  trivially greppable.
- Group blocks reuse `report.rs`'s `KEEP` / `REMOVE` / `PROTECT` /
  `hard-linked:` row vocabulary so a user reading both is not learning two
  languages — but carry **no `Group i/N` header**, per §1.
- The `===== RESULT` banner is FR-20's required legibility mechanism: its
  absence means the run did not finish. It is also the anchor for AC-26 — the
  bytes after it are the stdout report, appended verbatim.

#### 4b. NDJSON (under `--json`)

One JSON object per line, `"type"` always the first field, `"elapsed_ms"`
(milliseconds since run start) on every line.

| `type` | Payload (beyond `type` + `elapsed_ms`) |
|---|---|
| `header` | `record_version`, `tool`, `tool_version`, `started_at` (RFC 3339 UTC), `invocation` (a **structured** object: `roots`, `keep`, `apply`, `delete`, `json`, `no_recurse`, `hidden`, `follow_symlinks`, `ext`, `exit_code`, `no_progress`, `timings`, `output`) |
| `milestone` | `phase` (`scanning`\|`hashing`\|`verifying`\|`done`), `current`, `total` (nullable — indeterminate), `percent` (nullable), `run_total` |
| `group` | `keeper`, `removable[]`, `protected[]`, `hard_link_sets[]`, `reclaimable_bytes` — **the exact field names, shapes and serializers of `json.rs`'s `JsonGroup`** |
| `already_deduplicated` | `representative`, `siblings[]` — the `JsonHardLinkSet` shape |
| `warning` | `stage` (`enumerate`\|`sniff`\|`engine`), `path`, `reason` — the `JsonWarning` shape |
| `error` | `scope` (`root`), `path`, `reason` — the `JsonRootError` shape plus `scope` |
| `action` | `action` (`removed`\|`skipped`), `mode` (`trash`\|`delete`), `path`, `reason` (skips only) |
| `result` | `record_version`, `result` — see below |

Example:

```json
{"type":"header","elapsed_ms":0,"record_version":1,"tool":"imgdedup","tool_version":"0.4.1","started_at":"2026-08-06T11:22:33Z","invocation":{"roots":["/photos"],"keep":"shortest","apply":false,"…":"…"}}
{"type":"milestone","elapsed_ms":12040,"phase":"hashing","current":1200,"total":28450,"percent":4,"run_total":28450}
{"type":"group","elapsed_ms":131500,"keeper":{"path":"/photos/a.jpg","size":8192,"mtime":1750000000},"removable":[…],"protected":[],"hard_link_sets":[],"reclaimable_bytes":8192}
{"type":"result","elapsed_ms":141300,"record_version":1,"result":{"schema_version":1,"mode":"dry-run","groups":[…],"…":"…"}}
```

**Anti-drift rule (PRD §11's named risk).** `group`, `already_deduplicated`,
`warning` and `error` lines **reuse `json.rs`'s existing serde types**, not
parallel copies. A field added to `--json`'s group shape appears in the record
automatically; a rename cannot land in one surface only. This is a hard
implementation constraint, not a preference — pin it with a test that asserts a
`group` line's payload equals the corresponding group object in the same run's
`result` line.

**The completion line is wrapped (D1), not bare (D2)** —
`{"type":"result","record_version":1,"result":{…}}`, where the nested value is
the complete `--json` document including its own `schema_version`. Wrapping
keeps every line in the file
self-describing — a consumer never has to guess whether a line is an event or
the result — and `jq -c 'select(.type=="result").result'` recovers the document
exactly. Appending the pretty document verbatim (D3) was rejected outright: it
is multi-line, so a kill mid-write leaves unparsable lines and destroys the one
property NDJSON was chosen for.

**Consequence the PRD must absorb:** stdout under `--json` is
`serde_json::to_string_pretty`. A single NDJSON line is necessarily compact, so
the completion line is the **same JSON value** as stdout's document, not the
same **bytes**. FR-20 and AC-27 say "byte-identical" / "the complete result
document that stdout received" without distinguishing. See
[§10](#10-prd-wording-fold-ins--accepted-tracked-outside-this-adr) item 2. Byte-identity is
exact and testable for the **human-readable** record (§4a) and remains the right
wording there; the `--json` case needs "the same document, serialized compactly"
and a test that compares parsed `serde_json::Value`s, not bytes.

#### 4c. Versioning and stability posture

- **`record_version: 1`** versions the *event vocabulary* and is entirely
  independent of the nested document's `schema_version: 1`. Either may move
  without the other. It appears on the `header` line (first line, always
  present) and on the `result` line (last line), so both a head-reading and a
  tail-reading consumer learn it.
- **The event vocabulary is explicitly declared *unstable* for now**, and this
  must be stated in the user guide, not just here. New event types and new
  fields may be added without a `record_version` bump; consumers **must** ignore
  unknown `type` values and unknown fields. `record_version` is bumped only for
  a change that breaks a consumer obeying those two rules.
- **The nested `result` object is *not* unstable** — it is the existing
  `schema_version: 1` contract, unchanged, with its existing guarantees. This
  split is the point: the surface with a stability promise stays exactly one
  document, and the new surface earns its promise later rather than inheriting
  one by accident.

### 5. `--output` pointing inside a scan root

**Decision: no special-case filter. The existing magic-byte gate is the
guarantee, and it is sufficient — but the reasoning is recorded here so a
reviewer does not have to re-derive it.**

- The file is created **before** enumeration (AC-28's fail-fast), so enumeration
  can see it. With the default allow-list, `run.txt` / `run.ndjson` never match
  the extension filter and it ends there.
- If the user forces the match (`--output shots/rec.jpg`, or `--ext txt`), the
  file becomes a candidate and reaches sniffing — where it is **always
  rejected**: its first bytes are `# imgdedup …` or `{"type":"header"…`, never
  an image signature. NFR-2/FR-3 is the guarantee that it can never be grouped,
  trashed, or deleted, and that guarantee does not depend on anything new.
- Defence in depth: even in a hypothetical where it passed sniffing, it is
  growing during the run, so byte-verification against a keeper (FR-4) and the
  apply-time re-validation (FR-16) would both reject it. Three independent gates
  stand between `--output` and data loss.
- **Residual, documented:** naming an image-extension `--output` path inside a
  scan root produces one harmless `not a recognized image` warning — which then
  appears in the record file's own transcript, self-referentially. Accepted as
  noise, not a defect.
- An explicit "exclude the `--output` path from enumeration" filter was
  considered and rejected: it buys only noise suppression in a rare case, while
  adding a canonicalization comparison that must handle canonicalize failures,
  case-insensitive filesystems, and symlinks — more code with more failure modes
  than the problem is worth.
- **Pin it with a test** anyway (it is a safety-adjacent claim, and cheap):
  `--output <root>/rec.jpg` with a permissive `--ext` must warn, never group,
  and never remove.

### 6. Pre-existing `--output` target — **truncate**

> **Superseded 2026-11-13** by
> [Amendment 2026-11-13](#amendment-2026-11-13--the---output-target-is-never-destroyed-unasked-task-0342):
> an existing file is no longer simply truncated. The text below is the
> original decision, kept for the record.

`File::create` semantics: create, or truncate an existing regular file.

- **Append was rejected on honesty grounds.** A file with a previous run's
  completion document followed by a new partial transcript reads as *finished*.
  That destroys the single signal AC-25 depends on.
- **Refuse was rejected on ergonomics.** Re-running with the same `--output` is
  the normal workflow; making it an error to re-run is a worse default than
  matching what `>` does.
- **Truncate matches the mental model.** `--output` is the flag that replaces
  `> file` for this purpose; behaving like `>` is the least surprising choice,
  and unlike a scanned file this is a path the user typed explicitly. FR-9's
  "a default run changes nothing on disk" is about *scanned* files.
- Residual risk — a mistyped `--output` clobbers a file — is identical to `>`'s
  and is handled by documenting it.
- A directory, or any path that cannot be opened for writing, fails at
  `File::create` and is reported by the AC-28 startup check with
  `exit_code::USAGE` (2) — the usage-level code the AC asks for — before any
  scanning begins.
- **File permissions:** created with the process umask, exactly as `>` would.
  Forcing `0600` was considered — the record does contain the user's full path
  list — and rejected as surprising and inconsistent with the redirection it
  replaces. The guide should note that the record contains every scanned path,
  so the destination should be chosen accordingly on a shared machine.

### 7. Mid-run write failure — continue the run, report once, exit 1

**Settled by the human owner, 2026-08-06** (raised by this ADR's first draft as
a proposal; ruled and now normative). If the record file cannot be written
mid-run — disk full, the network path drops, the file is removed underneath the
process:

1. **The run continues and completes on its own terms**, fully independent of
   the record failure. This explicitly **includes an in-flight `--apply`**: its
   trashing/deleting proceeds and finishes normally. A record-write failure
   never gates, interrupts, or partially aborts FR-10/FR-11's destructive
   actions. FR-20 states `--output` never changes what is scanned, grouped, or
   removed, and AC-28 pins stdout byte-identical with and without the flag;
   aborting would violate both, and aborting *mid-apply* would leave a
   half-applied tree with no report — the worst outcome on an irreversible path
   (the same reasoning `apply.rs` already applies to its
   `KeeperInRemovableSet` case, which records rather than panics).
2. **Exactly one error is reported, on the first failure.**
3. **The run's exit code becomes `exit_code::ERROR` (1)**, extending FR-15's
   run-level-failure code, even though the scan and any apply succeeded. The
   user asked for a durable record and did not get one; the exit code is the
   only channel that survives for a script to detect it. This does not violate
   AC-28, which pins stdout bytes and the scan/group/remove set, not the exit
   status.

**Mechanics, concretely** (no open questions for the implementer):

- **Where it is detected.** `FileRecord::record` is the single choke point —
  every event, from every stage, on every thread, goes through it, and it
  already holds the writer's mutex. The `io::Result` of the one `write_all`
  for that record is the only failure signal there is; no other call site needs
  error handling.
- **The latch.** One `AtomicBool` (`degraded`) on `FileRecord`, plus the error
  itself stored once. `record()` becomes:
  1. If `degraded` is already set (`Relaxed` load), **return immediately** —
     before taking the mutex, so a full disk costs nothing on the hot path.
  2. Otherwise format and `write_all` under the mutex.
  3. On `Err(e)`: `compare_exchange` `degraded` from `false` to `true`. **Only
     the thread that wins the exchange** prints the message; every loser
     returns silently. This is what makes "reported exactly once" true under
     the rayon-driven `advance` path, where several workers can fail
     concurrently. The winner also stores the `io::Error`'s string for the
     summary in step 5.
- **The message.** `error: could not write to <path>: <reason>` on **stderr**,
  with both the path and the reason passed through
  `safe_text::escape_control_chars`. stderr is correct here under FR-21 without
  any carve-out: a failure to produce something the user explicitly asked for is
  a genuine failure, not routine status.
- **Ordering hazard.** This is the one new stderr writer that can fire *while a
  live progress line is being redrawn* — `progress.rs`'s module doc forbids
  exactly that, and the existing `main.rs` discipline ("every other stderr
  writer runs strictly after `finish()`") cannot help, because the failure
  happens mid-run by definition. Resolution: the reporting thread emits a
  **leading newline** before the message when the progress target is a TTY, so
  the message starts on a clean line rather than splicing into the in-place
  render. The routing value from §8 already carries the `tty` boolean needed to
  decide this, so no second probe is introduced. Under `--json`, or with
  `--no-progress`/`--timings`, or on a non-TTY progress target, no leading
  newline is added.
- **The file is left exactly as it is** — partial, with no completion document,
  which is precisely the correct reading: this record is not complete. No
  attempt is made to reopen, retry, truncate, or delete it. A retry policy was
  considered and rejected: the two realistic causes (a full disk, a dropped
  network mount) are both states a retry inside the same run will not resolve,
  and retrying would put I/O latency back onto the path this design just made
  free.
- **The completion document is not exempt.** A failure while writing the final
  result follows the identical path, so a run whose transcript succeeded but
  whose completion document failed still exits 1 and still leaves a file with no
  completion document.
- **Exit-code composition.** `exit_code::compute` already maps a run-level
  failure to 1; the degraded flag joins `!root_errors.is_empty()` as an input to
  that first argument. Interaction with `--exit-code`'s 3 follows the existing
  precedence unchanged — a run-level failure outranks "duplicates found", exactly
  as it does for `root_errors` today.
- **Testable without a full disk.** The failure path is exercised by injecting a
  writer whose `write_all` returns `Err`, which is why `FileRecord` must be
  generic over its writer (or hold a `Box<dyn Write + Send>`) rather than a
  concrete `File`. The "reported exactly once" property is asserted by failing
  every write and counting the messages.

### 8. Stream routing — one pure decision, computed once, returning `(stream, tty)`

**Chosen: E1.** A single pure function, in `main.rs` or a small `streams`
module:

```text
fn route(json: bool, no_progress: bool, timings: bool,
         stdout_tty: bool, stderr_tty: bool) -> Routing

struct Routing {
    progress: Option<Target>,   // None when suppressed
    status:   Option<Target>,   // apply-time removed:/deleted: lines (§9 #6);
                                // None in every case but default-mode + stdout TTY
}
struct Target { stream: Stream /* Stdout | Stderr */, tty: bool }
```

`status` never resolves to stderr: apply-time removal lines are routine status
and are simply not written when stdout is unavailable to them (§9 #6). Apply
*skip* lines are not routed at all — they are genuine failures and always go to
stderr (§9 #7).

Three properties make this the right shape:

1. **It is a pure function of five booleans.** All 32 combinations are
   table-testable in a plain unit test. **No pty is needed for any case.** The
   only impurity — `std::io::stdout().is_terminal()` /
   `stderr().is_terminal()` — is probed exactly once, at the boundary, exactly
   as `progress.rs` already mandates for stderr. Renderers keep taking an
   injected writer and an injected `tty: bool` and stay unit-testable unchanged.
2. **`Target` binds the stream and its TTY-ness together.** This is what makes
   the "control codes in a captured file" question answerable structurally
   rather than by inspection: it is *impossible* to select the in-place renderer
   using one stream's TTY-ness while writing to the other, because the boolean
   travels with the stream it describes.
3. It reads exactly like the PRD's own case table, so the mapping is auditable
   line by line.

The routing table (default mode = not `--json`):

| Case | Progress target | Render mode | Apply removal lines | stderr carries |
|---|---|---|---|---|
| default, stdout TTY | **stdout** | in-place (AC-21) | **stdout** | genuine failures only (AC-29) |
| default, stdout TTY, stderr redirected | **stdout** | in-place | **stdout** | genuine failures only — **no control codes reach the file** |
| default, stdout redirected/piped | **stderr** | per stderr's own TTY-ness — see §10 item 1 | not written | progress + genuine failures |
| `--json`, any | **stderr** | per stderr's TTY-ness (unchanged) | not written | progress + genuine failures |
| `--no-progress` | none | — | per the rows above | genuine failures only |
| `--timings` | none (ADR-0017) | — | per the rows above | timing lines + genuine failures |

**Does stdout-TTY ever disagree with stderr-TTY harmfully? No.** The four
default-mode combinations are enumerated above; in every one, the in-place
renderer is selected only for a stream that is itself a terminal, so
`\r` / `\x1b[K` can never reach a redirected file. Under `--json` nothing
changes at all. The one disagreement case that *matters* —
`imgdedup … > report.txt` on a terminal, where stdout is not a TTY but stderr
is — is a UX question, not a leak (§10 item 1).

**Two implementation traps that must be called out, because moving to stdout
exposes them:**

- **Rust's `Stdout` is a `LineWriter`.** A `\r`-terminated in-place repaint
  contains no newline and would sit in that buffer forever. `Progress`'s TTY
  path already calls `flush()` after every repaint (that is why it works on
  stderr, which is unbuffered) — this call is now **load-bearing** and must not
  be removed. Add a regression test that pins it.
- **Ordering with the report.** `sink.finish()` already runs strictly before
  `print!("{rendered}")` (AC-20). With progress on stdout, that ordering stops
  being merely tidy and becomes correctness: both now target the same stream.
  `SinkGuard`'s idempotent `Drop` keeps the invariant across early returns.

`Progress<W>` is generic over `W: Write`; selecting the stream at runtime needs
either `Box<dyn Write + Send>` or a small `enum ProgressStream { Out(Stdout),
Err(Stderr) }` implementing `Write`. Prefer the enum — no allocation, no dyn
dispatch, and it keeps the concrete type visible in test setups.

**Elapsed time (ruling 4 / AC-32).** `Progress::paint` already computes
`elapsed_secs = now / 1000`; AC-32 generalizes the suffix from the indeterminate
walk state to **every** render and every phase. `Progress::line` already carries
seven parameters and its own doc says an eighth must become a params struct —
so bundle into a `LineParams` before adding the elapsed field to the remaining
branches. AC-22's "TTY-only" reasoning (a non-redrawn line records a stale
value) applies to the **indeterminate found-so-far count**, not to a milestone
line that is emitted at that instant with an accurate timestamp; and FR-20 item
2 *requires* elapsed time on the record file's milestones, which are always
non-TTY. Non-TTY milestone lines therefore **do** carry elapsed time. See §10
item 3 for the small AC-22 wording consequence.

### 9. Reclassifying every current stderr writer

Complete inventory of everything the binary writes to stderr today:

| # | Writer | Classification | Destination under FR-21 |
|---|---|---|---|
| 1 | `--delete` without `--apply` usage error (`cli::validate`) | **genuine failure** | stderr, unchanged (exit 2) |
| 2 | run-level `root_errors` (`error: <path>: <reason>`) | **genuine failure** | stderr, unchanged (exit 1) — FR-21 names it explicitly |
| 3 | `error: not applying: N scan root(s) could not be read` | **genuine failure** | stderr, unchanged |
| 4 | progress (`progress.rs`) | routine status | per §8's routing table |
| 5 | `--timings` stage lines | diagnostic | stderr — explicit PRD carve-out |
| 6 | apply `trashed:` / `deleted:` lines (`Event::Removed`) | **routine status**, no exception | stdout when stdout is an interactive terminal in default mode; **not written at all** otherwise — see below |
| 7 | apply `warning: <path>: <reason>` (`Event::Skipped`) | **genuine failure** — see below | stderr, unchanged |
| 8 | panic / abort messages | genuine failure | stderr, out of scope |
| 9 | clap's own parse errors (`--help`/`--version` already go to **stdout**) | genuine failure | stderr, unchanged — already correct |
| 10 | record-write failure (`error: could not write to <path>: …`) | **genuine failure** (new, §7) | stderr |

**So stderr's "genuine failures only" classification has zero exceptions** other
than `--timings`, which the PRD itself carves out. Two entries still need
reasoning rather than a bare verdict.

**#7 — apply skip lines: classified as genuine failure, deliberately as a
whole.** `SkipReason` has ten variants, and this decision was first drafted
against six of them — `Unverifiable`, `RemovalFailed` and
`KeeperInRemovableSet` (an internal-invariant breach) are unambiguous failures;
`ContentChanged` and `NotARegularFile` are TOCTOU events AC-17 explicitly calls
"skipped with a warning"; `Vanished` is documented in `apply.rs` as *not* an
error (it is what makes a re-run a no-op, FR-12). A whole-diff review against
this ADR (2026-08-06) found three more that are documented in `apply.rs` the
same way as `Vanished` — routine, not failures: `IsASymlink` and
`KeeperIsASymlink` ("the tool never unlinks a symlink standing in for a
scanned file, because the user asked about the target"), reachable
*systematically* under `--follow-symlinks --apply`, not as a rare edge case;
and `SameFileAsKeeper`, a hard link created since the scan, where removing it
"would delete one of the keeper's own names" (FR-17) — a correct refusal, not
a problem. `KeeperUnusable` joins the unambiguous-failure group (removing the
candidate could destroy the last copy of its content). **Decision unchanged:
keep the whole `Skipped` stream on stderr.** Splitting the enum across two
streams would push a `SkipReason` taxonomy decision — a product surface — into
a stream-routing change, and would mean an `--apply` run's skips arrive on two
different streams. Every skip means "you approved this removal and it did not
happen", which is usually a deviation from the plan the user approved, and
keeping them together keeps AC-29 satisfiable (stderr silent unless something
genuinely went wrong) at the cost of also printing the four routine variants.
Recorded as *arguable*, now for four variants rather than one: a purist
reading of FR-21 would move `Vanished`, `IsASymlink`, `KeeperIsASymlink` and
`SameFileAsKeeper` off stderr, since none of them represent a problem. Revisit
if `--follow-symlinks --apply` skip noise is reported from the field — the
symlink pair makes that materially more likely than the original one-variant
framing suggested.

**#6 — apply removal lines: routine status, with no stderr fallback (human
owner, 2026-08-06).** They follow FR-21's routing for routine status — stdout
when stdout is an interactive terminal in default mode — and are **simply not
written** in every other case (stdout redirected or piped, and always under
`--json`). No new stream, no carve-out, no suppression-versus-fallback special
case.

The reason this costs nothing is that the destructive-action ledger is **already
covered twice over**, so these lines were never load-bearing:

- The result document's **`actions` object** (FR-14/TASK-0308, shipped) is the
  authoritative record once a run completes — `actions.removed` and
  `actions.skipped` with reasons, in both the human report and `--json`.
- **`--output`'s `action` event** (§4b) captures each removal and skip
  incrementally, the instant it happens, for a run that does not complete.

*Superseded reasoning, recorded rather than deleted* (this repo's practice —
see PRD-0001's ruling 6). This ADR's first draft proposed a narrow **stderr
carve-out**: removal lines would fall back to stderr in every non-stdout case,
including `--json`, on the ground that `apply.rs`'s own doc says they are
streamed because *"a process killed partway through an irreversible run would
otherwise leave the user with no record of what was destroyed"* — making them
simultaneously "routine status" and "the irreversible-action record", which
FR-21's two buckets appeared to have no slot for. The owner's ruling dissolves
that rather than overriding it: the record the draft was trying to preserve is
exactly what `actions` provides on completion and what `--output`'s `action`
events provide incrementally, so there is nothing left for a stderr fallback to
protect. The draft's framing also conflated "the only ledger *today*" with "the
only ledger *available*" — the second was already false the moment FR-20 was
ruled. Consequence: FR-21's taxonomy holds without exception, and §8's routing
table has one fewer special case.

Everything else sorts cleanly. Note also that the report's own `Warnings:`
section is part of the stdout document and is unaffected (PRD ruling 3), and
that apply skips consequently appear both live on stderr and in the result
document's actions — which is already today's behaviour, not a new duplication.

### 10. PRD wording fold-ins — accepted, tracked outside this ADR

This ADR has **no open design questions**. Everything the first draft raised for
the human owner was ruled on 2026-08-06 and is folded in above as settled
design: the record-write failure policy (§7), the destructive-action line
routing and the withdrawal of the proposed stderr carve-out (§9 #6), and the
`--json`-without-`--output` design boundary (§3). The `SkipReason::Vanished`
classification (§9 #7) is an architect decision, recorded there with its
rationale and its revisit trigger, not an open question.

Four **PRD text** reconciliations remain outstanding. All four are accepted in
substance; they are folded into PRD-0001 separately, and none of them gates
implementation, because §1–§9 above state each one normatively for this design.
They are listed here only so the trail is legible from either document:

1. **AC-30's fallback rendering.** AC-30 pins progress to "AC-22's plain
   milestone lines on stderr" when stdout is redirected. But
   `imgdedup ./photos > report.txt` run from a terminal has a **TTY stderr**,
   and today that case shows a live in-place progress line — so the literal
   text would downgrade the documented save-a-report workflow to coarse plain
   lines. Nothing leaks either way. Accepted wording: "on stderr, rendered per
   AC-21 or AC-22 according to **stderr's own** TTY-ness", which preserves
   today's behaviour exactly. §8's mechanism implements this.
2. **FR-20 / AC-27 "byte-identical" under `--json`.** Byte-identity is exact for
   the human-readable record and stays the right wording there; the NDJSON
   completion line is necessarily compact, so it is the *same document*, not the
   same bytes (§4b). Accepted as a wording split.
3. **AC-22 / AC-32 elapsed time on non-TTY lines.** AC-22's "TTY-only" reasoning
   is about the *indeterminate found-so-far* state, not about determinate
   milestone lines, and FR-20 requires elapsed time on record-file milestones,
   which are always non-TTY. Accepted: AC-22 is scoped to the indeterminate
   state explicitly.
4. **Ruling 2 / FR-20 wording — "final".** The claim holds as written (§1); the
   recommendation is a clarifying sentence, not a correction: *final* means the
   group's **membership** (keeper, removable, protected, hard-link notes), while
   its **position, its `Group i/N` index, and every run total remain
   provisional** — plus the honest note that groups stream during the verify
   phase, so a run killed during hashing leaves none. This property is stated
   normatively in §1 and in
   [ADR-0008](0008-core-deduplication-algorithm-and-semantics.md)'s 2026-08-06
   consequence note, so implementation does not wait on the PRD edit.

The only genuinely open questions in this problem space belong to **FR-22
(paging)** — default-on versus opt-in, the Windows story, `$PAGER`-through-a-
shell and its injection surface, and exit-code/`EPIPE` mechanics. FR-22 is a
separate slice with its own ADR; it is out of this ADR's scope by design and
nothing here depends on its answers.

## Consequences

- **Positive.** A long run's results survive both the terminal's scrollback and
  the process's death. Group finalization turns out to be a genuine property of
  the ADR-0008 pipeline rather than something to be engineered, so FR-20 needs
  no result-model restructuring. The NDJSON surface reuses `json.rs`'s serde
  types, so the two machine-readable surfaces are structurally prevented from
  drifting. The routing decision becomes a pure function, so a class of
  "which stream?" bugs becomes table-testable without a pty.
- **Negative / trade-offs.** A second machine-readable surface now exists and
  must be documented, tested and versioned. The `record` module touches five
  existing modules (small, additive changes, but not zero). Progress on stdout
  makes `Progress`'s existing `flush()` load-bearing in a way it was not.
  Groups only stream in the final stretch of a run. The record survives process
  death but not power loss, and the docs must say so.
- **Follow-ups.** (a) Extract `MilestoneGate` shared by `Progress` and
  `FileRecord`. (b) `Progress::line` → `LineParams` before adding elapsed time
  everywhere. (c) A regression test pinning the stdout `flush()`. (d) A test
  asserting a `group` line equals its counterpart in the same run's `result`
  line. (e) A test pinning `--output` inside a scan root. (f) A test failing
  every record write and asserting exactly one stderr message plus exit 1 (§7).
  (g) The four PRD wording fold-ins in §10, tracked against PRD-0001, none of
  which gates implementation. (h) FR-22 (paging) is a separate slice and a
  separate ADR question — including the `$PAGER`-through-a-shell security
  question, untouched here.

## Implementation notes (2026-11)

FR-20, FR-21 and the FR-19 elapsed-time amendment were implemented on
`feat/output-record-and-stream-routing` (TASK-0340/TASK-0341, slice 1) from
[the implementation plan](../superpowers/plans/2026-11-12-output-record-and-stream-routing.md).
The decision above stands as written. Where the implementation settled
something this ADR left open, or differs from its illustrations, the code is
authoritative and the difference is recorded here:

1. **Elapsed-time wording.** Determinate progress lines end with
   `(<n>s elapsed)`; the completion line ends with `(<n>s total)` — for
   example `hashing: 1200 of 28450 (4%) (12s elapsed)` and
   `done: 302 of 302 (100%) (141s total)`. The suffix is always last. The
   non-terminal indeterminate `scanning...` line stays bare (AC-22).
2. **A run-scoped `error` event.** The `not applying: N scan root(s) could not
   be read` refusal is recorded as an `error` with `scope: "run"` and no
   `path` — an additive extension of §4b's `error` vocabulary, needed for
   FR-20's "every error".
3. **`MilestoneSink` is its own type.** §2 sketched `FileRecord` implementing
   `ProgressSink` directly. The implementation instead wraps an
   `Arc<dyn RunRecorder>` in a separate `MilestoneSink`, teed with the terminal
   renderer and sharing the same `MilestoneGate` cadence. `FileRecord` keeps
   one job, and milestones are unit-tested against a capturing recorder.
4. **Record order is occurrence order.** The `done` milestone therefore
   precedes apply-time `trashed:` / `deleted:` lines; §4a's example shows the
   reverse and is illustrative only.
5. **Two defaulted `RunRecorder` methods.** `is_active()` lets the engine skip
   a per-group clone when `--output` is absent; `is_degraded()` feeds the exit
   code after a record-write failure (§7).
6. **Report-once latch.** §7's "exactly one message" is enforced by a flag set
   under the record writer's mutex, not by an atomic `compare_exchange`: the
   failing write already holds that mutex, so the latch needs no second
   synchronization primitive.
7. **Three recording sites in enumeration.** `enumerate.rs` records a scan
   root that cannot be read (an `error`), a walk error below a root and a
   metadata failure (both warnings). §2's table listed fewer sites.
8. **What paths the record holds.** §6 says the record "contains every scanned
   path". It does not: no event lists every scanned file. The record holds the
   scan roots, the members of each duplicate group and of each
   already-deduplicated hard-link set, the files warned about, the paths of
   `error` events, the paths acted on and — in the NDJSON header's
   `invocation` — the `--output` path itself. The guide states that
   narrower set; §6's advice to choose the record's location with care on a
   shared machine stands.
9. **A failed write of the completion document can leave part of it.** §7
   says a mid-run write failure "leaves a file with no completion document".
   That holds when an earlier record fails, because every later write is then
   skipped. When the failing write is the completion document itself —
   the largest single write, so the likeliest to hit a nearly full disk —
   `write_all` may have written part of it before the error: the human record
   can then end with the `===== RESULT` banner and a cut-short report. Exit
   code `1` and the stderr message are the signal; the guide says so. An
   end-of-record trailer was rejected because it would break AC-26 (the record
   ends with the exact stdout bytes), and truncating back contradicts §7's
   "no truncate". NDJSON is unaffected: a cut-short `result` line does not
   parse, and a finished line ends with a newline.
10. **The human header names every invocation flag (TASK-0344).** §4a's
    example shows two `# invocation:` lines, the flags that shape the result.
    The header now has a third — `json=`, `no-progress=`, `timings=` and
    `output=` — so the human record carries the same invocation as the NDJSON
    header. The `--output` path is escaped like every other path in the
    record.
11. **No blank line before a late record-failure message.** §7's leading line
    break is written only while a progress line is live; once progress has
    finished (a failed apply-time or result write) the message is written
    without it.
12. **The two `--output` errors no longer read alike.** The startup failure
    (exit `2`, before scanning) says `error: cannot open output file <path>:
    <reason>`; it used to say `cannot write to`, one word away from the
    mid-run `error: could not write to <path>: <reason>` (exit `1`), which is
    unchanged. Owner decision, 2026-11-13.

## Amendment 2026-11-13 — the `--output` target is never destroyed unasked (TASK-0342)

**Decision (owner, 2026-11-13): a deduplication tool must not destroy any
file without being asked.** §6 chose `File::create` semantics — truncate, like
`>` — and judged the residual risk "identical to `>`'s … handled by documenting
it". The owner overturned that judgment: `--output` is typed by hand next to a
tree of irreplaceable photos, it truncates before scanning and even on a dry
run, and `imgdedup --output photos/img_0412.jpg photos` destroyed the photo.
FR-9's "a default run changes nothing on disk" was read too narrowly in §6.

§6's two rejections still hold and shaped the replacement: **refuse-always**
would break the normal re-run with the same `--output`, and **append** would
make an interrupted run read as finished. The new rule, implemented in
`src/output_file.rs` and run at startup with the other `--output` checks
(exit `2`, before scanning, no report):

| The `--output` path… | What happens |
|---|---|
| does not exist | Created (create-new, so a file appearing in between is classified, not clobbered). |
| is an empty file | Written. Nothing to keep. |
| is an earlier run record | Moved to `<file>.bak`, then written. No flag. |
| is any other file | Refused. With `--overwrite-output`: moved to `<file>.bak`, then written. |
| is a directory, or cannot be opened | Refused, as before. |

- **An earlier run record** is a file whose first line is the human header
  `# imgdedup <version> run record`, or an NDJSON header line beginning
  `{"type":"header","elapsed_ms":` that carries `"tool":"imgdedup"` — the two
  formats of §4. A looser prefix match was rejected in review: a Markdown note
  titled `# imgdedup …` would be taken for a record, moved to `.bak`, and lost
  on the next re-run. A file that cannot be read, has a byte-order mark, or is
  cut short inside its first line is "any other file": when in doubt, ask.
- **One backup generation.** An existing `<file>.bak` is replaced when it is
  empty or itself a run record. Anything else at that name — a foreign file,
  a directory, a symlink (even one to a record) — refuses the run **with or without** `--overwrite-output`: the
  flag is permission to replace the output file, not a file the user never
  named. Numbered backups were rejected: nothing is ever replaced, but an
  unattended nightly run leaves hundreds of files.
- **Rename, not copy.** The backup is a rename within the target's own
  directory: instant, and it cannot fail halfway. When the `--output` path is
  a symlink the content is copied instead — into a newly created `<file>.bak`
  (an existing record backup is removed first; a file that reappears at that
  name refuses the run) — and the file behind the link is rewritten, so the
  link stays in place and keeps pointing where it did.
- **A target that is not a regular file** (a FIFO, a device such as
  `/dev/full`) is opened for writing as before: writing to it replaces
  nothing. A FIFO with no reader blocks the open until one appears, as
  `File::create` did. A dangling symlink's target is created, as before. If
  what was opened turns out to be a non-empty regular file (it appeared in
  the meantime), the run is refused. An empty file is likewise opened without
  truncating and checked again on the open handle.
- **The flag is `--overwrite-output`,** not `--force`, which beside `--delete`
  could be read as forcing removals. Without `--output` it is a usage error.
  It is part of the record's `invocation` in both formats, like every other
  flag (implementation note 10).
- **A backup that cannot be made refuses the run.** A record the user can
  write, in a directory they cannot — a pre-created log file in a system log
  directory — used to be truncated in place. The rename now fails, and the
  run stops with `error: cannot back up <path> to <backup>: <reason>` rather
  than replace the file without a backup.
- **Consequences of renaming instead of truncating**, accepted by the
  product-owner: permissions and ownership set on an earlier record stay
  with its `.bak` and the new file gets the defaults; a hard link to the old
  record keeps the old content; a `tail -f` left open across runs follows
  the `.bak` (`tail -F` follows the name).
- **Residual windows (accepted).** Each check and the action after it are
  separate system calls, so a process that changes the output directory in
  between can defeat the check. A file swapped in at the `--output` path
  between its classification and the rename is moved to `<file>.bak`, not
  destroyed. But a file created at `<file>.bak` between that name's check and
  the rename is replaced by the rename and lost: a no-replace rename
  (`renameat2(RENAME_NOREPLACE)`, `renamex_np(RENAME_EXCL)`) is not available
  in `std` and was not worth platform-specific code for this. imgdedup is a
  single-user tool: the rule protects against the user's own mistyped path,
  not against a concurrent process — hostile or not — with write access to
  the output directory. (In a sticky shared directory such as `/tmp`,
  `rename` cannot replace another user's file.) Choose an `--output`
  directory only you write to.

### Writers that cannot panic

`eprintln!` and `println!` panic when their stream cannot be written; a pipe
whose reader has gone away is enough. Found in the PR #107 review: a
diagnostic written during `--apply` could abort the apply partway through
(exit `101`, record left without its completion document), and
`imgdedup photos | head` panicked once `head` exited. This predates this ADR
but touches §7 and §8, so it is recorded here.

- Every stderr diagnostic in `main.rs` goes through `streams::diag`, which
  ignores a failed write. If nobody can read a diagnostic there is nothing
  better to do with it, and the run, its record and its exit code must not
  depend on it.
- The `--timings` subscriber is built with `log_internal_errors(false)`. By
  default `tracing-subscriber` reports a failed write with `eprintln!`, which
  panics on the same closed stderr; found in the TASK-0342 review.
- The report is written with `streams::write_stdout`. When it cannot be
  delivered, the completion document is still written to the record (§3), no
  further message is printed, and the process exits `1` — the report the user
  asked for did not arrive (owner decision, 2026-11-13; the alternative was
  `0`). Like a degraded record (§7), this takes precedence over
  `--exit-code`'s `3`.

Verification beyond the automated suite, recorded in the commit messages of
`37c1fb4` and `be94d61`: the §8 routing table was exercised on a real
pseudo-terminal in four stream configurations, and AC-25 against real
`SIGKILL`s on a 2,021-file corpus (header and milestones present, no
completion banner; every complete NDJSON line parseable).

## Pros and cons of the options

### A1 — emit groups as they are decided (chosen)

- Good: the pipeline already supports it (§1); no result-model change; AC-25
  becomes literally true.
- Bad: groups arrive only during the verify phase, so a hashing-phase kill
  yields none.

### A2 — "provisional group, may still gain members", with corrections

- Good: would be the honest framing *if* groups could grow.
- Bad: they cannot (§1), so it would document a weakness that does not exist and
  force consumers to implement a correction protocol for an event that can never
  fire.

### A3 — no groups in the transcript at all

- Good: trivially correct; smallest change.
- Bad: guts FR-20 — the whole point is that a killed run keeps what it found.

### B1 — separate `RunRecorder` observer (chosen)

- Good: orthogonal to `--no-progress`/`--timings`, so milestones survive both;
  one lock, one writer, occurrence-ordered; zero cost when `--output` is absent.
- Bad: a second observer parameter on the engine; five modules touched.

### B2 — extend `ProgressSink`

- Good: no new plumbing at all.
- Bad: `--no-progress` would silently disable the record file; conflates a
  display concern with a result concern; `NoProgress` would swallow groups.

### B3 — engine returns a stream of groups

- Good: the "purest" streaming shape.
- Bad: restructures the result model, the canonical sort, and every caller, for
  a feature the current model already supports observationally. Rejected.

### C1 — unbuffered write per record, no fsync (chosen)

- Good: survives every process-death mode including `SIGKILL`; no syscall on the
  hot path beyond the milestone cadence; no network round-trips.
- Bad: does not survive power loss — must be documented, not glossed.

### C2 — `fsync` per record

- Good: survives host crash and power loss.
- Bad: a network round-trip per event on the exact NAS setup that motivated this
  work; pays a large, continuous cost to protect data that a re-run regenerates.

### C3 — buffered with periodic flush

- Good: fewest syscalls.
- Bad: `process::exit` runs no destructors, so the tail is lost even on
  *successful* runs unless every exit path flushes. Strictly worse than C1 for
  this use case.

### D1 — wrapped completion line (chosen)

- Good: every line self-describing; `jq -c 'select(.type=="result").result'`
  recovers the exact document; one uniform parse rule.
- Bad: one nesting level for consumers; not byte-identical to stdout's pretty
  form (§10 item 2).

### D2 — bare compacted `--json` value

- Good: no unwrapping.
- Bad: a consumer must distinguish result from event by sniffing for
  `schema_version`; also not byte-identical.

### D3 — pretty document appended verbatim

- Good: literally byte-identical.
- Bad: multi-line, so a kill mid-write leaves unparsable lines — destroys the
  single property NDJSON was chosen for (AC-27). Rejected.

### E1 — one pure routing function returning `(stream, tty)` (chosen)

- Good: 32 combinations table-testable with no pty; makes a control-code leak
  structurally impossible; matches the existing inject-the-boolean pattern.
- Bad: one more type to thread from `main`.

### E2 — each writer probes its own stream

- Good: no plumbing.
- Bad: N probe sites that can disagree; untestable without a terminal;
  directly contradicts `progress.rs`'s documented "probed exactly once, in
  `main.rs`" rule.

### E3 — global routing singleton

- Good: reachable from anywhere.
- Bad: hidden global state in a crate that has none; test isolation problems;
  fights the injected-writer design that makes the renderers testable.

## Links

- [PRD-0001 Amendment 2026-08-06](../prd/PRD-0001-exact-image-deduplication.md#amendment-2026-08-06--durable-run-record-stream-model-and-report-paging-product-owner-ruling-task-0340--task-0341)
  — rulings 1–6, the "Handed to the architect" brief, and the FR-22 open
  question this ADR does not touch.
- [ADR-0008](0008-core-deduplication-algorithm-and-semantics.md) — the pipeline
  whose closure properties make §1 true; see its 2026-08-06 consequence note.
- [ADR-0010](0010-concurrency-and-memory-model.md) — parallel hashing, the
  canonical sort as the single determinism mechanism, and the deferred
  verification pass (2026-08-01) that bounds how early groups can stream.
- [ADR-0017](0017-performance-instrumentation-tracing-spans.md) — the
  `--timings` stderr carve-out, the `process::exit`-runs-no-destructors
  constraint, and the "instrumentation never influences results" invariant this
  ADR inherits.
- `src/progress.rs` module documentation — the injected-TTY-boolean pattern and
  the "nothing else may write to the progress stream while a line is live" rule.
- [NDJSON specification](https://github.com/ndjson/ndjson-spec) — the
  one-object-per-line format ruling 2 selected.
