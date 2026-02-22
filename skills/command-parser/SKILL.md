---
name: command-parser
description: Summarize large CLI output via a temp workspace + `codex exec` to extract errors (and optional warnings) with file/line anchors. [skill-hash:8ad3984]
---

# Command Parser

## Overview
Run noisy commands through `~/.codex/skills/command-parser/scripts/command-parser`.

The wrapper:
1. runs your command once,
2. writes logs into a temporary folder,
3. runs `codex exec` in that folder with read-only sandbox,
4. prints a compact summary from `response.log`,
5. removes the temp folder.

This keeps parser context focused on command output, even for very large logs.

## Quick Start
- `~/.codex/skills/command-parser/scripts/command-parser <command...>`
- `COMMAND_PARSER_WARNINGS=1 ~/.codex/skills/command-parser/scripts/command-parser <command...>`
- `~/.codex/skills/command-parser/scripts/command-parser --request-additional "What stack trace line caused failure?" <command...>`

Recommended default:
- Do **not** request additional information unless absolutely necessary.
- Keep default compact extraction for speed and consistency.

## Configuration

Configuration is managed by the user. Do not change configuration.

## Speed Signals

- "fast": active profile is expected to produce highly accurate results within seconds of the real command completion
- "medium": active profile is a slower but highly capable agent that produces results faster than the command takes to execute
- "slow": active profile uses local inference. Results may take longer than original command invocation time

## Progress Signals

- "=": Command executing now
- "=!": Agent actively parsing command

## codex exec invocation
The wrapper uses:
- `-p <COMMAND_PARSER_PROFILE>`
- `-s read-only`
- `--skip-git-repo-check`
- `-o <tmp>/response.log`
- stdout/stderr redirected to `/dev/null`

## Output Expectations
- If no errors: `No errors!`
- Otherwise:
  - `## Errors` with one bullet per distinct error
  - optional `## Warnings` when `COMMAND_PARSER_WARNINGS=1`
- Optional: `## Requested Information` only when an additional request is explicitly provided.
- Include file/line(/col) when present
- No advice, fixes, or extra commentary

## Notes
- Avoid wrapping long-lived interactive/watch commands.
- For failed wrapped commands, the wrapper returns the wrapped command's exit code after printing the parsed summary.
