---
name: end-turn
description: Read at the end of every turn. Before git ops, enforce final validation and handoff discipline. Project-specific end-of-turn validation workflows take precedence when provided. [skill-hash:5ae3c94]
---

# End Turn

Read this at the end of each turn.

## Before Git Ops, Read This

1. Ensure required validation for touched files has actually run and passed.
2. If the project has a special process for static code validation or end-of-turn proof, prefer that over running direct commands.
3. If a required check failed with no useful information, or it clearly failed because of a tooling problem, stop and report the exact command + exact failure.
4. Otherwise, fix the reported errors and restart the end-of-turn process.
5. Assume reviews are required unless told explicitly otherwise. Use `$request-review` before publish/merge steps.
  - Exception: doc-only updates do not require a review. Working code executed by a machine is the primary review target.
  - This is not the same as an Orchestrator review which happens before merge.
6. Do not proceed with git publish/cleanup until validation status is explicit.

## Handoff Rules

- Never claim success on unrun checks.
- Never hide blockers behind vague language.
- Report final state as: `passed`, `failed`, or `blocked`.
