# Authoring Skills

Write skills as operational instructions, not essays.
Teach the process. Do not narrate the machinery behind it unless that detail is required to execute the process correctly.

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
- `SKILL.md` should dispatch the agent into the workflow, not act as a full system reference.
- Include only commands and rules the caller actually needs at that level.
- Put detailed background material in `references/` instead of bloating `SKILL.md`.
- If the skill behavior changes materially, update the skill hash in the description. This includes changes to /resources containing relevant skill instructions.

## Process Over Implementation

- Document the intended workflow, not the hidden implementation.
- Prefer "run this tool/script" over explaining how the tool is wired internally.
- Do not include internal env vars, transport details, auth wiring, storage layout, or other plumbing unless the agent truly needs that fact to follow the workflow.
- Do not document implementation details that invite agent-side override behavior or identity spoofing.
- If the tooling is the source of truth, say that plainly and stop there.
- If the tooling is broken, instruct the agent to report the tooling failure rather than improvise around it.
- Do not teach the agent how to bypass the public tool surface.
- Do not under *any circumstances* introduce timeouts in scripts. Let agents hang in perpetuity if they run a script and are stuck in `command_execution_wait`. Lost time for hanging commands can be recovered easily. An agent experiencing a timeout, and spending countless hours destroying a codebase with reckless abandon to fix a problem that doesn't exist is the undesired and forbidden path.
- Introducing delays in scripts is OK, especially if the operator has control of it in a `.env` file.

## Writing Style

- Prefer hard requirements when the workflow is fragile.
- Prefer concise selection guidance when multiple approaches are valid.
- Assume the agent is already capable; include only the context needed to follow the workflow reliably.
- Avoid redundant operator detail that does not improve execution.
- Avoid "interesting but unnecessary" details. If a fact does not change what the agent should do, cut it.

## Design Pattern

- Keep `SKILL.md` as the dispatcher.
- Put deep details in one-level-deep references.
- Prefer scripts when reliability matters more than prose.
- Reuse existing patterns from nearby skills instead of inventing a new shape without reason.
- Keep role-specific capability and policy in role-specific resources rather than bloating the shared entry file.
- Optimize command lists for purpose. Show the smallest command surface that helps the agent accomplish the task.

## Validation

- Read back the edited skill to ensure the wording is crisp and the trigger description still matches the real use case.
- If the skill references scripts or resources, make sure the paths actually exist.
- Do not claim a workflow exists unless the supporting script, MCP tool, or reference file really exists.
- Check for leaked implementation detail. If the text teaches how the tool works internally more than it teaches what to do, tighten it.
- Check for agent-side override hints. If the text suggests a caller could swap identity, config authority, or execution mode, remove that detail unless the workflow explicitly requires it.
