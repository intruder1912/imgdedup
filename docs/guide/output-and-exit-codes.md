# Output and exit codes

[← Guide index](README.md)

This page covers how `imgdedup` reports results and how it signals outcomes
through its exit code.

> **In development.** The human-readable report and JSON output are being
> implemented; the `--json` and `--exit-code` flags are already part of the CLI
> surface. The contract below is the intended design (PRD-0001,
> [ADR-0008](../adr/0008-core-deduplication-algorithm-and-semantics.md)). Real
> sample output will be added here as the report stage lands.

## Human-readable report (default)

By default `imgdedup` prints a human-readable report: each duplicate group with
its keeper and the files that would be (or were) removed, plus any files that were
skipped with the reason.

## `--json`: machine-readable output

For scripting and tooling, `--json` emits the same information as structured JSON
instead of the human-readable report:

```sh
imgdedup --json ./photos
```

## `--exit-code`: signal "duplicates found"

Normally a successful run exits `0` whether or not duplicates were found. Pass
`--exit-code` to make a *clean run that found duplicates* exit `3` instead — handy
in CI or scripts that want to fail when duplicates exist:

```sh
imgdedup --exit-code ./photos
```

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Success. With `--exit-code`, also means no duplicates were found. |
| `1` | Run-level error — e.g. an input path could not be scanned. |
| `2` | Usage error — invalid arguments. |
| `3` | Duplicates were found. Only emitted when `--exit-code` is passed. |

## Next

- Back to the [guide index](README.md).
