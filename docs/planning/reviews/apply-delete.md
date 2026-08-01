# Review log — Slice S3 `apply-delete` (`feature/apply-delete`)

- **PRD:** [PRD-0001](../../prd/PRD-0001-exact-image-deduplication.md)
- **Slice scope:** TASK-0309 (trash apply + apply-time re-validation) and
  TASK-0310 (permanent delete). FR-10, FR-11, FR-12, FR-16, NFR-1, NFR-2,
  NFR-6; AC-3, AC-5, AC-11, AC-17.
- **Status:** closed — converged; slice complete and opened as a PR
  (2026-08-01, sessions 2–3).

In-progress review log for this slice
([ADR-0013](../../adr/0013-prd-slice-delivery-cadence.md)). This is the first
slice that mutates the filesystem, so findings are recorded as they surface
rather than only at slice end.

## Implementer notes — TASK-0309 core (2026-08-01, session 2)

**Shape.** `src/apply.rs` holds a `Remover` trait (the removal mechanism,
injected), `TrashRemover`, the `revalidate` TOCTOU check, and an `apply`
executor over the engine's groups. TASK-0310 adds permanent deletion by
supplying a second `Remover`, not by branching inside the executor.

**Re-validation refuses on anything it cannot prove.** A candidate is removed
only when it still exists as a regular file (`symlink_metadata`, so a symlink
swapped in since the scan is detected rather than followed), the keeper still
exists as a regular file, the two are still byte-for-byte identical, and they
are **not the same physical file**. That last check is an addition beyond
ADR-0008's list: a hard link created between scan and apply would pass a byte
comparison while meaning the candidate *is* the keeper, and FR-17's promise that
hard-linked siblings are never removal candidates has to hold at apply time too.
Every failure mode, including an I/O error during the check itself, is a skip —
the file stays.

### Finding A (defect in a dependency's default, fixed) — `trash` hangs forever on macOS

`trash::delete` blocked **indefinitely** on macOS: three separate runs had to be
killed after 3–5 minutes, inside the sandbox and outside it.

Cause: `trash` 5.2.6 defaults to `DeleteMethod::Finder`, which shells out to
`osascript` to ask Finder to perform the delete. That requires macOS Automation
permission; in any non-interactive context — a script, an SSH session, CI — the
TCC prompt cannot be answered and the call never returns.

This matters beyond convenience: **ADR-0009's fail-safe design assumes a trash
failure returns an error** so the file can be skipped and reported. A call that
never returns defeats that entirely, and a CLI that hangs forever on `--apply`
is worse than one that reports an error.

Fixed by holding a configured `TrashContext` and selecting
`DeleteMethod::NsFileManager` on macOS, which calls `trashItemAtURL` directly,
needs no permission, and is faster. Documented trade-off: "Put Back" may not
appear in Finder's context menu on some systems (a macOS bug, not a crate one);
the file is still in the Trash and still recoverable, which is what FR-10 asks
for. Recorded as a consequence note in
[ADR-0009](../../adr/0009-cross-platform-trash-and-recycle.md).

### Finding B (dangerous test, fixed) — a destructive flag pointed at the checkout

`tests/cli_integration.rs::apply_delete_together_exits_0` ran
`imgdedup . --apply --delete` — a destructive invocation whose scan root was
**the repository working directory**. It was harmless only because both flags
were inert. Wiring `--apply` up in this unit made it live, and TASK-0310 would
have turned it into a permanent deletion of whatever images sit in a developer's
checkout or the CI workspace.

Rewritten to scan a `tempfile::tempdir()` it owns. The rule this establishes,
worth enforcing in review for the rest of the slice: **no test may point a
destructive flag at a directory it did not create.**

### Deliberate gaps from session 2 — how each closed

- **`--apply --delete` refused** — replaced by the real `DeleteRemover`
  (TASK-0310). The refusal existed so a user asking for permanent deletion was
  never silently handed trashing instead.
- **Performed-action reporting was minimal** — the report now lists every
  removed path and every skip with its reason, and `--json` carries `mode`
  (`dry-run`/`trash`/`delete`) plus an `actions` object. Additive, so
  `schema_version` stays 1.
- **No end-to-end TOCTOU test** — closed as far as is honest. A genuine
  scan-then-mutate race cannot be driven from outside the process, so the
  per-branch coverage stays in `src/apply.rs`'s unit tests, which drive
  `revalidate` directly. What *was* added end to end is a real failure path: a
  duplicate inside a read-only directory, where the scan can read the file but
  `remove_file` cannot unlink it.
- **Trash recoverability asserted only indirectly** — still true, and now an
  explicit item for TASK-0312 rather than an implicit gap. The AC-3 test asserts
  an XOR (gone ⇒ reported trashed; present ⇒ reported skipped), which a lying
  implementation would fail, but it cannot prove the file is *recoverable*. The
  architect verified by hand on macOS that the file lands in `~/.Trash`.

## Product-owner acceptance — FR-10/11/12/16, NFR-1/2/6 (2026-08-01)

**Accept**, all requirement groups and AC-3/5/7/11/17, checked against the
shipped code and the integration tests that drive the real binary.

Two product rulings requested and given:

1. **A bare `--apply` keeps no extra guard for v0.1.0.** It is already opt-in,
   its default mechanism is recoverable, `--delete` needs a second explicit
   flag, and AC-11 parity makes "dry-run first" a reliable habit. Interactive
   prompting is a PRD non-goal; a size-threshold guard was considered and left
   as a kaizen idea rather than adopted (YAGNI).
2. **The report must list removed paths, not just a count** — a count alone does
   not tell someone which files to fish out of the trash, worst exactly when
   several share a basename across directories. Implemented in this slice.

## Architect review — whole slice (2026-08-01)

**Verdict: approve after fixes** — one process blocker and ten should-fixes, all
resolved in-unit. The reviewer built the binary and exercised the destructive
paths by hand, including overlapping scan roots, `--follow-symlinks`, a locked
directory, a failing scan root and a path containing ANSI escapes, and reported
finding **no way to destroy a keeper or empty a group**.

| # | Severity | Finding | Resolution |
|---|---|---|---|
| 1 | blocker | The planning gate was not satisfied: both task files and both backlog rows still read `todo`, and `status.md` still described `--apply --delete` as refusing to run. Anyone reading the repo after merge would have concluded the slice had not shipped. | Fixed — statuses flipped, status.md rewritten, this log closed out. |
| 2 | should-fix | Re-validation proved metadata and identity of one file and content of *possibly another*: `files_equal` re-opened both paths from scratch, so the whole duration of a byte-for-byte comparison — seconds on a multi-gigabyte file — sat inside the window. | Fixed — `revalidate` opens both files once and drives identity (`Handle::from_file`) and content (`files_equal_handles`) from those handles, narrowing the residual to the single unavoidable path→remove gap. |
| 3 | should-fix | The asymmetry was backwards: skips were streamed to stderr, removals were only reported at the end. A run killed partway through a permanent delete left the user **no record of what was destroyed**. | Fixed — the executor emits an `Event` per decision and `main` streams removals as they happen. |
| 4 | should-fix | `--follow-symlinks` silently broke dry-run/apply parity *and* reported it misleadingly: a symlink candidate got "no longer a regular file" (it never was one) and a symlink keeper got "the file to keep is missing or unusable" (it was neither). | Fixed — distinct `IsASymlink` and `KeeperIsASymlink` reasons with truthful wording, and the parity exception is now documented in the guide. Whether to drop symlinks from candidacy entirely is a product call → **TASK-0320**. |
| 5 | should-fix | A destructive run proceeded over a partially-scanned invocation: an unreadable root exits 1, yet the deletion over the other roots still happened. A script trusting the exit-code table would assume nothing had. | Fixed — the run refuses to apply when any scan root failed. "The scan I was asked for did not happen" is a poor basis for an irreversible action, and the likeliest cause is a typo in one of several paths. |
| 6 | should-fix | The guide claimed re-validation made the tool "safe to re-run even if something else is touching the same files concurrently" — it does not, and ADR-0008 still listed three checks when the implementation has four, with an unqualified claim that TOCTOU "cannot" cause data loss. | Fixed — ADR-0008 gains a consequence note recording check 4 (identity, FR-17 at apply time) and the residual window; the guide now distinguishes what the check genuinely buys (a tree that drifted) from what it cannot (winning a deliberate race). |
| 7 | should-fix | The stream contract said per-file warnings appear "only" in the JSON document and are "not echoed to stderr" — but apply-stage skips are, in `--json` too. | Fixed — the contract documents apply-stage removals and skips as a third category, streamed live, and notes the deliberate double-reporting in human mode. |
| 8 | should-fix | The TASK-0310 reporting half had **zero** unit tests: nothing pinned the dry-run guard (the check that stops a dry run claiming it acted), the pluralization, or the JSON `actions` shape, and `mode: "trash"` was asserted nowhere at all. | Fixed — four report tests and two JSON tests, including a dry run handed a non-empty outcome. |
| 9 | should-fix | Mechanism and reported mode were two independent branches on the same flag — the only place a report could claim an action that did not happen. | Fixed — chosen together as a tuple. |
| 10 | should-fix | The keeper guard used `debug_assert!(false, …)`, which **panics** in every dev and test build: an invariant violation would abort mid-loop, leaving a partially-destroyed tree and no report — precisely the scenario finding 3 is about. It also recorded nothing, breaking `ApplyOutcome`'s own accounting promise. | Fixed — records a `KeeperInRemovableSet` skip so the violation is loud in the output on every build profile, without destroying the run. |
| 11 | should-fix | `json.rs`'s module-doc schema omitted `actions` and still said `mode` is `"dry-run"` until this slice extends it. | Fixed. |
| 12–17 | nit | Keeper re-read cost per candidate; a dead `DryRun` arm in `past_tense`; `DeleteRemover` constructible anywhere despite its doc; the trash path accepting a directory where delete would not; hostile paths reaching the terminal unescaped; and repeated scan roots reporting a tree as clean when it is not. | Trade-offs and constraints documented in the module doc; escape-injection sites added to **TASK-0318**'s scope (this slice does not widen the class — `report.rs` already printed raw paths); repeated-roots tracked as **TASK-0319**. |

**Confirmed clean by the reviewer:** no escalation path from a trash failure to a
deletion anywhere (one `remove_file`, one trash call, both behind the trait, both
constructed only in `main` under validated flags, no retry loop); `mode` cannot
claim an action that did not happen; stdout purity holds under `--json` with the
apply in the pipeline; NFR-2 holds transitively at apply time via the byte
comparison against a sniffed keeper; the macOS `NsFileManager` fix is
architecturally right and no other platform needs an override; `cargo-deny` clean
on the new dependency tree; and every worked example in the guide matches the
binary byte-for-byte.
