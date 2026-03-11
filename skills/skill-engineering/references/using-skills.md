# Using Skills

Use skills with progressive disclosure. Read only what is needed to execute the current task.

## Trigger Rules

- Use a skill when the user explicitly names it or when the task clearly matches its description.
- Do not automatically read every skill at the beginning, middle, or end of a turn.

## When To Read A Skill

Read a `SKILL.md` only when both are true:
- the skill is likely relevant to the action you are about to take
- you do not already have that exact skill contents in working memory

If the skill description includes a `[skill-hash:xxxxxxx]` token:
- treat `(skill path, skill-hash)` as the cache key
- if the hash matches what you already read this session, do not re-read it unless the user explicitly asks

## Context Discipline

- Prefer the smallest set of skills that covers the task.
- Read only the specific referenced files you need.
- Prefer scripts, templates, and reference files over rewriting the same instructions from scratch.
- Keep context small. Do not duplicate long instructions that already live in the skill.

## Tooling Discipline

- When a skill requires a script-first workflow, use the script path the skill specifies.
- Do not substitute worktree-local wrappers for shared `~/.codex` wrappers unless the skill explicitly says to.
- If a required skill script or MCP tool fails in a non-input way, stop and escalate instead of improvising around the failure.
