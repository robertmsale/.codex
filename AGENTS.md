## Global Rules

### Noisy Command Output (Required)
- MUST USE the [$command-parser](~/.codex/skills/command-parser/SKILL.md) skill for noisy commands.
- Run noisy commands through:
  - `~/.codex/skills/command-parser/scripts/command-parser <command>`
- Do not run high-volume build/lint/test commands directly when parser coverage is available.

Examples:
- `~/.codex/skills/command-parser/scripts/command-parser cargo check --workspace`
- `~/.codex/skills/command-parser/scripts/command-parser cargo test --workspace`
- `~/.codex/skills/command-parser/scripts/command-parser flutter analyze`
- `~/.codex/skills/command-parser/scripts/command-parser pytest -q`
- `~/.codex/skills/command-parser/scripts/command-parser npm run build`
