Role: Review-Requesting Worker

Use this when you need code review on the current worktree branch.

Required path:
- Run `~/.codex/skills/request-review/scripts/request-review "<commit message>"`
- Launch it with the command-execution skill/MCP.
- Keep the returned `job_id`.
- Wait with `command_execution_wait(job_id)`.
- Do not poll stdin.
- Do not kill the review because it is taking a long time.
- Do not call MCP review tools or alternate legacy review commands.
- Use the shared `~/.codex` script path unless a project skill explicitly requires a repo-local wrapper.

Review rules:
- Review output is written to `review.log` in the worktree root.
- Review mode and review disable are operator-controlled from the canonical config.
- Non-working-code changes such as docs, policy text, or comment-only edits do not require request-review.
- In remote mode, GitHub review state is the source of truth for whether the review actually happened.
- In remote mode, once cloud review is in progress, the wrapper waits indefinitely for completion.
- `review.log` is the local publish gate, not the remote source of truth.

Inputs:
- Required: commit message text
- Optional: `--use-existing-commit`
- Optional: `--existing-commit <sha-or-ref>`

Existing-commit rules:
- Use `--use-existing-commit` when reviewing an already-created commit instead of creating a new one.
- Use `--existing-commit <sha-or-ref>` when the review target is a specific existing commit or ref instead of `HEAD`.
- Do not use `--use-existing-commit` when intended changes are still uncommitted or when the next correct action is to create a fresh commit for review.

Verification:
- If remote mode completes cleanly and `review.log` is present, use it as the local publish gate.
- If remote mode ran and `review.log` is empty or absent, do not rerun request-review just because the artifact is missing.
- Inspect GitHub directly for the completed remote review result.

Guardrails:
- Protected integration branches are refused.
- Caller-supplied process env does not override operator-controlled behavior.
- For working code or runtime behavior changes, keep using request-review unless the operator explicitly says otherwise.
