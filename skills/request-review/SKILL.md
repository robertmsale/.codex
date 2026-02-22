---
name: request-review
description: Request code review for the current branch/PR. Default flow commits first; optional opt-in flow can target an existing commit without commit/push. [skill-hash:6a0d8f3]
---

# Request Review

Use this skill when an agent needs to request code review for the current branch/PR and get the result in stdout.

Run:
- `~/.codex/skills/request-review/scripts/request-review <commit-message>`

Examples:
- `~/.codex/skills/request-review/scripts/request-review "fix: address review findings"`
- `~/.codex/skills/request-review/scripts/request-review "chore: review checkpoint"`
- `REQUEST_REVIEW_DISABLE=1 ~/.codex/skills/request-review/scripts/request-review "chore: bypass review"`

## Preferred workflow
- If explicitly asked to use this skill, do not manually commit/push first.
- Run this skill directly and let it handle commit/push in the default path.
- Use the opt-in existing-commit mode only as recovery when a commit was already created/pushed by mistake.
- Wait for review, do not finish your turn until the agent is done reviewing your code.
- Take remediary action as needed and request another review.
- If you pass, this is a good stopping point.

## Behavior
- Default path: commits first using the provided commit message (`git add -A` then `git commit -m ...`).
- Default path: uses the newly created `HEAD` commit as the target review SHA.
- Opt-in recovery path: set `REQUEST_REVIEW_USE_EXISTING_COMMIT=1` to skip `git add`/`git commit` and review an existing commit (defaults to `HEAD`, or `REQUEST_REVIEW_EXISTING_COMMIT_SHA`).
- Disable path: set `REQUEST_REVIEW_DISABLE=1` to skip both remote and local review execution. In this mode the script prints `all clear!` and exits success.
- Protected-branch guard: refuses to run on integration branches (default: `main master staging prod production`; configurable via `REQUEST_REVIEW_INTEGRATION_BRANCHES`).
- Runs one review request at a time per scoped lock (project + PR when available, otherwise project + branch).
- Prints final review text to stdout.

## Mode switch (from `.env`)
- `REQUEST_REVIEW_MODE=remote`
  - Default path pushes branch (`git push -u origin HEAD`).
  - With `REQUEST_REVIEW_USE_EXISTING_COMMIT=1`, skips push and hooks review onto the existing commit SHA.
  - Finds the PR for current branch.
  - Posts `@codex review` to trigger cloud review explicitly.
  - Polls until one condition is met for the target commit SHA:
    1. New inline PR review comment(s) from `chatgpt-codex-connector[bot]` on that commit.
    2. New `+1` reaction from `chatgpt-codex-connector[bot]` on the PR issue after the commit timestamp.
  - If inline comments exist, returns detailed findings with links.
  - If only thumbs-up exists, returns `👍` summary.
- `REQUEST_REVIEW_MODE=local`
  - Runs `codex exec -s read-only --json review --commit <sha> --title <commit-message>` in the current repo.
  - Uses `REQUEST_REVIEW_LOCAL_PROFILE` from `.env` and reads model/reasoning from that profile in `~/.codex/config.toml`.
  - If the profile is missing in `config.toml`, exits with an error.
  - Writes output to `review.log` and stderr to `review.err.log`.

## Config source
- `~/.codex/skills/request-review/.env`

Useful variables:
- `REQUEST_REVIEW_MODE=local|remote`
- `REQUEST_REVIEW_BOT_LOGIN=chatgpt-codex-connector[bot]`
- `REQUEST_REVIEW_TRIGGER_COMMENT=@codex review`
- `REQUEST_REVIEW_POLL_INTERVAL_SECONDS=20`
- `REQUEST_REVIEW_LOCAL_PROFILE=local-review`
- `REQUEST_REVIEW_USE_EXISTING_COMMIT=0|1` (default `0`)
- `REQUEST_REVIEW_EXISTING_COMMIT_SHA=<sha-or-ref>` (default `HEAD` when existing-commit mode is enabled)
- `REQUEST_REVIEW_DISABLE=0|1` (default `0`; when `1`, bypasses review execution and returns `all clear!`)
- `REQUEST_REVIEW_INTEGRATION_BRANCHES="main master staging prod production"` (space-separated protected branches that fail fast)

## Critical discipline
- Only run one review request at a time for the same project/PR scope.
- Do not launch concurrent review requests.
- CRITICAL: Do not modify `~/.codex/skills/request-review/.env` and do not switch `REQUEST_REVIEW_MODE` (`local`/`remote`). Agents are not allowed to change review settings at all.
- CRITICAL: After starting `request-review`, wait patiently and do not cancel/interrupt the review command under any circumstances.
