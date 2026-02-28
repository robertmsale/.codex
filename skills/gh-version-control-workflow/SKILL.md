---
name: gh-version-control-workflow
description: MCP-first issue/branch/worktree/PR workflow using `gitops` tools for all mutating git and GitHub operations; direct git/gh usage is fallback-only and must be justified. MANDATORY, READ WHOLE SKILL FILE ONE TIME AND NEVER READ AGAIN, DO NOT EDIT FILES UNTIL THIS IS READ. [skill-hash:8c41f2a]
---

# GH Version Control Workflow

## Overview
Use this workflow for issue-driven development with strict MCP-first mutation:
- one issue per branch
- one branch per worktree
- one PR per issue branch
- no direct feature commits to integration branches

All mutating git/GitHub operations should go through MCP `gitops` tools.

## MCP contract (required)
When `gitops` MCP is available, use it for all writes:
- Git/worktree mutation:
  - `git_worktree_create`
  - `git_worktree_cleanup`
  - `git_fetch`
  - `git_rebase`
  - `git_commit`
  - `git_request_review_and_wait`
- GitHub mutation:
  - `github_create_issue`
  - `github_update_issue`
  - `github_add_issue_comment`
  - `github_add_pull_request_comment`
  - `github_add_pull_request_review_comment`
- GitHub reads (preferred over CLI):
  - `github_list_issues`
  - `github_get_issue`
  - `github_get_pull_request`
  - `github_list_pull_request_review_comments`

Direct `git`/`gh` commands are fallback-only when MCP is unavailable or missing a required capability.
If fallback is used, state why.

## Process

### 0) Ensure release tracker exists (`Version Bump: X.Y.Z`)
Find open issue titled `Version Bump: X.Y.Z`:
- Use `github_list_issues(state="open")`
- Reuse if exactly one match exists
- If missing, create with `github_create_issue(...)`
- If duplicates exist, stop and ask user which one to keep

### 1) Create and triage issue
Create a scoped issue with acceptance criteria:
- `github_create_issue(title, body, labels, assignees)`
- refine state/labels with `github_update_issue(...)`

### 1b) Link issue into the version tracker
If issue is in scope for the release, add tracker comment with:
- issue reference (`#<number>`)
- draft manual test notes

Tool: `github_add_issue_comment(...)`

### 1c) Optional: split large work into sub-issues
When work is large:
- create parent + child issues via `github_create_issue`
- track hierarchy in issue bodies/comments (MCP tools do not expose dedicated sub-issue graph helpers)
- still maintain one branch/worktree/PR per child issue

### 2) Derive branch from issue
Pick branch name:
- `codex/issue-<issue-number>-<slug>`
- for non-issue tasks: `codex/adhoc-<slug>`

Choose integration base branch per repo policy.

### 3) Create dedicated worktree
Create worktree from integration branch:
- `git_worktree_create(repo_path, base_branch, branch_name, worktree_name)`

`git_worktree_create` base branch input rules (to avoid ambiguous failures):
- Pass `base_branch` as a plain branch name (for example `master`), not a remote-qualified ref (for example not `origin/master`).
- The MCP tool resolves the remote internally; passing `origin/...` can produce invalid `origin/origin/...` refs.
- If the default integration base is missing in the repo (for example `origin/integration` does not exist), explicitly pick the repo's actual integration branch name from `git branch -r` (commonly `master`).

Always implement inside the created worktree, not the primary checkout.

### 4) Bootstrap PR branch and publish
Bootstrap branch with empty commit if needed:
- `git_commit(message="chore: bootstrap PR", add_all=false, allow_empty=true, repo_path=<worktree>)`

When syncing your worktree branch with integration:
- `git_fetch(repo_path=<worktree>, remote="origin", prune=true)`
- `git_rebase(repo_path=<worktree>, upstream="origin/<integration-branch>")`

Do not push manually.
Remote branch + PR lifecycle should run through `git_request_review_and_wait`.

### 5) Open PR and link issue
When ready for review, call:
- `git_request_review_and_wait(commit_message, repo_path=<worktree>, create_pr_if_missing=true, pr_title, pr_body)`

PR body must include `Closes #<issue-number>` for issue-linked work.

For regular implementation commits:
- `git_commit(message, add_all=true, allow_empty=false, repo_path=<worktree>)`

### 5b) Read review feedback (conversation + review summaries + inline code comments)
Read and respond to feedback via MCP:
- PR summary/state: `github_get_pull_request`
- inline comments: `github_list_pull_request_review_comments`
- reply:
  - top-level thread: `github_add_pull_request_comment`
  - inline code location: `github_add_pull_request_review_comment`

### 6) Validate and merge
Use repository CI/status policy for merge readiness.
If merge automation is not available via MCP in this environment, request user/maintainer merge through standard repo flow.

### 6a) Label auto-closed issues for QA
After merge, mark related issues for QA:
- `github_update_issue(issue_number, labels=...)`
- optionally add handoff details with `github_add_issue_comment`

### 6b) Add QA handoff comment to the version tracker
For each issue entering QA, add tracker comment:
- issue number
- PR link
- explicit manual validation steps
- expected results

Tool: `github_add_issue_comment`

### 7) Mandatory local cleanup after merge
After merge, cleanup is required:
- `git_worktree_cleanup(repo_path, branch_name, worktree_path, delete_local_branch=true, delete_remote_branch=true, force=false)`

Never delete branches/worktrees freehand when MCP cleanup is available.

## Guardrails
- Never commit directly to shared integration branches for feature work.
- Never request review/PR without linking issue in PR body (`Closes #...`) for issue-scoped work.
- Keep one issue per branch and one branch per worktree.
- Do not delete a worktree until its PR is merged or intentionally abandoned.
- Do not leave merged issue branches/worktrees on disk; cleanup is required.
- Use `git_worktree_cleanup` for cleanup; avoid freehand branch/worktree deletion commands.
- If an issue is in-scope for a release, it must be referenced in `Version Bump: X.Y.Z`.
- When adding `qa` to a closed issue, always add manual testing instructions to the version tracker.
- Do not work inside the base repo; implement inside worktrees.

## Minimal tool playbook
- Create issue: `github_create_issue`
- Update issue labels/state/assignee: `github_update_issue`
- Comment issue: `github_add_issue_comment`
- Create worktree branch: `git_worktree_create`
- Sync with remote integration branch: `git_fetch`, `git_rebase`
- Commit changes: `git_commit`
- Request review (PR/push/wait): `git_request_review_and_wait`
- Read PR: `github_get_pull_request`
- Read inline review comments: `github_list_pull_request_review_comments`
- Reply on PR: `github_add_pull_request_comment` or `github_add_pull_request_review_comment`
- Cleanup merged/abandoned branch: `git_worktree_cleanup`

## Command Reference
See `references/commands.md` only for fallback troubleshooting when MCP is unavailable.
