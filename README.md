# Personal Codex Config

This directory contains my Codex CLI personal configuration (`config.toml`),
state, and installed skills.

## Skills

- `command-parser`: Wrap noisy commands and summarize errors/warnings with
  file/line anchors.
- `gh-version-control-workflow`: Run issue-driven git/gh flow with worktrees,
  PRs, and disciplined cleanup.
- `request-review`: Commit and trigger review (remote bot or local `codex exec`)
  and send results to tmux.
- `safe-delete`: Stage deletions into `/tmp/safe-delete` instead of using `rm`.
- `gitops` MCP (`git_worktree_cleanup`): Deterministic, policy-aware
  path-only worktree cleanup (trash dir + prune metadata).

## `config.toml` at a glance

- Model providers:
  - `lmstudio-local`: local Responses-compatible provider at
    `http://localhost:1234/v1`
- Profiles (`[profiles.*]`):
  - `psql`: DB query helper profile with custom `developer_instructions` for
    `psql` + clipboard workflow
  - `command-parser-spark`: parser profile on `gpt-5.3-codex-spark`
  - `command-parser-local`: parser profile using local LM Studio model
    (`openai/gpt-oss-20b`)
  - `command-parser`: parser profile on `gpt-5.1-codex-mini`
  - `local-review`: local review profile on `gpt-5.3-codex`
  - `planner`: orchestrator/delegation profile with detailed planning
    instructions
