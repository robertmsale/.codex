---
name: command-execution
description: Every command execution yields a stable `job_id`; if command completion is not immediate, wait with MCP `command_execution_wait(job_id)`. [skill-hash:5b1e4a9]
---

# Command Execution

## Purpose

Use this skill for all command execution.

Execution model is simple:
1. Every command invocation yields a stable `job_id`.
2. If the command is still running when you regain control, wait on that same
   ID with MCP `command_execution_wait(job_id)`.

## Required Rules

- Always capture the `job_id` from the `stderr` line:
  `job_id: <uuid>`.
- Reuse that `job_id` for waiting and resuming.
- Do not launch duplicate command runs while an existing run is active.
- Do not use manual polling loops when `command_execution_wait` is available.

## Workflow

1. Run the command normally.
2. Capture the printed `job_id`.
3. If completion is not immediate, call:
   `command_execution_wait(job_id=<that_id>)`.
4. Continue after waiter returns.

## Output Expectations

- MCP waiter output is intentionally minimal.
- Command output comes from the launched command execution path, not from waiter output.
