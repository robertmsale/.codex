---
name: command-parser
description: Run noisy command parsing via `launch-job ~/.codex/skills/command-parser/scripts/command-parser ...`. Keep command execution on the launch-job workflow. [skill-hash:4a9825d]
---

# Command Parser

## Purpose

Use this skill when a command is noisy (high-volume build/test/lint/tool output)
and you only need compact extraction.

Do not use this for simple commands.

## Command

Script path:
- `~/.codex/skills/command-parser/scripts/command-parser`

Build image script:
- `~/.codex/skills/command-parser/scripts/build-command-parser-image`

## Execution Model

1. Runs the target command directly in the caller environment.
2. Captures command stdout/stderr to `output.log`.
3. Runs `codex exec --json` inside Docker to parse `output.log`.
4. Prints only the final parser message (not full event noise).
5. Exits with the original command exit code.

Sandbox/approval behavior for the target command is whatever the calling agent
already has. This skill does not add custom sandbox bypass logic.

## Usage

Default:
- `launch-job ~/.codex/skills/command-parser/scripts/command-parser <command...>`

With warnings:
- `launch-job ~/.codex/skills/command-parser/scripts/command-parser --warnings <command...>`

With additional analysis request:
- `launch-job ~/.codex/skills/command-parser/scripts/command-parser --request-additional "<analysis request>" <command...>`

Recommended default:
- Leave `--request-additional` empty.
- Enable `--warnings` only when the user or task explicitly calls for it.

## Guardrails

- `--request-additional` is analysis-only.
- The parser cannot run commands, rerun commands, retry commands, or inspect anything outside captured files.
- If the request asks parser to run commands, it must return:
  `I cannot run commands, do not ask me again.`
- Simple commands (for example `ls`, `rg`, `echo`, `cargo fmt`) should be run with `launch-job` directly, not via command-parser.

## Output Contract

- Success with no errors: `No errors!`
- Failure or issues:
  - `## Errors` with concise bullets and file/line when available
  - optional `## Warnings` when warnings are requested
- Optional `## Requested Information` appears only when `--request-additional` is provided.
- Output should be concise and extraction-only (no remediation plans or extra chatter).

## Operator Config

Managed via `~/.codex/skills/command-parser/.env`:
- `COMMAND_PARSER_IMAGE` (default `command-parser:0.110.0`)
- `COMMAND_PARSER_CODEX_VERSION` (default `0.110.0`)
- `COMMAND_PARSER_PROFILE`
- `COMMAND_PARSER_WARNINGS`
- `COMMAND_PARSER_ADDITIONAL_REQUEST`

Agents should not edit operator config unless explicitly instructed by the user.
