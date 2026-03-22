---
name: robdex-orchestrator
description: Use Robdex communication via `scripts/robdex`. This skill is only for the tool surface and shared usage rules. Role behavior lives in the base instructions. [skill-hash:5d0a9b3]
---

# Robdex Orchestrator

Use this skill for Robdex-backed communication.

## Required Path

- Use: `~/.codex/skills/robdex-orchestrator/scripts/robdex ...`
- Do not call Robdex MCP tools directly for normal orchestration.

## Common Commands

- Agents:
  - `robdex list-agents`
  - `robdex list-projects`
- Messaging:
  - `robdex send-message --name "<agent name>" --text "<message>"`
  - `robdex send-message --to-thread-id "<thread id>" --text "<message>"`
  - `robdex send-message --name "<agent name>" --text-file <path>`
  - `robdex send-message --to-thread-id "<thread id>" --text-stdin`
- Thread groups:
  - `robdex list-thread-groups`
  - `robdex create-thread-group ...`
  - `robdex update-thread-group ...`
  - `robdex move-thread-to-group ...`
  - `robdex delete-thread-group ...`
  - `robdex archive-thread-group ...`
- Approvals:
  - `robdex list-pending-approvals`
  - `robdex approve-approval --approval-id <id>`
  - `robdex decline-approval --approval-id <id> [--message "<note>"]`
- Agent lifecycle and bookkeeping:
  - `robdex spawn-agent ...`
  - `robdex archive-agent ...`
  - `robdex rename-agent ...`
  - `robdex set-worker-metadata ...`

## Shared Guardrails

- Keep orchestration within the bridge-authorized project scope.
- Bridge-owned authorization decides who can list, message, archive, approve, or mutate bookkeeping state.
- Use the public `robdex` script surface instead of inventing ad hoc bridge calls.
- Prefer `--text-file` or `--text-stdin` for shell-sensitive message text.
- If tooling fails in a non-input way, stop and escalate with the exact command, cwd, and output.
- When the issue is upstream in Robdex rather than local config, forward it to `Robdex Orchestrator`.
- Robdex runtime fixes are restart-bound until the app has actually been rebuilt and restarted from the integrated code.
