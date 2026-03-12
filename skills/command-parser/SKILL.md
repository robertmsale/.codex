---
name: command-parser
description: Run noisy command parsing via `~/.codex/skills/command-parser/scripts/command-parser ...`. It enforces `command-parser.rule`, runs the command directly, captures `output.log`, and returns only the parser's final extraction. Temporary parser artifacts are removed on exit. [skill-hash:3d4b7a1]
---

# Command Parser

## Purpose

Use this skill when a command is noisy and you only need compact extraction.

Do not use this for simple commands.

## Command

Script path:
- `~/.codex/skills/command-parser/scripts/command-parser`

Build image script:
- `~/.codex/skills/command-parser/scripts/build-command-parser-image`

## Execution Model

1. Enforces `command-parser.rule` with `codex execpolicy check`.
2. Runs the target command directly in the caller environment.
   - The wrapped command receives `IS_USING_COMMAND_PARSER=true`.
3. Captures command stdout/stderr to `output.log`.
4. Runs `codex exec --json` inside Docker to parse `output.log`.
5. Prints only the parser's final message.
6. Exits with the original command exit code.

This skill does not mutate the workspace or add command-specific behavior.
Callers should not need to prefix `IS_USING_COMMAND_PARSER=true` manually.

## Usage

Default:
- `~/.codex/skills/command-parser/scripts/command-parser <command...>`

With warnings:
- `~/.codex/skills/command-parser/scripts/command-parser --warnings <command...>`

With additional analysis request:
- `~/.codex/skills/command-parser/scripts/command-parser --request-additional "<analysis request>" <command...>`

Recommended default:
- Leave `--request-additional` empty.
- Enable `--warnings` only when the user or task explicitly calls for it.

## Guardrails

- `--request-additional` is analysis-only.
- The parser cannot run commands, rerun commands, retry commands, or inspect anything outside captured files.
- Expected noisy parser targets such as `flutter test` and `flutter drive` remain valid command-parser targets.
- Parser-routed wrapper commands that explicitly require command-parser, such as `db_test.sh test ...` and `db_test.sh exec ...`, also remain valid command-parser targets even when the nested command is small.
- If the request asks parser to run commands, it must return:
  `I cannot run commands, do not ask me again.`
- Simple commands (for example `ls`, `rg`, `echo`, `cargo fmt`) should be run directly, not via command-parser.

## Output Contract

- Success with no errors: `No errors!`
- Failure or issues:
  - `## Errors` with concise bullets and file/line when available
  - optional `## Warnings` when warnings are requested
- Optional `## Requested Information` appears only when `--request-additional` is provided.
- Output is extraction-only.

## Operator Config

Managed via `~/.codex/skills/command-parser/.env`:
- `COMMAND_PARSER_IMAGE` (default `command-parser:0.110.0`)
- `COMMAND_PARSER_CODEX_VERSION` (default `0.110.0`)
- `COMMAND_PARSER_PROFILE`
- `COMMAND_PARSER_WARNINGS`
- `COMMAND_PARSER_ADDITIONAL_REQUEST`

Agents should not edit operator config unless explicitly instructed by the user.
