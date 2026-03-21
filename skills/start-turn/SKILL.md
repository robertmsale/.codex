---
name: start-turn
description: Read at the start of every turn. Establish the execution pipeline before work begins. First run `scripts/start-turn-role-instructions` so the current thread loads only its worker or orchestrator guidance. Project-specific turn-boundary skills may further refine the process and must be followed when relevant. [skill-hash:9b1c5d4]
---

# Start Turn

Read this at the start of each turn.

## Required Path

- Run: `~/.codex/skills/start-turn/scripts/start-turn-role-instructions`

## Shared Guardrails

- Follow project-specific turn-boundary skills when the current phase calls for them.
- Do not bypass required skill workflows.
- Do not invent fallback paths when a required skill/tool is available.
