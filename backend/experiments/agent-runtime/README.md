# Experimental Agent Runtime

This nested workspace is isolated from stable Robdex. It is not a member of the
main backend workspace and is not wired into supervisor, the stable bridge, the
stable database, or the GUI.

## Database

The runtime requires host PostgreSQL. Configure the connection with:

```sh
export ROBDEX_AGENT_RUNTIME_DATABASE_URL='postgres://postgres:postgres@127.0.0.1:5432/robdex_agent_runtime'
```

Initialize the schema:

```sh
robdex-agent-runtime init-db
```

Use local host Postgres for all experimental runtime state.

## Model adapter boundary

Runtime code depends on the local `ModelClient` trait. The current adapter is
`model::codex_adapter::CodexBackedModelClient`, an experimental direct
Responses adapter that performs HTTPS calls to the ChatGPT Codex Responses
endpoint with Codex auth material from the local environment or `auth.json`.
It does not claim to use the full vendored Codex provider/client runtime. All
direct HTTP, Responses request shaping, Codex auth headers, SSE parsing, and raw
model response handling must stay inside `model::codex_adapter`.

## Script output

Starlark scripts emit final tool output only through:

```python
output(value)
```

Host API calls return values to the script, but they do not implicitly append to
the final tool output. This keeps tool result packets deterministic and concise.

## Typed command registry

Postgres is the runtime source of truth for concrete `cmd[...]` commands. Rust
owns finite kernel/native action categories and enforcement semantics; concrete
command definitions and immutable command versions live in `command_definitions`
and `command_versions`. Seed files under `command-seeds/` are bootstrap/import
material only. `init-db` imports the bundled seed commands only when the command
registry is empty. After the registry exists, `init-db` only applies schema
migrations; it does not overwrite, re-enable, or repoint current command
definitions. Live registry changes must use explicit command-registry requests.
Command definitions are scoped. `global` commands are visible to subsequent
sessions in every project. `project` commands are visible only to sessions whose
stored `project_key` matches the command scope. Session creation accepts
`new-session --project <key>` and stores that project key so every
`execute_code` boundary can resolve visible commands deterministically.

The bundled seed import creates:

- `cmd["rg"].run(args=[...], cwd=".")`
- `cmd["git"].status()`
- `cmd["git"].diff(args=[...])`
- `cmd["cargo"].check(args=[...])`

Every command version stores the action id, binary name and resolution
candidates, Starlark object/method surface, argv prefix/argument policy,
cwd/env/timeout/output policy, `mutationClass` metadata, model-facing
description, the approver-selected final execution policy, and creation metadata.
`mutationClass` is descriptive trace/model metadata in this phase; it is not an
execution policy boundary. Scoped command execution authority comes from the
stored final execution policy, registry argv/cwd/env/timeout/output fields, and
native kernel protections. `execute_code` queries the enabled current DB command
versions at every tool boundary, merges global plus matching project commands,
rejects ambiguous action identifier conflicts before surfacing commands, builds the Starlark `cmd`
surface from that live registry, and generates
the model-visible `execute_code` contract from the same live rows. Agents receive
the current interface directly in the tool schema and prompt; they are not told
to read README, manifests, or source files to understand command semantics.

Registry-defined command execution remains structured. There is no raw shell:
commands run only through argv arrays, execution-root cwd enforcement, explicit
env policy, timeout/output limits, binary resolution policy, and the stored
final execution policy selected by the approver. A final policy of `allow`
executes immediately, `deny` leaves the command visible but blocks before side
effects, and `ownerApproval` or `orchestratorApproval` creates the matching
approval request and paused action before side effects. Role policy does not
override scoped command final execution policy. Each `command_runs` row records the exact `command_version_id` used
so historical traces remain attributable after later registry changes.

Role policy remains authority for native kernel actions such as
`tool.execute_code`, `tool.request_command_registry_change`, and
`command_registry.*`. Scoped DB command visibility and execution do not require
role policy entries for command action ids.

The model has two native tools and must choose exactly one per turn:
`execute_code` for current Starlark execution, or
`request_command_registry_change` when the current registry lacks a needed
command. `request_command_registry_change` is a native model tool outside
Starlark; it is not a `cmd[...]` helper, raw shell, or execute_code workaround.
The request schema captures operation (`add`, `update`, `disable`, `enable`),
proposed command definition, rationale, intended use, current blocker or need,
and requester context. Requesters do not choose authoritative scope or execution
policy. Requester-provided policy text is advisory only.

Command-registry changes use structured requests in `command_registry_requests`.
A request stores the proposed command, requester context, role/session/turn
context, approval status, and application status. Approval records a decision
only. When approving, the approver must provide final scope (`global` or
`project` plus project key), final execution policy (`allow`, `deny`,
`ownerApproval`, or `orchestratorApproval`), and final command definition edits.
The separate `command-registry requests apply <id>` command re-validates those
approver-selected final values and performs the mutation. Denied requests,
unapproved requests, missing final scope/policy/command, conflicts, failed
validation, and failed apply attempts do not mutate the registry and do not mark
requests applied. Operations are strict: `add` fails if the scoped action exists;
`update`, `enable`, and `disable` fail if the scoped action does not exist;
`enable` and `disable` must change exactly one row.

CLI affordances:

```sh
robdex-agent-runtime command-registry list
robdex-agent-runtime command-registry show <action-id>
robdex-agent-runtime command-registry seed-requests --session <session-id> --mode missing|refresh
robdex-agent-runtime command-registry requests create --session <session-id> <json-file>
robdex-agent-runtime command-registry requests list
robdex-agent-runtime command-registry requests show <id>
robdex-agent-runtime command-registry requests decide --session <session-id> <id> --status denied
robdex-agent-runtime command-registry requests decide --session <session-id> <id> --status approved --final-scope global --final-policy allow --final-command-file <json-file>
robdex-agent-runtime command-registry requests decide --session <session-id> <id> --status approved --final-scope project --final-project <key> --final-policy allow --final-command-file <json-file>
robdex-agent-runtime command-registry requests apply --session <session-id> <id>
```

`seed-requests` is the explicit seed import/refresh affordance. It creates
ordinary registry requests from `command-seeds/`; it does not approve or apply
them. `--mode missing` requests only absent bundled commands. `--mode refresh`
requests updates for existing bundled commands and adds for absent bundled
commands.

## Role policy foundation

Postgres is the runtime source of truth for roles. JSON manifests and prompt files in `roles/` are seed/import/export artifacts only. Import resolves prompt files into immutable `role_versions.instruction_text`; runtime session creation reads the current DB role version and stores a complete immutable `sessions.role_snapshot`.

Active actions implemented by this slice:
- `tool.execute_code`
- `tool.request_command_registry_change`
- `fs.read`
- `fs.write`
- `patch.apply`
- `command_registry.request`
- `command_registry.decide`
- `command_registry.apply`

Concrete `cmd.*` actions are active when present as enabled current DB command
versions visible through global or matching project scope. Their stored final
execution policy controls allow, deny, and approval-required behavior.

Reserved future action names documented but not implemented here:
- `agent.spawn.<role>`
- `agent.archive`
- `requirements.set.self`
- `requirements.set.other`
- `requirements.change.active`
- `message.send`
- `message.route`

Manifest decision values are `allow`, `deny`, `ownerApproval`, and `orchestratorApproval`. Runtime policy maps approval decisions to `approvalRequired` and does not execute those actions in this task. Missing action policy defaults to deny. Policy is execution authority; `capabilities` are validated to exactly match policy keys so they cannot contradict enforcement. Sessions store immutable role snapshots at creation time; turns use the stored snapshot rather than rereading the latest manifest. The direct Responses adapter receives the model name and instruction text from the session snapshot. Reasoning effort is stored in the DB role version and snapshot but is not applied by the current direct adapter yet.

## Approval and routing foundation

Approval-required policy decisions are durable kernel objects. When a role policy uses `ownerApproval` or `orchestratorApproval`, runtime policy returns `approvalRequired`, records `policy.decision`, creates an `approval_requests` row with the required approver kind, records `approval.requested`, and blocks the action. The runtime does not auto-approve and does not resume blocked actions after a decision in this phase. CLI inspection and persistence are available through `approvals list`, `approvals show <id>`, and `approvals decide <id> --decision approved|denied --reason <text>`; decisions record `approval.decided` only.

Routing metadata is structured role data. The supported mode is `direct`, with `defaultRecipient` and `allowedRecipients`. Recipients may be reserved principals such as `owner` and `orchestrator` or DB-canonical role IDs. Import-time validation uses existing DB roles plus seed/import context, so newly imported role IDs can be referenced without Rust code changes. Route evaluation records `route.decision`; no multi-agent message delivery is implemented in this phase.

## Action-only approval resume

Approval resume is explicit and action-only. `approvals decide` only persists a decision and never executes the blocked action. `approvals resume <approval-id>` requires an approved request and a linked pending paused action. Resumable command actions are any DB registry command action with immutable stored input including `commandVersionId`; native resumable mutation actions are `fs.write` and `patch.apply`. Resume does not call the model, does not replay the script or turn, and does not rewrite the original failed turn. Resume records `approval.resume.started`, `policy.resumeDecision`, mutation/command evidence, and `approval.resume.completed` or `approval.resume.failed`.

## Validation database hygiene

Manual experiments use the normal runtime database configured by `ROBDEX_AGENT_RUNTIME_DATABASE_URL`, for example:

```sh
export ROBDEX_AGENT_RUNTIME_DATABASE_URL='postgres://postgres:postgres@127.0.0.1:5432/robdex_agent_runtime'
```

Validation scripts do not use that database by default. Scripts under `scripts/validate-*.sh` create a per-run isolated Postgres database whose name starts with `robdex_agent_runtime_validation_`, point `ROBDEX_AGENT_RUNTIME_DATABASE_URL` at that temporary database for the script process, and drop the temporary database on exit. Cleanup runs on success and failure.

Run validation scripts from the nested workspace:

```sh
scripts/validate-db-canonical-roles.sh
scripts/validate-approvals-routing.sh
scripts/validate-action-resume.sh
scripts/validate-mutation-actions.sh
scripts/validate-command-registry.sh
```

Validation database administration defaults to `ROBDEX_AGENT_RUNTIME_VALIDATION_ADMIN_DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/postgres`. Override that admin connection only when the same local Postgres server requires a different maintenance database. Do not point validation cleanup at the normal runtime database.

If cleanup fails, the script prints the leftover validation database name, admin connection, and exact manual cleanup SQL. Manual cleanup must only target names with the strict validation prefix:

```sql
DROP DATABASE IF EXISTS "robdex_agent_runtime_validation_<run_id>" WITH (FORCE);
```

The cleanup helper refuses destructive cleanup for database names that do not start with `robdex_agent_runtime_validation_`.
