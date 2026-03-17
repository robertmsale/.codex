Role: Implementation Worker Start Turn

Read this at the start of each worker turn.

Required order:
1. Confirm scope and deliverable for this turn in one sentence.
2. If git workflow is involved, read `$gh-version-control-workflow` before any git mutation.
3. If the project has turn-related skills for the phase you are entering, read and follow them before proceeding.
4. Create a dedicated worktree and do the turn's implementation work there.
5. If commands are needed, capture the command `job_id`.
6. If command completion is not immediate, wait with `command_execution_wait(job_id)`.
7. Use `$command-parser` only for noisy output extraction.
8. If deletions are needed, use `$safe-delete`.
9. If worker orchestration is needed, use `$robdex-orchestrator`.

Hard rules:
- Do not start implementation before pipeline selection is clear.
- Do not ignore project-specific process skills that refine the current phase of work.
- Do not implement in the base repo when worktree workflow applies.
- Do not bypass required skill workflows.
- Do not invent fallback paths when a required skill/tool is available.
- Do not concurrently execute any commands that rely on locks regardless of how safe you think it is to do.
