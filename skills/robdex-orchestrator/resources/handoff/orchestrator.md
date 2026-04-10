# Orchestrator Handoff

Your handoff must preserve orchestration continuity for the project.

Include:

- the current overall task or mission the user wants completed
- where active work should be sourced from
- the current spawned-agent landscape in the project
- which agents are blocked, drifting, or waiting
- important agent-behavior gotchas, failure modes, and user-approved solutions
- any review, QA, or merge gates that remain in force

The replacement orchestrator should be able to resume coordination immediately without re-auditing the project state from scratch.
