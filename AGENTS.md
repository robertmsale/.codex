## Global Rules

### Noisy Command Output (Required)

- MUST USE the [$command-parser](~/.codex/skills/command-parser/SKILL.md) skill
  for noisy commands.
- Do not run high-volume build/lint/test commands directly when parser coverage
  is available.
- DO NOT RUN CODE FORMATTING TOOLS.

## Skill Rules

### When to read a SKILL.md

Read when all of the following conditions are met:

- The action you are about to take or think about taking matches a skill
  description with reasonable likelihood
- When you do not have `cat /path/to/SKILL.md` in your working memory (you
  didn't run the command and you do not see the results verbatim in chat
  history)

If any of those conditions are not met, only read the skill file if the user
explicitly asks you to.

### When not to read a SKILL.md

- At the beginning, middle, or end of every turn
- You've already read it before and know how to execute the skill

### Skill hash invalidation

- Skills may include a short hash token in the `description:` field:
  `[skill-hash:xxxxxxx]`.
- Treat `(skill path, skill-hash)` as the cache key for prior skill reads.
- If a skill match is likely and you have not read that exact hash yet in this
  session, read the SKILL once.
- If the hash matches what you already read, do not re-read unless the user
  explicitly asks you to.
- If a skill has no hash token, fall back to the standard "When to read a
  SKILL.md" rules.

### MCP & Skill Problems

Workflows are mandatory, especially in regards to skills and MCP tools. If you
run into issues with MCP tools that are not input errors, please stop and notify
the user so they can fix your tooling. If the robdexOrchestrator MCP tooling is
available, contact your project's orchestrator, who can pass any messages along
to the Config orchestrator, then stop until you receive confirmation the tooling
is fixed. This includes tool input params, and output params (truncated to
remove noise)
