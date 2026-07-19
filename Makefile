# imgdedup — local validation entry points.
#
# Phase 0 validates documentation only.
# Phase 3 adds Rust build/lint/test/fmt targets and wires them into `validate`.
#
# Requires Node.js (for npx) and cargo (for Rust checks). Tools are fetched
# on demand via `npx -y`; cargo is assumed to be on PATH (pinned via
# rust-toolchain.toml).

.DEFAULT_GOAL := help
.PHONY: help validate mdlint spell links build test fmt lint coverage deny

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) \
		| sort \
		| awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2}'

# ── Composite gate ────────────────────────────────────────────────────────────

validate: mdlint spell fmt lint test ## Run all local validation (run before every push)
	@echo "✓ local validation passed"

# ── Documentation checks ──────────────────────────────────────────────────────

mdlint: ## Lint Markdown
	npx -y markdownlint-cli2@0.23.1 "**/*.md"

spell: ## Spell-check documentation
	npx -y cspell@10.0.1 --no-progress "**/*.md"

links: ## Check links in Markdown (best-effort; CI runs the authoritative check)
	npx -y markdown-link-check@3.14.2 --quiet --config .markdown-link-check.json $$(git ls-files '*.md')

# ── Rust checks ───────────────────────────────────────────────────────────────

build: ## Build the binary (debug)
	cargo build

test: ## Run all Rust tests
	cargo test

deny: ## Audit dependencies (advisories/licenses/bans/sources; needs cargo-deny installed)
	cargo deny check

fmt: ## Check Rust formatting (use `cargo fmt` to auto-fix)
	cargo fmt --check

lint: ## Run clippy with warnings-as-errors
	cargo clippy --all-targets -- -D warnings

# ── Coverage (not in the blocking validate path) ──────────────────────────────
# NOTE: enforced coverage floor is deferred; see TASK-0301 Deviation note in
# docs/planning/tasks/TASK-0301-crate-scaffold-cli-surface-toolchain.md

coverage: ## Measure test coverage with cargo-llvm-cov (requires: cargo install cargo-llvm-cov)
	cargo llvm-cov --all-features
