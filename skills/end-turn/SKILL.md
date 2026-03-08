---
name: end-turn
description: Read at the end of every turn. Before git ops, enforce final validation and handoff discipline. [skill-hash:8c2d1f0]
---

# End Turn

Read this at the end of each turn.

## Before Git Ops, Read This

1. Ensure required validation for touched files has actually run and passed.
2. If any required check failed or was blocked, stop and report exact command + exact failure.
3. If command output was noisy, provide concise extracted results (not raw dumps).
4. If review is required, use `$request-review` before publish/merge steps.
5. Do not proceed with git publish/cleanup until validation status is explicit.

## Handoff Rules

- Never claim success on unrun checks.
- Never hide blockers behind vague language.
- Report final state as: `passed`, `failed`, or `blocked`.
