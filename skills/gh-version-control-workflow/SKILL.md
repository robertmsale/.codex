---
name: gh-version-control-workflow
description: Script-first worktree/branch/PR workflow using local scripts under `scripts/` plus direct `git`/`gh`. Use dedicated worktrees and PRs as the delivery units. Working-code changes require review. [skill-hash:8ac43d1]
---

# GH Version Control Workflow

Use this workflow for branch/worktree/PR delivery.

## Required Rules

- Use dedicated worktrees for implementation.
- Never commit on integration branches (`main`, `master`, etc.).
- Use script wrappers in this skill for git mutations.
- Merge PRs with squash.
- `git-merge-worktree` is the authoritative merge-and-cleanup path for worktree branches.
- Do not use raw `gh pr merge --delete-branch` for linked-worktree branches; it does not own local worktree cleanup, prune, and branch deletion safely.
- Request review before publish for working-code changes.
- Non-working-code docs, policy text, and comment-only edits may skip request-review when there is no runtime or security impact.
- Use the shared `~/.codex` skill script paths shown here. Do not swap them for worktree-local `.codex/...` wrapper paths unless a project-local skill explicitly requires a repo-local wrapper.

## Core Scripts

- Create worktree:
  - `~/.codex/skills/gh-version-control-workflow/scripts/git-worktree-create <repo_path> <base_branch> <branch_name> <worktree_name>`
- Sync worktree:
  - `~/.codex/skills/gh-version-control-workflow/scripts/git-sync-worktree <worktree_path> [integration_branch]`
- QA fast-forward:
  - `~/.codex/skills/gh-version-control-workflow/scripts/qa-fastforward <worktree_path> [integration_branch]`
  - stashes scratch/untracked QA artifacts, updates the worktree onto the latest integration branch, then restores the stash
- Commit:
  - `~/.codex/skills/gh-version-control-workflow/scripts/git-commit <worktree_path> "<message>"`
- Publish (push + PR, force-with-lease on non-FF for non-integration branches):
  - `~/.codex/skills/gh-version-control-workflow/scripts/git-publish-worktree <worktree_path> [integration_branch]`
- Merge (squash merge the PR, delete the remote branch, remove the local worktree, prune worktree metadata, and delete the local branch):
  - `~/.codex/skills/gh-version-control-workflow/scripts/git-merge-worktree <worktree_path> [integration_branch]`
  - if the squash merge fails, the worktree and branch are left in place for conflict resolution or retry
- Cleanup (stash dirty parent repo state, fast-forward the parent integration branch, restore the stash, then remove the worktree):
  - `~/.codex/skills/gh-version-control-workflow/scripts/git-worktree-cleanup <worktree_path> [integration_branch]`
  - dirty worktree content is also stashed before removal so scratch work is recoverable instead of being rejected

When these scripts need bridge-backed gitops, they transparently forward to the host bridge at `http://127.0.0.1:8765`.

## Visibility And Recovery Scripts

- Status:
  - `~/.codex/skills/gh-version-control-workflow/scripts/git-status <repo_or_worktree_path>`
- Branch list:
  - `~/.codex/skills/gh-version-control-workflow/scripts/git-branch-list <repo_or_worktree_path> [--local-only]`
- Diff:
  - `~/.codex/skills/gh-version-control-workflow/scripts/git-diff <repo_or_worktree_path> [ref] [pathspec]`
- Show object:
  - `~/.codex/skills/gh-version-control-workflow/scripts/git-show <repo_or_worktree_path> <object>`
- Resolve ref:
  - `~/.codex/skills/gh-version-control-workflow/scripts/git-rev-parse <repo_or_worktree_path> [ref]`
- Merge base:
  - `~/.codex/skills/gh-version-control-workflow/scripts/git-merge-base <repo_or_worktree_path> <left> <right>`
- Stage specific paths:
  - `~/.codex/skills/gh-version-control-workflow/scripts/git-stage-paths <repo_or_worktree_path> <path> [path...]`
- Unstage specific paths:
  - `~/.codex/skills/gh-version-control-workflow/scripts/git-unstage-paths <repo_or_worktree_path> <path> [path...]`
- Abort in-progress rebase:
  - `~/.codex/skills/gh-version-control-workflow/scripts/git-rebase-abort <repo_or_worktree_path>`

## Review Requirement

- Run request review via:
  - `~/.codex/skills/request-review/scripts/request-review "<commit message>"`
- `git-publish-worktree` refuses when `review.log` is missing.

## Delivery Steps

1. Create a worktree with `git-worktree-create`.
2. Implement in that worktree.
3. Commit with `git-commit`.
4. Run `request-review` when review is required.
5. Publish with `git-publish-worktree`.
6. Merge and clean up with `git-merge-worktree`.

## Guardrails

- One worker, one worktree, one branch, one PR.
- Do not improvise alternate merge or cleanup paths.
- If publish, review, or cleanup state is unclear, inspect the real state and continue with the canonical script instead of inventing a new flow.
- On protected integration branches, only additive mutations and abort-style recovery are allowed.
