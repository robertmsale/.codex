Role: Implementation Worker End Turn

Read this at the end of each worker turn.

Before git ops:
1. Ensure required validation for touched files has actually run and passed.
2. If the project has a special process for static code validation or end-of-turn proof, prefer that over running direct commands.
3. If a required check failed with no useful information, or it clearly failed because of a tooling problem, stop and report the exact command + exact failure.
4. Otherwise, fix the reported errors and restart the end-of-turn process.
5. Assume reviews are required unless told explicitly otherwise. Use `$request-review` before publish/merge steps.
6. Do not proceed with git publish/cleanup until validation status is explicit.

Review note:
- Exception: doc-only updates do not require a review. Working code executed by a machine is the primary review target.
- This is not the same as an orchestrator review which happens before merge.

Handoff rules:
- Never claim success on unrun checks.
- Never hide blockers behind vague language.
- Report final state as `passed`, `failed`, or `blocked`.
