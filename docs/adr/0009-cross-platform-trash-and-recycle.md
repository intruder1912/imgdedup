# ADR-0009: Cross-platform trash / recycle

- **Status:** accepted
- **Date:** 2026-06-14
- **Deciders:** architect, human owner
- **Related:** [PRD-0001](../prd/PRD-0001-exact-image-deduplication.md)
  (FR-10, FR-11, FR-16; NFR-1, NFR-5, NFR-6; AC-3, AC-7, AC-17),
  [ADR-0007](0007-technology-stack-rust-clap.md) (this requirement decided
  Rust over Go), [ADR-0008](0008-core-deduplication-algorithm-and-semantics.md)
  (what is removed and the apply-time re-validation that precedes removal)

## Context and problem statement

`imgdedup`'s default destructive action is **recoverable**: `--apply` moves every
non-keeper to the operating system's **trash / recycle bin** so a mistake can be
undone (FR-10, the central safety affordance of NFR-1). `--apply --delete`
**permanently** removes non-keepers instead (FR-11, opt-in). This must work
**identically across Linux, macOS, and Windows** (NFR-5), each of which has a
completely different trash mechanism. This was named the project's **riskiest
requirement** and was the deciding factor for the Rust stack (ADR-0007).

The decision: how to perform recoverable removal portably and robustly, and how to
behave when the trash is unavailable (NFR-6). Worked through and confirmed with
the human owner.

## Decision drivers

- **Recoverability (FR-10, NFR-1):** the default action must land files where the
  user can restore them through their normal OS workflow — not a custom location.
- **Portability (NFR-5):** one code path, correct on all three platforms, honoring
  each platform's native trash semantics.
- **Single self-contained binary (ADR-0007):** no shelling-out to external tools,
  no cgo if avoidable.
- **Robust errors (NFR-6):** when trashing fails or trash is unavailable, fail
  **safe** — never silently fall back to permanent deletion, never abort the whole
  run on one file.
- **Safety invariant (NFR-1):** removal only ever happens *after* the ADR-0008
  apply-time re-validation; this ADR covers the removal mechanism itself.

## Considered options

1. **The `trash` crate** for `--apply`; **direct filesystem removal**
   (`std::fs::remove_file`) for `--delete`. (chosen)
2. **Shell out** to per-OS tools (`gio trash`, AppleScript/`osascript`,
   PowerShell recycle APIs).
3. **Hand-roll** each platform's trash protocol ourselves (XDG spec files, Cocoa
   FFI, COM `IFileOperation`).

## Decision outcome

**Chosen: option 1** — use the maintained Rust **`trash`** crate for the
recoverable `--apply` path, and **direct `std::fs::remove_file`** for the opt-in
permanent `--delete` path.

- **`--apply` → `trash`:** the crate implements each platform's native recycle
  mechanism behind one API, so a single code path satisfies FR-10 on all three OSes
  (NFR-5), keeping the single-binary property (ADR-0007). Trashed files land in the
  user's normal trash and are restorable through the OS UI (AC-3).
- **`--apply --delete` → `remove_file`:** permanent deletion is an ordinary,
  portable filesystem call; no trash indirection (AC-7). `--delete` is rejected
  without `--apply` at the CLI layer (clap), guarding against accidental permanent
  loss (AC-12) before any filesystem call is reached.

### Per-OS semantics (handled by the `trash` crate)

- **Linux:** the **FreeDesktop / XDG Trash specification** — files move to
  `$XDG_DATA_HOME/Trash` (or the correct per-mount `.Trash-$uid` for files on other
  mounts), with the `files/` + `info/` records the spec requires so desktop
  environments can restore them.
- **macOS:** the **Finder / `NSFileManager`** trash
  (`trashItem(at:resultingItemURL:)` semantics), so items appear in the user's
  Trash exactly as a Finder delete would, with put-back support.
- **Windows:** the **Recycle Bin** via the shell **`IFileOperation`** API, so items
  are recoverable from the Recycle Bin with original-location restore.

### Error handling when trash is unavailable (NFR-6)

Trashing can fail — no desktop trash on a headless Linux box, a filesystem/mount
with no valid trash directory, a permission error, or a removable volume. Behavior:

- A trash failure on a file is a **per-file error**: the file is **reported and
  skipped**, the keeper and all other files are untouched, and the **run
  continues** and still summarizes (NFR-6, mirroring AC-10/AC-17). The exit code
  follows the ADR-0008 scheme.
- **Never silently fall back to permanent deletion.** If `--apply` cannot trash a
  file, the file is **left in place**, not deleted — failing *safe* (NFR-1). Opting
  into permanent removal is only ever the explicit `--delete` flag.
- The summary distinguishes **trashed**, **skipped (trash failed)**, and (under
  `--delete`) **permanently deleted**, so the user always knows what happened to
  each file.
- Because removal is preceded by the ADR-0008 apply-time re-validation, a file that
  changed since the scan is skipped *before* any trash/delete is attempted.

### Consequences

- **Positive:** one portable code path for recoverable removal (NFR-5); native,
  restorable trash on every platform (FR-10, AC-3); fail-safe behavior that never
  upgrades a trash failure into permanent deletion (NFR-1); single binary preserved.
- **Negative / trade-offs:** we depend on the `trash` crate's per-OS correctness
  and its (transitive) platform FFI — mitigated by the dependency-audit gate
  (cargo-audit/cargo-deny, ADR-0007) and integration tests per platform in CI.
  Trash semantics genuinely differ per OS (e.g. cross-mount behavior on Linux), so
  some behavior is environment-dependent by nature and is documented, not hidden.
- **Follow-ups / new tasks created:** Phase 3 implementation tasks for the
  trash/delete executor and its per-platform integration tests are seeded after
  this PR; the `trash` crate is added to the dependency set audited by TASK-0208's
  reframed gate.

## Pros and cons of the options

### Option 1 — `trash` crate + `remove_file` (chosen)

- Good: one maintained, cross-platform API for the riskiest requirement; native
  recycle semantics on each OS; no shelling-out; single binary; permanent delete is
  a trivial, well-understood std call.
- Bad: relies on a third-party crate's per-OS FFI correctness (audited, tested).

### Option 2 — shell out to per-OS tools

- Good: minimal own-code; reuses the OS's own utilities.
- Bad: breaks the single-binary/no-runtime goal; tools are not always present
  (`gio` absent on minimal Linux, etc.); brittle output parsing; a larger,
  harder-to-audit attack surface (spawning processes with user-derived paths).

### Option 3 — hand-roll each platform's trash

- Good: zero third-party trash dependency; full control.
- Bad: re-implements three intricate, security-sensitive protocols (XDG spec,
  Cocoa FFI, COM `IFileOperation`) — exactly the work the `trash` crate exists to
  do once, correctly; high risk on the requirement we least want to get wrong.

## Links

- [PRD-0001](../prd/PRD-0001-exact-image-deduplication.md) FR-10/FR-11/NFR-5/NFR-6
  — the recoverable/permanent removal and portability requirements.
- [ADR-0007](0007-technology-stack-rust-clap.md) — native cross-platform trash was
  the deciding factor for Rust over Go.
- [ADR-0008](0008-core-deduplication-algorithm-and-semantics.md) — the apply-time
  re-validation (FR-16) that gates every call into this removal mechanism.
- The `trash` crate; the FreeDesktop Trash specification; `NSFileManager`
  `trashItem`; Win32 shell `IFileOperation`.
