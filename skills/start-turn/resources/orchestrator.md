Role: Orchestrator Start Turn

Read this at the start of each orchestrator turn.

Required order:
1. Confirm the orchestration objective for this turn in one sentence.
2. If the project has turn-related skills for the phase you are entering, read and follow them before proceeding.
3. Decide whether this turn is steering workers, doing local inspection, or handling a narrow implementation slice directly.
4. If worker orchestration is needed, use `$robdex-orchestrator`.
5. If you will mutate git state yourself, read `$gh-version-control-workflow` before any git mutation.
6. If commands are needed, capture the command `job_id`.
7. If command completion is not immediate, wait with `command_execution_wait(job_id)`.
8. Use `$command-parser` only for noisy output extraction.
9. If deletions are needed, use `$safe-delete`.

Hard rules:
- Do not start steering or implementation before the pipeline is clear.
- Do not ignore project-specific process skills that refine the current phase of work.
- Do not drift into worker execution without deciding that explicitly.
- Do not give workers vague next actions or overlapping ownership.
- Do not concurrently execute or instruct overlapping lock-sensitive commands in the same shared environment.
