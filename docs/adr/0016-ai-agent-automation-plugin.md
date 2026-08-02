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
