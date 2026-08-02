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

If `root_errors` is non-empty, say so clearly: some input paths could not be
scanned at all.

## Step 3 — Gate 1: confirm before `--apply`

If `totals.duplicate_groups` is 0, stop here — nothing to do.

Otherwise, ask the user to explicitly approve moving the listed files to the
trash. Do not proceed on an ambiguous or implied "ok" from earlier in the
conversation — the user must approve *this* action, now. If they don't
approve, stop; nothing on disk changes.

On approval, run:

```sh
imgdedup <path> --apply --json
```

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

- `totals.duplicate_groups`, `totals.reclaimable_bytes`
- `groups[]`: each has `keeper` (File) and `removable[]` (File[])
- `File`: `path`, `size`, `mtime`
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
