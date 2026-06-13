# imgdedup — local validation entry points.
#
# Phase 0 validates documentation (the only artifacts that exist yet).
# Language-specific targets (build, test, fmt, lint) are added in Phase 2
# once the technology stack is chosen (see docs/adr/).
#
# Requires Node.js (for npx). Tools are fetched on demand via `npx -y`.

.DEFAULT_GOAL := help
.PHONY: help validate mdlint spell links

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) \
		| sort \
		| awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2}'

validate: mdlint spell ## Run all local validation (run before every push)
	@echo "✓ local validation passed"

mdlint: ## Lint Markdown
	npx -y markdownlint-cli2 "**/*.md"

spell: ## Spell-check documentation
	npx -y cspell --no-progress "**/*.md"

links: ## Check links in Markdown (best-effort; CI runs the authoritative check)
	npx -y markdown-link-check --quiet --config .markdown-link-check.json $$(git ls-files '*.md')
