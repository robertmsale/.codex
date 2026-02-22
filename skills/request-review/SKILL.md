---
name: request-review
description: Request code review for the current branch/PR using MCP-first flow (`git_request_review_and_wait`), with script fallback for legacy contexts. [skill-hash:c3d7f21]
---

# Request Review

Use this skill when an agent needs to request code review for the current branch/PR and get the final result.

## Preferred workflow (MCP-first)

If MCP server `gitops` is available, use MCP tool `git_request_review_and_wait` instead of the shell script.

Behavior in MCP flow:
- Refuses to run on protected integration branches.
- Commits and pushes as part of the review flow (unless explicitly using an existing commit).
- Creates a PR if one does not exist yet.
- Posts trigger comment (default `@codex review`).
- Waits until final review outcome is detected.
- Returns inline findings or approval summary.

## Legacy fallback (script)

Run only when MCP is unavailable:
- `~/.codex/skills/request-review/scripts/request-review <commit-message>`

Examples:
- `~/.codex/skills/request-review/scripts/request-review "fix: address review findings"`
- `~/.codex/skills/request-review/scripts/request-review "chore: review checkpoint"`

## Config source
- MCP flow reads:
  - `~/.codex/mcp/gitops/.env`
  - `~/.codex/skills/request-review/.env` (existing review knobs)
- Legacy script reads:
  - `~/.codex/skills/request-review/.env`

## Env knobs (authoritative)
- `REQUEST_REVIEW_MODE=local|remote`
- `REQUEST_REVIEW_BOT_LOGIN=chatgpt-codex-connector[bot]`
- `REQUEST_REVIEW_TRIGGER_COMMENT=@codex review`
- `REQUEST_REVIEW_POLL_INTERVAL_SECONDS=20`
- `REQUEST_REVIEW_LOCAL_PROFILE=local-review`
- `REQUEST_REVIEW_USE_EXISTING_COMMIT=0|1`
- `REQUEST_REVIEW_EXISTING_COMMIT_SHA=<sha-or-ref>`
- `REQUEST_REVIEW_DISABLE=0|1`
- `REQUEST_REVIEW_INTEGRATION_BRANCHES="main master staging prod production"`

## Critical discipline
- Only one review request per project/PR scope at a time.
- Do not launch duplicate review requests.
- Do not modify review `.env` knobs. Agents are not allowed to change review settings.
- After starting review, wait patiently; do not cancel/interrupt unless an operator explicitly asks.
