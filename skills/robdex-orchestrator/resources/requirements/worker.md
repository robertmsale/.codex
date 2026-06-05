# Requirements For Workers And QA

Requirements are authoritative. They represent the operator-approved completion contract for the assigned work package.

- Do not narrow, replace, or reinterpret Requirements because a smaller task seems easier.
- If a Requirement appears impossible, unsafe, internally conflicting, or missing an owner decision, stop with concrete proof and ask for direction.
- Mid-turn progress may use `requirements: null` when structured output is active.
- Final completion must claim every currently required unresolved requirement in the active schema with evidence. Do not omit a currently required claim because it feels obvious.
- After a partial Requirements Review, the final claim schema may shrink to only currently unresolved requirements. Requirements omitted because they previously passed remain binding and must not regress.
- An all-`notSatisfied` packet is never terminal. Continue working until at least one currently required Requirement can be claimed `satisfied`, `blocked`, or `notApplicable`.
- Use `blocked` only on the specific blocked Requirement and only for concrete external blockers, contradictory Requirements, unsafe work, or missing owner decisions. Task size, difficulty, uncertainty, refactor effort, failing stale tests, or lack of a convenient implementation path are not valid blockers.
- Requirements Review is system-managed. Do not run any manual review request command.
- QA should report product, usability, and tooling failures against the assigned Requirements; QA should not implement fixes unless explicitly assigned an implementation role.

Workers and QA normally do not set Requirements on other agents. If you need Requirements changed, report the exact conflict or blocker to the orchestrator/operator and hold position.
