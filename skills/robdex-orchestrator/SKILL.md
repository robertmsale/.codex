---
name: robdex-orchestrator
description: Orchestrate workers via `scripts/robdex` (script-first). Thread identity is auto-resolved from `$CODEX_THREAD_ID`; never pass sender ID manually. Use worker metadata bookkeeping for issue, PR, and blocked state. [skill-hash:6c1b2e4]
---

# Robdex Orchestrator

Use this skill to list, spawn, steer, rename, unarchive, and maintain worker bookkeeping.

## Required Path

- Use: `~/.codex/skills/robdex-orchestrator/scripts/robdex ...`
- Do not call robdex MCP tools directly for normal orchestration.
- Do not pass `from_thread_id` manually.

## Identity

- `$CODEX_THREAD_ID` is the sender identity source.
- The script injects sender identity automatically.
- If `$CODEX_THREAD_ID` is missing, stop and report tooling failure.

## Commands

- List projects:
  - `robdex list-projects`
- List agents:
  - `robdex list-agents`
  - `robdex list-agents --include-archived`
  - `robdex list-agents --all-projects`
- Spawn:
  - `robdex spawn-agent --name "<title>" --prompt "<task>"`
  - `robdex spawn-agent --name "<title>" --prompt "<task>" --issue-number <issue>`
- Message:
  - `robdex send-message --name "<agent name>" --text "<message>"`
  - `robdex send-message --to-thread-id "<thread id>" --text "<message>"`
- Rename:
  - `robdex rename-agent --name "<old>" --new-name "<new>"`
- Unarchive:
  - `robdex unarchive-agent --name "<title>" --prompt "<message>"`
- Worker metadata:
  - `robdex set-worker-metadata --name "<agent name>" --issue-number <issue>`
  - `robdex set-worker-metadata --name "<agent name>" --pr-number <pr>`
  - `robdex set-worker-metadata --name "<agent name>" --blocked-reason "<reason>" --unblock-when "<time or condition>"`
  - `robdex set-worker-metadata --name "<agent name>" --clear-blocked`
  - `robdex set-worker-metadata --name "<agent name>" --clear-issue-number`
  - `robdex set-worker-metadata --name "<agent name>" --clear-pr-number`

## Bookkeeping

- Keep issue, PR, and blocked state current on active workers.
- `list-agents` includes issue, PR, and blocked metadata when present.
- To clear blocked state, use `--clear-blocked`. Do not write placeholder values like `active` or `now`.

## Guardrails

- Keep orchestration within project boundaries.
- Use `send-message` for active workers.
- Use `unarchive-agent` only when a worker is archived.
- Only set bookkeeping on worker threads inside your own project.
- Do not set worker metadata on orchestrator threads.
- If tooling fails in a non-input way, stop and escalate.
