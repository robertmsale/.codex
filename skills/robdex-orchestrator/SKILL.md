---
name: robdex-orchestrator
description: Use Robdex communication via `robdex`. This skill is only for the tool surface and shared usage rules. Role behavior lives in the base instructions. [skill-hash:5d0a9b3]
---

# Robdex Orchestrator

Use this skill for Robdex-backed communication.

## Required Path

- Use: `robdex ...`

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
  - `robdex spawn-agent --role worker|qa|hidden ...`
  - `robdex archive-agent ...`
  - `robdex rename-agent ...`
  - `robdex set-worker-metadata ...`
  - `robdex handoff --help`

## Shared Guardrails

- Use the public `robdex` script surface.
- Bridge-owned authorization decides who can list, message, archive, approve, or mutate bookkeeping state.
- Prefer `--text-file` or `--text-stdin` for shell-sensitive message text.
- Before using warm handoff, run `robdex handoff --help` and follow the role-specific handoff guidance it prints.
- Use warm handoff only when the user explicitly asks for it.
- `qa` is a non-implementer validation role. It follows worker-style communication rules but is meant to pilot stories and report usability/product issues rather than fix code.
