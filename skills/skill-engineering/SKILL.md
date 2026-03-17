---
name: skill-engineering
description: Read this before using or modifying any skills. It covers skill selection, progressive loading, workflow discipline, and how to report skill/tooling failures correctly. [skill-hash:3bd6a71]
---

# Skill Engineering

Use this skill before:
- using a skill you are not already holding in working memory
- creating a new skill
- modifying an existing skill

## Required Workflow

- Use a skill when the user explicitly names it or when the task clearly matches its description.
- Do not automatically read every skill at the beginning, middle, or end of a turn.
- Read a `SKILL.md` only when it is likely relevant to the next action and you do not already have that exact contents in working memory.
- Treat `(skill path, skill-hash)` as the cache key. If the hash matches what you already read this session, do not re-read unless the user explicitly asks.
- Prefer the smallest set of skills that covers the task.
- Read only the specific referenced files you need.
- Follow the workflow required by the triggered skill instead of inventing an ad-hoc replacement.
- Prefer skill scripts, templates, and referenced resources over rewriting the workflow from scratch.
- When a skill requires a script-first workflow, use the script path the skill specifies.
- Do not substitute worktree-local wrappers for shared `~/.codex` wrappers unless the skill explicitly says to.
- Keep context small. Do not duplicate long instructions that already live in the skill.
- If a required skill script or MCP tool fails in a non-input way, stop and report the tooling failure.
- If Robdex orchestration is available, contact the project's orchestrator with the failing tool or script invocation and truncated output, then pause until tooling is fixed.

## Creating Or Updating Skills

- For creating or updating a skill, also read [`references/authoring-skills.md`](references/authoring-skills.md).
- Do not load authoring guidance unless you are actually creating or updating a skill.
