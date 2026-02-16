---
name: request-review
description: Commit changes, then run either remote GitHub Codex review polling or local codex exec review, and send results back to a specified tmux pane.
---

# Request Review

Use this skill when an agent needs to request code review and route the result back into a specific tmux pane.

Run:
- `~/.codex/skills/request-review/scripts/request-review <tmux-pane-target> <commit-message>`

Examples:
- `~/.codex/skills/request-review/scripts/request-review "coolproject-78-dialpad:1.1" "fix: address review findings"`
- `~/.codex/skills/request-review/scripts/request-review "%9" "chore: review checkpoint"`

Use stable pane identifiers. Prefer `%<pane-id>` or `<session>:<window-index>.<pane-index>`.
Avoid window-name-based targets because names can change with the active command.

## Behavior
- Commits first using the provided commit message (`git add -A` then `git commit -m ...`).
- Uses the newly created `HEAD` commit as the target review SHA.
- Runs exactly one review request at a time (global lock).
- Sends final review text back to the target pane, waits 5 seconds, then sends Enter.

## Mode switch (from `.env`)
- `REQUEST_REVIEW_MODE=remote`
  - Pushes branch (`git push -u origin HEAD`).
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
- `REQUEST_REVIEW_TIMEOUT_SECONDS=1800`
- `REQUEST_REVIEW_LOCAL_PROFILE=local-review`

## Critical discipline
- Only run one review request at a time.
- Do not launch concurrent review requests.
