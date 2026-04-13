---
name: gh-version-control-workflow
description: Script-first worktree/branch/PR workflow using local scripts under `scripts/` plus direct `git`/`gh`. Use this when you need the sanctioned mutating git and GitHub workflow tools. [skill-hash:8ac43d1]
---

# GH Version Control Workflow

Use this skill when you need the sanctioned wrappers for mutating git or GitHub operations.

## What This Skill Covers

- create or clean up managed worktrees
- stage, unstage, commit, sync, publish, and merge through sanctioned wrappers
- use raw `git` directly for read-only inspection commands
- use `request-review` before publish when the current project/operator workflow requires review

Use the shared `~/.codex` skill script paths shown here unless a project-local skill explicitly says otherwise.

## Mutating Commands

- Create worktree:
  - `git-worktree-create <repo_path> <base_branch> <branch_name> <worktree_name>`
- Sync worktree:
  - `git-sync-worktree <worktree_path> [integration_branch]`
- QA fast-forward:
  - `qa-fastforward <worktree_path> [integration_branch]`
  - for a dedicated `.worktrees/...` checkout, stashes scratch/untracked QA artifacts, updates that checkout onto the latest integration branch, then restores the stash
  - for a QA device-specific checked-out integration repo, fast-forwards the checked-out integration branch to `origin/<integration_branch>` and surfaces dirty/conflict failures directly
- Commit:
  - `git-commit <worktree_path> "<message>"`
- Publish (push + PR, force-with-lease on non-FF for non-integration branches):
  - `git-publish-worktree <worktree_path> [integration_branch]`
  - Publish output is the sanctioned PR metadata artifact. It includes the PR number, URL, state, draft flag, branch, base branch, and title when GitHub has a PR for the branch.
- Merge (squash merge the PR, delete the remote branch, remove the local worktree, prune worktree metadata, and delete the local branch):
  - `git-merge-worktree <worktree_path> [integration_branch]`
  - if the squash merge fails, the worktree and branch are left in place for conflict resolution or retry
- Cleanup (remove the local worktree, prune worktree metadata, and delete the local branch when it is no longer checked out elsewhere):
  - `git-worktree-cleanup <worktree_path> [integration_branch]`
  - cleanup refuses the checked-out base repo and only operates on dedicated managed worktrees under `.worktrees/`

## Recovery Scripts

- Stage specific paths:
  - `git-stage-paths <repo_or_worktree_path> <path> [path...]`
- Unstage specific paths:
  - `git-unstage-paths <repo_or_worktree_path> <path> [path...]`
- Abort in-progress rebase:
  - `git-rebase-abort <repo_or_worktree_path>`

Read-only `git` commands such as `git status`, `git branch`, `git diff`, `git show`, `git rev-parse`, and `git merge-base` are intentionally not wrapped. Use raw `git` directly for inspection.

## Review

- Run request review with:
  - `request-review "<commit message>"`
- `git-publish-worktree` checks for `review.log`.

## Typical Sequence

1. Create a worktree with `git-worktree-create`.
2. Implement in that worktree.
3. Commit with `git-commit`.
4. Run `request-review` when review is part of the current workflow.
5. Publish with `git-publish-worktree`.
6. Merge and clean up with `git-merge-worktree`.

## Guardrails

- Sanctioned mutating workflow scripts refuse the checked-out base repo.
- `git-merge-worktree` owns merge plus worktree cleanup for managed worktree branches.
- If publish, review, or cleanup state is unclear, inspect the real state and continue with the sanctioned script instead of inventing a new path.
