---
name: request-review
description: Request review via `scripts/request-review` with a commit message. Review output is written to `review.log` in the worktree root. Skip review for non-working-code changes such as docs, policy text, or comment-only edits. [skill-hash:4b7f0d1]
---

# Request Review

Use this skill when you need code review on the current worktree branch.

## Required Path

- Run: `~/.codex/skills/request-review/scripts/request-review "<commit message>"`
- Do not call MCP review tools.
- Do not run alternate legacy review commands.

## Behavior

- Review output is written/read from `review.log` in the worktree root.
- Review mode and review disable are operator-controlled.
- Non-working-code changes such as docs, policy text, or comment-only edits do not require request-review.

## Input

- Required: commit message text.

## Guardrails

- Refuses protected integration branches.
- Do not launch duplicate review requests for the same branch/PR scope.
- For changes that affect working code or runtime behavior, keep using request-review before publish/merge.
