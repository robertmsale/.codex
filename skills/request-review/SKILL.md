---
name: request-review
description: Request review via `scripts/request-review` with a commit message. Review output is written to `review.log` in the worktree root. Skip review for non-working-code changes such as docs, policy text, or comment-only edits. MUST USE $command-execution SKILL WITH THIS PROCESS. [skill-hash:8a4d2f1]
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
- Review mode and review disable are operator-controlled from the canonical request-review config file.
- Non-working-code changes such as docs, policy text, or comment-only edits do not require request-review.
- In remote mode, GitHub review state is the source of truth for whether the review actually happened.
- In remote mode, once cloud review is in progress, the wrapper waits indefinitely for completion; there is no caller override for a shorter timeout.
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

- Remote review is still pending after a long time:
  - do not treat long cloud review wait alone as a failed review outcome
  - if the review process is still alive and GitHub shows the trigger comment / in-progress signal, classify it as a long-wait in-progress state
- Review succeeded remotely but `review.log` is absent:
  - do not rerun request-review just because `review.log` is empty
  - inspect GitHub directly
  - look for a thumbs-up reaction on the trigger comment or for new inline review comments on the target commit
  - if GitHub shows the remote review result, treat remote review as completed and classify the local empty `review.log` as a tooling artifact problem
  - if GitHub does not show a completed remote review result yet, classify it as still in progress
- `--use-existing-commit` on a clean rerun:
  - valid when the intended review target is already committed
  - invalid when the operator actually expects the wrapper to capture new uncommitted work

## Verification Guidance

- If remote mode completes cleanly and `review.log` is present, use it as the local publish gate.
- If `review.log` is empty or absent, verify remote status on GitHub before doing anything else.
- GitHub is the source of truth for whether remote review actually happened.
- `review.log` is only the local artifact the publish script looks for.

## Guardrails

- Refuses protected integration branches.
- Do not launch duplicate review requests for the same branch/PR scope.
- Caller-supplied process env does not override operator-controlled request-review behavior.
- For changes that affect working code or runtime behavior, keep using request-review before publish/merge.
