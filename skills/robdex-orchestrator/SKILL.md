---
name: robdex-orchestrator
description: Use Robdex communication via `robdex`, including Requirements workflows that preserve operator-approved scope and fan out large work by complete responsibility boundaries. Role behavior lives in the base instructions. [skill-hash:5d91c8b]
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
  - For multi-line text, pipe stdin with a heredoc:

    ```bash
    robdex send-message --to-thread-id "<thread id>" --text-stdin <<'EOF'
    Message text goes here.
    EOF
    ```

    ```bash
    robdex send-message --name "<agent name>" --text-stdin <<'EOF'
    Message text goes here.
    EOF
    ```

  - Never run `robdex send-message ... --text-stdin` without a heredoc, pipe, or redirected file attached. Bare `--text-stdin` waits for interactive stdin and can leave the agent stuck in the terminal.
- Thread groups:
  - `robdex list-thread-groups`
  - `robdex create-thread-group ...`
  - `robdex update-thread-group ...`
  - `robdex move-thread-to-group ...`
  - `robdex delete-thread-group ...`
  - `robdex archive-thread-group ...`
- Approvals:
  - `robdex list-pending-approvals`
  - `robdex decline-approval --approval-id <id> [--message "<note>"]`
- Agent lifecycle and bookkeeping:
  - `robdex spawn-agent --role worker|qa|hidden|requirements-reviewer ...`
  - `robdex archive-agent ...`
  - `robdex rename-agent ...`
  - `robdex set-worker-metadata ...`
  - `robdex handoff --help`
- Requirements:
  - For prose input, pipe stdin with a heredoc:

    ```bash
    robdex requirements-from-prose --title "<title>" --text-stdin <<'EOF'
    Requirement prose goes here.
    EOF
    ```

    ```bash
    robdex requirements-from-prose --title "<title>" --include-composable non-negotiables --include-composable no-legacy --text-stdin <<'EOF'
    Requirement prose goes here.
    EOF
    ```

    ```bash
    robdex requirements-from-prose --title "<title>" --include-composable non-negotiables --include-composable no-legacy --text-stdin --attach --name "<agent name>" <<'EOF'
    Requirement prose goes here.
    EOF
    ```

  - Never run `robdex requirements-from-prose ... --text-stdin` without a heredoc, pipe, or redirected file attached.
  - `robdex requirements-from-prose --title "<title>" --include-composable no-legacy --text-stdin --interrupt --name "<agent name>"`
    - For running target agents: generates Requirements, interrupts the target, sets Requirements, then sends `Requirements updated`.
  - `robdex requirements-from-prose --title "<title>" --include-composable no-legacy --text-stdin --to-self`
    - For the current thread only: generates Requirements, sets them on self, briefly delays, interrupts self, then sends `Begin`.
  - `robdex requirements-composables list --name "<agent name>"`
  - `robdex requirements-composables show review-evidence --name "<agent name>"`
  - `robdex requirements-compose --title "<title>" --include-composable review-evidence --requirements-file /absolute/path/to/task-requirements.json`
  - `robdex requirements-compose --title "<title>" --include-composable review-evidence --requirements-file /absolute/path/to/task-requirements.json --attach --name "<agent name>"`
  - `robdex set-requirements --name "<agent name>" --requirements-file /absolute/path/to/requirements.json`
    - Use for an idle target agent before sending the execution prompt.
  - `robdex set-requirements --name "<agent name>" --requirements-file /absolute/path/to/requirements.json --interrupt`
    - For running target agents: interrupts the target, sets Requirements, then sends `Requirements updated`.
  - `robdex set-requirements --to-self --requirements-file /absolute/path/to/requirements.json`
    - For the current thread only: sets Requirements, briefly delays, interrupts self, then sends `Begin`.
  - `requirements-from-prose` and `set-requirements` require `--name` or `--to-thread-id` when attaching to another agent unless `--to-self` is provided. `--attach`, `--interrupt`, and `--to-self` are mutually exclusive on `requirements-from-prose`; `--interrupt` and `--to-self` are mutually exclusive on `set-requirements`.

## Requirements

Use Requirements when task constraints must become an explicit completion contract rather than prompt prose.

Requirements preserve the operator-approved outcome. Worker recommendations are advisory evidence only; do not convert a worker's reduced scope, alternate implementation, documentation-only compromise, or "small first step" into the Requirements contract unless the operator explicitly authorizes that change.

Normal worker flow:
- Spawn workers without Requirements when the first turn is discovery, triage, planning, or pre-implementation.
- When the worker stops at pre-implementation, compare their plan against the operator-approved outcome and reject drift before setting Requirements.
- Convert the operator-approved outcome for the full assigned work package into Requirements. Use the worker plan only to identify implementation steps, dependencies, validation evidence, and missing owner decisions.
- Attach Requirements while the worker is idle with `robdex set-requirements --name "<agent name>" --requirements-file <file>`, before sending the execution prompt.
- Then send the implementation prompt. The next turn will be requirements-gated from the start.

Do not try to attach Requirements to a running turn. Requirements apply to `turn/start`; they cannot change the schema of an already-running turn or a mid-turn steer.

If a target worker is already running and you must replace its Requirements, use `--interrupt`. For prose-generated Requirements, use `robdex requirements-from-prose --title "<title>" --text-stdin --interrupt --name "<agent name>"`. For an existing file, use `robdex set-requirements --name "<agent name>" --requirements-file <file> --interrupt`. This interrupts the target, sets the new Requirements, and sends `Requirements updated` so the target resumes under the new contract.

If you are setting Requirements on your own current thread, use `--to-self`. For prose-generated Requirements, use `robdex requirements-from-prose --title "<title>" --text-stdin --to-self`. For an existing file, use `robdex set-requirements --to-self --requirements-file <file>`. Do not omit `--to-self`; self-setting intentionally performs a set, brief delay, self-interrupt, and `Begin` self-message sequence so your next turn starts under the new Requirements.

Large work is handled by dependency-ordered fan-out, not micro-slice Requirements. If the operator's requested outcome is too large or cross-cutting for one worker, create complete work packages by responsibility boundary, such as contracts, backend implementation, frontend integration, design/system polish, and QA validation. Each package's Requirements must cover that package's full responsibility and map back to the top-level operator outcome.

Do not create Requirements for only the easiest first step, a partial pattern, or a documentation placeholder unless the operator requested that narrowed outcome. Scope changes require proof of impossibility, internal conflict, unsafe work, or a missing owner decision, followed by explicit operator authorization.

Composable Requirements:
- Use `robdex requirements-composables list` to discover reusable global and recipient-project composables before drafting Requirements.
- Use `robdex requirements-composables show <id>` to inspect exact requirement text before selecting a composable.
- Project-specific composables are resolved from the recipient agent's tracked project, not the sender's project.
- Select composables only when they are relevant to the assigned work package.
- When an operator has made a composable mandatory for the current work stream, include it at Requirements creation time. For clean-slate or no-legacy work, include `--include-composable no-legacy`; for broad engineering work, consider `--include-composable non-negotiables`.
- Composables supplement task-specific Requirements; they must not replace, narrow, or drift from the operator-approved outcome.
- For prose-generated Requirements, use `robdex requirements-from-prose --include-composable <id>` so preview and attach both include the selected composables.
- For existing RequirementSet JSON files, use `robdex requirements-compose` or `set-requirements --include-composable <id>` to attach a composed set through the sanctioned Requirements route.

The requirements file is JSON. It may be either an array of requirement objects or an object with a `requirements` array. Use semantic keys, not numbered keys.

```json
{
  "requirements": [
    {
      "key": "nativeGuiIsSourceOfTruth",
      "statement": "The web GUI must mirror the native Flutter GUI. Native chat timeline, composer, controls, and density are source of truth.",
      "severity": "blocker",
      "verificationMethod": "diffReview"
    },
    {
      "key": "noInventedWebsocketEventShapes",
      "statement": "Do not invent websocket or HTTP event shapes. Use the existing bridge protocol unless deliberately updating the protocol and proving compatibility.",
      "severity": "blocker",
      "verificationMethod": "diffReview"
    }
  ]
}
```

When active, Robdex injects a structured output schema into the source agent's turns. Each requirement becomes a required top-level JSON property. A completed claim is routed to a requirements reviewer when one is configured or available in the same project.

Review lifecycle:
- Requirements reviews are system-managed. Do not ask workers, QA, designers, operators, or orchestrators to manually request a Requirements review.
- A failed review routes the failed requirements back to the source agent.
- An accepted true blocker routes to the owner/orchestrator.
- A passing review clears the active Requirements and detaches/archives the reviewer so future Requirements get a fresh reviewer.

## Shared Guardrails

- Use the public `robdex` script surface.
- Bridge-owned authorization decides who can list, message, archive, decline, or mutate bookkeeping state.
- Prefer `--text-file` or heredoc-fed `--text-stdin` for shell-sensitive message text. Bare `--text-stdin` is invalid operationally because it waits for interactive input.
- Before using warm handoff, run `robdex handoff --help` and follow the role-specific handoff guidance it prints.
- Use warm handoff only when the user explicitly asks for it.
- If an approval request appears, do not approve it.
- If an approval request appears, load the `privileged-exec` skill immediately and follow that workflow.
- If a sanctioned, non-destructive, necessary command still triggers an approval request or privileged-exec rejection, report the exact command and the relevant error output to the user or orchestrator instead of improvising.
- `qa` is a non-implementer validation role. It follows worker-style communication rules but is meant to pilot stories and report usability/product issues rather than fix code.
