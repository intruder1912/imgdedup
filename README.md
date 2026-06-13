# imgdedup

A fast, cross-platform command-line tool for detecting and removing duplicate
and near-duplicate images.

> **Status: Phase 0 — Foundation.** The product is not yet implemented. This
> repository currently contains the project's governance, process, and planning
> foundation. The image-deduplication CLI is built in later phases through
> documented PRD → ADR → plan → implement → review cycles. See
> [`docs/planning/status.md`](docs/planning/status.md) for the live status and
> [`docs/planning/roadmap.md`](docs/planning/roadmap.md) for the path to v1.0.

## What this project is

`imgdedup` will scan directories for images, identify exact and near-duplicate
files, and help the user safely remove or organize them. The exact feature set,
detection strategy, and supported formats are being defined in the product
requirements documents — see [`docs/prd/`](docs/prd/).

The **technology stack is intentionally undecided**. It will be chosen through a
fully-open comparison in an Architecture Decision Record before implementation
begins — see [`docs/adr/`](docs/adr/).

## How this repository works

This project is built and maintained by a crew of AI agents and a human owner.
Everything needed to pick up the work lives **in the repository itself** — not in
any chat session — so any contributor (human or agent) can resume from the repo
content alone.

| If you want to… | Read |
|---|---|
| Understand the AI agent setup | [`CLAUDE.md`](CLAUDE.md) and [`docs/process/crew.md`](docs/process/crew.md) |
| Contribute (humans or agents) | [`CONTRIBUTING.md`](CONTRIBUTING.md) |
| Follow the development cycle | [`docs/process/sdlc.md`](docs/process/sdlc.md) |
| See what's planned / in progress | [`docs/planning/`](docs/planning/) |
| Read product requirements | [`docs/prd/`](docs/prd/) |
| Read architecture decisions | [`docs/adr/`](docs/adr/) |
| Report a security issue | [`SECURITY.md`](SECURITY.md) |

## Conventions at a glance

- **Conventional Commits** for all commit/PR titles.
- **Semantic Versioning**, with releases automated from commit history.
- **Trunk-based**: short-lived `feature/`, `fix/`, `chore/`, `docs/` branches →
  squash-merge into protected `main` via reviewed PRs.
- **Test-Driven Development** is mandatory for product code, with a security gate
  before every merge.

## License

No license is published yet. The repository is **private**; all rights reserved.
A licensing decision will be recorded in an ADR before any public release.
