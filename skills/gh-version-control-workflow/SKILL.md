---
name: gh-version-control-workflow
description: Script-first issue/worktree/PR workflow using local scripts under `scripts/` plus direct `git`/`gh`. Do not use MCP gitops mutation tools for normal flow. [skill-hash:2f69c13]
---

# GH Version Control Workflow

Use this workflow for issue-driven branch/worktree/PR delivery.

## Required Rules

- Use dedicated worktrees for implementation.
- Never commit on integration branches (`main`, `master`, etc.).
- Use script wrappers in this skill for git mutations.
- Request review before publish.

## Core Scripts

- Create worktree:
  - `~/.codex/skills/gh-version-control-workflow/scripts/git-worktree-create <repo_path> <base_branch> <branch_name> <worktree_name>`
- Sync worktree:
  - `~/.codex/skills/gh-version-control-workflow/scripts/git-sync-worktree <worktree_path> [integration_branch]`
- Commit:
  - `~/.codex/skills/gh-version-control-workflow/scripts/git-commit <worktree_path> "<message>"`
- Publish (push + PR, force-with-lease on non-FF for non-integration branches):
  - `~/.codex/skills/gh-version-control-workflow/scripts/git-publish-worktree <worktree_path> [integration_branch]`
- Cleanup:
  - `~/.codex/skills/gh-version-control-workflow/scripts/git-worktree-cleanup <worktree_path>`

## Review Requirement

- Run request review via:
  - `~/.codex/skills/request-review/scripts/request-review "<commit message>"`
- `git-publish-worktree` refuses when `review.log` is missing.

## Issue + PR Steps

1. Create/update issue with `gh issue ...`.
2. Create worktree branch with `git-worktree-create`.
3. Implement in worktree.
4. Commit with `git-commit`.
5. Request review (`request-review`).
6. Publish (`git-publish-worktree`).
7. Merge and cleanup (`git-worktree-cleanup`).
