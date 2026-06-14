# TASK-0212 — Implement unit-of-work cadence & planning gate (ADR-0011)

- **Phase:** 2 — Architecture & process
- **Role:** architect (CI + process docs), orchestrator (planning), owner (branch protection)
- **Status:** in-progress
- **Depends on:** ADR-0011 (accepted-on-merge)
- **Blocks:** —

> **For agentic workers:** this is both the backlog task and the implementation
> plan. Execute task-by-task; steps use checkbox (`- [ ]`) syntax. Everything
> rides **one `chore` branch** (`chore/process-unit-of-work-cadence`) into a
> single squash PR — this task dogfoods the very cadence it introduces.

**Goal:** Put [ADR-0011](../../adr/0011-unit-of-work-cadence-and-planning-gate.md)
into effect — restructure CI so a required `ci-gate` job fronts a path-filtered
Rust matrix, and update the process docs + agent definitions so one branch
carries a whole unit of work (design + code + tech/user docs + planning).

**Architecture:** One engineering change (the `ci.yml` `ci-gate` + `changes`
path-filter jobs) plus prose edits across the process docs, agent definitions,
and `CLAUDE.md`. No product code changes.

**Tech stack:** GitHub Actions YAML (`dorny/paths-filter`), Markdown.

---

## Migration ordering (read first — avoids a deadlock)

This PR **modifies `.github/workflows/ci.yml`**, which is inside the `code` path
filter, so the Rust 3-OS matrix **still runs on this PR** and satisfies the
*currently-required* `rust (…-latest)` checks. The owner switches the required
check to `ci-gate` **after** merge (Task 6). Future docs-only PRs then skip the
matrix. No chicken-and-egg: the old gate covers this PR; the new gate covers the
next one.

---

## Task 1 — CI: add `changes` path-filter + `ci-gate`, gate the Rust matrix

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Add the `changes` job** (place it just before the `rust` job, after `linkcheck`). It computes whether code paths changed:

```yaml
  changes:
    name: changes
    runs-on: ubuntu-latest
    permissions:
      contents: read
      pull-requests: read
    outputs:
      code: ${{ steps.filter.outputs.code }}
    steps:
      - uses: actions/checkout@v6
      - uses: dorny/paths-filter@v3
        id: filter
        with:
          filters: |
            code:
              - 'Cargo.toml'
              - 'Cargo.lock'
              - 'rust-toolchain.toml'
              - 'build.rs'
              - 'src/**'
              - 'tests/**'
              - '.github/workflows/ci.yml'
```

- [ ] **Step 2: Gate the `rust` matrix on `changes`.** Edit the `rust` job header to add `needs` + `if`:

```yaml
  rust:
    name: rust (${{ matrix.os }})
    needs: changes
    if: needs.changes.outputs.code == 'true'
    runs-on: ${{ matrix.os }}
```

(Leave the rest of the `rust` job — `strategy`, `permissions`, `steps` — unchanged.)

- [ ] **Step 3: Add the `ci-gate` aggregator job** (append as the last job). It always runs and fails only if the Rust matrix actually failed; a *skipped* matrix is a pass:

```yaml
  ci-gate:
    name: ci-gate
    needs: [changes, rust]
    if: always()
    runs-on: ubuntu-latest
    steps:
      - name: Evaluate required jobs
        run: |
          set -euo pipefail
          changes_result="${{ needs.changes.result }}"
          rust_result="${{ needs.rust.result }}"
          echo "changes: $changes_result | rust: $rust_result"
          if [[ "$changes_result" == "failure" || "$changes_result" == "cancelled" ]]; then
            echo "::error::path-filter job did not succeed"; exit 1
          fi
          if [[ "$rust_result" == "failure" || "$rust_result" == "cancelled" ]]; then
            echo "::error::Rust checks failed"; exit 1
          fi
          # success or skipped (docs/chore-only) both pass the gate
          echo "ci-gate passed"
```

- [ ] **Step 4: Lint the workflow locally.** Run:

```bash
npx -y @action-validator/cli .github/workflows/ci.yml 2>/dev/null \
  || python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/ci.yml')); print('yaml OK')"
```

Expected: YAML parses (`yaml OK`) / validator passes. (Full end-to-end behaviour — matrix runs on this code-touching PR, and *skips* on a future docs-only PR — is verified at PR time; see the Migration note.)

- [ ] **Step 5: Commit.**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: front Rust matrix with a path-filtered ci-gate required check"
```

---

## Task 2 — Process docs: `sdlc.md`

**Files:**
- Modify: `docs/process/sdlc.md`

- [ ] **Step 1:** Add a new **Step 0 — Classify the unit** at the head of "Steps in detail", before "Select work":

  > **0. Classify the unit of work.** Before branching, decide the class —
  > **feature**, **fix**, or **chore** — which sets the cadence (branch prefix,
  > expected lifetime, what rides along, CI weight, review kind). See the
  > classification table below and [ADR-0011](../adr/0011-unit-of-work-cadence-and-planning-gate.md).

  Insert the feature/fix/chore table (copy from ADR-0011 §2).

- [ ] **Step 2:** In the cycle, change "update status.md (in progress)" framing so the **planning/status update rides the unit's own branch/PR** (no separate doc PR). Add a sentence: "The status flip for a task lands in that task's own PR."

- [ ] **Step 3:** Revise the **review-timing** subsection: replace "review the *uncommitted* working-tree diff before the single commit" with "the architect reviews the **branch diff vs `main` before the PR is opened**; the author iterates on the branch (pushes are CI-free); the PR opens **ready** (not draft) and a green `ci-gate` + confirmation pass are the merge gate." Keep the document-vs-code review kinds.

- [ ] **Step 4:** Add a **"Hold the PR"** note: pushing to a topic branch triggers no CI; open the PR only once converged and reviewed.

- [ ] **Step 5:** Update the **Definition of Done** list: add "Technical docs (implementer) and user-facing docs (product-owner) updated with the behaviour they describe" and "Planning/status updated in this same PR."

- [ ] **Step 6: Commit.**

```bash
git add docs/process/sdlc.md
git commit -m "docs(process): adopt unit-of-work cadence, Step 0 classification & hold-the-PR review"
```

---

## Task 3 — Process docs: `branching.md` + `commits.md`

**Files:**
- Modify: `docs/process/branching.md`, `docs/process/commits.md`

- [ ] **Step 1 (`branching.md`):** Under "Topic branches", add a paragraph stating a branch = **one unit of work** that carries design + code + tech/user docs + planning into one squash PR; reference the feature/fix/chore classification (link ADR-0011). Add the **hold-the-PR** rule to the Lifecycle list (push freely; open PR only when converged + reviewed).

- [ ] **Step 2 (`commits.md`):** Add a short note: squash-merge is unchanged, so only the **PR title** must be a release-driving Conventional Commit; per-commit messages on the branch are for **review/history hygiene** (still Conventional, but they collapse into the squash). Link ADR-0011.

- [ ] **Step 3: Commit.**

```bash
git add docs/process/branching.md docs/process/commits.md
git commit -m "docs(process): branch = one unit of work; clarify squash vs per-commit hygiene"
```

---

## Task 4 — Crew + agent definitions: doc ownership & kaizen

**Files:**
- Modify: `docs/process/crew.md`, `.claude/agents/implementer.md`, `.claude/agents/product-owner.md`, `.claude/agents/architect.md`

- [ ] **Step 1 (`implementer.md` + `crew.md`):** Add to the implementer's responsibilities: "Maintain **technical documentation** (rustdoc on public items / `docs/` design notes) for the code you write — part of Definition of Done, reviewed as code."

- [ ] **Step 2 (`product-owner.md` + `crew.md`):** Add to the product-owner's responsibilities: "Author/maintain **user-facing documentation** (CLI/usage guide, relevant README sections) for any user-visible behaviour, riding the feature's PR."

- [ ] **Step 3 (kaizen — `crew.md`, and a line in each of `architect.md`, `product-owner.md`):** State that the **orchestrator, product-owner, and architect** may propose process improvements when they spot material potential, raised as a **chore unit** and, if lasting, captured in an ADR.

- [ ] **Step 4: Commit.**

```bash
git add docs/process/crew.md .claude/agents/implementer.md .claude/agents/product-owner.md .claude/agents/architect.md
git commit -m "docs(process): fold tech/user docs into roles; add kaizen provision"
```

---

## Task 5 — `CLAUDE.md` + ADR index

**Files:**
- Modify: `CLAUDE.md`, `docs/adr/README.md`

- [ ] **Step 1 (`CLAUDE.md`):** In §4 (working rules), adjust rule 6 ("One task = one branch = one focused PR") to reference the unit-of-work model and the up-front classification; add a brief pointer that planning/doc updates ride the unit's PR (no separate doc PR) and link ADR-0011. Add a one-line kaizen note in §7.

- [ ] **Step 2 (`docs/adr/README.md`):** Add the index row for **ADR-0011** (status `proposed`/`accepted`), matching the existing table format.

- [ ] **Step 3: Commit.**

```bash
git add CLAUDE.md docs/adr/README.md
git commit -m "docs: reference unit-of-work cadence in CLAUDE.md; index ADR-0011"
```

---

## Task 6 — Planning update + owner hand-off (rides this PR)

**Files:**
- Modify: `docs/planning/backlog.md`, `docs/planning/status.md`

- [ ] **Step 1 (`backlog.md`):** Add the TASK-0212 row to the Phase 2 table (Role: architect; Status: done — flipped at end of this PR; Parallel: no; Depends on: ADR-0011).

- [ ] **Step 2 (`status.md`):** Note ADR-0011 + TASK-0212 under Done; under **Blockers / waiting on owner**, add the branch-protection switch (below). Set this task's status to `done` in this file (its work completes in this PR).

- [ ] **Step 3 — flip ADR-0011 status:** Once the owner is ready to merge, change ADR-0011 `Status:` from `proposed` to `accepted` (same PR or on merge per house style).

- [ ] **Step 4: Commit.**

```bash
git add docs/planning/backlog.md docs/planning/status.md docs/adr/0011-unit-of-work-cadence-and-planning-gate.md
git commit -m "docs(planning): record ADR-0011 + TASK-0212; queue owner branch-protection switch"
```

- [ ] **Step 5 — Owner action (after merge, not a code change):** In GitHub branch-protection for `main`, **remove** required checks `rust (ubuntu-latest)`, `rust (macos-latest)`, `rust (windows-latest)` and **add** `ci-gate`. Leave `markdownlint`, `spellcheck`, `structure`, `pr-title`, `secret-scan` required.

---

## Definition of Done

- `ci.yml` has `changes` + `ci-gate`; Rust matrix gated on `needs.changes.outputs.code`.
- `make validate` (mdlint, spell) green locally before the PR opens.
- Process docs, agent definitions, and `CLAUDE.md` reflect the unit-of-work cadence.
- ADR-0011 indexed and (on merge) `accepted`; backlog + status updated in this PR.
- Owner branch-protection switch recorded as a waiting-on-owner item.
