---
name: robdex-orchestrator
description: Use Robdex communication via `robdex`. This skill is only for the tool surface and shared usage rules. Role behavior lives in the base instructions. [skill-hash:a782d9f]
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
  - `robdex decline-approval --approval-id <id> [--message "<note>"]`
- Agent lifecycle and bookkeeping:
  - `robdex spawn-agent --role worker|qa|hidden|requirements-reviewer ...`
  - `robdex archive-agent ...`
  - `robdex rename-agent ...`
  - `robdex set-worker-metadata ...`
  - `robdex handoff --help`
- Requirements:
  - `robdex set-requirements --name "<agent name>" --requirements-file /absolute/path/to/requirements.json`
  - `robdex request-requirements-review --name "<agent name>" [--note "<checkpoint context>"]`

## Requirements

Use Requirements when task constraints must become an explicit completion contract rather than prompt prose.

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

## Shared Guardrails

- Use the public `robdex` script surface.
- Bridge-owned authorization decides who can list, message, archive, decline, or mutate bookkeeping state.
- Prefer `--text-file` or `--text-stdin` for shell-sensitive message text.
- Before using warm handoff, run `robdex handoff --help` and follow the role-specific handoff guidance it prints.
- Use warm handoff only when the user explicitly asks for it.
- If an approval request appears, do not approve it.
- If an approval request appears, load the `privileged-exec` skill immediately and follow that workflow.
- If a sanctioned, non-destructive, necessary command still triggers an approval request or privileged-exec rejection, report the exact command and the relevant error output to the user or orchestrator instead of improvising.
- `qa` is a non-implementer validation role. It follows worker-style communication rules but is meant to pilot stories and report usability/product issues rather than fix code.
