---
name: start-turn
description: Read at the start of every turn. Establish the execution pipeline, lock in required skills, and do not start implementation out of order. [skill-hash:1b7e9a4]
---

# Start Turn

Read this at the start of each turn.

## Required Order

1. Confirm scope and deliverable for this turn in one sentence.
2. If git workflow is involved, read `$gh-version-control-workflow` before any git mutation.
3. Create a dedicated worktree and do the turn's implementation work there.
4. If commands are needed, capture the command `job_id`.
5. If command completion is not immediate, wait with `command_execution_wait(job_id)`.
6. Use `$command-parser` only for noisy output extraction.
7. If deletions are needed, use `$safe-delete`.
8. If worker orchestration is needed, use `$robdex-orchestrator`.

## Hard Rules

- Do not start implementation before pipeline selection is clear.
- Do not implement in the base repo when worktree workflow applies.
- Do not bypass required skill workflows.
- Do not invent fallback paths when a required skill/tool is available.
