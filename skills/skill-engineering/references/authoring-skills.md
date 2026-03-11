# Authoring Skills

Write skills as operational instructions, not essays.

## Structure

Every skill needs:
- `SKILL.md`

Optional resources:
- `scripts/` for deterministic or repetitive operations
- `references/` for detailed material that should only be loaded when needed
- `assets/` for templates or files used in outputs

Do not create extra docs like:
- `README.md`
- `CHANGELOG.md`
- `QUICK_REFERENCE.md`

## SKILL.md Guidance

- Keep frontmatter clear and triggerable:
  - `name`
  - `description`
- Make the description describe both what the skill is and when it should be used.
- Keep the body concise and procedural.
- Put detailed background material in `references/` instead of bloating `SKILL.md`.
- If the skill behavior changes materially, update the skill hash in the description.

## Writing Style

- Prefer hard requirements when the workflow is fragile.
- Prefer concise selection guidance when multiple approaches are valid.
- Assume the agent is already capable; include only the context needed to follow the workflow reliably.
- Avoid redundant operator detail that does not improve execution.

## Design Pattern

- Keep `SKILL.md` as the dispatcher.
- Put deep details in one-level-deep references.
- Prefer scripts when reliability matters more than prose.
- Reuse existing patterns from nearby skills instead of inventing a new shape without reason.

## Validation

- Read back the edited skill to ensure the wording is crisp and the trigger description still matches the real use case.
- If the skill references scripts or resources, make sure the paths actually exist.
- Do not claim a workflow exists unless the supporting script, MCP tool, or reference file really exists.
