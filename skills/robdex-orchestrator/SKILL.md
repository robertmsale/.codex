---
name: robdex-orchestrator
description: Use Robdex communication via `robdex`, including role-scoped Requirements workflows. Role behavior lives in the base instructions. [skill-hash:8f2c6a9]
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
  - Orchestrators setting Requirements on workers must read `resources/requirements/orchestrator.md`.
  - Planners setting Requirements on non-hidden agents in their project must read `resources/requirements/orchestrator.md` and follow the same Requirements authoring discipline.
  - Orchestrators authoring Requirements must also read `resources/requirements/authoring.md`.
  - Workers and QA with active Requirements must read `resources/requirements/worker.md`.
  - Non-orchestrator agents asked to set Requirements on themselves must read `resources/requirements/self-setting.md`.
  - For command syntax and formatting examples, run `robdex requirements-from-prose --help`.

## Requirements

Requirements instructions are role-scoped resources. Load only the resource that matches your current role and task:

- `resources/requirements/orchestrator.md`: orchestrators creating or replacing Requirements on workers, and planners creating or replacing Requirements on non-hidden agents in their project.
- `resources/requirements/authoring.md`: orchestrators compiling owner intent into Requirements.
- `resources/requirements/worker.md`: workers and QA operating under active Requirements.
- `resources/requirements/self-setting.md`: operators or other self-setting-eligible non-orchestrator agents explicitly asked to set Requirements on themselves.

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
