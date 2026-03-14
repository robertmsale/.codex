---
name: request-review
description: Request review via `scripts/request-review` with a commit message. Review output is written to `review.log` in the worktree root. Skip review for non-working-code changes such as docs, policy text, or comment-only edits. MUST USE $command-execution SKILL WITH THIS PROCESS. [skill-hash:1d62a8e]
---

# Request Review

Use this skill when you need code review on the current worktree branch.

## Required Path

- Run: `~/.codex/skills/request-review/scripts/request-review "<commit message>"`
- Launch it with the command-execution skill/MCP.
- Keep the returned `job_id`.
- Wait with `command_execution_wait(job_id)`.
- Do not poll stdin.
- Do not kill the review just because it is taking a long time.
- Do not call MCP review tools.
- Do not run alternate legacy review commands.
- Use the shared `~/.codex` script path shown here. Do not rewrite it to a worktree-local `.codex/...` path unless a project-local skill explicitly requires a repo-local wrapper.

## Behavior

- Review output is written/read from `review.log` in the worktree root.
- Review mode and review disable are operator-controlled from the canonical request-review config file.
- Non-working-code changes such as docs, policy text, or comment-only edits do not require request-review.
- In remote mode, GitHub review state is the source of truth for whether the review actually happened.
- In remote mode, once cloud review is in progress, the wrapper waits indefinitely for completion.
- `review.log` is the local publish-gate artifact, not the remote source of truth.
- The wrapper no longer uses local request-review lock files for review serialization.

## Input

- Required: commit message text.
- Optional: `--use-existing-commit`
- Optional: `--existing-commit <sha-or-ref>`

## Existing Commit Rules

- Use `--use-existing-commit` when the intended action is to review an already-created commit instead of creating a new one.
- Use `--existing-commit <sha-or-ref>` when the review target is a specific existing commit or ref instead of `HEAD`.
- Do not use `--use-existing-commit` when intended changes are still uncommitted or when the next correct action is to create a fresh commit for review.

## Source Of Truth

- Remote GitHub PR state decides whether the review trigger comment landed and whether the remote review actually ran.
- Local `review.log` decides whether `git-publish-worktree` will proceed.

## Verification Guidance

- If remote mode completes cleanly and `review.log` is present, use it as the local publish gate.
- If remote mode ran and `review.log` is empty or absent, do not rerun request-review just because of the missing artifact.
- Inspect GitHub directly.
- Look for a thumbs-up reaction on the trigger comment or for new inline review comments on the target commit.
- If GitHub shows a completed remote review result, treat the review as complete and classify the missing `review.log` as a local tooling artifact problem.
- If GitHub does not show a completed remote review result yet, treat the review as still in progress.

## Guardrails

- Refuses protected integration branches.
- Caller-supplied process env does not override operator-controlled request-review behavior.
- For changes that affect working code or runtime behavior, keep using request-review before publish/merge.
