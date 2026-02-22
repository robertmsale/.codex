---
name: command-parser
description: Run command execution through MCP `commandParser.command_parser_run`, enforcing sandbox policy and returning compact parser output (or sandbox failure text) without wrapper-script workflow. [skill-hash:d4f31a2]
---

# Command Parser

## Overview
Use MCP tool `commandParser.command_parser_run` for noisy command execution and log extraction.
Do not use the legacy shell wrapper as the default path.

## Quick Start
- `commandParser.command_parser_run(command=[...])`
- `commandParser.command_parser_run(command=[...], include_warnings=true)`
- `commandParser.command_parser_run(command=[...], additional_request="...")`

Recommended default:
- Do **not** request additional information unless absolutely necessary.
- Keep default compact extraction for speed and consistency.

## Configuration

Configuration is managed by the user. Do not change configuration.

## MCP Behavior
- Tool executes the command once under resolved sandbox policy.
- If sandbox blocks execution, tool returns sandbox failure text directly.
- If command runs, tool returns parser extraction output.
- Tool output is plain text to minimize token usage.

## Output Expectations
- If no errors: `No errors!`
- Otherwise:
  - `## Errors` with one bullet per distinct error
  - optional `## Warnings` when `COMMAND_PARSER_WARNINGS=1`
- Optional: `## Requested Information` only when an additional request is explicitly provided.
- Include file/line(/col) when present
- No advice, fixes, or extra commentary

## Notes
- Avoid long-lived interactive/watch commands.
- Prefer explicit `command=[...]` arrays over shell-joined strings.
