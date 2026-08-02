# Design: AI automation support (TASK-0218)

- **Status:** approved (brainstormed 2026-08-02, session 9)
- **Backlog:** [TASK-0218](../../planning/backlog.md) — "AI automation support: in-repo
  Claude usage skill + slash command + `imgdedup-operator` agent (dry-run-first,
  `--json`-driven, no unattended `--delete`)". Depends on TASK-0308 (JSON output,
  done).
- **Next step:** this spec is the input to an ADR (architect-owned,
  proposed number **ADR-0016**) plus implementation. No product code (`src/`)
  changes are in scope.

## 1. Scope & audience

**Audience: end users of `imgdedup`**, not imgdedup contributors. This ships as
a distributable **Claude Code plugin** so anyone who has both `imgdedup`
installed and Claude Code can drive the tool safely through natural-language
requests or a slash command, without hand-writing CLI invocations or JSON
parsing themselves.

This is separate in purpose from the existing `.claude/agents/` in this repo
(`product-owner`, `architect`, `implementer`, `tester`), which are the crew that
*builds* imgdedup, not something end users install.

**In scope:**
- One Claude Code **plugin** (`claude-plugin/`) shipped in this repo, containing:
  - a **skill** (`using-imgdedup`) defining the safe operating protocol and the
    `--json` schema reference,
  - a **slash command** (`/imgdedup:dedupe`) that triggers the skill inline in
    the user's own conversation,
  - a **subagent** (`imgdedup-operator`) for read-only, isolated reconnaissance
    on large/multi-path scans.
- User-facing documentation of this as a real product capability (§6).

**Non-goals:**
- No new `imgdedup` CLI flags or behavior changes — this wraps the existing
  contract (PRD-0001, TASK-0308's JSON schema) as-is.
- No auto-install of the `imgdedup` binary; the skill detects it's missing and
  points at install docs.
- No unattended `--apply` or `--delete` under any circumstance — every
  destructive action requires an explicit, in-the-moment human confirmation in
  the main conversation (§4).
- No independent versioning scheme for the plugin (§3).

## 2. Artifact type & ownership

Classified as an **architect-owned design (ADR)**, not a PRD-0001 slice — it
doesn't change what the product *does*, only how an AI agent may safely operate
it, so it doesn't need a full product-owner brainstorm/grill cycle to design.

**However** (owner's explicit direction, 2026-08-02): AI-agent automation is
expected to become an increasingly important way people use imgdedup, so it
must be **documented with full product-feature weight** even though the
product-owner didn't design it — see §6. This is a deliberate split: architect
owns the design and the ADR; documentation treats it as a first-class,
discoverable capability of the product, not internal tooling.

## 3. Repository layout & packaging

New top-level directory, kept separate from this repo's own contributor
`.claude/`:

```text
claude-plugin/
  .claude-plugin/
    plugin.json                    # name: "imgdedup", description, version
  skills/
    using-imgdedup/
      SKILL.md                     # safety protocol + CLI/JSON reference
  commands/
    dedupe.md                      # /imgdedup:dedupe — thin trigger, loads the skill inline
  agents/
    imgdedup-operator.md           # read-only recon subagent
```

Users add this repo as a plugin source (marketplace entry or local path) and
`/plugin install imgdedup`, giving them `/imgdedup:dedupe`, the `using-imgdedup`
skill, and the `imgdedup-operator` agent in any project.

**Versioning:** `plugin.json`'s `version` tracks the `imgdedup` crate version
(bumped in the same release commits/PRs going forward) rather than being
independently semver'd — one version number for the whole product.

## 4. The safety protocol (skill = single source of truth)

`using-imgdedup/SKILL.md` defines one linear protocol. Both `/imgdedup:dedupe`
and any freeform request ("dedupe my Pictures folder") follow it, **always
inline in the main conversation** — the human who must approve destructive
actions is always the one present when they happen:

1. **Preflight:** confirm `imgdedup` is on `PATH` (`imgdedup --version`); if
   missing, stop and point to install docs — never attempt to install it.
2. **Dry run, always first:** run `imgdedup <path> --json` (no action flag).
   Parse the JSON (groups, keeper, `warnings`, `root_errors` — TASK-0308's
   schema) and present a human-readable summary: group count, files,
   reclaimable space, and any `warnings`/`root_errors` surfaced plainly, never
   swallowed.
3. **Gate 1 — apply:** if duplicate groups were found, ask the user to
   explicitly approve before running `imgdedup <path> --apply --json` (trash,
   recoverable). No approval → stop; nothing on disk changes.
4. **Gate 2 — delete:** `--delete` is only ever run if the user, **in that same
   turn**, explicitly asks for *permanent* deletion in words (e.g. "delete
   permanently", "don't use trash") — a reused earlier "yes" to `--apply` does
   not carry over. This is a **prompt-level gate, not a technical one** — it
   protects against *accidental/unattended* deletion, not against a user who
   deliberately asks for it. Worth stating explicitly in the ADR's risk section
   so it isn't overclaimed as a hard technical control.
5. **Report back:** after any action, show exactly what changed (moved/deleted,
   space reclaimed) using the CLI's own report output — never a paraphrase that
   could drift from what actually happened.

### The `imgdedup-operator` subagent's boundary

The subagent is given the same skill as its operating instructions but **stops
after step 2**: it only ever hands back the parsed dry-run summary/plan to the
parent conversation. Its own agent definition grants it no role in the
apply/delete decision — those steps only ever happen back in the main thread,
per the protocol above. Used when a user wants to offload scanning
large/multiple directory trees without flooding the main conversation's context
with a large JSON payload.

**Known limitation (to document, not hide):** this boundary is enforced by the
subagent's tool access and instructions, not by an OS-level sandbox. The
mitigations are: (a) the subagent's instructions never include `--apply`/
`--delete` invocation as a valid action, (b) even if bypassed, `--apply`
without `--delete` is recoverable via trash (existing NFR-1 safety invariant),
and (c) nothing the subagent does ever reaches the user's disk without passing
back through the main-thread confirmation gates, since the subagent has no
path to present its own approval prompt as if it were the protocol's gates.

## 5. Verification approach

No Rust/product code changes here, so crate-level TDD doesn't apply — but this
still needs real functional verification, not just reading the plugin files:

- **Install and drive it for real** against the golden-set fixtures
  (TASK-0313): run `/imgdedup:dedupe` through dry-run → approve `--apply` →
  confirm the trash move happened and matches the expected manifest.
- **Adversarially test the boundary directly:** invoke the `imgdedup-operator`
  subagent and explicitly ask it to apply/delete; confirm it refuses and only
  ever returns a plan. This is the one property the whole design's safety claim
  rests on, so it gets a direct negative test, not just a read of the prompt.
- **Warnings/root-errors path:** run a dry run against a directory containing an
  unreadable file and confirm the skill surfaces `warnings`/`root_errors`
  rather than dropping them.

## 6. Documentation & planning updates (same PR)

- **ADR-0016** (`docs/adr/`) — this design, written by the architect, using
  `docs/adr/TEMPLATE.md`.
- **`docs/guide/using-with-claude.md`** — new user-facing guide page
  (product-owner-owned per ADR-0012): install steps, what `/imgdedup:dedupe`
  does, the two confirmation gates, a worked example.
- **`README.md`** — short section pointing at the new guide page, alongside how
  `--json`/`--apply` are already surfaced there.
- **PRD-0001** — a short, dated amendment paragraph (same pattern as the
  2026-07-19 progress-indication amendment) recording AI-agent automation as a
  supported product capability and pointing at ADR-0016, rather than
  re-deriving the design in the PRD. This is what satisfies the owner's
  direction in §2 without a full PRD-slice cycle.
- **`docs/planning/backlog.md` / `status.md`** — TASK-0218 → done; resume-point
  sync.

## 7. Open items for the implementation plan

- Exact `plugin.json` fields (name/description/keywords) — small, decide during
  planning.
- Exact wording of the two confirmation gates in `SKILL.md` (trigger phrases
  for permanent delete) — needs to be unambiguous; draft during planning, not
  finalized here.
- Whether `/imgdedup:dedupe` takes a path argument or always prompts for one —
  decide during planning as a small UX detail, not architecturally significant.
