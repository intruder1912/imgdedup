---
allowed-tools: Bash(imgdedup *)
description: Safely scan (and, on your confirmation, clean up) duplicate images with imgdedup
disable-model-invocation: false
---

Use the using-imgdedup skill to scan the path below for duplicate images,
following its protocol exactly: preflight, dry run, then Gate 1 (confirm
before `--apply`) and Gate 2 (a separate explicit confirmation before
`--delete`). Never skip a gate or infer approval from earlier in the
conversation.

Path: $ARGUMENTS

If no path was given above, ask the user which directory to scan before doing
anything else.
