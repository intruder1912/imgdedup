# AI Automation Plugin (TASK-0218) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship an official `imgdedup` Claude Code plugin (skill + slash command + read-only operator subagent) that lets end users safely drive the `imgdedup` CLI from Claude, with a dry-run-first, two-gate confirmation protocol and no unattended `--apply`/`--delete`.

**Architecture:** A single Claude Code plugin under `claude-plugin/` wraps the existing CLI (`--json`, `--apply`, `--delete` — no product-code changes). The `using-imgdedup` skill is the single source of truth for the safety protocol; `/dedupe` triggers it inline in the main conversation; the `imgdedup-operator` subagent is read-only recon only (dry-run/`--json`), never applying or deleting. Documented as a first-class product capability (README, a new guide page, a PRD-0001 amendment) and formalized as ADR-0016.

**Tech Stack:** Markdown (skill/command/agent definitions, ADR, guide page), JSON (`plugin.json`), no Rust/product code changes. Manual functional verification against a real `imgdedup` binary (no new automated test suite — see Task 5).

## Global Constraints

- **Branch:** `feature/ai-automation-support` (already created; the design spec commit `2a2fb90` is already on it). All work in this plan happens on this branch. Do not commit to `main`.
- **No product code changes.** Nothing under `src/`, `tests/`, or `Cargo.toml` is touched by this unit.
- **Never unattended `--apply`/`--delete`.** Every destructive CLI invocation described in any file this plan creates must be gated behind an explicit human confirmation in the main conversation (spec §4, Gates 1–2). The `imgdedup-operator` subagent must never be given a path to run `--apply` or `--delete`.
- **License header/attribution:** none required — this repo doesn't use per-file license headers (verify by checking an existing `.md`/`.rs` file has none before adding one).
- **Commit style:** Conventional Commits (this is a `feat:` unit — the branch is `feature/`).
- **Design source of truth:** `docs/superpowers/specs/2026-08-02-ai-automation-design.md` (already committed on this branch). Every task below implements a specific section of it — re-read the relevant section if anything here seems ambiguous.
- **Plugin name:** `imgdedup` throughout (directory `claude-plugin/`, `plugin.json` `"name": "imgdedup"`, skill `using-imgdedup`, command `dedupe` → invoked as `/imgdedup:dedupe`, agent `imgdedup-operator`).

---

### Task 1: Scaffold the plugin package

**Files:**
- Create: `claude-plugin/.claude-plugin/plugin.json`
- Create: `claude-plugin/skills/.gitkeep` (removed in Task 2 once `SKILL.md` lands — see Step 3)
- Create: `claude-plugin/commands/.gitkeep` (removed in Task 3)
- Create: `claude-plugin/agents/.gitkeep` (removed in Task 4)

**Interfaces:**
- Produces: the `claude-plugin/` directory tree and `plugin.json` manifest that Tasks 2–4 populate. No code interfaces (this is package metadata only).

- [ ] **Step 1: Create the directory tree with placeholders**

```bash
mkdir -p claude-plugin/.claude-plugin claude-plugin/skills claude-plugin/commands claude-plugin/agents
touch claude-plugin/skills/.gitkeep claude-plugin/commands/.gitkeep claude-plugin/agents/.gitkeep
```

- [ ] **Step 2: Write `claude-plugin/.claude-plugin/plugin.json`**

```json
{
  "name": "imgdedup",
  "description": "Safely operate the imgdedup CLI from Claude: a dry-run-first duplicate scan protocol, a /dedupe slash command, and a read-only imgdedup-operator subagent for large scans. Never applies or deletes without explicit in-conversation confirmation.",
  "version": "0.1.0",
  "homepage": "https://github.com/intruder1912/imgdedup",
  "repository": "https://github.com/intruder1912/imgdedup",
  "license": "GPL-3.0-only",
  "keywords": ["imgdedup", "deduplication", "images", "cli", "automation"]
}
```

`version` starts at `0.1.0` as a placeholder, mirroring how `Cargo.toml`'s
`version` field is a dev placeholder (releases are tagged, not committed back —
ADR-0015). Keeping the plugin in sync with real crate releases on every future
release is out of scope for this task (design spec §3 flags it as a
forward-looking intent, not a mechanism to build now).

- [ ] **Step 3: Verify the JSON is valid**

Run: `python3 -m json.tool claude-plugin/.claude-plugin/plugin.json`
Expected: pretty-printed JSON echoed back, no error.

- [ ] **Step 4: Commit**

```bash
git add claude-plugin/
git commit -m "feat(claude-plugin): scaffold imgdedup Claude Code plugin package"
```

---

### Task 2: Write the `using-imgdedup` skill

**Files:**
- Create: `claude-plugin/skills/using-imgdedup/SKILL.md`
- Delete: `claude-plugin/skills/.gitkeep`

**Interfaces:**
- Consumes: the `--json` schema documented in `docs/guide/output-and-exit-codes.md` (top-level fields `schema_version`, `mode`, `groups[]`, `totals`, `actions`, `warnings[]`, `root_errors[]`; `groups[].keeper`/`removable[]`; `File` has `path`/`size`/`mtime`; `Warning` has `stage`/`path`/`reason`; `RootError` has `path`/`reason`; `actions.removed[]`/`actions.skipped[]`, `Skip` has `path`/`reason`).
- Produces: the `using-imgdedup` skill, referenced by name in Task 3's command and Task 4's agent instructions. Its four steps (Preflight, Dry run, Gate 1, Gate 2) are the exact step names Task 3/4 must refer back to.

- [ ] **Step 1: Write `claude-plugin/skills/using-imgdedup/SKILL.md`**

```markdown
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
```

- [ ] **Step 2: Remove the placeholder**

```bash
git rm claude-plugin/skills/.gitkeep
```

- [ ] **Step 3: Commit**

```bash
git add claude-plugin/skills/using-imgdedup/SKILL.md
git commit -m "feat(claude-plugin): add using-imgdedup safety-protocol skill"
```

---

### Task 3: Write the `/imgdedup:dedupe` slash command

**Files:**
- Create: `claude-plugin/commands/dedupe.md`
- Delete: `claude-plugin/commands/.gitkeep`

**Interfaces:**
- Consumes: the `using-imgdedup` skill from Task 2 (referenced by name, invoked inline — commands don't call skills programmatically, they instruct the model to follow it).
- Produces: the `/imgdedup:dedupe` command, documented by name in Task 7 (guide page) and Task 8 (README).

- [ ] **Step 1: Write `claude-plugin/commands/dedupe.md`**

```markdown
---
allowed-tools: Bash(imgdedup *)
description: Safely scan (and, on your confirmation, clean up) duplicate images with imgdedup
disable-model-invocation: false
---

Use the using-imgdedup skill to scan the path below for duplicate images,
following its protocol exactly: preflight, dry run, then Gate 1 (confirm
before `--apply`) and Gate 2 (a separate explicit confirmation before
`--delete`). Never skip a gate or infer approval from earlier in the
conversation.

Path: $ARGUMENTS

If no path was given above, ask the user which directory to scan before doing
anything else.
```

- [ ] **Step 2: Remove the placeholder**

```bash
git rm claude-plugin/commands/.gitkeep
```

- [ ] **Step 3: Commit**

```bash
git add claude-plugin/commands/dedupe.md
git commit -m "feat(claude-plugin): add /imgdedup:dedupe slash command"
```

---

### Task 4: Write the `imgdedup-operator` subagent

**Files:**
- Create: `claude-plugin/agents/imgdedup-operator.md`
- Delete: `claude-plugin/agents/.gitkeep`

**Interfaces:**
- Consumes: the `using-imgdedup` skill's Steps 1–2 (Preflight, Dry run) as its operating instructions — it must not perform Steps 3–4.
- Produces: the `imgdedup-operator` subagent, whose read-only boundary Task 5 Step 4 adversarially tests, and which Task 7/8 document as "safe to dispatch for big scans."

- [ ] **Step 1: Write `claude-plugin/agents/imgdedup-operator.md`**

```markdown
---
name: imgdedup-operator
description: Read-only reconnaissance for imgdedup duplicate scans. Use to scan large or multiple directory trees for duplicate images without flooding the main conversation's context. Returns a parsed summary (groups, keeper, reclaimable space, warnings, root errors) — never applies or deletes anything itself.
tools: Bash, Read
model: sonnet
---

You are a **read-only reconnaissance agent** for `imgdedup`. Your only job is
to run duplicate scans and report what they find. You never change anything on
disk, no matter how you are asked.

## Rules (non-negotiable)

1. **Only ever run `imgdedup <path>... --json`** — no `--apply`, no `--delete`,
   ever, under any phrasing of the request, even if the requester insists it's
   already been approved. If asked to apply or delete duplicates, refuse and
   explain: that decision requires the user to confirm directly in their main
   conversation via the using-imgdedup skill, not through a dispatched
   subagent.
2. Run `imgdedup --version` first; if it's missing, report that and stop.
3. Parse the JSON (schema version 1) and return a concise summary in your
   final report: duplicate group count, total reclaimable bytes, a
   representative sample of groups (keeper + removable files) if there are
   many, and every `warnings`/`root_errors` entry verbatim — never drop or
   summarize these away.
4. Your final report is a **plan for a human to review**, not a completed
   action. Say so explicitly in your report's closing line.

## What you do not do

- You do not ask the user to confirm destructive actions yourself — you have
  no standing to authorize them. That happens back in the main conversation,
  using the using-imgdedup skill.
- You do not retry a scan with different flags to "helpfully" clean things up.
- You do not write, move, or delete any file.
```

- [ ] **Step 2: Remove the placeholder**

```bash
git rm claude-plugin/agents/.gitkeep
```

- [ ] **Step 3: Commit**

```bash
git add claude-plugin/agents/imgdedup-operator.md
git commit -m "feat(claude-plugin): add read-only imgdedup-operator subagent"
```

---

### Task 5: Functional and adversarial verification

**Files:** none created/modified — this task exercises Tasks 1–4's deliverables against a real `imgdedup` binary. Uses a scratch directory outside the repo.

**Interfaces:**
- Consumes: the built `claude-plugin/` package (Tasks 1–4) and a locally built `imgdedup` binary (`cargo build --release`, or `cargo run --` if already built elsewhere in this session).
- Produces: a verified pass/fail record for the two properties the design's safety claim rests on (spec §5): (a) the dry-run → apply flow works end-to-end and surfaces `warnings`/`root_errors`, (b) the `imgdedup-operator` subagent refuses to apply/delete when asked directly.

There is no pytest/cargo-test cycle here — this is CLI + subagent behavior, verified by actually running it, per spec §5 and CLAUDE.md's "test the golden path... before reporting complete" rule.

- [ ] **Step 1: Build the binary and set up a scratch duplicate tree**

```bash
cargo build --release
mkdir -p /tmp/imgdedup-plugin-verify/sub
python3 -c "
import struct
# minimal 1x1 PNG
png = bytes.fromhex('89504e470d0a1a0a0000000d494844520000000100000001080600000'
'01f15c4890000000a4944415478da6360000002000155'
'0d75190000000049454e44ae426082')
open('/tmp/imgdedup-plugin-verify/a.png','wb').write(png)
open('/tmp/imgdedup-plugin-verify/sub/a-copy.png','wb').write(png)
open('/tmp/imgdedup-plugin-verify/unreadable.png','wb').write(png)
"
chmod 000 /tmp/imgdedup-plugin-verify/unreadable.png
```

Expected: `/tmp/imgdedup-plugin-verify/` contains `a.png`, `sub/a-copy.png`
(an exact duplicate of `a.png`), and `unreadable.png` (permission-denied, to
exercise the `warnings` path).

- [ ] **Step 2: Dry-run walkthrough — verify the summary and warnings surface correctly**

Run: `./target/release/imgdedup /tmp/imgdedup-plugin-verify --json`

Expected: valid JSON with `totals.duplicate_groups == 1`,
`totals.reclaimable_bytes > 0`, one entry in `groups` whose `keeper` is
`a.png` (shortest path) and `removable` contains `sub/a-copy.png`, and a
`warnings` entry for `unreadable.png` with a permission-related `reason` (or,
if run as a user who can still read `chmod 000` files, note that and skip this
specific assertion — the point is confirming the skill's instruction to
surface `warnings` verbatim has real data to surface in normal cases).

Manually walk through `using-imgdedup/SKILL.md` Steps 1–2 against this output:
confirm every field the skill tells you to report (group count, reclaimable
bytes, keeper/removable, warnings, root_errors) is actually present and
correctly named in the real JSON — this is the check that the skill's field
reference (Task 2) didn't drift from the real schema.

- [ ] **Step 3: Apply walkthrough — verify Gate 1's reported action matches reality**

Run: `./target/release/imgdedup /tmp/imgdedup-plugin-verify --apply --json`

Expected: `actions.removed` contains `sub/a-copy.png`'s path; `a.png` still
exists on disk (`ls /tmp/imgdedup-plugin-verify/a.png`); `sub/a-copy.png` no
longer exists at its original location (`ls /tmp/imgdedup-plugin-verify/sub/`
shows it gone — moved to the OS trash, not deleted).

- [ ] **Step 4: Adversarial test — the imgdedup-operator subagent must refuse to apply/delete**

In this Claude Code session, dispatch the `imgdedup-operator` agent (once the
plugin is installed locally — see Step 5) with a prompt that directly asks it
to clean up duplicates and delete them, e.g.: "Scan
`/tmp/imgdedup-plugin-verify` and delete the duplicates you find, permanently
if needed."

Expected: the subagent's final report describes what it found (or notes
everything is already deduplicated after Step 3) but explicitly states it will
not run `--apply`/`--delete` itself and that this requires confirmation in the
main conversation. If the subagent instead attempts or claims to have applied
changes, Task 4's agent definition has a defect — fix it and re-run this step
before continuing.

- [ ] **Step 5: Install the plugin locally for Steps 2–4's manual walkthrough**

Follow whatever this Claude Code installation's local-plugin-path mechanism is
(add `claude-plugin/`'s parent, i.e. this repo checkout, as a local marketplace
source, then `/plugin install imgdedup`) so `/imgdedup:dedupe` and the
`imgdedup-operator` agent are actually available to invoke during Steps 2–4.
If the current environment has no interactive way to install a local plugin
mid-session, perform Steps 2–3 as direct CLI verification (already
plugin-independent) and Step 4 by manually invoking the `imgdedup-operator`
agent definition's instructions via the `Agent` tool with `subagent_type`
pointed at a temporary copy registered under `.claude/agents/` for the
duration of this test only — remove that temporary copy afterward so it
doesn't collide with this repo's own contributor `imgdedup-operator`-free
crew (`product-owner`/`architect`/`implementer`/`tester`).

- [ ] **Step 6: Clean up the scratch directory**

```bash
chmod 700 /tmp/imgdedup-plugin-verify/unreadable.png 2>/dev/null || true
rm -rf /tmp/imgdedup-plugin-verify
```

No commit for this task — it produces a verification record, not files. Note
the pass/fail outcome of Steps 2–4 in the PR description (Task 11 covers the
planning-doc side of this).

---

### Task 6: Write ADR-0016

**Files:**
- Create: `docs/adr/0016-ai-agent-automation-plugin.md`
- Modify: `docs/adr/README.md` (index table)

**Interfaces:**
- Consumes: the design spec (`docs/superpowers/specs/2026-08-02-ai-automation-design.md`), `docs/adr/TEMPLATE.md`'s structure.
- Produces: ADR-0016, referenced by Task 7 (guide page) and Task 9 (PRD amendment).

- [ ] **Step 1: Write `docs/adr/0016-ai-agent-automation-plugin.md`**

```markdown
# ADR-0016: AI-agent automation — Claude Code plugin (skill, command, operator agent)

- **Status:** accepted
- **Date:** 2026-08-02
- **Deciders:** architect, human owner
- **Related:** [TASK-0218](../planning/backlog.md), [TASK-0308](../planning/tasks/TASK-0308-json-output.md) (`--json`, this design's only dependency), [design spec](../superpowers/specs/2026-08-02-ai-automation-design.md), [ADR-0012](0012-documentation-ownership-and-placement.md) (doc ownership)

## Context and problem statement

`imgdedup` exposes a scriptable `--json` contract (TASK-0308) and a
dry-run-by-default safety model (PRD-0001 FR-9/FR-10/FR-11), but nothing in
the product tells an AI agent *how* to operate it safely. As AI-agent-driven
usage becomes a more common way people run CLI tools, an ungoverned agent
could plausibly run `--apply --delete` on a whole photo library based on a
single ambiguous "clean this up," with no human ever having reviewed what
would be removed. We need an official, safe integration surface rather than
leaving every user to improvise their own prompt.

Confirmed with the owner (2026-08-02): this ships for **end users** of
`imgdedup`, as a distributable **Claude Code plugin**, and is expected to
become an important product capability over time — documented with full
product-feature weight even though it is architect-designed rather than a
PRD-0001 slice (see [Decision outcome](#decision-outcome) and the PRD-0001
amendment this ADR's implementation adds).

## Decision drivers

- **Safety:** no unattended `--apply`/`--delete` — this is the one hard
  requirement from the backlog (TASK-0218) and non-negotiable per the
  product's existing safety model (PRD-0001 NFR-1).
- **No drift:** the safe-operation protocol should exist in exactly one place,
  not be re-derived separately by a skill and an agent.
- **Distribution:** the mechanism must actually be something end users can
  install into their own Claude Code setup, not just files sitting in this
  repo.
- **Isolation for scale:** large scans produce large JSON payloads; there
  should be a way to keep that out of the main conversation's context without
  weakening the confirmation gates.

## Considered options

1. **Agent-centric** — the `imgdedup-operator` subagent owns the full
   protocol, including applying/deleting, gated by asking the user through its
   own tools.
2. **Skill-only** — all protocol logic lives in a skill triggered inline; no
   dedicated subagent.
3. **Layered** — the skill is the single source of truth for the protocol; a
   slash command triggers it inline in the main conversation; the operator
   subagent is strictly read-only recon, never touching `--apply`/`--delete`.

## Decision outcome

**Chosen option:** "Layered" (option 3), because it is the only one of the
three that makes the "no unattended delete" requirement an **architectural**
property rather than only a prompted one. Dispatched subagents run to
completion and hand back a report; they are not a good fit for a live "here's
what I'd delete, confirm to continue" exchange, so option 1 would have put the
confirmation gate in the weakest possible place. Option 2 is safe but silently
drops the subagent TASK-0218 asked for, losing the context-isolation benefit
for large scans.

Concretely:

- `using-imgdedup` (skill) defines Preflight → Dry run → Gate 1 (`--apply`
  confirmation) → Gate 2 (separate, same-turn confirmation for `--delete`).
- `/imgdedup:dedupe` (command) triggers the skill inline, in the user's own
  conversation — the human who must approve destructive actions is always the
  one present when they happen.
- `imgdedup-operator` (subagent) reuses the skill's Preflight and Dry-run
  steps only, and hands back a plan. It has no instructed path to Gates 1–2;
  those only ever happen in the main thread.

### Consequences

- Positive: the safety boundary between "can scan" and "can destroy" is
  structural (different components), not just a prompt instruction repeated
  in two places.
- Positive: one protocol definition (the skill) — the command and the
  subagent both defer to it instead of re-specifying the rules.
- Negative / trade-off: the `imgdedup-operator` boundary is still enforced by
  its tool access and instructions, not an OS-level sandbox — see the
  "Known limitation" note below. Mitigated by: the subagent's instructions
  never include `--apply`/`--delete` as a valid action; even a hypothetical
  bypass without `--delete` is recoverable via the existing trash mechanism
  (PRD-0001 NFR-1); and the subagent has no way to present its own approval
  prompt as if it were Gate 1/2, since applying only ever happens from the
  main-thread skill flow.
- Follow-ups / new tasks created: keeping `claude-plugin/plugin.json`'s
  version in sync with future crate releases is a manual step for now (no
  task filed — small enough to fold into future release PRs).

**Known limitation (documented, not hidden):** `imgdedup-operator`'s read-only
guarantee is a prompt-and-tool-access boundary, not a hard technical sandbox.
This is an accepted trade-off, not an oversight — see Consequences above for
the mitigations.

## Pros and cons of the options

### Option 1 — Agent-centric

- Good: matches the backlog wording ("`imgdedup-operator` agent") most
  literally; one component to reason about.
- Bad: puts the destructive-action confirmation inside a dispatched subagent,
  the weakest place for a live human-in-the-loop gate; risks protocol drift
  if the agent's copy of the rules and any future skill's copy diverge.

### Option 2 — Skill-only

- Good: simplest; everything happens where the user already is; no subagent
  boundary to get wrong.
- Bad: silently narrows TASK-0218's scope (no subagent shipped at all); loses
  the context-isolation benefit for scans over many/large directory trees.

### Option 3 — Layered (chosen)

- Good: strongest safety boundary of the three; one protocol definition; still
  delivers all three artifacts the backlog asked for.
- Bad: slightly more moving parts than option 2; the subagent's boundary is
  still ultimately instruction-enforced (see Known limitation).

## Links

- [Design spec](../superpowers/specs/2026-08-02-ai-automation-design.md)
- [TASK-0218](../planning/backlog.md)
- [`docs/guide/output-and-exit-codes.md`](../guide/output-and-exit-codes.md) — the `--json` schema this design wraps
- [PRD-0001 Amendment 2026-08-02](../prd/PRD-0001-exact-image-deduplication.md#amendment-2026-08-02--ai-agent-automation-as-a-supported-capability)
```

- [ ] **Step 2: Add ADR-0016 to the index table in `docs/adr/README.md`**

Add this row after the `0015` row (before the closing `>` note block):

```markdown
| [0016](0016-ai-agent-automation-plugin.md) | AI-agent automation — Claude Code plugin (skill, command, operator agent) | accepted |
```

- [ ] **Step 3: Commit**

```bash
git add docs/adr/0016-ai-agent-automation-plugin.md docs/adr/README.md
git commit -m "docs(adr): add ADR-0016 AI-agent automation plugin design"
```

---

### Task 7: Write the user guide page

**Files:**
- Create: `docs/guide/using-with-claude.md`
- Modify: `docs/guide/README.md` (add to the numbered page list)

**Interfaces:**
- Consumes: ADR-0016 (Task 6), the built plugin (Tasks 1–4), Task 5's verified behavior.
- Produces: the guide page linked from Task 8 (README) and Task 9 (PRD amendment).

- [ ] **Step 1: Write `docs/guide/using-with-claude.md`**

```markdown
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
  **read-only** — it can only run a dry-run scan and report back; it can never
  move or delete a file, no matter how it's asked.

## Installing it

The plugin lives in the `imgdedup` repository under `claude-plugin/`. Add the
repository as a plugin source and install it:

```text
/plugin marketplace add intruder1912/imgdedup
/plugin install imgdedup
```

(The repository is currently private — see [the README](../../README.md#installing)
for authenticated-access notes that also apply to adding it as a plugin
source.)

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
```

- [ ] **Step 2: Add the page to `docs/guide/README.md`'s numbered list**

Add as item 6, after "Output and exit codes":

```markdown
6. [Using imgdedup with an AI agent](using-with-claude.md) — the official
   Claude Code plugin: the `/imgdedup:dedupe` command, the dry-run-first
   safety protocol, and the read-only `imgdedup-operator` subagent for large
   scans.
```

- [ ] **Step 3: Commit**

```bash
git add docs/guide/using-with-claude.md docs/guide/README.md
git commit -m "docs(guide): add Using imgdedup with an AI agent guide page"
```

---

### Task 8: Update the top-level README

**Files:**
- Modify: `README.md`

**Interfaces:**
- Consumes: Task 7's guide page.
- Produces: nothing consumed downstream — this is the last documentation-linking step.

- [ ] **Step 1: Insert a new section between "## Detailed guide" and "## How this repository works"**

Insert this section immediately after the "## Detailed guide" section's bullet
list (i.e., right before the `## How this repository works` heading):

```markdown
## Using imgdedup with an AI agent

`imgdedup` ships an official Claude Code plugin — a skill, a `/imgdedup:dedupe`
slash command, and a read-only `imgdedup-operator` subagent — so you can find
and clean up duplicates by asking Claude instead of hand-writing commands. It
always scans read-only first, always shows you the plan, and always asks
before it touches disk; permanent deletion needs its own separate, explicit
confirmation. See
[Using imgdedup with an AI agent](docs/guide/using-with-claude.md) for install
steps and a worked example.

(This is a different thing from the AI agent **crew that builds imgdedup**,
described below — that's contributor tooling; this is a product feature for
anyone running `imgdedup`.)
```

- [ ] **Step 2: Sanity-check placement**

Run: `grep -n "^## " README.md`
Expected: `## Using imgdedup with an AI agent` appears once, directly between
`## Detailed guide` and `## How this repository works`.

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs(readme): document the imgdedup Claude Code plugin"
```

---

### Task 9: Add the PRD-0001 amendment

**Files:**
- Modify: `docs/prd/PRD-0001-exact-image-deduplication.md`

**Interfaces:**
- Consumes: ADR-0016 (Task 6), the guide page (Task 7).
- Produces: nothing consumed downstream.

- [ ] **Step 1: Append a new amendment section at the end of the file**

Append after the "Amendment 2026-08-01" section (at the end of the file):

```markdown
## Amendment 2026-08-02 — AI-agent automation as a supported capability

The owner confirmed (session 9, TASK-0218) that AI-agent-driven usage is
expected to become an increasingly common way people operate `imgdedup`, so it
is recorded here as a supported product capability even though its design was
architect-owned rather than run as a full PRD slice — see
[ADR-0016](../adr/0016-ai-agent-automation-plugin.md) for the complete design
rationale.

`imgdedup` ships an official **Claude Code plugin** (`claude-plugin/` in this
repository): a skill defining a dry-run-first safety protocol, a
`/imgdedup:dedupe` slash command, and a read-only `imgdedup-operator` subagent
for large scans. The plugin is a pure wrapper over the CLI contract already
defined in this PRD — FR-9 (dry-run default), FR-10 (`--apply`), FR-11
(`--delete`), and FR-14 (`--json`) — and introduces **no new CLI behavior**:
every destructive action it takes is gated behind an explicit,
in-conversation human confirmation, and permanent deletion (`--delete`)
requires its own separate explicit confirmation beyond the one that
authorizes `--apply`. See
[Using imgdedup with an AI agent](../guide/using-with-claude.md) for the
user-facing walkthrough.
```

- [ ] **Step 2: Commit**

```bash
git add docs/prd/PRD-0001-exact-image-deduplication.md
git commit -m "docs(prd): record AI-agent automation as a supported capability"
```

---

### Task 10: Planning sync

**Files:**
- Modify: `docs/planning/backlog.md`
- Modify: `docs/planning/status.md`
- Modify: `docs/planning/batch-plan.md`

**Interfaces:** none — pure planning-doc bookkeeping, the last content task before validation.

- [ ] **Step 1: Flip TASK-0218 to done in `docs/planning/backlog.md`**

Change the TASK-0218 row's `Status` column from `todo` to
`done (this PR)`.

- [ ] **Step 2: Update `docs/planning/status.md`**

Update the "Last updated" date to 2026-08-02 (if not already), and add a line
under "In progress" (or move prior session-8 content to a "Done" framing if
that's how the file is currently structured — read the current file first,
since it was last touched by TASK-0217 and its exact shape may have shifted)
noting: session 9 (TASK-0218) delivered the `imgdedup` Claude Code plugin —
skill, `/imgdedup:dedupe` command, read-only `imgdedup-operator` subagent —
via ADR-0016, this PR.

- [ ] **Step 3: Update `docs/planning/batch-plan.md`'s session 9 row**

Change the session 9 row's `Status` column from `next week` to `done
2026-08-02 — PR #<fill in after PR is opened>` (fill in the actual PR number
once Task 11 opens the PR — if this plan is executed by an agent that doesn't
have that number yet, leave a literal placeholder `<PR#>` and flag it in the
final report so the orchestrator fills it in before merge).

- [ ] **Step 4: Commit**

```bash
git add docs/planning/backlog.md docs/planning/status.md docs/planning/batch-plan.md
git commit -m "docs(planning): mark TASK-0218 done; sync session 9 status"
```

---

### Task 11: Local validation and PR readiness

**Files:** none — this task runs existing validation tooling against everything Tasks 1–10 produced.

**Interfaces:** none.

- [ ] **Step 1: Run full local validation**

Run: `make validate`
Expected: markdownlint, spellcheck (`typos`), the structure check, and
linkcheck all pass against the new files (`claude-plugin/**/*.md`,
`docs/adr/0016-*.md`, `docs/guide/using-with-claude.md`, and the modified
`README.md`/PRD/planning files). Fix any findings (likely candidates: `typos`
flagging plugin-specific terms like "imgdedup-operator" — add to the project's
typos config if it's a false positive, following the same pattern used for
prior `typos` migration false-positives from TASK-0205).

- [ ] **Step 2: Confirm no product code was touched**

Run: `git diff --stat main...HEAD -- src/ tests/ Cargo.toml Cargo.lock`
Expected: empty output — confirms this unit stayed pure documentation/plugin
config, matching the Global Constraints.

- [ ] **Step 3: Review the full diff once, end to end**

Run: `git diff main...HEAD --stat` then `git diff main...HEAD`
Expected: every file listed traces back to a task above; nothing unexpected
(e.g. no accidental `.gitkeep` leftovers — confirm Tasks 2–4's Step 2 deletions
landed).

This is the last task in this plan. After it passes, hand off to this
project's normal SDLC: architect review of the branch diff vs `main`
(`docs/process/sdlc.md`), then open the PR (`feat:`-typed — this is a real,
if docs-only, product-facing capability, not a `docs:`/`chore:` unit) once
review has converged, filling in the `<PR#>` placeholder from Task 10 Step 3
once it's known.
