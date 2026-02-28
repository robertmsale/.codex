---
name: command-parser
description: Run command execution through MCP `commandParser.command_parser_run`, using runtime sandbox policy and returning compact parser output (or sandbox failure text) without wrapper-script workflow. Callers cannot override sandbox settings. [skill-hash:6a41d2c]
---

# Command Parser

## Overview

Use MCP tool `commandParser.command_parser_run` for noisy command execution and
log extraction. Do not use the legacy shell wrapper as the default path.

## Quick Start

- `commandParser.command_parser_run(command=[...])`
- `commandParser.command_parser_run(command=[...], include_warnings=true)`
- `commandParser.command_parser_run(command=[...], additional_request="...")`

Recommended default:

- Do **not** request additional information unless absolutely necessary.
- Keep default compact extraction for speed and consistency.

## Configuration

Configuration is managed by the user. Do not change configuration.

Command-parser specific knobs (from `skills/command-parser/.env`):

- `COMMAND_PARSER_PROFILE=<profile>`
- `COMMAND_PARSER_DELAY=<seconds>`: delay before command execution

Policy file:

- `skills/command-parser/command-parser.rule` is checked via:
  `codex execpolicy check --rules <rule-file> -- <raw-command...>`
- This check runs before sandbox/execution.
- Forbidden decisions block execution with a plain-text refusal message.

Usage log:

- Each command invocation appends a plaintext entry to:
  `~/.codex/command-parser-usage.log`
- Format includes timestamp + raw command + cwd.

## MCP Behavior

- Tool executes the command once under resolved sandbox policy.
- Callers cannot pass sandbox override args (`sandbox_mode`, `network_access`, `thread_id`).
- Callers cannot override parser profile; profile is operator-managed via `COMMAND_PARSER_PROFILE`.
- If sandbox blocks execution, tool returns sandbox failure text directly.
- If command runs, tool returns parser extraction output.
- Tool output is plain text to minimize token usage.

## Output Expectations

- If no errors: `No errors!`
- Otherwise:
  - `## Errors` with one bullet per distinct error
  - optional `## Warnings` when `COMMAND_PARSER_WARNINGS=1`
- Optional: `## Requested Information` only when an additional request is
  explicitly provided.
- Include file/line(/col) when present
- No advice, fixes, or extra commentary

## Notes

- Avoid long-lived interactive/watch commands.
- Prefer explicit `command=[...]` arrays over shell-joined strings.
- Command resolution is **non-login/non-interactive** by default. The tool does
  not source shell startup files (`.zshrc`, `.bashrc`, etc.) before running
  `command=[...]`.
- `command=[...]` relies on the MCP server process environment for `PATH`. If a
  toolchain binary (for example `cargo`) is not on that `PATH`, you may see:
  `[Errno 2] No such file or directory: '<binary>'`.
- Approved remediation for missing toolchain binaries:
  - Use an absolute executable path in `command=[...]` when known (for example
    `/Users/<user>/.cargo/bin/cargo`).
  - Or run via a shell bootstrap wrapper when environment init is required, e.g.
    `command=["bash","-lc","source ~/.cargo/env && cargo check -p api"]`.
  - Use bootstrap wrapper only when necessary; prefer direct array execution for
    deterministic behavior.
