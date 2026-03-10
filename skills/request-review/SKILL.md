---
name: request-review
description: Request review via `scripts/request-review` with a commit message. Review output is written to `review.log` in the worktree root. Skip review for non-working-code changes such as docs, policy text, or comment-only edits. [skill-hash:6d1a2c4]
---

# Request Review

Use this skill when you need code review on the current worktree branch.

## Required Path

- Run: `~/.codex/skills/request-review/scripts/request-review "<commit message>"`
- Do not call MCP review tools.
- Do not run alternate legacy review commands.
- Use the shared `~/.codex` script path shown here. Do not rewrite it to a worktree-local `.codex/...` path unless a project-local skill explicitly requires a repo-local wrapper.

## Behavior

- Review output is written/read from `review.log` in the worktree root.
- Review mode and review disable are operator-controlled.
- Non-working-code changes such as docs, policy text, or comment-only edits do not require request-review.
- In remote mode, GitHub review state is the source of truth for whether the review actually happened.
- `review.log` is the local publish-gate artifact, not the remote source of truth.
- The local lock under `~/.codex/tmp/request-review.lock.*` is only local serialization state for this wrapper.

## Input

- Required: commit message text.
- Optional: `--use-existing-commit`
- Optional: `--existing-commit <sha-or-ref>`

## Existing Commit Rules

- Plain clean reruns may already reuse `HEAD` automatically; use `--use-existing-commit` when you need to make that intent explicit or when you want to target an already-created commit without creating a new one.
- `--existing-commit <sha-or-ref>` is for reviewing a specific existing commit or ref instead of the default `HEAD`.
- Treat `--use-existing-commit` as required when the operator intent is "review this existing commit only" and no new commit should be created.
- Treat `--use-existing-commit` as invalid when intended changes are still uncommitted or when the next correct action is to create a fresh commit for review.
- If the branch is dirty and those changes belong in the review target, do not use `--use-existing-commit`; commit the intended changes and rerun normally.

## Source Of Truth

- Remote GitHub PR state decides whether the review trigger comment landed and whether the remote review actually ran.
- Local `review.log` decides whether `git-publish-worktree` will proceed.
- Local lock state decides only whether another local `request-review` process is already active for the same branch/PR scope.

## Edge Cases

- Remote review succeeded but the local wrapper hung or left a stale lock:
  - inspect the lock path printed by the wrapper, then inspect `<lock path>/owner`
  - if that PID is still active for the same scope, do not launch a duplicate run
  - if the PID is gone and GitHub already shows the correct trigger comment or bot review for the intended PR+commit, classify it as local stale-lock state, remove the stale lock directory, and rerun once
- Review succeeded remotely but `review.log` is absent:
  - treat this as a local artifact problem, not as proof that remote review failed
  - if GitHub shows the correct remote review result, rerun once to repopulate `review.log`
  - if rerun still cannot restore `review.log`, stop and classify it as a tooling blocker because publish depends on the local artifact
- `--use-existing-commit` on a clean rerun:
  - valid when the intended review target is already committed
  - invalid when the operator actually expects the wrapper to capture new uncommitted work

## Lock / Retry Guidance

- Safe retry:
  - remote review state is already correct, but the local wrapper exited poorly, hung, or failed to write `review.log`
  - the printed lock owner PID is gone or clearly stale
  - rerun once after removing the stale lock
- Inspect before retry:
  - lock path exists and the owner PID may still be live
  - remote GitHub state is unclear for the intended PR/commit
- True blocker:
  - repeated reruns keep recreating or colliding on the same lock without a live owner
  - remote review succeeded but `review.log` still cannot be restored
  - remote GitHub state and local wrapper state disagree in a way you cannot reconcile from one safe inspection

## Known Edge Cases

- Request-review serializes by PR scope in remote mode and by branch scope otherwise, so a rerun can legitimately collide with another active local operator run.
- A stale local lock does not mean remote review failed.
- A missing `review.log` after visible remote success is a local wrapper artifact issue until proven otherwise.

## Guardrails

- Refuses protected integration branches.
- Do not launch duplicate review requests for the same branch/PR scope.
- For changes that affect working code or runtime behavior, keep using request-review before publish/merge.
