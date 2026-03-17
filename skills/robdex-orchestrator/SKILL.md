---
name: robdex-orchestrator
description: Use Robdex communication via `scripts/robdex`. First run `scripts/robdex-role-instructions` so the live thread loads only its role-specific guidance from `resources/orchestrator.md` or `resources/worker.md`. Shared `SKILL.md` content stays limited to common surface and guardrails. [skill-hash:2d4c8af]
---

# Robdex Orchestrator

Use this skill for Robdex-backed communication. Role-specific operating rules live only in the matching role file.

## First Step

Before acting on this skill, run:

- `~/.codex/skills/robdex-orchestrator/scripts/robdex-role-instructions`

That script uses the live Robdex thread identity to determine whether the caller is an orchestrator or a worker and prints only the matching role reference.
Do not preload both role files just to figure out which one applies.

## Required Path

- Use: `~/.codex/skills/robdex-orchestrator/scripts/robdex ...`
- Use: `~/.codex/skills/robdex-orchestrator/scripts/robdex-role-instructions`
- Do not call Robdex MCP tools directly for normal orchestration.

## Shared Commands

- Role / identity:
  - `robdex role-instructions`
- Agents:
  - `robdex list-agents`
- Messaging:
  - `robdex send-message --name "<agent name>" --text "<message>"`
  - `robdex send-message --to-thread-id "<thread id>" --text "<message>"`
  - `robdex send-message --name "<agent name>" --text-file <path>`
  - `robdex send-message --to-thread-id "<thread id>" --text-stdin`

Everything else is role-specific. Get the exact allowed/expected surface from `robdex role-instructions`.

## Shared Guardrails

- Keep orchestration within the bridge-authorized project scope.
- Bridge-owned authorization decides who can list, message, archive, approve, or mutate bookkeeping state.
- Use the public `robdex` script surface instead of inventing ad hoc bridge calls.
- Prefer `--text-file` or `--text-stdin` for shell-sensitive message text that may contain backticks or other command-substitution syntax.
- If tooling fails in a non-input way, stop and escalate with the exact command, cwd, and output.
- When the issue is upstream in Robdex rather than local config, forward it to `Robdex Orchestrator`.
- Robdex runtime fixes are restart-bound until the app has actually been rebuilt/restarted from the integrated code.
