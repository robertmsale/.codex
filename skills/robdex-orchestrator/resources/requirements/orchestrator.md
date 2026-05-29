# Requirements For Orchestrators

Use Requirements when task constraints must become an explicit completion contract rather than prompt prose.

Requirements preserve the operator-approved outcome. Worker recommendations are advisory evidence only; do not convert a worker's reduced scope, alternate implementation, documentation-only compromise, or "small first step" into the Requirements contract unless the operator explicitly authorizes that change.

## Normal Worker Flow

- Spawn workers without Requirements when the first turn is discovery, triage, planning, or pre-implementation.
- When the worker stops at pre-implementation, compare their plan against the operator-approved outcome and reject drift before setting Requirements.
- Convert the operator-approved outcome for the full assigned work package into Requirements. Use the worker plan only to identify implementation steps, dependencies, validation evidence, and missing owner decisions.
- Attach Requirements while the worker is idle with `robdex set-requirements --name "<agent name>" --requirements-file <file>`, before sending the execution prompt.
- Then send the implementation prompt. The next turn will be requirements-gated from the start.

Do not try to attach Requirements to a running turn. Requirements apply to `turn/start`; they cannot change the schema of an already-running turn or a mid-turn steer.

If a target worker is already running and you must replace its Requirements, use `--interrupt`. For prose-generated Requirements, use `robdex requirements-from-prose --title "<title>" --text-stdin --interrupt --name "<agent name>"`. For an existing file, use `robdex set-requirements --name "<agent name>" --requirements-file <file> --interrupt`. This interrupts the target, sets the new Requirements, and sends `Requirements updated` so the target resumes under the new contract.

Large work is handled by dependency-ordered fan-out, not micro-slice Requirements. If the operator's requested outcome is too large or cross-cutting for one worker, create complete work packages by responsibility boundary, such as contracts, backend implementation, frontend integration, design/system polish, and QA validation. Each package's Requirements must cover that package's full responsibility and map back to the top-level operator outcome.

Do not create Requirements for only the easiest first step, a partial pattern, or a documentation placeholder unless the operator requested that narrowed outcome. Scope changes require proof of impossibility, internal conflict, unsafe work, or a missing owner decision, followed by explicit operator authorization.

## From Prose

`requirements-from-prose` converts each non-empty bullet, numbered item, or line into one requirement. Put one complete requirement per line.

Good prose input:

```text
- Preserve existing CLI behavior unless this task explicitly changes it.
- Add tests proving the new parser flag is accepted and old syntax still works.
- Update active docs that discuss this command so users see the new behavior.
```

Avoid paragraph blobs where multiple requirements are packed into one line. The parser will treat that as one large requirement.

Preview generated Requirements:

```bash
robdex requirements-from-prose --title "<title>" --text-stdin <<'EOF'
- Requirement one.
- Requirement two.
EOF
```

Attach to an idle worker:

```bash
robdex requirements-from-prose --title "<title>" --include-composable non-negotiables --text-stdin --attach --name "<agent name>" <<'EOF'
- Requirement one.
- Requirement two.
EOF
```

Replace Requirements on a running worker:

```bash
robdex requirements-from-prose --title "<title>" --include-composable no-legacy --text-stdin --interrupt --name "<agent name>" <<'EOF'
- Requirement one.
- Requirement two.
EOF
```

Never run `robdex requirements-from-prose ... --text-stdin` without a heredoc, pipe, or redirected file attached.

## Composables

- Use `robdex requirements-composables list` to discover reusable global and recipient-project composables before drafting Requirements.
- Use `robdex requirements-composables show <id>` to inspect exact requirement text before selecting a composable.
- Project-specific composables are resolved from the recipient agent's tracked project, not the sender's project.
- Select composables only when they are relevant to the assigned work package.
- Permanent project composables are server-enforced and marked in composable listing output.
- Composables supplement task-specific Requirements; they must not replace, narrow, or drift from the operator-approved outcome.
- For prose-generated Requirements, use `robdex requirements-from-prose --include-composable <id>` so preview and attach both include the selected composables.
- For existing RequirementSet JSON files, use `robdex requirements-compose` or `set-requirements --include-composable <id>` to attach a composed set through the sanctioned Requirements route.

## JSON Requirement Files

The requirements file is JSON. It may be either an array of requirement objects or an object with a `requirements` array. Use semantic keys, not numbered keys.

```json
{
  "requirements": [
    {
      "key": "nativeGuiIsSourceOfTruth",
      "statement": "The web GUI must mirror the native Flutter GUI. Native chat timeline, composer, controls, and density are source of truth.",
      "severity": "blocker",
      "verificationMethod": "diffReview"
    }
  ]
}
```

## Review Lifecycle

- Requirements reviews are system-managed. Do not ask workers, QA, designers, operators, or orchestrators to manually request a Requirements review.
- A failed review routes the failed requirements back to the source agent.
- After a partial failure, the source agent's next claim schema may include only currently unresolved requirements. Previously passed requirements remain binding and reviewers still check the full canonical set.
- Reviewers should keep evidence brief for unrelated requirements or requirements that are repeatedly passing because nothing relevant changed.
- An accepted true blocker routes to the owner/orchestrator.
- A passing review clears the active Requirements and detaches or archives the reviewer so future Requirements get a fresh reviewer.
