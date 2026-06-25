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
endpoint with non-expired ChatGPT token material from `$CODEX_HOME/auth.json`.
If that file is missing usable ChatGPT token material, the adapter falls back to
`OPENAI_API_KEY` and uses the public OpenAI Responses endpoint. It does not claim
to use the full vendored Codex provider/client runtime. All direct HTTP,
Responses request shaping, Codex auth headers, SSE parsing, and raw model
response handling must stay inside `model::codex_adapter`.

Create Session model choices are Rust-owned. The Agent Runtime constructs model
options through the vendored Codex `codex-models-manager` `OpenAiModelsManager`
and its `models_cache.json` cache semantics, using the same auth preference as
the model adapter: non-expired ChatGPT subscription token material in
`$CODEX_HOME/auth.json` first, then API-key fallback. Flutter receives these
options only through generated Rinf `AgentRuntimeWorkbenchViewModel.modelOptions`.
Dart must not hardcode fallback model names, derive the selectable list from role
admin draft/detail fields, or add a server environment variable that replaces the
Codex model manager source.

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
session detail, typed selected-session chat entries, separate history/event rows, pending approvals, role summaries, command
registry summaries, workflow memory summaries, and a top-level watermark. Selected-session chat is backed by turns.input_text, model_events final_response payloads, and tool/script/process/output-artifact records; audit events remain available through History/Diagnostics surfaces.
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
registry list/show/request preview/decide/apply, project runtime config
validate/import/activate/archive/export/evaluation-inspection, and
workflow-memory feedback. Project runtime config changes are typed Rust/Rinf
operations; Dart must not edit global skills, send opaque config blobs outside
`GuiOperationRequest`, or own activation business logic.
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
scope/title/reason/summary/helpfulness/source metadata/recent events and
feedback action state. The selected workflow-memory id is Rust-owned local
controller state: row selection sends a typed intent, the view model falls back
deterministically when the selected memory disappears, and feedback actions use
the selected detail plus the Rust-owned selected/active session id.

Proof coverage lives in the projection crate tests and runtime/server tests.
Run:

```sh
cargo test -p robdex-agent-runtime-projection
cargo test
scripts/validate-resident-server.sh
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
| `CreateSession` | `/sessions` | POST | `{role, project, model, workdir, worktreeRoot, title, name}`; `project` may be the explicit `__unassigned__` sentinel | `{sessionId}` | `WaitForDelta` |
| `ListProjects` | `/projects` | GET | none | canonical DB-backed project rows | `DirectResult` |
| `CreateProject` | `/projects` | POST | `{projectKey, displayName, defaultWorkdir, defaultWorktreeRoot, defaultRoleId, defaultModel}` | `{project}` | `WaitForDelta` |
| `UpdateProject` | `/projects/{projectKey}` | POST | `{displayName, defaultWorkdir, defaultWorktreeRoot, defaultRoleId, defaultModel}` | `{project}` | `WaitForDelta` |
| `ArchiveProject` | `/projects/{projectKey}/archive` | POST | `{}` | `{project}` | `WaitForDelta` |
| `UnarchiveProject` | `/projects/{projectKey}/unarchive` | POST | `{}` | `{project}` | `WaitForDelta` |
| `SendMessage` | `/sessions/{sessionId}/send` | POST | `{message}` | `{sessionId, turnId?, submittedInputId, disposition, status, orderingKey, lifecycle}` | `WaitForDelta` |
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
| `RoleEditorOptions` | `/roles/editor/options` | GET | none | `RoleEditorOptions` | `DirectResult` |
| `ValidateRoleDraft` | `/roles/editor/validate` | POST | `RoleEditorDraft` with inline `instructionText` | `RoleEditorValidationResult` | `DirectResult` |
| `CreateRoleFromDraft` | `/roles` | POST | `RoleEditorDraft` | `{roleId, versionId, status}` then projection refresh/delta evidence | `WaitForDelta` |
| `UpdateRoleFromDraft` | `/roles/{roleId}/versions` | POST | `RoleEditorDraft` | `{roleId, versionId, status}` then projection refresh/delta evidence | `WaitForDelta` |
| `ShowRoleDetail` | `/roles/{roleId}` | GET | none | `RoleSnapshot` | `DirectResult` |
| `ListRoleVersions` | `/roles/{roleId}/versions` | GET | none | version rows | `DirectResult` |
| `ShowRoleVersion` | `/roles/versions/{versionId}` | GET | none | `RoleSnapshot` | `DirectResult` |
| `ExportRole` | `/roles/{roleId}/export` | GET | none | DB-backed export with inline `instructionText` | `DirectResult` |
| `ActivateRoleVersion` | `/roles/{roleId}/activate` | POST | `{versionId}` | `{roleId, versionId, status}` then projection refresh/delta evidence | `WaitForDelta` |
| `ArchiveRole` | `/roles/{roleId}/archive` | POST | `{}` | `{roleId, status}` then projection refresh/delta evidence | `WaitForDelta` |
| `UnarchiveRole` | `/roles/{roleId}/unarchive` | POST | `{}` | `{roleId, status}` then projection refresh/delta evidence | `WaitForDelta` |

Resolved audit mismatches:

- `DecideApproval.reason` is required in the GUI contract because the server
  requires it. The controller must not invent an implicit reason.
- Command-registry decision input uses the server JSON shape directly:
  nested `finalScope`, nested `finalExecutionPolicy`, and typed `finalCommand`
  fields. Dart must not flatten or transform this request ad hoc.
- Command-registry request rows are converted to
  `CommandRegistryRequestSummary`, which carries `canPreview`, `canDecide`,
  and `canApply`. Dart must not infer those controls from raw request internals.

Role Admin GUI operations are implemented: validation/options/detail/export direct results plus create/update/activate/archive/unarchive wait-for-delta mutations. Dart renders Rust-shaped `roleAdmin` view-model fields and sends typed role intents only.

The mounted Agent Runtime GUI now uses the canonical Robdex Workbench structure:
brushed-metal left project/session rail, center shared `ChatTimeline`, shared
`ComposerPanel`, and toolbar-opened modal or sheet surfaces for operations. The
connected layout must not mount a permanent operations pane. Diagnostics,
History, Statistics, Settings, Process Manager, Role Admin, Workflow Memory,
Approvals, Command Registry, and Compaction live behind typed toolbar
affordances.

Workflow Memory inspection is implemented inside that modal operations surface.
It is an inspector plus feedback surface for
execute_code/Starlark workflow memories only: memory rows, selected detail,
read-only source Starlark, recent help/feedback events, and attempted/helpful/
not-helpful feedback actions are Rust-shaped and session-scoped. Selecting a row
updates Rust-owned selected workflow-memory state; Dart renders the selected
detail returned by Rust and does not choose feedback authority. It does not edit,
rewrite, delete, hide, promote, recompute embeddings, or curate memories.

Remaining scoped-out GUI operations:

- Workflow-memory editing/curation remains out of scope; the implemented
  modal surface is inspection plus feedback only.


#### Rust/Rinf GUI backend controller boundary

The Rust-side GUI backend boundary for a typed Rinf layer is
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

Realtime server messages are consumed by the Rust native hub, not by Dart. The
hub owns the selected-session stream loop and calls the controller-owned stream
reader internally. `GuiBackendController::next_stream_outcome()` remains the
Rust reducer entrypoint for reading one message from the owned WebSocket stream,
applying deltas/resync/shutdown through `RuntimeSyncClient`, and mirroring the
reduced projection/controller state back onto the controller. Disconnect and
session replacement cancel pending stream reads before the next control intent
is reduced. Deltas are applied to the current projection only through the shared
`RuntimeProjection` reducer. The
controller does not create a second GUI state path: persisted runtime state
remains `RuntimeProjection` plus `RuntimeDelta`; `GuiControllerState` remains
local-only coordination state. Subsequent slices mounted the Flutter-facing
Workbench shell as a thin renderer of this Rust-owned state; Flutter does not own
runtime decisions.

Proof coverage includes deterministic controller tests for hydrate/connect,
disconnect, selected-session switching, server-backed payload dispatch, typed
error mapping, command-registry direct-result summary mapping, and delta
convergence through the controller.

#### Stable hub Rinf transport binding

The experiment-local Rinf-shaped transport proof lives in
`robdex_agent_runtime::rinf_transport`. The owner selected the direct-dependency
strategy, and the first stable hub Rust binding now forwards generated Rinf
signals to that transport path. Later slices added the workbench-shell Flutter
shell, design-system scenarios, user-scoped service packaging, and per-user
LaunchAgent install/load/unload/status on top of the same transport. Root/system
LaunchDaemons, sudo service installation, mDNS/Bonjour discovery, iOS profile
sync UX beyond the implemented iCloud profile sentinel, and stable Robdex
backend/supervisor behavior remain out of scope.

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

Dart-to-Rust signals now use generated typed request variants:
`AgentRuntimeRequestSignal { requestId, request }`. The request enum covers
connect, disconnect, hydrate, rehydrate, local discovery refresh/connect,
iCloud remote profile refresh/connect, imported profile import/refresh/connect,
and typed GUI operation dispatch. Dart does not send stream-consume requests;
the Rust hub starts, replaces, and cancels the selected-session stream
subscription from lifecycle and selected-session intents. GUI operation variants
cover sessions, approvals, command-registry operations, Role Admin operations,
and Workflow Memory selection/feedback.

Rust-to-Dart signals now use generated typed output variants:
`AgentRuntimeOutputSignal { requestId, output }`. Output variants cover
projection snapshots, controller-state updates, operation results, stream
outcomes, typed API errors, and the Rust-shaped
`AgentRuntimeWorkbenchViewModel` consumed by the Flutter shell.

The stable hub creates one long-lived `GuiTransportHandle`, and the transport
runner owns exactly one `GuiBackendController` inside a single async action
loop. Dart sends typed generated intent variants only; Rust resolves selected-session
hydration, WebSocket watermark semantics, operation success, projection
reduction, approval/command/process enablement, and typed errors. Packet
payloads are typed at the Rinf boundary; the stable hub performs the only
request/output mapping into the internal experimental transport structs.

File bootstrap discovery is Rust-owned bootstrap input. The transport reads the
canonical per-user discovery file by default:
`~/Library/Application Support/Robdex Agent Runtime/service/discovery.json` on
macOS, or `${XDG_STATE_HOME:-~/.local/state}/robdex-agent-runtime/service/discovery.json`
on non-macOS hosts. This is the same packet produced by
`scripts/agent-runtime-service.sh discover` / `json-status`. Rust classifies
local service state and emits constructor-ready discovery fields on
`AgentRuntimeWorkbenchViewModel`. Dart may render the
discovered target and send refresh/connect-discovered intents, but Dart must not
parse the discovery file, decide health semantics, construct WebSocket URLs, or
derive service state from pid/path fields. Running/healthy discovery enables a
one-step Rust-owned connect using the discovered `baseUrl`; stopped, stale pid,
unhealthy, missing-config, stale-discovery, missing-file, and parse-error states
remain diagnostics and do not pretend to be connected. Manual base URL entry
remains available as a fallback input.

iCloud remote profile discovery is also Rust-owned bootstrap input. The profile
is a tiny sync-safe sentinel, not live discovery, auth, tunneling, or mDNS:
`{"kind":"robdex.agent-runtime.remote-profile","version":1,"hostHint","port","scheme","updatedAt","label","metadata"}`.
The default host hint is `robertmsale._peer.internal`; the default Agent Runtime
port remains `8765`. Rust resolves the default profile path to
`~/Library/Mobile Documents/com~apple~CloudDocs/Robdex Agent Runtime/remote-profile.json`
on macOS, or honors `ROBDEX_AGENT_RUNTIME_ICLOUD_REMOTE_PROFILE_PATH` for tests
and non-iCloud development. `scripts/agent-runtime-service.sh write-icloud-profile`
writes the profile without bearer tokens, database URLs, raw credentials, or
unredacted sensitive paths. The profile only creates a candidate `baseUrl`.
Rust probes `/health` before marking it connectable and emits distinct
`remoteDiscovery` state/copy for missing, malformed, stale, unhealthy,
unreachable, and healthy remote profiles. Dart only sends refresh/connect-remote
intents and renders Rust-shaped fields.

Document-import remote profile bootstrap is an additional acquisition path for
the same iCloud profile JSON. The intended phone/macOS flow is: the Mac service
writes the profile to iCloud Drive with `write-icloud-profile`; the user imports
that JSON document; Rust treats it as untrusted, validates the schema/version,
stores a sanitized app-local copy at
`~/Library/Application Support/Robdex Agent Runtime/imported-remote-profile/remote-profile.json`
by default, and probes `/health` before `importedRemoteDiscovery` becomes
connectable. Override the storage path with
`ROBDEX_AGENT_RUNTIME_IMPORTED_REMOTE_PROFILE_PATH` or the directory with
`ROBDEX_AGENT_RUNTIME_IMPORTED_REMOTE_PROFILE_DIR` for deterministic tests and
development. Flutter may initiate the import affordance and pass a sanctioned
profile path to Rust, but it does not parse the JSON, construct URLs, or decide
health/connectability. Unsupported native picker paths return a typed Rust error
instead of pretending import succeeded. The original iCloud-path discovery and
service-wrapper writer remain intact.

After connect, `RuntimeProjection`, `GuiControllerState`, stream outcomes, and
`GuiOperationResult` packets from Rust are authoritative. Dart must not compute
watermarks, construct WebSocket URLs, apply reducers, decide approval or command
availability, or infer operation success.

The first Flutter-facing shared shell is implemented as a thin renderer
over the generated typed Rinf carriers. It sends typed
`AgentRuntimeRequestSignal` request variants and consumes typed
`AgentRuntimeOutputSignal` output variants. Dart stores
only widget/controller-local facts such as the base URL text, pending request
ids, and latest render packets; Rust remains responsible for service
connection, WebSocket URLs, watermarks, reducer application, selected-session
semantics, operation success, and typed errors.

The transport now emits a Rust-owned `AgentRuntimeWorkbenchViewModel` output
for the shared shell and operations detail. The view model is constructor-ready:
connection state, base URL, status and watermark labels, session rows, product chat rows, separate history rows, action rows, runtime facts, recent output log, pending-request slot,
and typed error display text are shaped in Rust from `RuntimeProjection`,
`GuiControllerState`, and operation/stream outcomes. Dart decodes this
Rust-shaped view packet and renders it; Dart no longer interprets raw
projection or controller JSON to derive rows, labels, facts, or enablement
text.

The richer shared-shell UX slice extends that Rust-owned view model with
Workbench chat-shell presentation fields: status badges, selected-session label,
section titles, empty-state copy, session group labels, row tones, action state
text, and action/timeline/session severity tones. The design-system shell renders those fields directly to provide a clearer status strip, denser
session rail, selected-session chat transcript, readable attention list, runtime
detail panel, and explicit empty/error/loading states. The attention list contains
only real attention items present in the projection: pending/resumable
approvals and typed pending/actionable command-registry request summaries.
Command registry inventory is surfaced as inventory count/status detail, not as
required action. Dart still sends only generated typed Rinf intents and does not
infer durable runtime meaning from raw projection/controller internals.

The shell remains focused: discovery/connect input, Rust-shaped view-model
rendering, selected-session chat visibility when present, an attention list
from Rust-owned approval/resume and command-registry request rows, explicit
disconnected/error states, and Rust-owned selected-session streaming through
typed Rinf outputs. Reusable visual pieces live in the design-system package
under the agent-runtime shared shell and operations-detail
components, with Design Lab scenarios for disconnected, connecting, connected,
error, empty/no-session, no workflow memories, populated workflow memories, and
selected workflow-memory detail/feedback states, iCloud remote-profile states,
and imported app-local profile states for missing, malformed/stale, healthy,
and unreachable/unhealthy profiles. Remaining gates are mDNS/Bonjour discovery,
native iOS file-picker polish beyond the typed import boundary, and root/system
service integration beyond the completed per-user LaunchAgent workflow.

Selected-session settings and owner controls use the full-screen session control
plane. The old narrow selected-session settings dialog is not the production
route. The control plane is rendered from the shared Robdex design-system widget
in both production and Design Lab. Rust/PostgreSQL/Rinf own the typed projection
for session identity, active model, God Mode state, current `managed_processes`
rows, selected-session approvals, selected-session command registry requests,
Requirements Review state, running server/image summaries, and quick-action
availability. Dart dispatches typed operations for saves, lifecycle, compaction,
God Mode, process control, approvals, command registry, and Requirements
actions; it does not reconstruct those states from generic operation-surface
display rows.

## Resident server MVP

The experimental server binary is `robdex-agent-runtime-server`. It is isolated
from stable Robdex and uses the same Postgres runtime state and runtime
functions as the CLI. There is no auth or user-session boundary in this slice;
the intended trust boundary is VPN/network placement.

### Local developer service script

For local development and per-user host packaging, use the Agent Runtime service
wrapper. It writes state under the canonical user-scoped service directory by
default. It supports per-user macOS launchd autostart through
`~/Library/LaunchAgents`; it does not install root-owned LaunchDaemons, use
sudo, modify systemd/supervisor, or touch stable Robdex service tooling.

```sh
scripts/agent-runtime-service.sh start
scripts/agent-runtime-service.sh status
scripts/agent-runtime-service.sh discover
scripts/agent-runtime-service.sh write-icloud-profile
scripts/agent-runtime-service.sh logs
scripts/agent-runtime-service.sh restart
scripts/agent-runtime-service.sh stop
scripts/agent-runtime-service.sh stop --force
scripts/agent-runtime-service.sh logs --tail
scripts/agent-runtime-service.sh default-state-dir
scripts/agent-runtime-service.sh install-user-service
scripts/agent-runtime-service.sh package-status
scripts/agent-runtime-service.sh uninstall-user-service
scripts/agent-runtime-service.sh install-launchd
scripts/agent-runtime-service.sh load-launchd
scripts/agent-runtime-service.sh launchd-status
scripts/agent-runtime-service.sh unload-launchd
scripts/agent-runtime-service.sh uninstall-launchd
```

The default service state directory is user-scoped and outside the repo:
`~/Library/Application Support/Robdex Agent Runtime/service` on macOS, or
`${XDG_STATE_HOME:-~/.local/state}/robdex-agent-runtime/service` on non-macOS
hosts. Override it explicitly with `ROBDEX_AGENT_RUNTIME_SERVICE_STATE_DIR` for
tests and development. Validation scripts use temp override directories and do
not mutate the developer's real service state. The state directory contains:

- `server.pid` for the resident server process;
- `server.stdout.log` and `server.stderr.log`;
- `effective-config.json` with the base URL, pid, log paths, policy values,
  server binary path, and redacted database target;
- `discovery.json` with the machine-readable local discovery packet consumed by
  the Rust/Rinf bootstrap path;
- `service-package.json` when the script-based per-user package contract is
  installed;
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
`$ROBDEX_AGENT_RUNTIME_SERVICE_STATE_DIR/discovery.json`, or at the canonical
per-user state directory when no override is set. GUI/Rinf bootstrap uses the
Rust transport to read that stable file without shelling out; shell callers can
use `discover` when they need a fresh packet. The packet uses redacted database
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

`install-user-service`, `package-status`, and `uninstall-user-service` provide
the current per-user host packaging affordance beyond ad hoc start/stop calls.
`install-user-service` resolves the existing resident server binary/path
contract and writes `service-package.json` in the same state directory.
`package-status` prints that package descriptor or a `notInstalled` packet.
`uninstall-user-service` stops the wrapper-managed process if present and
removes the package descriptor.

Per-user launchd autostart is available through `install-launchd`,
`load-launchd`, `launchd-status`, `unload-launchd`, and `uninstall-launchd`.
`install-launchd` writes a deterministic plist at
`~/Library/LaunchAgents/com.robdex.agent-runtime.experimental.plist` by default
and updates the same `service-package.json` descriptor. The plist runs the
existing service script with `start`, preserves the canonical or overridden
`ROBDEX_AGENT_RUNTIME_SERVICE_STATE_DIR`, writes launchd stdout/stderr under the
service state directory, and leaves discovery/config/pid/package state owned by
the existing wrapper. `load-launchd` uses `launchctl bootstrap gui/$(id -u)` and
fails closed if `launchctl` is unavailable or returns an error; it does not fall
back to a non-launchd start while claiming launchd is active. `unload-launchd`
uses `launchctl bootout` and then stops the wrapper-managed server so service
state remains coherent. `launchd-status` and `package-status` distinguish
`notInstalled`, `installedUnloaded`, `loadedRunning`, `loadedUnhealthy`, and
`staleUnknown` from launchctl state plus service health rather than plist file
presence alone. `uninstall-launchd` unloads, stops, removes the plist, and
updates package state. Validation does not load the owner's real launchd job;
manual validation can run these commands from the experiment workspace when the
owner wants to enable autostart.

Validate the user-scoped service wrapper, iCloud remote profile writer, and
package/discovery contract with an isolated Postgres validation database and no
live model, LM Studio, or embedding-provider calls:

```sh
scripts/validate-local-service.sh
scripts/validate-launchd-packaging.sh
```

The validation starts the local service, verifies `status` and `/health`, checks
startup log evidence, verifies `discover` output and persisted discovery file
content, verifies the redacted iCloud remote profile sentinel, verifies
duplicate-start refusal, exercises `logs`, restarts and
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
GET  /sessions/{sessionId}/requirements
POST /sessions/{sessionId}/requirements
POST /sessions/{sessionId}/requirements/clear
GET  /sessions/{sessionId}/requirements/packets
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
  -d '{"message":"Use execute_code with exactly this Starlark source: content = fs.read(\"Cargo.toml\"); output({\"validation\":\"ok\",\"contains_workspace\":\"workspace\" in content})"}'
```

Message submission is unified through the Rust server. Sending while a live
session is idle persists a submitted input with `idle_turn_start` disposition
and starts the next normal turn. Sending while the session already has active
runtime work persists the input with a steering or queued-continuation
disposition; the current runtime drains the durable queue serially without
starting a second concurrent running turn. Closed or archived sessions reject
before runtime placement with a typed terminal lifecycle error and create no
turn.

The same composer action is used for idle messages and active steering. The
GUI does not decide whether a send is a normal turn or steering; it sends the
typed operation to Rust and renders the accepted queued/steering state from the
projection. Hidden Requirements Review sessions stay out of normal session
lists, but the source session's Requirements Review detail can route
clarification/correction text to the nested reviewer through the same unified
submit path. If final output is already committed, Rust lets that response
finish unchanged, completes the current turn, and starts the next turn with the
submitted input. If compaction is active, Rust waits for compaction completion
and applies the input after the compacted context is durable.

Direct-final steering is same-turn: when steering is pending before a direct
assistant final response completes, Rust persists the assistant text as part of
the turn, marks the submitted input applied to that turn, rebuilds the model
request from completed history plus the current-turn transcript, and continues
the same turn. Tool-boundary steering is same-turn as well: after a durable tool boundary, including execute_code results that contain registry command, managed process, or God Mode shell output records, Rust applies pending steering to the same turn, rebuilds model input from completed history plus the current-turn transcript, and continues without starting a parallel turn.

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
`robdex_agent_runtime::gui_sync` for macOS/iOS Rust/Rinf GUI backends.
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
snapshot and WebSocket URLs. Runtime audit deltas append to History/Diagnostics;
selected-session chat remains a typed transcript while semantic deltas update session/admin summaries through
the projection reducer. GUI surfaces render from the reduced
`RuntimeProjection` and request a fresh snapshot whenever resync state is set.

Resident server deterministic validation uses a real
`robdex-agent-runtime-server` process on a local HTTP/WebSocket listener and an
isolated validation database. It does not call OpenAI, LM Studio, or embedding
providers. Run it from this nested workspace:

```sh
scripts/validate-resident-server.sh
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

Minimal live-server validation with `gpt-5.4-mini` is intentionally separate from
deterministic validation and remains explicitly opt-in. Start
`robdex-agent-runtime-server`, then run:

```sh
ROBDEX_AGENT_RUNTIME_LIVE_SERVER_VALIDATION=1 scripts/validate-live-server-gpt54mini.sh
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

## Output artifacts and bounded retrieval

The experimental runtime follows a store-everything/show-little output contract.
Full script, command, God Mode shell, and managed-process stdout and stderr are
persisted in PostgreSQL as separate `execution_output_artifacts` rows with
artifact ids, stream names, byte counts, line counts, traceability fields, and
cheap `bytes / 4` estimated-token metadata. Fake stdout-plus-stderr combined
artifacts are not part of the canonical runtime contract. Tokenizer-specific
accounting such as tiktoken is intentionally not part of this design.

`execute_code`, synchronous `cmd[...]` calls, God Mode shell calls, and
managed-process `flush_buffer()` return bounded envelopes instead of raw log
dumps. Envelopes contain stdout/stderr artifact handles plus preview/tail
excerpts and truncation/omission metadata. Completion events and
compaction-visible summaries reference separate stdout/stderr artifact handles
and bounded excerpts; full output remains in the artifact table.

Agents retrieve stored output intentionally inside Starlark with bounded helpers:

```python
artifact = outputs.last()
output(outputs.tail(artifact, lines=200))
output(outputs.head(artifact, lines=50))
output(outputs.slice(artifact, start_line=500, end_line=650))
output(outputs.search(artifact, "error", context=20))
output(outputs.stats(artifact))
```

Every retrieval helper enforces hard byte/line caps and reports returned,
omitted, and truncation metadata. The helpers operate on stored artifact ids and
do not dump unbounded stdout, stderr, or artifact bodies into model-visible
responses. Same-turn steering transcript reconstruction stores idempotent
metadata summaries for command, shell, and managed-process boundaries; hidden
stdout/stderr remains artifact-only unless a Starlark program explicitly emits a
bounded value through `output(...)`. Retrieval is session-scoped: an artifact id
alone is not sufficient to read output from a different session.

## Model input context management

Agent Runtime assembles production Responses requests statelessly in Rust. The
runtime does not rely on `previous_response_id`, and production role/runtime
context is not placed in the Responses `instructions` field. Each model turn is
assembled from PostgreSQL state into an explicit input array.

The first active developer item is a machine-readable role block:

```xml
<role_instructions epoch="role-key:role-version:role-version-id" role_key="..." role_version="...">
...
</role_instructions>
```

Role instructions are not additive. The latest `role_instructions` block is the
active role authority. The next developer item is a machine-readable runtime
snapshot:

```xml
<runtime_context epoch="...">
  <session_id>...</session_id>
  <project key="...">...</project>
  <cwd state="known|unavailable" source="session.workdir|unavailable">...</cwd>
  <role epoch="..." key="..." version="..." />
  <model>...</model>
  <tools command_context_id="..." visible_command_count="..." />
  <context_event_watermark>...</context_event_watermark>
</runtime_context>
```

Every session has either a canonical CWD or an explicit unavailable CWD state.
`Unassigned` means no project assignment; it does not silently mean unknown CWD.
When CWD is known, the model can answer CWD questions from developer context
without a tool call. When CWD is unavailable, context says so and CWD-dependent
runtime actions must be unavailable rather than guessed.

Runtime context snapshots and context events are durable PostgreSQL rows in
`session_context_snapshots` and `session_context_events`. Model request evidence
records role/context epoch metadata, the context event watermark, prompt cache
key, and whether compacted state was present. Dart does not author or inject
these messages; it sends only user/UI lifecycle intents through Rinf.

Tool/command registry changes, CWD changes, sandbox/policy changes, and role
metadata changes are represented as typed context events. The next model turn
receives bounded developer context rather than raw oversized registry JSON.

Same-role compaction may use the runtime compaction flow as a replacement base
window. If a retained visible window contains stale role/context blocks, the
Rust assembler normalizes the visible window before the next request. Role or
policy authority changes are role-epoch boundaries; opaque compaction state is
not an auditable authority boundary for security-sensitive role isolation.

## Compaction checkpoints

Long sessions stop replaying all completed turns once a durable compaction
checkpoint exists. Checkpoints live in PostgreSQL in
`compaction_checkpoints`; they record session id, status, source turn/event
boundary, compacted-through turn, bounded replacement context, estimate
metadata, model/provider metadata when present, timestamps, and failure
information. Compaction never deletes or rewrites audit rows: original turns,
model events, tool calls, script runs, output artifacts, approvals, managed
processes, and event-stream entries remain queryable.

Compaction runs through runtime-owned send preflight and preserves the same PostgreSQL audit boundary. The connected Workbench surface displays checkpoint history in the Compaction modal; it does not expose a primary manual compaction button until the typed GUI operation is enabled for completed-turn sessions.

Model reconstruction uses the latest completed checkpoint as a semantic root
and then appends completed turns after the checkpoint boundary. Forked sessions
inherit applicable checkpoints only through their fork boundary; they never
inherit parent turns completed after the fork.

Before a live model request, the runtime estimates model-visible request
surfaces using deterministic byte accounting over the same JSON/text request
shape used for dispatch: final instructions, input items, runtime messages,
tool schemas, and the current user message. The estimate is serialized bytes
divided by four, plus fixed reserves. `ROBDEX_AGENT_RUNTIME_CONTEXT_BUDGET`,
`ROBDEX_AGENT_RUNTIME_MAX_OUTPUT_RESERVE`,
`ROBDEX_AGENT_RUNTIME_PRE_SEND_COMPACTION_THRESHOLD`, and
`ROBDEX_AGENT_RUNTIME_FAIL_CLOSED_THRESHOLD` override the safe defaults without
calling a model metadata service. If the pre-send estimate exceeds the
pre-send threshold, the runtime compacts completed history, rebuilds context,
rebuilds the same model request shape from checkpoint-rooted history, and
continues only if the rebuilt estimate is below the fail-closed threshold.
Compaction failure paths record failed checkpoint rows with `failure_info`
before returning an error.

Replacement context is bounded model-visible session memory. It marks itself as
a compaction checkpoint, preserves owner-instruction guidance, active task
goal, important decisions, touched surfaces, blockers, pending approvals,
continuing process handles, latest actionable state, and output artifacts by
handle and bounded metadata. It explicitly keeps command discovery out of
persisted role instructions or stale command catalogs.
Tokenizer-specific accounting such as tiktoken remains intentionally out of
scope.

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
robdex-agent-runtime requirements set-json --session <session-id> requirements.json
robdex-agent-runtime requirements set-composed --session <session-id> --permanent project.json --include shared.yaml --task task.json
robdex-agent-runtime requirements set-lines --session <session-id> requirements.txt
robdex-agent-runtime requirements status <session-id>
robdex-agent-runtime requirements packets <session-id>
robdex-agent-runtime requirements detail <session-id>
robdex-agent-runtime requirements clear <session-id>
```

## Requirements Review

Requirements Review is a session-scoped contract stored in PostgreSQL. A source
session may have one active `RequirementSet`; canonical requirements, progress,
review bindings, and claim/verdict packets are durable audit records. Source
turn final responses are classified as requirement packets when an active set is
enforced. A reviewable claim packet creates or reuses exactly one nested
`requirementsReviewer` session for the source session and active set.

Requirements reviewer sessions are real Agent Runtime sessions: they have role
snapshots, turns, model events, tool calls, script runs, output artifacts,
compaction records, and audit events. They are also marked
`session_kind='requirementsReviewer'`, `parent_session_id=<source>`, and
`hidden=true`. Normal session lists, project rails, recent-session surfaces, and
ordinary session-count UX exclude these hidden reviewer sessions. Direct audit
inspection by exact reviewer id remains available through exact session routes,
and the selected source session exposes Requirements Review summary in its
typed projection field and metadata. WebSocket deltas emit
`RequirementsReviewUpdate` for selected-source changes; nested reviewer sessions
are never inserted into normal top-level session lists by those deltas.

The Workbench/Rinf boundary exposes Requirements Review through typed
`GuiOperationRequest`/Rinf operations: set a RequirementSet, clear/deactivate
the active set, show status, and list packet history. Dart sends those typed
intents and renders Rust-owned status/detail state; Dart does not validate
RequirementSets, choose reviewer sessions, route verdicts, or synthesize review
progress.

Composable Requirements are import/config artifacts. Activation merges
project-permanent files first, explicit include files second, and the task file
last; duplicate semantic keys are rejected before PostgreSQL persistence. After
activation, PostgreSQL rows are the runtime source of truth.

The seeded `requirements-reviewer` role is adversarial and non-implementing. Its
default policy permits bounded inspection (`fs.read`, `workflow_memory.search`,
and `tool.execute_code`) and explicitly denies mutation and command-registry
administration actions such as `fs.write`, `patch.apply`,
`command_registry.request`, `command_registry.decide`, and
`command_registry.apply`.

Model requests that can produce final assistant text receive a Requirements
structured-output schema only when an active enforced set exists for the source
or when the turn belongs to the nested reviewer. Schema evidence records schema
kind, RequirementSet id, canonical requirement count, unresolved requirement
count, and source/reviewer mode at the model boundary. Source schemas contain
only unresolved claims; reviewer schemas carry the full canonical contract so a
reviewer can re-fail prior passes.

## Starlark lifecycle orchestration

Agent Runtime now has generic lifecycle-orchestration primitives under Rust and
PostgreSQL ownership. Project runtime Starlark is deterministic config/decision
code: Rust validates source syntax, activation manifests, hook names, intent
types, routing targets, resource types, and schema metadata before activation;
Starlark never mutates PostgreSQL, sends raw messages, spawns OS processes,
executes shell, reads or writes arbitrary files, accesses the network, or calls
unregistered host capabilities. Runtime side effects happen only when Rust
applies validated typed intents.

The lifecycle boundary list is:
`on_project_runtime_activate`, `on_session_create_request`,
`on_session_created`, `on_turn_submitted`, `on_turn_start`,
`on_model_request`, `on_model_final`, `on_tool_start`, `on_tool_complete`,
`on_packet_recorded`, `on_resource_reserved`, `on_resource_released`,
`on_turn_complete`, `on_session_close`, `on_session_archive`, and
`on_compaction_complete`.

Hook context is an immutable bounded summary containing project/session
identity, session kind, parent id, hidden state, role snapshot summary, workdir,
worktree root, turn summary, triggering lifecycle event, active contracts,
recent packet summaries, subagent summaries, resource lease summaries, visible
command summaries, tool metadata, and routing state. Full hidden stdout, full
hidden stderr, full shell output, secrets, auth material, and unbounded chat
history are excluded.

Validated hooks return typed intents only: `require_output_schema`,
`record_packet`, `route_packet`, `notify_session`, `ensure_subagent`,
`close_subagent`, `reserve_resource`, `release_resource`,
`add_turn_obligation`, `update_contract_progress`,
`request_owner_approval`, and `block_with_reason`. Each lifecycle boundary has
a Rust-owned allowlist. Every accepted intent receives a stable idempotency key
derived from the hook source hash, lifecycle event id, session id, and
intent-specific key fields.

Generic runtime records support the orchestration model:

- project runtime config versions and hook bindings store source text, source
  hash, compiled manifest, activation status, author, validation packet, and
  activation/audit timestamps;
- lifecycle events and hook evaluations record context hashes, returned
  intents, validation status, applied intent ids, errors, and timing metadata;
- runtime packets and envelopes are first-class records separate from ordinary
  human messages;
- generic hidden subagents record parent session, subagent key, workflow
  identity, kind, role id, workspace policy, hidden projection behavior,
  lifecycle status, and audit metadata;
- generic contracts, contract progress, resource leases, turn obligations, and
  structured-output schema evidence provide workflow state without bespoke
  per-workflow transport.

The shipped seed examples live in `project-runtime-seeds/`:
`requirements_review.star` declares the Requirements Review contract workflow
using schema, packet, subagent, route, and progress intents, and
`simulator_stewardship.star` declares iOS simulator stewardship with role-level
tools, resource leases, steward routing, and turn-completion notices.

Requirements Review is shipped as a hook-defined contract workflow on these
generic primitives. Source-session schemas come from hook-emitted
`require_output_schema` intents; source claims become typed runtime packets;
reviewer sessions are hidden generic subagents created through
`ensure_subagent`; claim and verdict packets route through runtime envelopes;
verdict packets update generic contract progress; pass clears the active
contract, fail routes correction back to the source, and waiver-required
verdicts route owner action. The user-facing Requirements API/CLI/GUI names
remain product surfaces, but bespoke runtime routing, schema injection,
reviewer dispatch, and verdict-progress logic are not the source of truth.

Project runtime config has Rust-owned CLI surfaces for validation, import,
inspection, version listing, activation, archival, export, and review request:

```bash
robdex-agent-runtime runtime-config validate project-runtime-seeds/requirements_review.star
robdex-agent-runtime runtime-config import --project my-project --author operator runtime.star manifest.json
robdex-agent-runtime runtime-config show --project my-project
robdex-agent-runtime runtime-config versions --project my-project
robdex-agent-runtime runtime-config activate --project my-project <version-uuid>
robdex-agent-runtime runtime-config archive --project my-project <version-uuid>
robdex-agent-runtime runtime-config export --project my-project <version-uuid>
robdex-agent-runtime runtime-config request-review --project my-project <version-uuid>
```

The server exposes the same Rust-owned operations:

```text
POST /projects/{projectKey}/runtime-config/validate
GET  /projects/{projectKey}/runtime-config
POST /projects/{projectKey}/runtime-config
GET  /projects/{projectKey}/runtime-config/versions
POST /projects/{projectKey}/runtime-config/versions/{versionId}/activate
POST /projects/{projectKey}/runtime-config/versions/{versionId}/archive
GET  /projects/{projectKey}/runtime-config/versions/{versionId}/export
GET  /projects/{projectKey}/runtime-config/versions/{versionId}/evaluations
```

The Rust/Rinf GUI transport exposes the same operations as typed
`GuiTransportRequest` variants and typed `GuiOperationRequest` variants.

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

God Mode is a session-scoped break-glass host shell grant. A grant is stored in
PostgreSQL with the session id, granting actor, reason, active/revoked status,
timestamps, optional expiry, and audit events. While a grant is active,
`execute_code` exposes `shell(script, mode="-lc").sync()` and
`shell(script, mode="-lc").async()`. The shell affordance is a native Starlark
host affordance, not a command-registry entry: it does not create a command
definition or action id, and it does not use command-registry path, argument,
environment, mutation-class, or forbidden-argument policy. Enabled shell runs
use `/bin/zsh` with accepted modes `-lc`, `-l`, and `-c`; explicit `-l` invokes
zsh as a login shell with `-c` script execution instead of being collapsed into
plain `-lc`. Shell runs default to the session execution root, run as the Agent
Runtime service OS user, and persist audit rows,
events, process handles, and separate stdout/stderr output artifacts. Hidden
shell stdout/stderr is durable audit/retrieval data and is not copied into
same-turn model history unless a bounded value is explicitly emitted through
`output(...)`. Without an active grant the
`shell(...)` function fails closed with `God Mode required: shell(...) disabled`
and does not spawn a process. Closing or archiving a session revokes the active
grant; async shell processes are session-owned managed processes with
end-of-session behavior `terminate`.

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

Registry-defined command execution remains structured. Outside an active
session-scoped God Mode grant, there is no shell escape hatch: commands run only
through argv arrays, execution-root cwd enforcement, explicit env policy,
max-runtime/output limits, binary resolution policy, and the stored final
execution policy selected by the approver. A final policy of `allow` executes
immediately, `deny` leaves the command visible but blocks before side effects,
and `ownerApproval` or `orchestratorApproval` creates the matching approval
request and paused action before side effects. Role policy does not override
scoped command final execution policy. Each `command_runs` row records the exact `command_version_id` used
so historical traces remain attributable after later registry changes.

Role policy remains authority for native kernel actions such as
`tool.execute_code`, `tool.request_command_registry_change`, and
`command_registry.*`. Scoped DB command visibility and execution do not require
role policy entries for command action ids.

The model has two native tools available, but ordinary assistant replies do not
require a tool call. The model uses `execute_code` for current Starlark
execution when runtime work is needed, or `request_command_registry_change`
when the current registry lacks a needed command. `request_command_registry_change`
is a native model tool outside Starlark; it is not a `cmd[...]` helper, raw
shell, or execute_code workaround.
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

Manifest decision values are `allow`, `deny`, `ownerApproval`, and `orchestratorApproval`. Runtime policy maps approval decisions to `approvalRequired` and does not execute those actions in this task. Missing action policy defaults to deny. Policy is execution authority; `capabilities` are validated to exactly match policy keys so they cannot contradict enforcement. Sessions store immutable role snapshots at creation time; turns use the stored snapshot rather than rereading the latest manifest. The direct Responses adapter receives the model name and role snapshot from the session snapshot, then emits role instructions as a tagged developer input block through the Rust-owned model-input assembler. Reasoning effort is stored in the DB role version and snapshot but is not applied by the current direct adapter yet.


## Role Admin GUI/editor contract

The experimental Workbench shell now includes a structured Role Admin editor. PostgreSQL remains the canonical role source of truth: GUI-created and GUI-updated roles are converted by Rust into canonical role manifests with inline `instructionText`, validated through the same role manifest/routing/command-policy rules as CLI imports, and persisted as immutable rows in `role_versions`. The GUI editor never creates prompt files and never treats seed JSON files as runtime truth.

Server routes added for the editor:

- `GET /roles/editor/options` returns Rust-owned editor metadata such as known actions, decision values, routing modes, and default recipients.
- `POST /roles/editor/validate` accepts a `RoleEditorDraft` and returns structured validation without mutating the database.
- `POST /roles` creates a new DB-backed role version from a draft and fails if the role id already exists.
- `POST /roles/{roleId}/versions` appends a new immutable version from a draft and fails if the role id is absent or mismatched.
- Existing role inspection/export/version/activate/archive/unarchive routes remain the mutation and rollback surface.

The Rust GUI operation vocabulary includes direct-result role operations for metadata, validation, inspection, version listing, version detail, and export. Create, update, activate, archive, and unarchive operations are wait-for-delta mutations; role changes are visible through `RuntimeProjection.roles` and role semantic deltas. The Rust-owned `AgentRuntimeWorkbenchViewModel` exposes a `roleAdmin` section with role rows, selected role detail, version rows, draft summary, validation errors, and role action states. Dart renders those constructor-ready fields and may hold only ephemeral editor/controller text state.

The design-system Role Manager is a full-screen page launched from the Runtime Operations Role Admin entry. It provides editable controls for role identity, version, display name, model defaults, capabilities, policy decisions, routing/default recipient/allowed recipients/reserved actions, visibility, lifecycle authority, validation feedback, version history, activation, archive/unarchive, export, and inspection actions. It uses `code_forge` as the Markdown instruction editor; edited CodeForge content is included as inline `instructionText` in validate/create/update draft submissions. Static Design Lab/workbench-shell mock states render the same exported page component used by the product route.

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
  -d '{"model":"qwen3-embedding-4b-dwq","input":"workflow memory validation input"}'
```

The optional validation helper is explicitly opt-in and performs only an embedding call:

```sh
ROBDEX_AGENT_RUNTIME_EMBEDDING_PROVIDER=lmstudio \
ROBDEX_AGENT_RUNTIME_EMBEDDING_BASE_URL=http://localhost:1234 \
ROBDEX_AGENT_RUNTIME_EMBEDDING_MODEL=qwen3-embedding-4b-dwq \
scripts/validate-lmstudio-embeddings.sh
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
scripts/validate-requirements-review.sh
```

Server/admin deterministic validation is covered by `cargo test` from this
nested workspace. It uses migrated temporary Postgres databases and does not
call a live OpenAI model or LM Studio. The resident server process validation is:

```sh
scripts/validate-resident-server.sh
```

The live server validation remains env-gated:

```sh
ROBDEX_AGENT_RUNTIME_LIVE_SERVER_VALIDATION=1 scripts/validate-live-server-gpt54mini.sh
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

## Selected chat and history boundary

The connected Agent Runtime shell treats the center panel as product chat, not as an event-stream viewer. Rust shapes selected-session chat entries from user turn input, assistant final responses, and canonical tool rows. Raw runtime events such as `role.imported`, `turn.started`, `policy.decision`, `model.final_response`, approval events, command-registry events, workflow-memory events, and compaction events remain audit history and must render in History or Diagnostics detail surfaces.

Submitted steering inputs are part of selected-session chat ordering. Active
turn steering appears inside the owning turn after the initial user input and
before later durable tool or assistant rows. Inputs queued after committed final
output appear as the next turn's first user input after the prior final
response. Inputs queued during compaction appear only after the compaction
handoff has produced a durable placement. The audit row remains in
`submitted_inputs` and the event stream even when the visible chat entry is
rendered from turn placement.

The Rust/Rinf view model keeps those concepts separate: typed `AgentRuntimeChatEntry` values feed the shared `ChatTimeline`; history and operations DTO fields feed modal or sheet surfaces. Dart maps each chat entry to `ChatEntry` one-to-one and must not translate raw runtime event names into chat messages.

## Robdex streaming transport notes

Workbench hydrate, thread selection, resync, and recovery may use full snapshots. Streaming hot paths should prefer bounded selected-chat deltas and coalesced native emissions so token bursts do not repeatedly decode full `WorkbenchViewData` snapshots in Dart. Local diagnostics should be used to confirm snapshot decode counts, selected-chat delta applies, emitted native signals, and coalesced/dropped stream updates while developing streaming changes.

## Agent Runtime starter kit

The starter kit exposes Rust-owned native Starlark helpers through `execute_code` without granting arbitrary shell or host access. All new file, tree, image, git, server, and tooling request helpers resolve CWD-relative paths against the session execution root, reject parent traversal, symlink escapes, absolute paths outside the root, and `.git` internals, and record bounded audit metadata in PostgreSQL.

Native file and tree helpers are `file.head`, `file.tail`, `file.read_lines`, `file.line_count`, `file.search`, `file.replace_exact`, `tree.list`, and `tree.find`. File reads are bounded and line-numbered; binary files are rejected by default. Mutations require concise non-generic descriptions before side effects. Existing `fs.write` and `patch.apply` now require mutation descriptions and use the same path policy.

Worker-safe git helpers are limited to `git.status`, `git.diff`, `git.restore`, `git.add`, and `git.commit`. They validate explicit paths, reject broad restores and `.git` internals, and never expose branch surgery, reset, cherry-pick, reflog, merge, pull, fetch, or push affordances.

Runtime-owned server helpers are `server.start`, `server.status`, `server.url`, `server.wait_ready`, `server.logs`, and `server.stop`. `server.start` accepts a visible registry command action, allocates a runtime-owned port, injects `PORT`, spawns the command through managed-process machinery, records the process/server metadata, and rejects user-specified ports. Server state is projected into selected-session state with handle, status, URL, port, readiness, and actions.

Image artifact helpers are `image.capture_from_file` and `image.describe`. Captured images are stored as first-class PostgreSQL artifacts with MIME type, byte count, dimensions when determinable, retrieval metadata, and binary content outside model-visible transcript text. Selected-session projection exposes artifact handles and bounded metadata so Requirements or design-evidence workflows can route actual image artifacts rather than path-only claims.

Screenshot capture contracts use the same image artifact storage model. Future simulator, browser, and Design Lab capture tools must create `starter_image_artifacts` rows, return `imageArtifactId` handles, expose metadata/thumbnail/full retrieval through the Rust API, and attach reviewer/model evidence as image artifacts rather than local paths. Requirements-native design claims must include the image artifact id, capture method, viewport or device, and reviewed screen/component/flow.

Missing or insufficient tooling must use `tooling.request(title, need, attempted, proposed="", urgency="normal")`. The runtime stores a typed request packet with source session, role, project, turn, script/tool linkage, bounded attempted evidence, routing metadata, and reviewable status. Project Progenitor is shipped as a project-local role for proposing roles, bundles, hooks, server profiles, tool bundles, workflow memory seeds, and documentation through typed approval paths; it has no authority to edit global skills, unrelated projects, or owner secrets.

Default starter-kit bundles are defined for worker, designer, QA, orchestrator, Project Progenitor, simulator steward, and operator/admin roles. Worker-like roles receive bounded file/tree reads, safe mutations according to policy, safe git helpers, output artifact helpers, and `tooling.request`; designers and QA receive screenshot/image and observation helpers without simulator-global repair tools; orchestrators receive lifecycle/requirements/integration and packet triage affordances without ordinary implementation mutation helpers; operator/admin bundles retain repair affordances with audit boundaries.
