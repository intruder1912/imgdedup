# ADR-0021: Issue-based planning for a public repository

- **Status:** accepted
- **Date:** 2026-11-14
- **Deciders:** human owner
- **Supersedes in part:** [ADR-0002](0002-in-repo-markdown-as-canonical-planning.md)
  (planning and tracking only; PRDs, ADRs and process documents stay in-repo
  Markdown)
- **Amends:** [ADR-0011](0011-unit-of-work-cadence-and-planning-gate.md) §6
  (planning updates)
- **Related:** TASK-0337, `CLAUDE.md` §2, `docs/process/sdlc.md`,
  `CONTRIBUTING.md`, `SECURITY.md`

## Context and problem statement

Since ADR-0002 the project has tracked its work in three Markdown files:
`docs/planning/backlog.md` (one row per task), `docs/planning/status.md` (a
session-by-session account of what was done) and `docs/planning/batch-plan.md`
(work batched against usage limits). ADR-0011 considered moving this to GitHub
Issues and rejected it, because it would split the source of truth and break
"resume from the repository alone".

The repository is about to become public (TASK-0337). That changes two things:

- **Outside people need a place to report bugs and ask for features.** On
  GitHub that place is the issue tracker. Once it is in use, the backlog file
  is a second task list beside it.
- **`status.md` is now about 2,600 lines of internal session narration.**
  TASK-0337 recorded it as an open question: it is the first thing a visitor
  finds under `docs/planning/`, and it is unusual for a public project.

On 2026-11-14 the owner proposed using GitHub Issues, "as is the standard for
public repositories", for tasks and, where possible, for status and planning.
This ADR records the decisions taken in the brainstorm that followed. Each
decision below says whether it is the owner's ruling or the orchestrator's
proposal that the owner accepted as a whole by approving this ADR.

## Decision drivers

- A public visitor should find a normal open-source project: issues, pull
  requests, design documents.
- One task list, not two.
- A new session must still be able to find where the work stands without chat
  history (the purpose of ADR-0002), using `git` and `gh` only (`CLAUDE.md`
  §4.7).
- References to `TASK-0xxx` in ADRs, code comments and commit messages must
  stay resolvable.
- The owner's usage figures are private working data.

## Considered options

1. **GitHub Issues with labels and templates** (chosen).
2. **Issues plus a GitHub Project board.**
3. **Issues plus GitHub Discussions** for questions and ideas.
4. **Issues for tasks, plus a short in-repo status file.**
5. **Keep everything in-repo** (ADR-0002 unchanged).

## Decision outcome

**Chosen option:** 1, GitHub Issues with labels and templates.

### 1. Tasks are issues

Owner's ruling. Every unit of work is a GitHub issue. The rows of
`backlog.md` that are open when this ADR is implemented become issues; closed
rows are not copied.

New work is referred to by issue number (`#123`). No new `TASK-` identifiers
are assigned.

### 2. The old narration leaves `main`

Owner's ruling: `docs/planning/status.md` is removed from `main`. It stays in
the git history, which is not rewritten (TASK-0337).

Orchestrator's proposals:

- `docs/planning/batch-plan.md` is removed with it. It is the same kind of
  internal working record.
- `docs/planning/backlog.md` and `docs/planning/tasks/` are **kept and
  frozen**. They are the index that makes `TASK-0xxx` references resolvable.
  A note at the top of `backlog.md` says it is closed, when, and where each
  open row went.
- `docs/planning/roadmap.md` stays, cut down to the direction of the project
  and a pointer to the issues.

Removing the file changes what a visitor sees first. It does not make the
content private: anything in `status.md` remains readable in the history.

### 3. Where the work stands

Orchestrator's proposal. The state of the project is read from GitHub:

- open issues are the work to do;
- the label `in progress` marks the unit being worked on;
- open pull requests are the work under review;
- the latest release is what has shipped.

A session starts with `gh issue list` and `gh pr list` where it used to read
`status.md`. `CLAUDE.md` §2 is rewritten to say so: PRDs, ADRs and process
documents remain in the repository and remain authoritative for *what* and
*how*; the issue tracker is authoritative for *what is open*.

### 4. What happened in a unit is recorded on its pull request

Orchestrator's proposal. The pull request description carries what
`status.md` carried per unit: what was verified and how, what the review
found, what was deliberately not done, what was not run. A decision that
outlives the unit goes in an ADR or PRD, as before. The pull request closes
its issue (`Closes #123`).

This replaces ADR-0011 §6. There is no planning file to update inside the
unit's pull request any more, and no planning-only pull requests.

### 5. Labels

Orchestrator's proposal. A small fixed set, no project board:

| Label | Meaning |
|---|---|
| `feature`, `fix`, `chore` | the unit's classification (ADR-0011 §2) |
| `needs-triage` | newly filed, not yet looked at by the owner |
| `accepted` | the owner has accepted it as work to be done |
| `needs-prd`, `needs-adr` | requirements or design must be written first |
| `in progress` | being worked on now |
| `blocked` | waiting on something named in the issue |

GitHub's default labels that this set replaces are deleted; the labels
Dependabot applies (`dependencies`, `rust`, `github-actions`,
`release-tooling`) stay. Milestones are
not used until there is a release to group work under.

### 6. Outside issues and pull requests

Owner's rulings:

- **Issues are welcome** from anyone. The owner triages them.
- **A pull request needs an accepted issue first.** A pull request without
  one is pointed at this rule and closed or parked.
- **Exception for trivial fixes:** typos, broken links and one-line
  documentation corrections may be sent as a pull request directly.

Orchestrator's proposals:

- Issue templates for a bug report and a feature request; both apply
  `needs-triage`. Blank issues stay allowed.
- The rule is stated in `CONTRIBUTING.md` and in the pull request template.
  GitHub does not enforce it.
- An accepted outside pull request goes through the same cycle as any other:
  CI, architect review, the owner merges.
- Security reports do not go in issues. GitHub's private vulnerability
  reporting is enabled when the repository becomes public, and `SECURITY.md`
  points to it.

### 7. Usage estimates and actuals leave the repository

Orchestrator's proposal. The owner's global rule (plan in batches, estimate
usage, compare with actuals) continues. Its figures are kept in the
assistant's local memory directory on the owner's machine, not in the
repository and not in issues.

### Consequences

- Positive: one task list; the tracker outside contributors expect; no
  planning file to keep in step in every pull request; the repository's
  planning directory shrinks to a roadmap and a frozen index.
- Negative: the state of the work is no longer in a clone. It needs GitHub
  and `gh`. ADR-0011 rejected issues for this reason; the owner accepts it
  for a public repository.
- Negative: the per-session narrative ends. What a pull request description
  does not record is not recorded.
- Negative: two identifier schemes exist side by side, `TASK-0xxx` for work
  up to this ADR and `#123` after it.
- To change when this is implemented: `CLAUDE.md` §2 and §6;
  `docs/process/sdlc.md`, `crew.md` and `branching.md` where they name the
  planning files; all four agent definitions in `.claude/agents/`; the
  review-log README and template in `docs/planning/reviews/`; the
  pull request template; the required-files list in `ci.yml`; the planning
  links in `README.md`, `docs/guide/README.md`, `docs/prd/README.md` and
  `CONTRIBUTING.md`; a status line on ADR-0002 and notes on ADR-0011 §6 and ADR-0013.
- Older ADRs, PRD-0001 and `CHANGELOG.md` link to `status.md` in places.
  Those links are changed to plain text naming the file and "removed from
  `main` by ADR-0021", so the link check passes and the history stays
  findable.
- At the flip, not before: enable private vulnerability reporting, and set
  fork pull request approval to "all external contributors" through the API
  (GitHub refuses both on a private repository).

## Pros and cons of the options

### Option 1 — Issues with labels and templates (chosen)

- Good: the least machinery that gives outside people a normal tracker and
  the owner one list.
- Bad: state lives outside the clone.

### Option 2 — Issues plus a Project board

- Good: a visual overview.
- Bad: a second view to keep in step, for one owner and a handful of open
  items. Can be added later without undoing anything.

### Option 3 — Issues plus Discussions

- Good: separates questions from work.
- Bad: a second inbox to triage. Can be added later if questions crowd the
  issues.

### Option 4 — Issues plus a short in-repo status file

- Good: a clone still says where things stand.
- Bad: two places describe the same state, and the file drifts.

### Option 5 — Keep everything in-repo

- Good: ADR-0002's single clone holds everything.
- Bad: leaves outside reports without a home, or creates two task lists the
  moment issues are enabled; keeps the narration in front of visitors.

## Links

- [ADR-0002](0002-in-repo-markdown-as-canonical-planning.md) — the decision
  this narrows.
- [ADR-0011](0011-unit-of-work-cadence-and-planning-gate.md) — Option 3 there
  is the earlier rejection of issues; §6 is replaced.
- [ADR-0003](0003-main-branch-protection-and-merge-model.md) — the owner
  merges; unchanged.
