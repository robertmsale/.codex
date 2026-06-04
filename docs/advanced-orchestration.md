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
- `privileged-exec`: sanctioned handling for approval or sandbox issues.
- optional project skills, such as design or GitHub workflow helpers.

Skill scripts should be available by basename on `PATH` after activation or
staging.

## Requirements

Requirements turn task constraints into a completion contract. A typical flow:

1. Worker does discovery or planning without Requirements.
2. Orchestrator converts the accepted plan into Requirements while the worker is
   idle, using `robdex set-requirements --name "<agent name>"
   --requirements-file <file>`.
3. The next worker turn starts with the Requirements schema.
4. Final claim packets use concise `summary` plus nested `requirements`.
5. Reviewer verdicts route failures back to the source, passes beyond the
   worker, accepted blockers to the owner/orchestrator, and human waiver cases
   to owner decision instead of loops.

Use Requirements for implementation gates, risky migrations, public bootstrap
changes, and review-sensitive claims.

Composable Requirements let an orchestrator merge reusable global or
project-specific constraints into task-specific Requirements. Inspect available
options before attaching them:

```bash
robdex requirements-composables list --name "<agent name>"
robdex requirements-composables show no-legacy --name "<agent name>"
```

For prose-generated Requirements, include composables directly in the generation
command so preview and attachment use the same contract:

```bash
robdex requirements-from-prose --title "<title>" --include-composable non-negotiables --include-composable no-legacy --text-stdin <<'EOF'
Task-specific requirement prose.
EOF
```

Add `--attach --name "<agent name>"` to attach the composed RequirementSet
atomically.

Project settings can mark composables as permanent. Permanent composables are
server-enforced for the recipient project: the bridge merges them into every
Requirements set/update for agents in that project even when the GUI, CLI, or
orchestrator omits them. Composable list/show output marks these as permanent so
operators and orchestrators can distinguish project policy from optional packs.
For optional composables that are not permanent, include them explicitly with
`--include-composable`.

Both `requirements-from-prose` and `set-requirements` have explicit sequencing
flags for non-idle cases:

- Use `--interrupt` when the target worker is already running and the
  orchestrator must replace Requirements immediately. The CLI interrupts the
  target, sets Requirements on that same target, then sends `Requirements
  updated`.
- Use `--to-self` only when setting Requirements on the current orchestrator or
  operator thread. The CLI sets Requirements on self, waits briefly, interrupts
  self, then sends `Begin` so the next turn starts under the new contract.
- Without `--to-self`, attaching Requirements requires an explicit target such
  as `--name` or `--to-thread-id`.
- On `requirements-from-prose`, `--attach`, `--interrupt`, and `--to-self` are
  mutually exclusive apply modes. With none of those flags, the command previews
  JSON only.
- On `set-requirements`, `--interrupt` and `--to-self` are mutually exclusive.

Examples:

```bash
robdex requirements-from-prose --title "<title>" --include-composable no-legacy --text-stdin --interrupt --name "<agent name>" <<'EOF'
Task-specific requirement prose.
EOF

robdex requirements-from-prose --title "<title>" --include-composable no-legacy --text-stdin --to-self <<'EOF'
Task-specific requirement prose.
EOF
```

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
