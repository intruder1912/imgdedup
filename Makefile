# imgdedup — local validation entry points.
#
# Phase 0 validates documentation only.
# Phase 3 adds Rust build/lint/test/fmt targets and wires them into `validate`.
#
# Requires Node.js (for npx) and cargo (for Rust checks). Tools are fetched
# on demand via `npx -y`; cargo is assumed to be on PATH (pinned via
# rust-toolchain.toml).

.DEFAULT_GOAL := help
.PHONY: help validate mdlint spell links shellcheck actionlint build test fmt lint coverage deny release-dry-run release-preflight

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) \
		| sort \
		| awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2}'

# ── Composite gate ────────────────────────────────────────────────────────────

validate: mdlint spell links shellcheck actionlint fmt lint test ## Run all local validation (run before every push)
	@echo "✓ local validation passed"

# ── Documentation checks ──────────────────────────────────────────────────────

mdlint: ## Lint Markdown
	npx -y markdownlint-cli2@0.23.1 "**/*.md"

spell: ## Spell-check documentation (needs: brew install typos-cli / cargo install typos-cli)
	typos $$(git ls-files '*.md')

links: ## Check links in Markdown (needs: brew install lychee / cargo install lychee); matches CI's lychee.toml gate
	lychee --config lychee.toml --no-progress './**/*.md'

# ── Tooling checks ────────────────────────────────────────────────────────────

shellcheck: ## Lint shell scripts (needs: brew install shellcheck / apt install shellcheck)
	shellcheck $$(git ls-files '*.sh')

actionlint: ## Lint GitHub Actions workflows, incl. embedded shell via shellcheck (needs: brew install actionlint / go install github.com/rhysd/actionlint/cmd/actionlint)
	actionlint

# ── Rust checks ───────────────────────────────────────────────────────────────

build: ## Build the binary (debug)
	cargo build

test: ## Run all Rust tests
	cargo test

deny: ## Audit dependencies (advisories/licenses/bans/sources; needs cargo-deny installed)
	cargo deny check

check-linux: ## Type-check the Linux target from any host (catches cfg-gated code the host never compiles)
	rustup target add x86_64-unknown-linux-gnu
	cargo check --all-targets --target x86_64-unknown-linux-gnu

scale: ## Run the NFR-4 scale fixtures (slow: builds 100k files and ~700 MB; not part of validate)
	cargo test --release --test scale -- --ignored --nocapture --test-threads=1

fmt: ## Check Rust formatting (use `cargo fmt` to auto-fix)
	cargo fmt --check

lint: ## Run clippy with warnings-as-errors
	cargo clippy --all-targets -- -D warnings

# ── Release (see docs/process/release.md, ADR-0015) ───────────────────────────

release-dry-run: ## Preview the next release without publishing (needs gh auth + npm)
	npm ci --ignore-scripts
	GITHUB_TOKEN=$$(gh auth token) npx --no-install semantic-release --dry-run --no-ci

release-preflight: ## Rehearse the assets job's build/package steps locally (needs podman; see docs/process/release.md)
	scripts/release-preflight.sh

# ── Coverage (not in the blocking validate path) ──────────────────────────────
# NOTE: enforced coverage floor is deferred; see TASK-0301 Deviation note in
# docs/planning/tasks/TASK-0301-crate-scaffold-cli-surface-toolchain.md

coverage: ## Measure test coverage with cargo-llvm-cov (requires: cargo install cargo-llvm-cov)
	cargo llvm-cov --all-features
