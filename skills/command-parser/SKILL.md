---
name: command-parser
description: Use `~/.codex/skills/command-parser/scripts/command-parser ...` when a command is noisy and to receive a compact extraction instead of raw output. [skill-hash:f403e5b]
---

# Command Parser

Use this skill when a command is noisy and you want a compact extraction.
Do not use it for simple commands.

## Command

Script path:
- `~/.codex/skills/command-parser/scripts/command-parser`

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

Runtime notes:
- On Linux, the wrapper automatically uses a fresh rootful container per invocation with the real `CODEX_HOME` mounted writable and the parser itself running with `danger-full-access` inside the container.
- On macOS, the wrapper keeps the existing shadow/rootless path.
- Override only for debugging with `COMMAND_PARSER_RUNTIME_MODE=shadow-rootless` or `COMMAND_PARSER_RUNTIME_MODE=linux-rootful`.

## Guardrails

- `--request-additional` is analysis-only.
- Expected noisy parser targets such as `flutter test` and `flutter drive` remain valid command-parser targets.
- Parser-routed wrapper commands that explicitly require command-parser, such as `db_test.sh test ...` and `db_test.sh exec ...`, also remain valid command-parser targets even when the nested command is small.
- Simple commands (for example `ls`, `rg`, `echo`, `cargo fmt`) should be run directly, not via command-parser.
- If the tool rejects a command, treat that as the rule doing its job. Do not try to explain around the rejection or work around it.
- If the command-parser tool itself is broken, report the tooling bug instead of bypassing the workflow.

## Output Contract

- Success with no errors: `No errors!`
- Failure or issues:
  - `## Errors` with concise bullets and file/line when available
  - optional `## Warnings` when warnings are requested
- Optional `## Requested Information` appears only when `--request-additional` is provided.
- Output is extraction-only.

## For Codex Config Orchestrator

- Important files:
  - skill doc: `~/.codex/skills/command-parser/SKILL.md`
  - rule file: `~/.codex/skills/command-parser/command-parser.rule`
  - wrapper script: `~/.codex/skills/command-parser/scripts/command-parser`
  - build script: `~/.codex/skills/command-parser/scripts/build-command-parser-image`
  - operator config: `~/.codex/skills/command-parser/.env`
  - MCP server: `~/.codex/mcp/command-parser/src/command_parser_mcp/server.py`
- Profile changes are allowed only if you receive rate limit reports.
- In that case, you may switch to the other non-local profile in `~/.codex/skills/command-parser/.env`.
- Otherwise the profile stays on spark.
- Only the operator may change the delay setting. Do not touch it.
