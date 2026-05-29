---
name: gh-version-control-workflow
description: Script-first worktree/branch/PR workflow using local scripts under `scripts/` plus direct `git`/`gh`. Use this when you need the sanctioned mutating git and GitHub workflow tools. [skill-hash:7b3f0c2]
---

# GH Version Control Workflow

Use this skill when you need the sanctioned wrappers for mutating git or GitHub operations.

## What This Skill Covers

- commit, publish, merge, and recover managed worktree branches
- stage, unstage, commit, sync, publish, and merge through sanctioned wrappers
- use raw `git` directly for read-only inspection commands

Use the shared `~/.codex` skill script paths shown here unless a project-local skill explicitly says otherwise.

Run each mutating git wrapper as its own command. Do not chain `git-stage-paths`,
`git-commit`, `git-publish-worktree`, or other mutating wrappers with `&&`,
`||`, `;`, pipes, or shell wrappers. Wait for one step to finish, inspect the
result if needed, then run the next step.

## Mutating Commands

- Create worktree manually only when hook-owned worker lifecycle is unavailable or an operator explicitly tells you to:
  - `git-worktree-create <repo_path> <base_branch> <branch_name> <worktree_name>`
- Refresh an existing persistent worktree onto a fresh non-integration branch from the latest origin integration branch:
  - `git-worktree-refresh-branch <worktree_path> <new_branch> [integration_branch]`
- Sync worktree:
  - `git-sync-worktree <worktree_path> [integration_branch]`
  - clean stale branches only; refuses dirty worktrees instead of stashing/reapplying local dirt
- Recover an existing published PR branch in place:
  - `git-recover-published-worktree <worktree_path> [integration_branch]`
  - use this only when the current branch already has a PR and is behind or non-mergeable against the integration branch
  - refuses unpublished branches and dirty worktrees; it is not a generic stale-branch update command
  - this keeps the existing worktree and PR branch, rebases there, and leaves rebase state in place if conflicts must be resolved manually
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
- Cleanup manually only when hook-owned archive cleanup did not handle it or an operator explicitly tells you to:
  - `git-worktree-cleanup <worktree_path> [integration_branch]`
  - cleanup refuses the checked-out base repo and only operates on dedicated managed worktrees under `.worktrees/`

## Recovery Scripts

- Stage specific paths:
  - `git-stage-paths <repo_or_worktree_path> <path> [path...]`
- Unstage specific paths:
  - `git-unstage-paths <repo_or_worktree_path> <path> [path...]`
- Abort in-progress rebase:
  - `git-rebase-abort <repo_or_worktree_path>`
- Continue in-progress rebase after conflict resolution:
  - `git-rebase-continue <repo_or_worktree_path>`

Read-only `git` commands such as `git status`, `git branch`, `git diff`, `git show`, `git rev-parse`, and `git merge-base` are intentionally not wrapped. Use raw `git` directly for inspection.

## Typical Sequence

1. Start in the assigned worktree, usually created by project hooks.
2. Implement in that worktree.
3. Commit with `git-commit`.
4. Publish with `git-publish-worktree`.
5. Merge with `git-merge-worktree`. Let hook-owned archive cleanup run unless manual cleanup is explicitly needed.

## Published PR Recovery

If a published PR branch is behind, conflicted, or non-mergeable:

1. Stay on the existing branch and worktree.
2. Verify the worktree is clean. If it is dirty or polluted, stop and report the exact status; do not preserve the dirt with stash/reapply.
3. Run `git-recover-published-worktree <worktree_path> [integration_branch]`.
4. If the rebase stops on conflicts, resolve them in that same worktree.
5. Stage the resolutions and run `git-rebase-continue <worktree_path>`.
6. Rerun proof in that same worktree.
7. Run `git-publish-worktree <worktree_path> [integration_branch]` to update the existing PR branch with force-with-lease if needed.

Do not create a fresh branch/worktree to replay the old commit.
Do not cherry-pick the old commit onto a new branch unless an operator explicitly tells you to do that.

For a clean unpublished branch that is merely stale, use `git-sync-worktree`.
For an idle/pre-implementation worker whose worktree is dirty or polluted,
stop and report the exact status so the orchestrator can archive or recreate it
cleanly. Do not run recovery commands that preserve broad unrelated state.

## Guardrails

- Sanctioned mutating workflow scripts refuse the checked-out base repo.
- `git-merge-worktree` owns merge plus worktree cleanup for managed worktree branches.
- `git-worktree-refresh-branch` refuses protected integration branch names as the target branch.
- A published PR branch must be recovered in place on that same clean worktree and branch.
- A stale unpublished branch should use `git-sync-worktree`, which refuses dirty worktrees.
- Do not manually create or clean up a worker worktree when project hooks already own that lifecycle.
- If publish, review, or cleanup state is unclear, inspect the real state and continue with the sanctioned script instead of inventing a new path.
- Do not chain mutating git wrappers. Stage, commit, publish, merge, and cleanup are separate operations.
