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

## Projection-first GUI/server state boundary

The shared GUI-facing state contract lives in the pure Rust crate
`crates/robdex-agent-runtime-projection`. That crate owns projection structs,
delta structs, and deterministic reducer logic for future server and Rust/Rinf
GUI use. It is intentionally limited to serde-compatible data and reducers. It
must not depend on SQLx, HTTP, WebSocket, runtime execution, Flutter, Rinf,
model adapters, or process-management code.

The top-level `RuntimeProjection` is the replacement shape for any future
full-state hydration. A server should hydrate a full snapshot first, then stream
ordered `RuntimeDelta` values. Both snapshots and deltas carry a monotonic
watermark derived from runtime event ordering. Reducers reject stale deltas,
apply repeated entity/event updates idempotently when ids or sequences make that
safe, preserve timeline ordering, and set explicit `resyncRequired` state when
they detect a gap or incompatible stream condition.

The projection includes server status, session list items, optional selected
session detail, timeline/event rows, pending approvals, role summaries, command
registry summaries, workflow memory summaries, and a top-level watermark.
Deltas cover session upsert/archive/close, selected-session replacement or
patch, timeline append, turn/tool/script/process status changes, approval
upsert/removal, role upsert/archive, command registry upsert/disable, workflow
memory updates/events, and explicit resync-required signaling.

SQL stays in the runtime crate. Runtime-side snapshot adapters live outside the
projection crate, under `robdex_agent_runtime::projection`, and map the current
Postgres schema into the shared projection types. The future VPN-protected
server should call those adapters to hydrate clients and should stream deltas
using the same projection crate types. A future Rinf GUI should use the same
reducer logic on the Rust side before sending already-reduced render state to
Dart.

## Script output

Starlark scripts emit final tool output only through:

```python
output(value)
```

Host API calls return values to the script, but they do not implicitly append to
the final tool output. This keeps tool result packets deterministic and concise.

## Session lifecycle

A session is the durable agent/thread record. PostgreSQL stores the role
snapshot, project key, workdir, optional worktree root, title/name metadata,
lifecycle state, lineage, turns, model events,
approvals, paused actions, and managed-process records. Runtime memory is
disposable; sends reconstruct context from PostgreSQL.

Canonical session commands:

```sh
robdex-agent-runtime sessions new --role <role-id> --project <key> --workdir <path> --worktree-root <path> --title <title> --name <name>
robdex-agent-runtime sessions list [--all]
robdex-agent-runtime sessions show <session-id>
robdex-agent-runtime sessions history <session-id>
robdex-agent-runtime sessions close <session-id> --reason <text>
robdex-agent-runtime sessions archive <session-id>
robdex-agent-runtime sessions fork <session-id> --at-turn <completed-turn-id>
```

The `worktree_root`, `title`, and `name` fields are explicit session metadata:
`worktree_root` records the optional owning worktree/root for audit and tooling,
`title` is user-visible display metadata, and `name` is a stable human-readable
operator label. `send` uses the stored session workdir and rejects sessions whose status is not
`open` before creating a turn. Archive is visibility-only: it sets
`tracked=false` and `archived_at` while preserving direct show/history access and
leaving rows in place. Close is terminal: it sets `status=closed`, records
`closed_at`, emits `session.closed`, rejects future sends, terminates any live
session process handles owned by this runtime, and marks remaining running
managed-process rows as `sessionClosed`.

Forking is legal only from a completed source turn. A fork creates a new open
session with inherited role snapshot, project key, workdir, and lineage fields.
Source rows are not copied. History reconstruction traverses lineage through the
fork boundary and then appends the fork session's own completed turns. Model
requests include reconstructed prior user/final-assistant history before the
current message.

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
stored `project_key` matches the command scope. Session creation uses `sessions new --project <key> --workdir <path>` and stores that project key and workdir so every send and `execute_code` boundary can resolve visible commands and execute from durable session state deterministically.

The bundled seed import creates:

- `cmd["rg"].run(args=[...], cwd=".").sync()` or `.start()`
- `cmd["git"].status().sync()` or `.start()`
- `cmd["git"].diff(args=[...]).sync()` or `.start()`
- `cmd["cargo"].check(args=[...]).sync()` or `.start()`

Every command version stores the action id, binary name and resolution
candidates, Starlark object/method surface, argv prefix/argument policy,
cwd/env/max-runtime/output policy, process policy, `mutationClass` metadata,
model-facing description, the approver-selected final execution policy, and creation metadata.
`mutationClass` is descriptive trace/model metadata in this phase; it is not an
execution policy boundary. Scoped command execution authority comes from the
stored final execution policy, registry argv/cwd/env/max-runtime/output fields, and
native kernel protections. `execute_code` queries the enabled current DB command
versions at every tool boundary, merges global plus matching project commands,
rejects ambiguous action identifier conflicts before surfacing commands, builds the Starlark `cmd`
surface from that live registry, and generates
the model-visible `execute_code` contract from the same live rows. Agents receive
the current interface directly in the tool schema and prompt; they are not told
to read README, manifests, or source files to understand command semantics.

Registry-defined command execution remains structured. There is no raw shell:
commands run only through argv arrays, execution-root cwd enforcement, explicit
env policy, max-runtime/output limits, binary resolution policy, and the stored
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
robdex-agent-runtime command-registry requests list
robdex-agent-runtime command-registry requests show <id>
robdex-agent-runtime command-registry requests review <id>
robdex-agent-runtime command-registry requests final-template <id> [--out <json-file>]
robdex-agent-runtime command-registry requests preview-decision <id> --status approved --final-scope global --final-policy allow --final-command-file <json-file>
robdex-agent-runtime command-registry requests decide --session <session-id> <id> --status denied
robdex-agent-runtime command-registry requests decide --session <session-id> <id> --status approved --final-scope global --final-policy allow --final-command-file <json-file>
robdex-agent-runtime command-registry requests decide --session <session-id> <id> --status approved --final-scope project --final-project <key> --final-policy allow --final-command-file <json-file>
robdex-agent-runtime command-registry requests apply --session <session-id> <id>
```

Agents create ordinary registry change requests through the native
`request_command_registry_change` model tool. The CLI request commands are for
approver/operator inspection, decision, and apply only. `seed-requests` is the
explicit registry maintenance/bootstrap staging affordance. It creates
reviewable registry requests from `command-seeds/`; it does not approve or apply
them. `--mode missing` requests only absent bundled commands. `--mode refresh`
requests updates for existing bundled commands and adds for absent bundled
commands.

Approver ergonomics commands keep decision and application separate while making
review legible. `requests review` emits a structured packet containing
requester/source context, proposed/final/current command state, readiness,
risk-relevant fields, and semantic diff summaries. `requests final-template`
exports editable final command JSON copied from the proposal; final scope and
final policy remain explicit `decide` inputs. `requests preview-decision` runs
the same final scope, final policy, command schema, operation strictness, scoped
conflict, and visibility-impact validation without mutating approval/application
status, registry definitions, command versions, or mutation events.

## Role policy foundation

Postgres is the runtime source of truth for roles. JSON manifests and prompt files in `roles/` are seed/import/export artifacts only. Import resolves prompt files into immutable `role_versions.instruction_text`; runtime session creation reads the current DB role version and stores a complete immutable `sessions.role_snapshot`. Runtime role administration uses the DB-backed `roles` CLI; prompt files are never a runtime source of truth after import.

Canonical role administration:

```sh
cargo run -- roles create <manifest-or-db-export.json>
cargo run -- roles update <manifest-or-db-export.json>
cargo run -- roles import <manifest-or-db-export.json>
cargo run -- roles import-seeds
cargo run -- roles list
cargo run -- roles show <role-id>
cargo run -- roles versions <role-id>
cargo run -- roles version <role-version-id>
cargo run -- roles activate <role-id> --version-id <role-version-id>
cargo run -- roles archive <role-id>
cargo run -- roles unarchive <role-id>
cargo run -- roles export <role-id> --out <db-backed-export.json>
cargo run -- roles validate --manifest <manifest-or-db-export.json>
```

`roles create`, `roles update`, `roles import`, and `roles import-seeds` share the same canonical DB role-version insertion path after their operation-specific preconditions pass. `roles create` is strict and fails if the role id already exists. `roles update` is strict and fails if the role id does not already exist. `roles import` is the general artifact ingestion path and remains import/upsert: it creates the role when missing or appends a new immutable role version and points the role at it when present. `roles import-seeds` is bootstrap/seed import and uses the same canonical insertion path for bundled seed manifests. `roles activate` changes only `roles.current_version_id`; it is the rollback mechanism and does not delete or rewrite historical `role_versions`. `roles archive` disables the role for new sessions while preserving the role, versions, DB-backed exports, inspection, and existing session snapshots. `roles unarchive` restores new-session availability using the existing current version and does not create a role version. `roles export` reads the DB current version and includes `instructionText`, so the export can be imported into a fresh database without the original prompt file.

Role validation emits structured packets with validity, role/version identity, prompt byte count, model defaults, policy actions, routing recipients, lifecycle authority, errors, and warnings. Validation rejects invalid JSON, invalid role ids, empty instruction text, missing model defaults, unknown native actions, concrete `cmd.*` role policy entries, capability/policy mismatches, unsupported routing modes, invalid reserved actions, and invalid routing recipients before activation/runtime use.

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

## Agent-led workflow memory

Workflow memory is agent-led and project-scoped by default. Raw script source remains canonical in `script_runs.source`; `workflow_memory_script_embeddings` stores one embedding/index row per script run, and promoted `workflow_memories` reference the source `script_run_id` instead of duplicating large source bodies. `workflow_memory_events` records help/search, attempted, not-helpful, promoted, and duplicate-collapsed evidence.

Embedding configuration is provider-agnostic:

```sh
export ROBDEX_AGENT_RUNTIME_EMBEDDING_PROVIDER=disabled        # default
export ROBDEX_AGENT_RUNTIME_EMBEDDING_PROVIDER=deterministic   # validation, no network
export ROBDEX_AGENT_RUNTIME_EMBEDDING_PROVIDER=lmstudio
export ROBDEX_AGENT_RUNTIME_EMBEDDING_BASE_URL=http://localhost:1234
export ROBDEX_AGENT_RUNTIME_EMBEDDING_MODEL=qwen3-embedding-4b-dwq
export ROBDEX_AGENT_RUNTIME_EMBEDDING_DIMENSIONS=2560
```

Host Postgres must have the pgvector extension package installed; `init-db` runs `CREATE EXTENSION IF NOT EXISTS vector` and stores embeddings as `halfvec(2560)` with cosine distance. The LM Studio provider is only used when explicitly configured and targets the OpenAI-compatible embeddings endpoint:

```sh
curl http://localhost:1234/v1/embeddings \
  -H "Content-Type: application/json" \
  -d '{"model":"qwen3-embedding-4b-dwq","input":"workflow memory smoke test"}'
```

The optional smoke helper is explicitly opt-in and performs only an embedding call:

```sh
ROBDEX_AGENT_RUNTIME_EMBEDDING_PROVIDER=lmstudio \
ROBDEX_AGENT_RUNTIME_EMBEDDING_BASE_URL=http://localhost:1234 \
ROBDEX_AGENT_RUNTIME_EMBEDDING_MODEL=qwen3-embedding-4b-dwq \
scripts/smoke-lmstudio-embeddings.sh
```

The model-visible Starlark API is concise: `workflow_memory.help()` searches using the latest prior relevant non-memory script in the same session, not the tiny current help script; `workflow_memory.remember_when(condition, title, reason)` records a candidate and promotes it only after the full script exits successfully with `condition == True`; `workflow_memory.mark_attempted(id, variant=True)` and `workflow_memory.mark_not_helpful(id, reason)` record bounded feedback events. First/plain attempts are not auto-promoted. The intended loop is plain attempt fails, call `workflow_memory.help()`, try exact or variant help when useful, and enter remember mode only for a later successful script with explicit success criteria.

Role policy gates native memory actions: `workflow_memory.search`, `workflow_memory.remember.project`, `workflow_memory.remember.global`, and `workflow_memory.feedback`. Seed runtime roles allow project-scoped validation memory; global memory remains approval-gated or denied.

`workflow_memory.help()` is lazy: ordinary `execute_code` scripts do not precompute help searches or embed the prior script unless the current source actually calls `workflow_memory.help()`. Raw script indexing still runs after script completion. Embedding provider/index failures are recorded as `workflow_memory.*_failed` or `workflow_memory.provider_failure` events with session/turn/script context and do not fail an otherwise successful script. Feedback APIs validate that the target memory exists and is visible in the session's project/global scope before recording attempted or not-helpful events.

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
scripts/validate-role-admin-ux.sh
scripts/validate-workflow-memory.sh
```

Validation database administration defaults to `ROBDEX_AGENT_RUNTIME_VALIDATION_ADMIN_DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/postgres`. Override that admin connection only when the same local Postgres server requires a different maintenance database. Do not point validation cleanup at the normal runtime database.

If cleanup fails, the script prints the leftover validation database name, admin connection, and exact manual cleanup SQL. Manual cleanup must only target names with the strict validation prefix:

```sql
DROP DATABASE IF EXISTS "robdex_agent_runtime_validation_<run_id>" WITH (FORCE);
```

The cleanup helper refuses destructive cleanup for database names that do not start with `robdex_agent_runtime_validation_`.

## Session-only managed process surface

Registry command versions carry process policy as data: `syncAllowed`, `asyncAllowed`, `maxRuntimeMs` (`null` means no configured maximum runtime kill), `endOfTurnBehavior`, `stdinPolicy`, await bounds, bounded output buffer bytes, and terminate grace. Seeded default commands use `maxRuntimeMs: null`; a finite value is an explicit command-specific maximum runtime policy, not a renamed default timeout. The model-facing Starlark surface is explicit: `cmd["name"].method(...).sync()` runs synchronously under the command version's max-runtime semantics, and `cmd["name"].method(...).start()` returns an opaque session-only process handle. Process handles are not OS PIDs. The handle API is exposed through `proc[handle]` with `is_running()`, `await_for(mins=N)`, `flush_buffer()`, `terminate()`, and `input(text)`.

The current experimental CLI executes each `send` as a short-lived runtime process. Same-runtime continuation is supported inside a single runtime instance, which is the target persistent server boundary. Across separate CLI invocations, handles are intentionally detached; startup reconciliation marks any previously `running` rows as `lost` instead of pretending to reattach them. Process metadata is persisted in `managed_processes`; incremental bounded output is persisted in `process_output_chunks` when handles are flushed. Command execution remains policy-controlled: approval-required commands pause before side effects, sync/async permission is checked before execution, stdin is rejected unless explicitly allowed, and command traces retain the exact `command_version_id`.
