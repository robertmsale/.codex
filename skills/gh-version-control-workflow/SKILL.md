---
name: gh-version-control-workflow
description: Script-first worktree/branch/PR workflow using local scripts under `scripts/` plus direct `git`/`gh`. Use dedicated worktrees and PRs as the delivery units; do not create child issues by default for every internal slice. Working-code changes require review. [skill-hash:7f5c2a9]
---

# GH Version Control Workflow

Use this workflow for branch/worktree/PR delivery.

## Required Rules

- Use dedicated worktrees for implementation.
- Never commit on integration branches (`main`, `master`, etc.).
- Use script wrappers in this skill for git mutations.
- Merge PRs with squash; do not use merge commits or rebase merges in the gitops workflow.
- `git-merge-worktree` is the authoritative merge-and-cleanup path for worktree branches.
- Do not use raw `gh pr merge --delete-branch` for linked-worktree branches; it does not own local worktree cleanup, prune, and branch deletion safely.
- Request review before publish for working-code changes.
- Non-working-code docs, policy text, and comment-only edits may skip request-review when there is no runtime or security impact.
- Do not create a child GitHub issue for every internal implementation slice by default.
- It is acceptable for multiple related PRs/workers to reference the same master issue.
- Use the shared `~/.codex` skill script paths shown here. Do not swap them for worktree-local `.codex/...` wrapper paths unless a project-local skill explicitly requires a repo-local wrapper.

## Core Scripts

- Create worktree:
  - `~/.codex/skills/gh-version-control-workflow/scripts/git-worktree-create <repo_path> <base_branch> <branch_name> <worktree_name>`
- Sync worktree:
  - `~/.codex/skills/gh-version-control-workflow/scripts/git-sync-worktree <worktree_path> [integration_branch]`
- Commit:
  - `~/.codex/skills/gh-version-control-workflow/scripts/git-commit <worktree_path> "<message>"`
- Publish (push + PR, force-with-lease on non-FF for non-integration branches):
  - `~/.codex/skills/gh-version-control-workflow/scripts/git-publish-worktree <worktree_path> [integration_branch]`
- Merge (squash merge the PR, delete the remote branch, remove the local worktree, prune worktree metadata, and delete the local branch):
  - `~/.codex/skills/gh-version-control-workflow/scripts/git-merge-worktree <worktree_path> [integration_branch]`
  - if the squash merge fails, the worktree and branch are left in place for conflict resolution or retry
- Cleanup (stash dirty parent repo state, fast-forward the parent integration branch, restore the stash, then remove the worktree):
  - `~/.codex/skills/gh-version-control-workflow/scripts/git-worktree-cleanup <worktree_path> [integration_branch]`

## Review Requirement

- Run request review via:
  - `~/.codex/skills/request-review/scripts/request-review "<commit message>"`
- `git-publish-worktree` refuses when `review.log` is missing, so review-skipping changes may need direct PR push/merge instead of the publish script.

## Source Of Truth

- GitHub PR state is the source of truth for publish, review, and merge status.
- Local repo and worktree state are the source of truth for whether cleanup finished.
- `review.log` is the local gate for `git-publish-worktree`, not the remote truth for whether review happened.
- Routed command or file approvals are governed by Robdex pending-approval state, not by GitHub or local git state.

## Preferred Planning Model

- One master issue may cover a larger effort.
- Independent workers can each have their own worktree/branch/PR under that same master issue.
- Create child issues only when they are useful outside the local execution workflow.

## Delivery Steps

1. Create/update the master issue when external tracking is needed.
2. Create worktree branch with `git-worktree-create`.
3. Implement in worktree.
4. Commit with `git-commit`.
5. Request review (`request-review`) for working-code changes, or skip it for non-working-code docs/policy/comment-only changes.
6. Publish (`git-publish-worktree`) when review is required and `review.log` is present.
7. Merge with `git-merge-worktree`, which performs the squash merge, deletes the remote branch, fast-forwards the parent integration branch, removes the worktree, prunes worktree metadata, and deletes the local branch.

## Multi-Worker Guidance

When several workers are active on one master issue:
- each worker should have its own worktree/branch
- each worker may open its own PR asynchronously
- downstream workers should sync/rebase after dependency merges rather than waiting for a giant combined branch

Prefer this over:
- one giant long-lived branch
- many child issues with no reliable closeout discipline

## Known Edge Cases

- Merged PR but incomplete local cleanup:
  - trust GitHub for the merge result
  - trust the local parent repo/worktree state for whether cleanup finished
  - rerun `git-merge-worktree` or `git-worktree-cleanup` instead of improvising raw `gh pr merge --delete-branch`
- Worktree removed but branch left behind:
  - confirm the PR really merged and the worktree is gone
  - then remove only the leftover feature branch; do not touch the integration branch
- Request-review lock serialization:
  - a local review lock is not a GitHub merge/review failure by itself
  - inspect remote PR state separately before classifying the issue
- Stale `db_test` stacks:
  - if a project validation workflow uses `db_test.sh`, ensure the wrapper stack is brought down during cleanup rather than leaving stale services attached to a finished worktree
