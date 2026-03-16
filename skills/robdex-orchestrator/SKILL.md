---
name: robdex-orchestrator
description: Use Robdex communication and orchestration via `scripts/robdex`. First run `scripts/robdex-role-instructions` so the live thread loads only its role-specific guidance from `resources/orchestrator.md` or `resources/worker.md`. Thread identity is auto-resolved from `$CODEX_THREAD_ID`; never pass sender ID manually. [skill-hash:c914c20]
---

# Robdex Orchestrator

Use this skill for Robdex-backed worker/orchestrator communication, bookkeeping, archive flows, and routed approvals.

## First Step

Before acting on this skill, run:

- `~/.codex/skills/robdex-orchestrator/scripts/robdex-role-instructions`

That script uses the live Robdex thread identity to determine whether the caller is an orchestrator or a worker and prints only the matching role reference:

- `resources/orchestrator.md`
- `resources/worker.md`

Do not preload both role files just to figure out which one applies.

## Required Path

- Use: `~/.codex/skills/robdex-orchestrator/scripts/robdex ...`
- Use: `~/.codex/skills/robdex-orchestrator/scripts/robdex-role-instructions`
- Do not call Robdex MCP tools directly for normal orchestration.
- Do not pass `from_thread_id` or sender identity manually.

## Identity And Scope

- `$CODEX_THREAD_ID` is the sender identity source.
- The script resolves sender identity automatically.
- If `$CODEX_THREAD_ID` is missing, stop and report tooling failure.
- `robdex whoami` shows the live resolved role, thread id, and project path.
- Bridge-owned authorization decides who can list, message, archive, or approve. Do not recreate that logic in shell scripts or prompts.

## Shared Communication Surface

- Projects:
  - `robdex list-projects`
- Role / identity:
  - `robdex whoami`
  - `robdex role-instructions`
- Agents:
  - `robdex list-agents`
  - `robdex list-agents --include-archived`
  - `robdex list-agents --all-projects`
  - `robdex spawn-agent --name "<title>" --prompt "<task>"`
  - `robdex rename-agent --name "<old>" --new-name "<new>"`
  - `robdex archive-agent --name "<title>"`
  - `robdex archive-agent --to-thread-id "<thread id>"`
  - `robdex unarchive-agent --name "<title>" --prompt "<message>"`
- Messaging:
  - `robdex send-message --name "<agent name>" --text "<message>"`
  - `robdex send-message --to-thread-id "<thread id>" --text "<message>"`
- Metadata:
  - `robdex set-worker-metadata --name "<agent name>" --issue-number <issue>`
  - `robdex set-worker-metadata --name "<agent name>" --pr-number <pr>`
  - `robdex set-worker-metadata --name "<agent name>" --blocked-reason "<reason>" --unblock-when "<condition>"`
  - `robdex set-worker-metadata --name "<agent name>" --clear-blocked`
- Routed approvals:
  - `robdex list-pending-approvals`
  - `robdex approve-approval --approval-id <approval id>`
  - `robdex decline-approval --approval-id <approval id> [--message "<note>"]`
- Thread groups:
  - `robdex list-thread-groups [--project-path <path>]`
  - `robdex create-thread-group --title "<title>" [--project-path <path>] [--seed-thread-id <thread>]`
  - `robdex update-thread-group --group-id <id> [--title "<title>"] [--collapsed|--expanded] [--project-path <path>]`
  - `robdex move-thread-to-group --thread-id <thread> [--group-id <id> | --remove] [--project-path <path>]`
  - `robdex delete-thread-group --group-id <id> [--project-path <path>]`
  - `robdex archive-thread-group --group-id <id> [--project-path <path>]`

## Source Of Truth

- The Robdex bridge is the source of truth for visible agents, message authorization, archive authorization, and pending approvals.
- The routed approval ledger is bridge-visible pending approval state, not the chat notification that announced it.
- Local metadata is bookkeeping only; it does not replace bridge authorization or scoped visibility.

## Approval Notes

- `list-pending-approvals` may include `approvalReason` and joined `fileChanges`; prefer those file summaries over generic grant-root text when deciding.
- `decline-approval --message "<note>"` declines the approval and asks Robdex to send the note as a normal follow-up worker message.
- If approval output reports a `follow-up error`, treat it as partial failure: the approval decision already happened, but the note did not reach the worker.

## System Experts

- When detailed `codex app-server` behavior matters, ask `Codex App-Server Expert`.
- `Codex App-Server Expert` is a read-only orchestrator inside the real app-server codebase and is specialized for code-level implementation lookup.
- When the issue is upstream in Robdex rather than local config, forward it to `Robdex Orchestrator`.

## Robdex Runtime

- `Robdex Orchestrator` owns the Robdex runtime itself.
- Robdex code changes are restart-bound. If a Robdex-side fix is reported, do not claim it is live until the app has actually been rebuilt/restarted from the integrated code.

## Shared Guardrails

- Keep orchestration within the bridge-authorized project scope.
- Use the public `robdex` script surface instead of inventing ad hoc bridge calls.
- Approval routing is for reasonable task-aligned engineering commands, not blatantly destructive commands.
- If tooling fails in a non-input way, stop and escalate with the exact command, cwd, and output.
- Role-specific operating discipline lives in the dynamic role instructions, not in this top-level file.
