# Using imgdedup with an AI agent

`imgdedup` ships an official **Claude Code plugin** so you can find and clean
up duplicate images by asking Claude, instead of hand-writing CLI commands and
parsing `--json` output yourself. It's a thin, safety-first wrapper over the
same CLI documented in the rest of this guide — it doesn't do anything the
CLI can't already do on its own.

## What it is

The plugin bundles three things:

- **A skill** (`using-imgdedup`) that defines the only safe way an agent may
  operate `imgdedup`: always scan read-only first, always show you the plan,
  and always ask before touching disk.
- **A slash command**, `/imgdedup:dedupe [path]`, that triggers the skill in
  your current conversation.
- **A subagent**, `imgdedup-operator`, for offloading large or multi-folder
  scans without flooding your conversation with a big JSON payload. It is
  **instructed to be read-only** — it will only ever run a dry-run scan and
  report back, and will refuse to apply or delete no matter how it's asked.
  And because anything it did would still need to pass your own Claude Code
  permission prompts, nothing reaches disk without you.

## Installing it

The plugin lives in the `imgdedup` repository under `claude-plugin/`. Add the
repository as a plugin source and install it:

```text
/plugin marketplace add intruder1912/imgdedup
/plugin install imgdedup
```

## Using it

Ask Claude in plain language, or use the command directly:

```text
/imgdedup:dedupe ~/Pictures
```

Every run follows the same protocol, no matter how you ask:

1. **It checks `imgdedup` is installed.** If it isn't, you'll be pointed at
   the [Installing](../../README.md#installing) section — the agent never
   installs it for you.
2. **It always scans read-only first** (`imgdedup <path> --json`) and shows
   you a plain-language summary: how many duplicate groups, how much space is
   reclaimable, which file would be kept in each group, and any warnings or
   errors from the scan.
3. **It asks before moving anything to the trash.** Nothing changes on disk
   until you explicitly approve — a vague "sure" earlier in the conversation
   doesn't count; you'll be asked directly.
4. **Permanent deletion needs its own explicit ask.** Approving the trash
   move in step 3 never authorizes `--delete`. If you want permanent deletion
   instead of trash, say so explicitly (e.g. "delete them permanently, don't
   use the trash") in the same turn.

A worked example:

```text
You: dedupe my Pictures folder

Claude: Found 1 duplicate group (2 files, 5,242,880 bytes reclaimable).
        KEEP  ~/Pictures/beach.jpg
        REMOVE ~/Pictures/backup/beach (copy).jpg
        Move this to the trash? (nothing changes until you confirm)

You: yes

Claude: Moved 1 file to the trash. beach (copy).jpg is recoverable from your
        system trash if you change your mind.
```

## Offloading large scans

For a big library or several directory trees at once, ask Claude to use the
`imgdedup-operator` subagent instead — it scans and reports back a plan
without filling up your conversation with the full JSON. It's read-only by
design: it will refuse to apply or delete anything itself, even if you ask it
to directly. The trash/delete steps above always still happen with you, in
your main conversation, afterward.

## Design background

The full rationale — why the protocol is split this way, and the known
limitation of the subagent's read-only boundary — is in
[ADR-0016](../adr/0016-ai-agent-automation-plugin.md).
