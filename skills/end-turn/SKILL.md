---
name: end-turn
description: Read at the end of every turn. Before git ops, enforce final validation and handoff discipline. First run `scripts/end-turn-role-instructions` so the current thread loads only its worker or orchestrator guidance. Project-specific end-of-turn validation workflows take precedence when provided. [skill-hash:b4e3d62]
---

# End Turn

Read this at the end of each turn.

## Required Path

- Run: `~/.codex/skills/end-turn/scripts/end-turn-role-instructions`
- Load the worker or orchestrator instructions for this turn before proceeding.

## Shared Guardrails

- Follow project-specific end-of-turn validation workflows when the project provides them.
- Do not proceed with git publish/cleanup until validation and review status are explicit.
