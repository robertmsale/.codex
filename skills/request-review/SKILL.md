---
name: request-review
description: Use `request-review` for review-gated work. First run `request-review-role-instructions` so the current thread loads only the worker or orchestrator guidance it actually needs. [skill-hash:91e3b8c]
---

# Request Review

Use this skill when review is part of the current workflow.
Role-specific guidance lives in the matching role file.

## Required Path

- Run: `request-review-role-instructions`
- Run: `request-review "<commit message>"`
- Everything else is role-specific. Load the current role instructions before proceeding.

## Guardrails

- Run the public `request-review` script directly.
- Let the wrapper wait for completion.
- Do not call MCP review tools or alternate legacy review commands.
- Review behavior is operator-controlled.
