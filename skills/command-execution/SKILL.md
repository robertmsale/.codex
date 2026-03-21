---
name: command-execution
description: Every command execution yields a stable `job_id`; if command completion is not immediate, wait with MCP `command_execution_wait(job_id)`. When this skill is active, treat its workflow as binding for the rest of the turn. [skill-hash:3a6d8f1]
---

# Command Execution

Use this skill for all command execution.

## Active Override

When this skill is active, treat the following as turn-level required behavior:

- Run shell commands only through the command execution path that returns a `job_id`.
- Capture the `job_id` from every command invocation.
- If the command is still running when control returns, call `command_execution_wait(job_id=<that_id>)` before doing anything that depends on the result.
- Do not poll stdin for completion.
- Do not launch a duplicate run of the same command while the earlier run is still active.
- Do not report command results until the run has actually completed.

## Minimal Workflow

1. Run the command.
2. Capture the `job_id`.
3. If the command did not finish immediately, wait with `command_execution_wait(job_id=<that_id>)`.
4. Continue only after that run is complete.

## Note

- Waiter output is intentionally minimal.
- The real command output comes from the command execution path, not from the waiter.
