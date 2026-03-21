---
name: request-review
description: Use `scripts/request-review` for review-gated work. First run `scripts/request-review-role-instructions` so the current thread loads only the worker or orchestrator guidance it actually needs. MUST USE $command-execution SKILL WITH THIS PROCESS. [skill-hash:91e3b8c]
---

# Request Review

Use this skill when review is part of the current workflow.
Role-specific guidance lives in the matching role file.

## Required Path

- Run: `~/.codex/skills/request-review/scripts/request-review-role-instructions`
- Run: `~/.codex/skills/request-review/scripts/request-review "<commit message>"`
- Use the shared `~/.codex` script path shown here. Do not rewrite it to a worktree-local `.codex/...` path unless a project-local skill explicitly requires a repo-local wrapper.
- Everything else is role-specific. Load the current role instructions before proceeding.

## Guardrails

- Request-review must run through command-execution.
- Do not poll stdin.
- Do not kill the review because it is taking a long time.
- Do not call MCP review tools or alternate legacy review commands.
- Review behavior is operator-controlled.
