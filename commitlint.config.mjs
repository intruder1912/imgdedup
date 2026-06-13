// Conventional Commits linting (see docs/process/commits.md).
// Used locally (`npx commitlint`) and available for CI commit linting.
// The CI PR-title gate (.github/workflows/ci.yml) enforces the same types,
// because squash-merge turns the PR title into the commit on `main`.
export default {
  extends: ["@commitlint/config-conventional"],
  rules: {
    "type-enum": [
      2,
      "always",
      [
        "feat",
        "fix",
        "docs",
        "test",
        "refactor",
        "perf",
        "build",
        "ci",
        "chore",
        "style",
        "revert",
      ],
    ],
    "subject-case": [0], // allow flexible subject casing
  },
};
