# Review log — Slice S3 `apply-delete` (`feature/apply-delete`)

- **PRD:** [PRD-0001](../../prd/PRD-0001-exact-image-deduplication.md)
- **Slice scope:** TASK-0309 (trash apply + apply-time re-validation) and
  TASK-0310 (permanent delete). FR-10, FR-11, FR-12, FR-16, NFR-1, NFR-2,
  NFR-6; AC-3, AC-5, AC-11, AC-17.
- **Status:** open — TASK-0309 core parked at the session-2 cut line;
  TASK-0310, acceptance, docs and the whole-slice review are session 3.

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

### Deliberate gaps, to close in session 3

- **`--apply --delete` currently refuses** with "not implemented yet
  (TASK-0310)" and exit 1, rather than falling through to trashing. Handing a
  user who asked for permanent deletion a different mechanism silently is
  precisely what ADR-0009 forbids. TASK-0310 replaces the guard.
- **Performed-action reporting is minimal** — a summary line on stdout and
  per-file skips on stderr. Rendering performed actions inside the report and
  `--json` (including `mode`, which still reads `"dry-run"`) is the rest of the
  slice.
- **The TOCTOU path has no end-to-end test.** The unit tests cover every
  `revalidate` branch through the injected `Remover`, but an integration test
  would need to mutate the tree between the scan and the apply within a single
  process run. Worth a deliberate hook or an accepted gap, decided in session 3.
- **Trash recoverability is asserted only indirectly** (the file leaves its
  original path and the OS call reports success). `~/.Trash` is not listable
  under this sandbox, so a real recoverability assertion needs the acceptance
  environment.
