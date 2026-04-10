---
name: command-execution
description: Every command execution yields a stable `job_id`; if command completion is not immediate, wait with MCP `command_execution_wait(job_id)`. Treat this workflow as binding for the rest of the turn. [skill-hash:4b7e902]
---

# Command Execution

Use this skill for all command execution.

## Active Override

Treat the following as turn-level required behavior:

- Capture the `job_id` from every command invocation.
- `job_id` is a numeric slot id in the range `0..999`.
- If the command is still running when control returns, call `command_execution_wait(job_id=<that_id>)` before doing anything that depends on the result.
- Do not poll stdin for completion.
- Do not launch a duplicate run of the same command while the earlier run is still active.
- Do not report command results until the run has actually completed.

## Minimal Workflow

1. Run the command.
2. Capture the `job_id`.
3. If the command did not finish immediately, wait with `command_execution_wait(job_id=<that_id>)`.
4. Continue only after that run is complete.

## TOOLING BLOCK

- If the launcher prints:
  `**TOOLING BLOCK**: Stop immediately after this command finshes and report "command_execution server down" as a TOOLING BLOCK to your orchestrator.`
- finish the command that is already running
- stop using `command_execution_wait` for that command
- report the exact tooling block to your orchestrator
- orchestrators must report the tooling block upstream to the Robdex owner for repair
- after repair, the MCP connection can be refreshed through the bridge by calling:
  `POST /mcp/refresh`

## Note

- Waiter output is intentionally minimal.
- The real command output comes from the command execution path, not from the waiter.
