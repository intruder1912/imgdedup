---
name: using-imgdedup
description: Use when the user asks to find, review, or remove duplicate images with imgdedup, or wants an AI-guided cleanup of a photo folder. Defines the only safe way to operate the imgdedup CLI — dry run first, explicit confirmation before touching disk, and a separate explicit confirmation before permanent deletion.
---

# Using imgdedup

## Overview

`imgdedup` finds byte-for-byte duplicate images and can move (or permanently
delete) the redundant copies. This skill defines the **only** safe way an
agent operates it: dry run first, confirm before touching disk, and never run
`--delete` without a fresh, explicit request for *permanent* deletion in the
same turn.

**Announce at start:** "I'm using the using-imgdedup skill to safely scan for duplicates."

## Step 1 — Preflight

Run `imgdedup --version`. If it fails (not installed / not on `PATH`), stop and
tell the user to install it (see the imgdedup README's Installing section).
Never attempt to install it yourself.

## Step 2 — Dry run, always first

Ask for a path if the user didn't give one. Run:

```sh
imgdedup <path> --json
```

Never pass `--apply` or `--delete` in this step. Parse the JSON (schema
version 1 — field reference below) and present a plain-language summary:

- the duplicate group count and reclaimable bytes (`totals`)
- for each group (or a representative sample if there are many): the keeper
  and the files that would be removed (`groups[].keeper`, `groups[].removable`)
- every entry in `warnings` and `root_errors`, verbatim — never drop these,
  even if they seem minor

**Exit codes matter — read them before deciding the run failed:**

- Exit `0`: clean run.
- Exit `1`: `root_errors` is non-empty (some input path couldn't be scanned) —
  but stdout still has valid, parseable JSON for everything that *could* be
  scanned. Parse it and report the results plus the `root_errors`, exactly as
  you would on a clean run. Do not treat exit `1` as "the command failed" and
  do not retry with different flags — a nonzero exit here means partial
  results, not no results.
- Exit `2`: usage error (bad flags/arguments) — fix the invocation, don't
  retry blindly.

If `root_errors` is non-empty, say so clearly: some input paths could not be
scanned at all.

## Step 3 — Gate 1: confirm before `--apply`

If `totals.removable_files` is 0, stop here — nothing to do (this can be true
even if `totals.duplicate_groups` is nonzero, when every group found is
already fully hard-linked — nothing to reclaim).

Otherwise, ask the user to explicitly approve moving the listed files to the
trash. Do not proceed on an ambiguous or implied "ok" from earlier in the
conversation — the user must approve *this* action, now. If they don't
approve, stop; nothing on disk changes.

On approval, run:

```sh
imgdedup <path> --apply --json
```

The apply run must be the **exact same command** the user saw in the dry run,
with only `--apply` added — same path(s), same `--keep`/scope flags if any
were used. If anything about the scope differs from what was shown and
approved, you are not authorized to run it — go back to Step 2 with the new
scope and get a fresh approval.

Report `actions.removed` and `actions.skipped` from the result — this is what
*actually* happened, not a restatement of the dry-run plan (a file can be
skipped at apply time if it changed since the scan — imgdedup re-verifies
every file immediately before removing it).

## Step 4 — Gate 2: `--delete` requires its own explicit request

Only ever run `--delete` (always together with `--apply`) if, in the **same
turn**, the user explicitly asks for permanent deletion — phrasing like
"delete permanently", "don't use the trash", "permanently remove". A prior
"yes" to Step 3 does not carry over and does not authorize this step.

If the user only says "delete the duplicates" without qualifying it, that
means Step 3 (trash), not this step — trash is the default destructive action;
permanent deletion is opt-in and must be named as such by the user.

```sh
imgdedup <path> --apply --delete --json
```

**Gate 2 is always in addition to Gate 1, never instead of it.** Never run any
destructive command (`--apply`, with or without `--delete`) in the same
assistant turn as the dry run. Run the dry run, show the user the plan, and
end your turn. The user's approval — for either gate — must arrive as a new
message after they have actually seen the file list, even if they pre-empt
you by requesting permanent deletion in the same message that asked for the
scan.

## Working with the imgdedup-operator subagent

For large or multi-path scans where the JSON would flood the main
conversation's context, you may dispatch the `imgdedup-operator` subagent to
do Steps 1–2 in isolation and return the summary. It never runs `--apply` or
`--delete` under any circumstance. Steps 3–4 always happen here, in the main
conversation, after it reports back — never delegate the confirmation gates
themselves.

## JSON field reference (schema version 1)

Fields you need, from `docs/guide/output-and-exit-codes.md` in the imgdedup
repository:

- `mode`: what the run actually did — `"dry-run"`, `"trash"`, or `"delete"`.
  State this plainly when reporting back (e.g. "this was a dry run" vs "files
  were trashed").
- `totals.duplicate_groups`, `totals.removable_files`, `totals.reclaimable_bytes`
- `groups[]`: each has `keeper` (File), `removable[]` (File[]), and
  `protected[]` (File[]) — hard-link anchors that are kept but free no space.
  Report `protected` entries too when present; don't silently drop them.
- `File`: `path`, `size`, `mtime` (integer seconds, or `null` when the
  platform didn't provide one — don't assume it's always present)
- `warnings[]`: each has `stage` (`"enumerate"` | `"sniff"` | `"engine"`),
  `path`, `reason`
- `root_errors[]`: each has `path`, `reason`
- `actions.removed[]` (paths actually trashed/deleted, empty on dry run)
- `actions.skipped[]`: each has `path`, `reason` (empty on dry run)

## Anti-patterns

- Running `--apply` or `--delete` without having shown the user a dry-run
  summary first, in this conversation.
- Treating a general "yes, clean it up" as authorization for `--delete`.
- Paraphrasing `actions.removed`/`actions.skipped` from memory instead of
  reading them from the actual command output.
- Letting the `imgdedup-operator` subagent run anything but a dry run.
