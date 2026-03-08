---
name: request-review
description: Request review via `scripts/request-review` (script-first, no MCP tool calls). The script auto-routes through `launch-job` and writes `review.log` in the worktree root. [skill-hash:1c7e2d4]
---

# Request Review

Use this skill when you need code review on the current worktree branch.

## Required Path

- Run: `~/.codex/skills/request-review/scripts/request-review "<commit message>"`
- Do not call MCP review tools.
- Do not run alternate legacy review commands.

## Behavior

- The script auto-routes through `launch-job` by default.
- Review output is written/read from `review.log` in the worktree root.
- Local vs remote mode is controlled by operator `.env` settings.

## Inputs

- Required: commit message text.
- Optional env toggles (operator-controlled):
  - `REQUEST_REVIEW_MODE=local|remote`
  - `REQUEST_REVIEW_DISABLE=0|1`
  - `REQUEST_REVIEW_USE_EXISTING_COMMIT=0|1`
  - `REQUEST_REVIEW_EXISTING_COMMIT_SHA=<sha-or-ref>`

## Guardrails

- Refuses protected integration branches.
- Do not modify review config unless the user explicitly instructs it.
- Do not launch duplicate review requests for the same branch/PR scope.
