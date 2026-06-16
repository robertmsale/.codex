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

### GUI contract

The GUI contract is typed in `robdex-agent-runtime-projection` so Rust/Rinf can
own runtime state and Dart can render constructor-ready values instead of
inferring policy from raw JSON. PostgreSQL remains the source of truth.
Persisted GUI state is derived only from:

- full hydration through `RuntimeProjection`;
- realtime persisted-state updates through `RuntimeDelta`;
- reducer-owned local projection mutation through the shared reducer.

Local controller state is deliberately separate from runtime state:
`GuiControllerState` models connection status, selected session id, selected
view, in-flight operations, transient typed API errors, pending resync,
pending rehydrate/reconnect, and draft text inputs. It must not duplicate
session lifecycle status, approval availability, command visibility, command
policy, role status, process status, timeline interpretation, or semantic
operation success. Those facts come from `RuntimeProjection`, `RuntimeDelta`,
and server operation results. Dart may own widget-local facts only: text-field
editing mechanics, focus, scroll position, hover/press state, animations, and
layout.

Operation vocabulary is typed by `GuiOperationRequest`, `GuiOperationResult`,
`GuiOperationOutcome`, `GuiOperationExpectation`, and `ApiErrorPacket`.
Operations cover connect, hydrate, rehydrate, disconnect, select session,
create/send/close/archive/fork session, decide/resume approval, command
registry list/show/request preview/decide/apply, and workflow-memory feedback.
Each request reports its expected projection effect:

- `Rehydrate` for initial hydrate and explicit rehydrate;
- `RehydrateAndReconnect` for selected-session switching;
- `WaitForDelta` for mutations that should arrive on the WebSocket stream;
- `DirectResult` for inspection/preview routes;
- `UpdateLocalState` for local disconnect.

Operation failures use the stable API error packet
`{ "error": { "code", "message", "details" } }` through `ApiErrorPacket`.
String-only failures are not the GUI operation contract.

Selected-session switching is a rehydration boundary. A GUI must request a new
snapshot with `selectedSessionId=<uuid>` and reconnect `/state/ws` with the
same selected-session identifier after the new snapshot watermark. It must not
reinterpret an existing projection as if it contains a different selected
session detail.

Raw JSON payloads remain available only for bounded inspection panes and debug
evidence. Core GUI control enablement is typed: the approval projection contains pending approvals and approved approvals with resumable paused actions; each approval exposes status, approver kind, `canDecide`, and `canResume` so Dart does not infer approval availability from `status` or raw `inputContext`; command summaries expose
scope/enabled/version/call-shape fields; command-registry request summaries
expose `canPreview`, `canDecide`, and `canApply`; roles expose
status/current version/model; and workflow memory summaries expose
scope/title/reason/helpfulness.

Proof coverage lives in the projection crate tests and runtime/server tests.
Run:

```sh
cargo test -p robdex-agent-runtime-projection
cargo test
scripts/smoke-resident-server.sh
scripts/validate-local-service.sh
```

#### GUI operation API audit

The durable operation mapping is also encoded in
`GuiOperationRequest::api_mapping()` and protected by projection crate tests.
Every operation uses the API error packet
`{ "error": { "code", "message", "details" } }` when a server route fails.

| GUI operation | Route or local action | Method | Request shape | Response/direct result | Projection effect |
| --- | --- | --- | --- | --- | --- |
| `Connect` | local `RuntimeSyncClient::new`, hydrate, then `connect_after` | local | base URL and optional selected session | `GuiControllerState`/`SyncOutcome` | `Rehydrate` |
| `Hydrate` | `/state/snapshot?selectedSessionId=<optional>` | GET | none | `RuntimeProjection` | `Rehydrate` |
| `Rehydrate` | `/state/snapshot?selectedSessionId=<optional>` | GET | none | `RuntimeProjection` | `Rehydrate` |
| `Disconnect` | local stream close and controller state update | local | local controller state | `GuiControllerState` | `UpdateLocalState` |
| `SelectSession` | local selected-session update, then snapshot and WebSocket reconnect with `selectedSessionId` | local + GET/WS | selected session id | fresh `RuntimeProjection` and stream | `RehydrateAndReconnect` |
| `CreateSession` | `/sessions` | POST | `{role, project, workdir, worktreeRoot, title, name}` | `{sessionId}` | `WaitForDelta` |
| `SendMessage` | `/sessions/{sessionId}/send` | POST | `{message}` | `{sessionId, turnId, status}` | `WaitForDelta` |
| `CloseSession` | `/sessions/{sessionId}/close` | POST | `{reason?}` | `{sessionId, status}` | `WaitForDelta` |
| `ArchiveSession` | `/sessions/{sessionId}/archive` | POST | `{}` | `{sessionId, tracked}` | `WaitForDelta` |
| `ForkSession` | `/sessions/{sessionId}/fork` | POST | `{atTurn}` | `{sessionId, forkedFromSessionId, forkedFromTurnId}` | `WaitForDelta` |
| `DecideApproval` | `/approvals/{approvalId}/decide` | POST | `{decision, reason}`; `reason` is required | `{approvalId, decision}` | `WaitForDelta` |
| `ResumeApproval` | `/approvals/{approvalId}/resume` | POST | `{}` | `{approvalId, status}` | `WaitForDelta` |
| `ListCommandRegistry` | `/command-registry?sessionId=<optional>&project=<optional>` | GET | none | command list/detail JSON | `DirectResult` |
| `ShowCommand` | `/command-registry/{actionId}?sessionId=<optional>&project=<optional>` | GET | none | command detail JSON | `DirectResult` |
| `ListCommandRegistryRequests` | `/command-registry/requests` | GET | none | map raw rows with `CommandRegistryRequestSummary::from_server_value` | `DirectResult` |
| `ShowCommandRegistryRequest` | `/command-registry/requests/{requestId}` | GET | none | request detail JSON | `DirectResult` |
| `PreviewCommandRegistryRequest` | `/command-registry/requests/{requestId}/preview-decision` | POST | `{sessionId?, status, finalScope?, finalExecutionPolicy?, finalCommand?}` | preview packet JSON | `DirectResult` |
| `DecideCommandRegistryRequest` | `/command-registry/requests/{requestId}/decide` | POST | `{sessionId, status, finalScope?, finalExecutionPolicy?, finalCommand?}` | `{requestId, status}` | `WaitForDelta` |
| `ApplyCommandRegistryRequest` | `/command-registry/requests/{requestId}/apply` | POST | `{sessionId}` | `{requestId, status}` | `WaitForDelta` |
| `WorkflowMemoryFeedback` | `/workflow-memories/{memoryId}/feedback` | POST | `{sessionId, feedback, payload}` | `{memoryId, feedback, status}` | `WaitForDelta` |

Resolved audit mismatches:

- `DecideApproval.reason` is required in the GUI contract because the server
  requires it. The controller must not invent an implicit reason.
- Command-registry decision input uses the server JSON shape directly:
  nested `finalScope`, nested `finalExecutionPolicy`, and typed `finalCommand`
  fields. Dart must not flatten or transform this request ad hoc.
- Command-registry request rows are converted to
  `CommandRegistryRequestSummary`, which carries `canPreview`, `canDecide`,
  and `canApply`. Dart must not infer those controls from raw request internals.

Deferred GUI operations for the first shell:

- Role-admin mutations beyond projection/read inspection are deferred. Server
  role routes exist, but first-shell GUI state uses projected role summaries and
  does not expose role create/update/archive/activate operations yet.
- Workflow-memory inspection uses `RuntimeProjection.workflowMemories` and
  selected-session/timeline detail. Dedicated memory list/show/events operation
  intents are deferred; feedback is included because it mutates state.


#### Rust/Rinf GUI backend controller boundary

The Rust-side GUI backend boundary for a future Rinf layer is
`robdex_agent_runtime::gui_backend::GuiBackendController`. This controller owns
the `RuntimeSyncClient`, the owned WebSocket stream handle, the current hydrated `RuntimeProjection`, the local
`GuiControllerState`, selected-session state, connection/resync state,
transient typed errors, and `GuiOperationResult` emission. Dart sends
`GuiOperationRequest` packets and receives constructor-ready projection,
controller, and result packets; Dart does not interpret raw HTTP/WebSocket
protocol details.

The controller has one dispatcher: `GuiBackendController::dispatch`. Local
operations are handled in Rust:

- `Connect` creates the sync client, hydrates `/state/snapshot`, opens
  `/state/ws?after=<snapshotWatermark>`, and owns the resulting stream handle.
- `Hydrate` and `Rehydrate` replace the current `RuntimeProjection` from the
  server snapshot and reconnect the WebSocket stream after the new watermark.
- `SelectSession` updates local selected-session state, then rehydrates and
  reconnects `/state/ws?after=<watermark>&selectedSessionId=<id>` without Dart
  deciding URL or watermark semantics.
- `Disconnect` drops the owned stream handle, clears sync/projection state, and
  marks connection state disconnected.

Server-backed operations use the API mappings encoded on `GuiOperationRequest`
and the same server JSON shapes documented above. The dispatcher maps server
and sync failures into `ApiErrorPacket` and returns `GuiOperationOutcome::Error`
so Dart never parses raw HTTP, WebSocket, SQL, or protocol errors. Direct
command-registry request lists are reduced into `CommandRegistryRequestSummary`
inside Rust, preserving typed `canPreview`, `canDecide`, and `canApply` control
fields.

Realtime server messages are consumed through the controller with
`GuiBackendController::next_stream_outcome()`, which reads one message from the
controller-owned WebSocket stream, applies deltas/resync/shutdown through
`RuntimeSyncClient`, and mirrors the reduced projection/controller state back
onto the controller. Deltas are applied to the current projection only through
the shared `RuntimeProjection` reducer. The
controller does not create a second GUI state path: persisted runtime state
remains `RuntimeProjection` plus `RuntimeDelta`; `GuiControllerState` remains
local-only coordination state. Flutter UI implementation remains out of scope.

Proof coverage includes deterministic controller tests for hydrate/connect,
disconnect, selected-session switching, server-backed payload dispatch, typed
error mapping, command-registry direct-result summary mapping, and delta
convergence through the controller.

#### Stable hub Rinf transport binding

The experiment-local Rinf-shaped transport proof lives in
`robdex_agent_runtime::rinf_transport`. The owner selected the direct-dependency
strategy, and the first stable hub Rust binding now forwards generated Rinf
signals to that transport path. The binding remains transport-only: it does not
implement Flutter UI, design-system widgets, Design Lab scenarios, launchd
installation, or stable Robdex backend/supervisor behavior.

Stable hub files touched by the binding:

- `frontend/robdex_app/native/hub/Cargo.toml`;
- `frontend/robdex_app/native/hub/Cargo.lock`;
- `frontend/robdex_app/native/hub/src/signals/agent_runtime.rs`;
- `frontend/robdex_app/native/hub/src/signals/mod.rs`;
- `frontend/robdex_app/native/hub/src/runtime.rs`.

Generated Rinf binding files changed by `rinf gen`:

- `frontend/robdex_app/lib/src/bindings/signals/agent_runtime_request_signal.dart`;
- `frontend/robdex_app/lib/src/bindings/signals/agent_runtime_output_signal.dart`;
- `frontend/robdex_app/lib/src/bindings/signals/signals.dart`;
- `frontend/robdex_app/lib/src/bindings/signals/signal_handlers.dart`.

Dart-to-Rust signals map to `GuiTransportRequestPacket` through
`AgentRuntimeRequestSignal { requestId, packetJson }`:

- `Connect { baseUrl, selectedSessionId }`;
- `Hydrate { selectedSessionId }`;
- `Rehydrate { selectedSessionId }`;
- `DispatchOperation { operation: GuiOperationRequest }`;
- `PollStreamOnce`;
- `Disconnect`.

Rust-to-Dart signals map to `GuiTransportOutputPacket` through
`AgentRuntimeOutputSignal { requestId, outputJson }`:

- `ProjectionSnapshot` with a JSON-backed `RuntimeProjection` payload;
- `ControllerState` with a JSON-backed `GuiControllerState` payload;
- `OperationResult` with the typed `GuiOperationResult`;
- `StreamOutcome` with typed hello/delta/resync/shutdown/closed outcomes plus
  current projection/controller state;
- `Error` with the stable `ApiErrorPacket`;
- `ControlTowerView` with the Rust-shaped
  `AgentRuntimeControlTowerViewModel` consumed by the first Flutter shell.

The stable hub creates one long-lived `GuiTransportHandle`, and the transport
runner owns exactly one `GuiBackendController` inside a single async action
loop. Dart sends intent packets only; Rust resolves selected-session
hydration, WebSocket watermark semantics, operation success, projection
reduction, approval/command/process enablement, and typed errors. Packet
payloads are JSON-backed where the projection internals are likely to evolve,
so future Rinf schemas can stay stable while the Rust projection contract
continues to develop.

Service discovery remains bootstrap input. Dart may read the local discovery
packet to find a base URL, then sends a connect request. After connect,
`RuntimeProjection`, `GuiControllerState`, stream outcomes, and
`GuiOperationResult` packets from Rust are authoritative. Dart must not compute
watermarks, construct WebSocket URLs, apply reducers, decide approval or command
availability, or infer operation success.

The first Flutter-facing control tower shell is implemented as a thin renderer
over the existing packet carriers. It sends JSON `GuiTransportRequestPacket`
intents through `AgentRuntimeRequestSignal` and consumes JSON
`GuiTransportOutputPacket` outputs from `AgentRuntimeOutputSignal`. Dart stores
only widget/controller-local facts such as the base URL text, pending request
ids, and latest render packets; Rust remains responsible for service
connection, WebSocket URLs, watermarks, reducer application, selected-session
semantics, operation success, and typed errors.

The transport now emits a Rust-owned `AgentRuntimeControlTowerViewModel` output
for the first control-tower widget. The view model is constructor-ready:
connection state, base URL, status and watermark labels, session rows, timeline
rows, action rows, controller facts, recent output log, pending-request slot,
and typed error display text are shaped in Rust from `RuntimeProjection`,
`GuiControllerState`, and operation/stream outcomes. Dart decodes this
Rust-shaped view packet and renders it; Dart no longer interprets raw
projection or controller JSON to derive rows, labels, facts, or enablement
text.

The richer control-tower UX slice extends that Rust-owned view model with
operations-first presentation fields: status badges, selected-session label,
section titles, empty-state copy, session group labels, row tones, action state
text, and action/timeline/session severity tones. The design-system control
tower renders those fields directly to provide a clearer status strip, denser
session rail, selected-session event stream, readable action queue, controller
detail panel, and explicit empty/error/loading states. The action queue contains
only real attention items present in the projection: pending/resumable
approvals and typed pending/actionable command-registry request summaries.
Command registry inventory is surfaced as inventory count/status detail, not as
required action. Dart still sends only Rinf JSON packet intents and does not
infer durable runtime meaning from raw projection/controller internals.

The shell remains focused: discovery/connect input, Rust-shaped view-model
rendering, selected-session timeline visibility when present, an action queue
from Rust-owned approval/resume and command-registry request rows, explicit
disconnected/error states, and manual stream polling through the Rust-owned
transport. Reusable visual pieces live in the design-system package under the
agent-runtime control tower
component, with Design Lab scenarios for disconnected, connecting, connected,
error, and empty/no-session states. Remaining gates are role-admin mutation UI,
workflow-memory inspection UI, service packaging beyond local scripts, and
launchd/system service installation.

## Resident server MVP

The experimental server binary is `robdex-agent-runtime-server`. It is isolated
from stable Robdex and uses the same Postgres runtime state and runtime
functions as the CLI. There is no auth or user-session boundary in this slice;
the intended trust boundary is VPN/network placement.

### Local developer service script

For local development, use the experiment-local service wrapper. It does not
install or modify launchd, systemd, supervisor, stable Robdex service tooling,
or host service configuration.

```sh
scripts/agent-runtime-service.sh start
scripts/agent-runtime-service.sh status
scripts/agent-runtime-service.sh discover
scripts/agent-runtime-service.sh logs
scripts/agent-runtime-service.sh restart
scripts/agent-runtime-service.sh stop
scripts/agent-runtime-service.sh stop --force
scripts/agent-runtime-service.sh logs --tail
```

The default service state directory is `.runtime-service` under this experimental
workspace. Override it with `ROBDEX_AGENT_RUNTIME_SERVICE_STATE_DIR`. The state
directory contains:

- `server.pid` for the resident server process;
- `server.stdout.log` and `server.stderr.log`;
- `effective-config.json` with the base URL, pid, log paths, policy values,
  server binary path, and redacted database target;
- `discovery.json` with the machine-readable local discovery packet for future
  GUI/Rinf clients;
- bounded health/status diagnostics created by the scripts.

The wrapper preserves the resident server environment. It honors the same server
configuration variables documented below, including
`ROBDEX_AGENT_RUNTIME_DATABASE_URL`, `ROBDEX_AGENT_RUNTIME_SERVER_HOST`,
`ROBDEX_AGENT_RUNTIME_SERVER_PORT`, `ROBDEX_AGENT_RUNTIME_IDENTITY`, and the
startup/shutdown policy variables. Set `ROBDEX_AGENT_RUNTIME_SERVER_BIN` to use
an existing binary; otherwise `start` builds `robdex-agent-runtime-server` when
`target/debug/robdex-agent-runtime-server` is missing.

`start` launches the server in the background, writes the pid/config/log files,
polls `GET /health` with a bounded deadline
(`ROBDEX_AGENT_RUNTIME_SERVICE_HEALTH_DEADLINE_SECONDS`, default `20`), and
prints the base URL, pid, log paths, config/discovery paths, and redacted
database URL. If `server.pid` points at a live process, duplicate `start` is refused. Use the
documented `restart` path to replace a running local service. If `server.pid` is
stale, `start` moves it aside with a `.stale.<timestamp>` suffix before
starting.

`stop` reads `server.pid`, sends a graceful termination signal, waits up to
`ROBDEX_AGENT_RUNTIME_SERVICE_STOP_DEADLINE_SECONDS` seconds, verifies the
process is gone, and removes `server.pid`. It does not force-kill unless
`stop --force` is explicitly passed. `restart` performs `stop` followed by
`start`, preserving the same graceful shutdown semantics. `status` reports the
pid-file state, process liveness, stale-pid detection, health endpoint result,
base URL, redacted database target, and log/config paths. `logs` prints stdout
and stderr logs; `logs --tail` tails them without requiring an external service
manager.

`discover` (alias: `json-status`) is the GUI-oriented status command. It prints
and persists the same JSON packet at
`$ROBDEX_AGENT_RUNTIME_SERVICE_STATE_DIR/discovery.json`. Future GUI/Rinf
clients can read that stable file without shelling out; shell callers can use
`discover` when they need a fresh packet. The packet uses redacted database
targets only and never exposes raw credentials or tokens. It contains:

- `contractVersion`;
- `serviceState`: `running`, `stopped`, `stalePid`, `unhealthy`, or
  `missingConfig`;
- `stateFlags`, including `staleDiscovery` when the previous discovery file was
  absent or older than service metadata before refresh;
- `baseUrl`, `healthUrl`, and `webSocketUrl`;
- `runtimeIdentity` when reported by `/health`;
- `pid` and `pidLiveness`;
- `stateDirectory` and `paths` for pid/config/stdout/stderr/discovery files;
- `databaseTarget.urlRedacted`;
- `effectivePolicy` values for bind address, schema initialization,
  seed-role import, command bootstrap, process reconciliation, and shutdown;
- `healthResult`, `diagnostics`, and relevant timestamps.

Lifecycle commands keep `discovery.json` coherent: `start` writes a running or
unhealthy packet after health polling, duplicate `start` leaves the running
packet intact, `status` refreshes the persisted packet while preserving
human-readable output, `restart` updates it for the new pid, and `stop` writes
the stopped packet after pid cleanup. Stale pid, unhealthy server, missing
config, and stale discovery conditions are represented explicitly in the JSON
without discarding diagnostics.

Validate the local service wrapper with an isolated Postgres validation database
and no live model, LM Studio, or embedding-provider calls:

```sh
scripts/validate-local-service.sh
```

The validation starts the local service, verifies `status` and `/health`, checks
startup log evidence, verifies `discover` output and persisted discovery file
content, verifies duplicate-start refusal, exercises `logs`, restarts and
checks the new healthy process, stops the service, verifies stopped discovery,
checks stale/unhealthy/missing-config diagnostics, verifies the process is gone,
and drops the isolated validation database.

Run with the conservative default bind:

```sh
export ROBDEX_AGENT_RUNTIME_DATABASE_URL='postgres://postgres:postgres@127.0.0.1:5432/robdex_agent_runtime'
cargo run --bin robdex-agent-runtime-server
```

Override host and port explicitly when placing it behind the VPN boundary:

```sh
cargo run --bin robdex-agent-runtime-server -- --host 127.0.0.1 --port 8765
# or
ROBDEX_AGENT_RUNTIME_SERVER_HOST=127.0.0.1 ROBDEX_AGENT_RUNTIME_SERVER_PORT=8765 cargo run --bin robdex-agent-runtime-server
```

Resident server startup is driven by a typed runtime configuration. The server
connects to Postgres, applies only the configured startup policies, prints a
single `[server-startup]` JSON report, serves HTTP/WebSocket traffic, handles
interrupt/termination with graceful shutdown, closes the database pool, and
prints a `[server-shutdown]` JSON report.

Operational environment variables and matching CLI flags:

```text
ROBDEX_AGENT_RUNTIME_DATABASE_URL / --database-url
ROBDEX_AGENT_RUNTIME_SERVER_HOST / --host
ROBDEX_AGENT_RUNTIME_SERVER_PORT / --port
ROBDEX_AGENT_RUNTIME_IDENTITY / --runtime-identity
ROBDEX_AGENT_RUNTIME_SCHEMA_POLICY / --schema-policy
ROBDEX_AGENT_RUNTIME_SEED_ROLE_POLICY / --seed-role-policy
ROBDEX_AGENT_RUNTIME_COMMAND_BOOTSTRAP_POLICY / --command-bootstrap-policy
ROBDEX_AGENT_RUNTIME_PROCESS_RECONCILIATION_POLICY / --process-reconciliation-policy
ROBDEX_AGENT_RUNTIME_SHUTDOWN_POLICY / --shutdown-policy
```

Supported startup policy values:

- schema policy: `apply` (default) or `skip`;
- seed-role policy: `importSeeds` (default) or `skip`;
- command bootstrap policy: `bootstrapDefaults` (default) or `skip`;
- process reconciliation policy: `markRunningLost` (default) or `skip`;
- shutdown policy: `gracefulMarkRunningLost` (default).

The defaults preserve convenient local development behavior: schema creation is
applied, command seed definitions are bootstrapped idempotently, seed roles are
imported only when needed, and previously running session-only process rows are
reconciled. Seed-role startup import is content-idempotent: when a seed role's
effective content matches the current DB role version, startup does not create a
new immutable `role_versions` row and does not append a new `role.imported`
event. A new role version is created only when the seed content changes or the
role is missing. Those startup mutations are no longer silent; the startup
report includes the runtime identity, redacted database target, database
identity, bind address, policy values, `seedRolesImported`,
`seedRolesUnchanged`, and reconciliation counts.

Session-only managed process handles do not survive runtime restart. On startup
with `markRunningLost`, any `managed_processes.status = 'running'` rows are set
to `lost` with `termination_reason = 'runtimeRestart'`. The server also emits
`process.lost` and `session.recoveryDegraded` event-stream rows for each
affected process. On interrupt or termination, `gracefulMarkRunningLost` stops
the HTTP listener, signals active WebSocket state loops with a `serverShutdown` message and close frame, terminates currently
owned live managed processes where the in-memory runtime can see them, marks
remaining running process rows as `lost` with `termination_reason =
'runtimeShutdown'`, emits event evidence through the same reconciliation path,
closes the database pool, and reports the outcome.

HTTP JSON routes:

```text
GET  /health
GET  /state/snapshot?selectedSessionId=<uuid>
GET  /sessions
POST /sessions
GET  /sessions/{sessionId}
GET  /sessions/{sessionId}/history
POST /sessions/{sessionId}/send
POST /sessions/{sessionId}/close
POST /sessions/{sessionId}/archive
POST /sessions/{sessionId}/fork
GET  /approvals
GET  /approvals/{approvalId}
POST /approvals/{approvalId}/decide
POST /approvals/{approvalId}/resume
GET  /roles
GET  /roles/{roleId}
GET  /roles/{roleId}/versions
GET  /roles/versions/{versionId}
POST /roles/{roleId}/activate
POST /roles/{roleId}/archive
POST /roles/{roleId}/unarchive
GET  /roles/{roleId}/export
GET  /command-registry?sessionId=<uuid>|project=<project-key>
GET  /command-registry/{actionId}?sessionId=<uuid>|project=<project-key>
GET  /command-registry/requests
GET  /command-registry/requests/{requestId}
GET  /command-registry/requests/{requestId}/review
GET  /command-registry/requests/{requestId}/final-template
POST /command-registry/requests/{requestId}/preview-decision
POST /command-registry/requests/{requestId}/decide
POST /command-registry/requests/{requestId}/apply
GET  /workflow-memories?sessionId=<uuid>
GET  /workflow-memories/{memoryId}?sessionId=<uuid>
GET  /workflow-memories/{memoryId}/events?sessionId=<uuid>
POST /workflow-memories/{memoryId}/feedback
```

Create/send example:

```sh
curl -sS -X POST http://127.0.0.1:8765/sessions \
  -H 'content-type: application/json' \
  -d '{"role":"runtime-no-rg","project":"agent-runtime","workdir":"/Users/robertsale/.codex/backend/experiments/agent-runtime"}'

curl -sS -X POST http://127.0.0.1:8765/sessions/<session-id>/send \
  -H 'content-type: application/json' \
  -d '{"message":"Use execute_code with exactly this Starlark source: content = fs.read(\"Cargo.toml\"); output({\"smoke\":\"ok\",\"contains_workspace\":\"workspace\" in content})"}'
```

The send route enforces one active send per session. A concurrent send for the
same session returns HTTP `409 Conflict` instead of queueing or racing.

Admin HTTP routes are thin JSON wrappers over the existing runtime/database
functions. Approval routes list pending requests, show one request, record a
decision, and explicitly resume an approved paused action. Role routes use the
DB-canonical role APIs for list/show/version inspection, activation,
archive/unarchive, and DB-backed export; existing sessions keep their immutable
role snapshots. Command-registry routes expose command/request inspection,
request review, final-template generation, decision preview, decision, and
apply while preserving approver-selected final scope/policy. Command list/show
without a scope query returns the administrative registry view. Supplying
`sessionId` or `project` selects the scoped runtime-visible command surface:
global commands plus project commands visible to that session/project only.
Workflow-memory routes require `sessionId` and validate that a
memory exists and is project/global visible to that session before listing,
showing events, or recording bounded `attempted`, `notHelpful`, or `helpful`
feedback.

Admin examples:

```sh
curl -sS http://127.0.0.1:8765/approvals
curl -sS -X POST http://127.0.0.1:8765/approvals/<approval-id>/decide \
  -H 'content-type: application/json' \
  -d '{"decision":"approved","reason":"owner approved from GUI"}'
curl -sS -X POST http://127.0.0.1:8765/approvals/<approval-id>/resume \
  -H 'content-type: application/json' -d '{}'

curl -sS http://127.0.0.1:8765/roles/runtime-no-rg/export
curl -sS -X POST http://127.0.0.1:8765/roles/runtime-no-rg/archive \
  -H 'content-type: application/json' -d '{}'

curl -sS 'http://127.0.0.1:8765/command-registry?sessionId=<session-id>'
curl -sS 'http://127.0.0.1:8765/command-registry/<action-id>?project=<project-key>'
curl -sS http://127.0.0.1:8765/command-registry/requests/<request-id>/final-template
curl -sS -X POST http://127.0.0.1:8765/command-registry/requests/<request-id>/apply \
  -H 'content-type: application/json' \
  -d '{"sessionId":"<session-id>"}'

curl -sS 'http://127.0.0.1:8765/workflow-memories?sessionId=<session-id>'
curl -sS -X POST http://127.0.0.1:8765/workflow-memories/<memory-id>/feedback \
  -H 'content-type: application/json' \
  -d '{"sessionId":"<session-id>","feedback":"attempted","payload":{"variant":true}}'
```

Server API errors use a stable JSON packet for GUI clients:

```json
{
  "error": {
    "code": "not_found",
    "message": "role not found: runtime-x",
    "details": { "entity": "role", "id": "runtime-x" }
  }
}
```

The server maps typed runtime/domain errors to stable HTTP categories:
malformed request bodies to `400 bad_request`, policy or visibility denials to
`403 forbidden`, missing runtime entities to `404 not_found`, concurrent sends
and invalid state transitions to `409 conflict`, role or command-registry
validation failures to `422 validation_failed`, dependency outages to
`503 unavailable`, and unexpected untyped failures to `500 internal_error` with
the safe generic message `unexpected server error`. Untyped internal error
strings are not substring-classified into GUI-facing domain statuses; routes
must surface domain failures through the typed runtime error layer to expose a
non-500 category.

WebSocket state stream:

```text
ws://127.0.0.1:8765/state/ws?after=<watermark>&selectedSessionId=<uuid>
```

The server sends an initial `hello` message with the current watermark, then
streams serde-compatible `RuntimeDelta` values derived from Postgres
`event_stream` rows through the shared projection crate. Each event-stream row
always produces a `TimelineAppend` delta for timeline rendering. The runtime
server adapter also emits semantic deltas from the same row when the current DB
state can be mapped to a projection entity: session create/archive/close,
turn/tool/script/process status, approval pending/removal, role changes,
command-registry changes, and workflow-memory summaries/events. When one
event-stream row produces multiple deltas, the server sends them in stable
order with the same row watermark: timeline append first, then semantic entity
deltas. Clients must apply all deltas in arrival order with the shared reducer;
same-watermark semantic deltas are part of the same event row and must not force
a snapshot refresh.

If the requested watermark cannot be continued safely, the server sends an
explicit `resyncRequired` message. Clients should hydrate with `/state/snapshot`
before applying deltas and should rehydrate when reducer state reports
`resyncRequired`.


### GUI state-sync client contract

The runtime crate exposes an isolated Rust state-sync layer at
`robdex_agent_runtime::gui_sync` for future macOS/iOS Rust/Rinf GUI backends.
It is independent of Flutter and Rinf. The layer owns `RuntimeSyncConfig`,
`RuntimeSyncClient`, `RuntimeStateStream`, `SyncOutcome`, and `SyncError` so a
GUI backend can configure the server base URL, an optional selected session,
hydrate state, connect to the WebSocket stream, decode server messages, and
mutate a local `RuntimeProjection`.

The intended client sequence is:

1. Call `RuntimeSyncClient::hydrate()` to fetch `GET /state/snapshot` with the
   current optional `selectedSessionId`.
2. Read the snapshot watermark from the local `RuntimeProjection` and open
   `/state/ws?after=<watermark>` through `connect_after(Some(watermark))`.
3. Process `hello` as protocol identity/watermark evidence for the runtime.
4. Process each `delta` by applying exactly one `RuntimeDelta` through the
   shared `robdex-agent-runtime-projection` reducer. The GUI sync layer does
   not duplicate reducer behavior.
5. Treat `resyncRequired` as a terminal stream condition for the current delta
   stream, expose that a fresh snapshot is required, and call `rehydrate()` to
   replace local projection state for the current selected session.
6. Treat `serverShutdown` as a distinct server-lifecycle outcome. It records
   shutdown detection without mutating or corrupting the local projection.

Selected-session state uses the same path with `selectedSessionId` on both the
snapshot and WebSocket URLs. Timeline deltas for the selected session append to
the local timeline while semantic deltas update session/admin summaries through
the projection reducer. A future GUI should render from the reduced
`RuntimeProjection` and request a fresh snapshot whenever resync state is set.

Resident server deterministic smoke uses a real
`robdex-agent-runtime-server` process on a local HTTP/WebSocket listener and an
isolated validation database. It does not call OpenAI, LM Studio, or embedding
providers. Run it from this nested workspace:

```sh
scripts/smoke-resident-server.sh
```

The harness creates and later drops a validation database through
`scripts/lib-validation-db.sh`, imports seed roles, builds the resident server
binary, starts it on `127.0.0.1:<ephemeral-port>`, polls `GET /health` with a
bounded deadline, exercises representative HTTP admin/session APIs, validates
structured error packets, opens the WebSocket state stream, verifies `hello`,
observes a semantic session archive delta, verifies `resyncRequired` for an
omitted watermark, and stops the server process on success or failure. Override
`ROBDEX_AGENT_RUNTIME_VALIDATION_ADMIN_DATABASE_URL` when the local Postgres
maintenance database differs from the default.

Minimal live-server smoke with `gpt-5.4-mini` is intentionally separate from
deterministic validation and remains explicitly opt-in. Start
`robdex-agent-runtime-server`, then run:

```sh
ROBDEX_AGENT_RUNTIME_LIVE_SERVER_SMOKE=1 scripts/smoke-live-server-gpt54mini.sh
```

The script imports a throwaway DB role with `modelDefaults.model` set to
`gpt-5.4-mini`, creates a session through `POST /sessions`, sends one read-only
`execute_code` prompt through `POST /sessions/{sessionId}/send`, and prints DB
evidence from `turns`, `model_events`, `tool_calls`, `script_runs`, and
`event_stream`, including the recorded model/tool/script status and mutation
event count.

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
rejects ambiguous action identifier conflicts before surfacing commands, and
builds the Starlark `cmd` surface from that live registry.

Model-facing native tool schemas are cache-stable. The `execute_code` and
`request_command_registry_change` tool descriptions describe permanent behavior
only and do not embed the live visible command list. For each model request, the
runtime computes a deterministic command context ID from policy-relevant visible
command data: action id, command version id, scope/project key, Starlark
object/method, sync/async allowance, args/cwd support, max runtime, stdin
policy, end-of-turn/end-of-session behavior, output limits, mutation class, and
concise model description. The visible catalog is sent as a synthetic request
input message with metadata `source: runtime_command_context`; it is generated
at request time and is not persisted as ordinary user conversation history. The
first turn for a command context includes a concise catalog. If the command
context ID is unchanged from the prior relevant model turn, the runtime sends a
small unchanged hint. If it changes, the runtime sends the new context ID plus
added/removed/changed counts and concise command summaries. Model event metadata
stores command context ID, catalog-included status, visible/added/removed/
changed counts, and compact command fingerprints for cache debugging without
copying a large catalog into every event.

Agents should inspect live command details inside `execute_code` when needed:
`cmd.describe()` returns the visible catalog, `cmd["object"].describe()`
returns visible methods on one object, and `cmd["object"].method.describe()`
returns the exact command invocation surface and policy details. Agents should use
`request_command_registry_change` when a needed command is missing or outdated.
Discovery output is informational only; actual execution still validates live
visibility and policy at the `execute_code` boundary, so unavailable commands
fail when called. Newly approved or inserted commands are visible in the
synthetic command context and Starlark discovery output on the next model/tool
boundary without server restart.

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

Server/admin deterministic validation is covered by `cargo test` from this
nested workspace. It uses migrated temporary Postgres databases and does not
call a live OpenAI model or LM Studio. The resident server process smoke is:

```sh
scripts/smoke-resident-server.sh
```

The live server smoke remains env-gated:

```sh
ROBDEX_AGENT_RUNTIME_LIVE_SERVER_SMOKE=1 scripts/smoke-live-server-gpt54mini.sh
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
