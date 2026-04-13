---
name: command-parser
description: Commands with large outputs automatically return in a compact format. Use this skill when the output of a command is confusing. [skill-hash:162507d]
---

# Command Parser

If a command output is excessively noisy, the shell wrapper may compact it automatically. Use the `command-parser` wrapper when you want that behavior on demand.

## Output Contract

- Success with no errors: `No errors!`
- Failure or issues:
  - `## Errors` with concise bullets and file/line when available
  - optional `## Warnings` when warnings are requested
- Output includes a path to the complete stdout/err logs incase the information you needed from the command is not classified as an error or a warning.

## For Codex Config Operator

- Important files:
  - skill doc: `~/.codex/skills/command-parser/SKILL.md`
  - rule file: `~/.codex/scripts/command-parser.rule`
  - wrapper script: `command-parser`
  - operator config: `~/.codex/scripts/command-parser.env`
  - aux server: `~/.codex/backend/crates/codex-aux-http/src/main.rs`
- Profile changes are allowed only if you receive rate limit reports.
- In that case, you may switch to the other non-local profile in `~/.codex/scripts/command-parser.env`.
- Otherwise the profile stays on spark.
- Only the operator may change the delay setting. Do not touch it.
