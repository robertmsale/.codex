# gitops-mcp

Local MCP server used by robdex/Codex for guarded git mutations and GitHub
workflows.

## Setup

1. `cd ~/.codex/mcp/gitops`
2. `uv sync`
3. Create `.env` from `.env.example`
4. Set `GITHUB_TOKEN` (required for remote review and `github_*` tools)

## Review control knobs (operator-owned)

These are loaded from `~/.codex/mcp/gitops/.env` and
`~/.codex/skills/request-review/.env`. Agents should not edit these values.

- `REQUEST_REVIEW_MODE=local|remote`
- `REQUEST_REVIEW_DISABLE=0|1`
- `REQUEST_REVIEW_BOT_LOGIN=...`
- `REQUEST_REVIEW_TRIGGER_COMMENT=...`
- `REQUEST_REVIEW_POLL_INTERVAL_SECONDS=...`
- `REQUEST_REVIEW_LOCAL_PROFILE=...`
- `REQUEST_REVIEW_LOCAL_OUTPUT_FILE=...`
- `REQUEST_REVIEW_LOCAL_ERROR_FILE=...`
- `REQUEST_REVIEW_LOCAL_KEEP_DEBUG_LOGS=0|1`
- `REQUEST_REVIEW_INTEGRATION_BRANCHES=...`

## Worktree defaults

- `GITOPS_INTEGRATION_BRANCH=...` (default base branch used by
  `git_worktree_create`)
- `GITOPS_WORKTREE_DIR=.worktrees` (repo-relative directory where worktrees are
  created)

## Worktree cleanup

- Use `git_worktree_cleanup(worktree_path)`.
- Cleanup flow is path-only:
  1. Validate path is a registered non-root git worktree
  2. Move the directory to Trash (`/usr/local/bin/trash` by default, fallback `trash` on `PATH`)
  3. Run `git worktree prune`
- Branch deletion is intentionally not performed by this tool.

## Trivial git ops

- Use `git_fetch` for `git fetch` equivalents.
- Use `git_rebase` for rebasing current worktree branch (protected branches
  blocked).

## Run (stdio)

- `uv --project ~/.codex/mcp/gitops run gitops-mcp`

## Output contract

- Tools return plain text (not JSON objects) to reduce token overhead.

## Smoke test GitHub auth

- `uv --project ~/.codex/mcp/gitops run gitops-smoke-github --cwd ~/Code/ezra/ezra`
