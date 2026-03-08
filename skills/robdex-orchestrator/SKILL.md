---
name: robdex-orchestrator
description: Orchestrate workers via `scripts/robdex` (script-first). Thread identity is auto-resolved from `$CODEX_THREAD_ID`; never pass sender ID manually. Use worker metadata bookkeeping for issue, PR, and blocked state. Consult `Codex App-Server Expert` for codex app-server internals, and route upstream Robdex runtime bugs to `Robdex Orchestrator`; Robdex fixes require a full app restart. [skill-hash:91a7c4e]
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

## System Experts

- When an agent needs detailed information about how some `codex app-server` behavior works, source that information from `Codex App-Server Expert`.
- `Codex App-Server Expert` is a read-only orchestrator inside the real codex app-server codebase and is specialized for finding the relevant implementation details there.
- Use that agent for understanding app-server internals that local configs or wrapper tooling depend on; do not guess when the answer needs code-level confirmation.

## Robdex Runtime

- `Robdex Orchestrator` is the orchestrator for the Robdex runtime itself.
- Robdex is the native macOS app/runtime wrapper around `codex app-server`; for example, messages in this environment may be flowing through Robdex.
- Bug reports should come to this orchestrator first so you can determine whether the problem is in local Codex config/workflow logic, codex app-server behavior, or the Robdex app/runtime.
- If more app-server detail is needed during triage, consult `Codex App-Server Expert`.
- If the issue is upstream in Robdex rather than the local config layer, forward the bug details to `Robdex Orchestrator`.
- Any Robdex code change requires a full app restart to take effect. Even if `Robdex Orchestrator` reports the fix is done, treat it as restart-required and do not claim the fix is live until the user restarts the app.

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
