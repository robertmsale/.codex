---
name: skill-engineering
description: Read this before using or modifying any skills. It covers efficient skill loading, skill authoring, mandatory workflow discipline, and how to report skill/tooling failures correctly. [skill-hash:5f1c2a8]
---

# Skill Engineering

Use this skill before:
- using a skill you are not already holding in working memory
- creating a new skill
- modifying an existing skill

## Required Workflow

- Follow the workflow required by the triggered skill instead of inventing an ad-hoc replacement.
- If a required skill script or MCP tool fails in a non-input way, stop and report the tooling failure.
- If Robdex orchestration is available, contact the project's orchestrator with the failing tool or script invocation and truncated output, then pause until tooling is fixed.

## Read Path

- For normal skill usage, read [`references/using-skills.md`](references/using-skills.md).
- For creating or updating a skill, also read [`references/authoring-skills.md`](references/authoring-skills.md).
- Do not bulk-load both references unless both tasks actually apply.
