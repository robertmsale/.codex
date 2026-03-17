Role: Orchestrator End Turn

Read this at the end of each orchestrator turn.

Before authorizing publish, merge, or archive:
1. Ensure the worker's required validation actually ran and passed.
2. If the project has a special process for static code validation or end-of-turn proof, require that process instead of ad hoc direct commands.
3. If a claimed check failed with no useful information, or clearly failed because of tooling, treat it as a blocker and require the exact command + exact failure.
4. Otherwise, send the worker back to fix the reported errors and restart the end-of-turn process.
5. Assume reviews are required unless the operator explicitly waived them.
6. Do not treat orchestrator review as a substitute for request-review on working code.
7. Do not authorize publish, merge, cleanup, or archive until validation and review status are explicit.

Handoff rules:
- Do not accept success claims on unrun checks.
- Do not accept vague blockers.
- Classify the current state honestly as `passed`, `failed`, or `blocked`.
