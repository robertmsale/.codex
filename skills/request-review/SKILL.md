---
name: request-review
description: Request code review via MCP `gitops.git_request_review_and_wait` only; do not run legacy shell review scripts unless the user explicitly asks. [skill-hash:7f4c1b8]
---

# Request Review

Use this skill when an agent needs code review for the current branch/PR.

## Required workflow (MCP-only)

Use MCP tool `git_request_review_and_wait`. Do not run
`~/.codex/skills/request-review/scripts/request-review` during normal agent
operation.

Expected behavior:

- Refuses protected integration branches.
- Handles commit/push/PR/review wait according to server policy/env.
- Returns findings summary or approval outcome.

## Tool usage

Preferred call:

- `git_request_review_and_wait(commit_message="<type>: <summary>", repo_path="<worktree path>")`

Optional fields only when needed by user/repo policy:

- `existing_commit_sha`
- `use_existing_commit`
- `create_pr_if_missing`
- `pr_title`
- `pr_body`

## Config source

- Review behavior is controlled by operator-managed env/config:
  - `~/.codex/mcp/gitops/.env`
  - `~/.codex/skills/request-review/.env` (legacy knob source still honored by
    gitops policy wiring)

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
- Do not modify review `.env` knobs. Agents are not allowed to change review
  settings.
- After starting review, wait patiently; do not cancel/interrupt unless an
  operator explicitly asks.

## Legacy script policy

- Legacy script path exists for operator-maintained compatibility only.
- Agents must not execute the script unless the user explicitly instructs script
  usage.
