# Advanced Robdex Orchestration

This guide describes the public orchestration model without assuming Robert's
private projects or live `config.toml`.

## Core Roles

- **Operator** owns the local control plane, bootstrap tooling, and cross-project
  policy. Operators should keep live config and state changes deliberate.
- **Orchestrator** plans and coordinates work. Orchestrators spawn workers,
  attach Requirements, route review outcomes, and keep project context aligned.
- **Worker** implements scoped changes. Workers should inspect first, make
  targeted patches, validate, and report exact evidence.
- **QA** validates behavior and reports defects. QA should not implement fixes
  unless explicitly reassigned.
- **Designer** handles design-oriented UI work, screenshots, visual inspection,
  and interaction review.
- **Requirements reviewer** performs adversarial review of active Requirements.
  It should not implement fixes or relax the contract.

Do not collapse these roles. Shared rules belong in global docs; role-specific
responsibilities belong in role files; task-specific techniques belong in
skills.

## Skills

Skills package reusable local workflows. Public bootstrap should expose only the
skills needed for the selected profile:

- `robdex-orchestrator`: Robdex communication and lifecycle commands.
- `request-review`: review-gated development workflow.
- `privileged-exec`: sanctioned handling for approval or sandbox issues.
- optional project skills, such as design or GitHub workflow helpers.

Skill scripts should be available by basename on `PATH` after activation or
staging.

## Requirements

Requirements turn task constraints into a completion contract. A typical flow:

1. Worker does discovery or planning without Requirements.
2. Orchestrator converts the accepted plan into Requirements while the worker is
   idle.
3. The next worker turn starts with the Requirements schema.
4. Final claim packets use concise `summary` plus nested `requirements`.
5. Reviewer verdicts route failures back to the source, passes beyond the
   worker, accepted blockers to the owner/orchestrator, and human waiver cases
   to owner decision instead of loops.

Use Requirements for implementation gates, risky migrations, public bootstrap
changes, and review-sensitive claims.

## Request Review

Use request-review when a task needs an independent review but not a full
Requirements contract. Run role-specific review instructions first, keep review
scope explicit, and provide exact validation evidence.

## Simplified QA Runtime

The active QA model is intentionally lightweight:

- The orchestrator or owner assigns QA a normal worktree and a device UDID.
- QA remains a non-implementer even though it has a normal checkout.
- QA launches from that worktree with `designer-flutter-run`.
- QA pilots with `designer-drive`, captures evidence with `designer-drive
  screenshot`, and crops evidence with `designer-crop-screenshot` when useful.
- QA reports product, usability, environment, or tooling findings to the
  orchestrator with concrete repro steps.

The old managed QA harness, Flutter simulator broker, hidden runtime roots,
broker-owned source sync, and device lease flow are legacy/deprecated. They are
not required for normal Robdex orchestration and should not be the default path
for new QA work.

## Privileged Execution

Do not approve Robdex command execution directly. Use sanctioned wrappers and
the privileged-exec workflow. If a sanctioned plain command unexpectedly asks
for approval, report the exact command, cwd, and output; also verify the command
was not hidden inside shell redirection, substitution, or compound execution.

## Hooks

Hooks can integrate project lifecycle events such as worker creation, archive,
or metadata updates. Public setup should treat hooks as optional and project
owned. A failed hook should report exact exit status and output without mutating
global config.

## Safe Multi-Agent Workflow

- Keep the core path headless: bridge, CLI, Requirements, roles, and skills.
- Add GUI, device-driver, simulator, and design-lab tooling only when needed.
- Prefer scoped worker worktrees and exact validation commands.
- Preserve user-owned config and live Robdex state unless the owner requested a
  specific mutation.
- Keep reviewers independent: failed reviews require fixes and proof, not prose
  disagreement.
