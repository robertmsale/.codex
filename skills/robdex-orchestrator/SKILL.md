---
name: robdex-orchestrator
description: Orchestrate workers via `scripts/robdex` (script-first). Prefer one master issue plus a small number of independent worker slices, keep worker metadata current, use direct worker-to-worker coordination when interfaces or dependencies matter, and route reasonable approval/escalation requests through the orchestrator when needed. Thread identity is auto-resolved from `$CODEX_THREAD_ID`; never pass sender ID manually. [skill-hash:b4dc7fe]
---

# Robdex Orchestrator

Use this skill to list, spawn, steer, rename, archive, unarchive, maintain worker bookkeeping, and coordinate reasonable approval routing when a worker hits sandbox limits.

## Preferred Delivery Model

Default to:
- one master issue that represents the overall effort
- a small number of worker agents for non-cross-cutting, dependency-light slices
- one PR per worker branch/worktree

Do not default to:
- creating a child issue for every implementation slice
- leaving many open child issues after the real work has already moved to PRs
- treating issue creation as mandatory for internal decomposition

Use child issues only when they add real tracking value outside the orchestrator itself, for example:
- separately triaged external work
- work owned by different people outside the worker pool
- user-requested public breakdown
- follow-on items that should survive beyond the current orchestration run

If a master issue is sufficient, keep decomposition in worker prompts, metadata, and PRs rather than multiplying GitHub issues.

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
- Thread groups:
  - `robdex list-thread-groups [--project-path <path>]`
  - `robdex create-thread-group --title "<title>" [--project-path <path>] [--seed-thread-id <thread>]`
  - `robdex update-thread-group --group-id <id> [--title "<title>"] [--collapsed|--expanded] [--project-path <path>]`
  - `robdex move-thread-to-group --thread-id <thread> [--group-id <id> | --remove] [--project-path <path>]`
  - `robdex delete-thread-group --group-id <id> [--project-path <path>]`
  - `robdex archive-thread-group --group-id <id> [--project-path <path>]`
- Spawn:
  - `robdex spawn-agent --name "<title>" --prompt "<task>"`
  - `robdex spawn-agent --name "<title>" --prompt "<task>" --issue-number <issue>`
- Message:
  - `robdex send-message --name "<agent name>" --text "<message>"`
  - `robdex send-message --to-thread-id "<thread id>" --text "<message>"`
- Rename:
  - `robdex rename-agent --name "<old>" --new-name "<new>"`
- Archive:
  - `robdex archive-agent --name "<title>"`
  - `robdex archive-agent --to-thread-id "<thread id>"`
  - `robdex archive-agent --name "<title>" --project-path <path>`
- Unarchive:
  - `robdex unarchive-agent --name "<title>" --prompt "<message>"`
- Worker metadata:
  - `robdex set-worker-metadata --name "<agent name>" --issue-number <issue>`
  - `robdex set-worker-metadata --name "<agent name>" --pr-number <pr>`
  - `robdex set-worker-metadata --name "<agent name>" --blocked-reason "<reason>" --unblock-when "<time or condition>"`
  - `robdex set-worker-metadata --name "<agent name>" --clear-blocked`
  - `robdex set-worker-metadata --name "<agent name>" --clear-issue-number`
  - `robdex set-worker-metadata --name "<agent name>" --clear-pr-number`

## Approval Routing

Approval requests are an explicit supported orchestration path when a worker is blocked by sandboxing or other command-execution approval requirements.

Use this path for commands that:
- make sense for the task at hand
- are normal engineering operations such as tests, builds, checks, or other routine validation steps
- may need sandbox escalation, writable-root changes, or similar approval to proceed

Do not route or approve commands that are blatantly destructive or nonsensical, for example:
- `rm -rf /`
- obvious wipe/reset/destructive commands unrelated to the task
- similarly high-risk commands with no credible engineering justification

Practical bar:
- the command must make sense for the work
- the rationale must be coherent
- blatantly destructive commands are not approval-worthy

## Delegation Guidance

Before spawning workers, decide how many are justified.

Use more than one worker only when the slices are:
- meaningfully independent
- low-conflict in touched files
- low-dependency or dependency-manageable with branch sync/rebase

Prefer fewer workers when:
- the work is cross-cutting in the same files
- there is a single risky integration seam
- the coordination overhead will exceed the throughput gain

Good default split patterns:
- frontend vs backend
- API contract vs consuming UI
- infrastructure/tooling vs product surface
- independent feature slices with clear boundaries

## Worker Coordination

Workers can and should coordinate directly when needed.

Use direct worker-to-worker messaging when:
- one worker needs an interface contract from another
- a dependency merged and a downstream worker needs to sync/rebase
- two workers touch adjacent seams and need to agree on boundaries
- a worker is blocked on another worker's output rather than on tooling

Do not wait for workers to "just notice" each other. If coordination is needed, instruct it explicitly.

Keep the orchestrator aware of coordination-critical state:
- who depends on whom
- which worker should sync/rebase after a merge
- whether a blocker is code dependency vs tooling vs product decision

## Bookkeeping

- Keep issue, PR, and blocked state current on active workers.
- `list-agents` includes issue, PR, and blocked metadata when present.
- To clear blocked state, use `--clear-blocked`. Do not write placeholder values like `active` or `now`.
- If workers share one master issue, it is acceptable for multiple workers to carry the same issue number.
- Do not invent child issue numbers solely to satisfy bookkeeping.

## Guardrails

- Keep orchestration within project boundaries.
- Use `send-message` for active workers.
- Once a worker is confirmed completely done with its task, the orchestrator may archive it with `archive-agent`.
- Use `unarchive-agent` only when a worker is archived.
- Approval routing is for reasonable task-aligned commands that need escalation, not for blatantly destructive commands.
- Only set bookkeeping on worker threads inside your own project.
- Do not set worker metadata on orchestrator threads.
- If tooling fails in a non-input way, stop and escalate.
- Prefer PRs and worker metadata as the execution ledger; do not use a swarm of stale child issues as pseudo-status tracking.
