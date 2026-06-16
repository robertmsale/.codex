# Agent Runtime Rinf Transport Binding Plan

This is the binding record for connecting Dart/Rinf packets to the experimental
agent runtime `GuiTransportHandle` and `GuiBackendController` path. The owner
selected the direct-dependency strategy. The first stable hub Rust binding is
implemented. Later slices mounted the Flutter-facing control-tower shell,
design-system scenarios, user-scoped service packaging, and per-user launchd
autostart on this transport. Service supervisor changes outside the
experimental per-user LaunchAgent flow and stable Robdex production behavior
remain out of scope.

## Implemented direct-dependency binding

The direct binding touches these stable hub files:

- `frontend/robdex_app/native/hub/Cargo.toml`
  - Adds a direct dependency on
    `backend/experiments/agent-runtime/crates/robdex-agent-runtime`.
  - Adds the same `tokio-tungstenite`/`tungstenite` patch used by the
    experimental runtime workspace so the direct dependency resolves when the
    hub is checked as its own Cargo project.
- `frontend/robdex_app/native/hub/Cargo.lock`
  - Updated by the hub Cargo check after adding the direct dependency.
- `frontend/robdex_app/native/hub/src/signals/agent_runtime.rs`
  - Adds `AgentRuntimeRequestSignal { request_id, packet_json }`.
  - Adds `AgentRuntimeOutputSignal { request_id, output_json }`.
- `frontend/robdex_app/native/hub/src/signals/mod.rs`
  - Exports the new signal module.
- `frontend/robdex_app/native/hub/src/runtime.rs`
  - Creates one long-lived `GuiTransportHandle` on the Rust side.
  - Receives Dart-originated agent-runtime request signals.
  - Validates that carrier `request_id` matches the JSON packet id.
  - Forwards packets to `GuiTransportHandle`.
  - Emits every `GuiTransportOutputPacket` through
    `AgentRuntimeOutputSignal`, preserving request-id correlation.

Generated Rinf binding files changed by `rinf gen`:

- `frontend/robdex_app/lib/src/bindings/signals/agent_runtime_request_signal.dart`;
- `frontend/robdex_app/lib/src/bindings/signals/agent_runtime_output_signal.dart`;
- `frontend/robdex_app/lib/src/bindings/signals/signals.dart`;
- `frontend/robdex_app/lib/src/bindings/signals/signal_handlers.dart`.

The binding is intentionally a transport only. Dart carries JSON packets and
renders outputs. Rust owns service connection, selected-session behavior,
WebSocket URL/watermark handling, reducer application, operation semantics, and
typed errors.

## Current binding boundary

Experimental runtime source-of-truth files:

- `crates/robdex-agent-runtime/src/rinf_transport.rs`
  - Defines `GuiTransportRequestPacket`, `GuiTransportRequest`,
    `GuiTransportOutputPacket`, `GuiTransportOutput`,
    `GuiStreamOutcomePacket`, and `GuiTransportHandle`.
  - Owns a single async action loop around one `GuiBackendController`.
- `crates/robdex-agent-runtime/src/gui_backend.rs`
  - Owns `RuntimeSyncClient`, the current `RuntimeProjection`,
    `GuiControllerState`, selected-session state, the owned WebSocket stream,
    operation dispatch, and `next_stream_outcome()`.
- `crates/robdex-agent-runtime-projection/src/lib.rs`
  - Defines the shared typed contract: `RuntimeProjection`, `RuntimeDelta`,
    `GuiControllerState`, `GuiOperationRequest`, `GuiOperationResult`, and
    `ApiErrorPacket`.

Stable app/hub binding files:

- `frontend/robdex_app/native/hub/src/lib.rs`
  - Starts `runtime::run()` and uses `write_interface!()`. No agent-runtime
    special case is required in this file.
- `frontend/robdex_app/native/hub/src/runtime.rs`
  - Owns the stable hub action loop.
  - Creates one `GuiTransportHandle`.
  - Receives `AgentRuntimeRequestSignal`, validates packet id correlation,
    forwards packets to the handle, and emits output packets.
- `frontend/robdex_app/native/hub/src/signals/agent_runtime.rs`
  - Defines the Rinf signal carriers.
- `frontend/robdex_app/native/hub/src/signals/mod.rs`
  - Exports the agent-runtime signal module.

## Implemented binding strategy

The stable-hub adapter forwards generated Dart signals to `GuiTransportHandle`
and forwards `GuiTransportOutputPacket`s back to Dart. The adapter is not a
second runtime state owner.

Required flow:

1. Stable hub receives `AgentRuntimeRequestSignal` carrying a request id and a JSON request
   packet.
2. Stable hub deserializes that JSON into `GuiTransportRequestPacket`.
3. Stable hub sends the packet to a single long-lived `GuiTransportHandle`.
4. `GuiTransportHandle` serializes all access through its action loop and
   drives one `GuiBackendController`.
5. `GuiBackendController` performs service connection, hydration, dispatch,
   selected-session rehydration/reconnect, stream polling, reducer application,
   and typed error mapping.
6. Stable hub emits every returned `GuiTransportOutputPacket` to Dart through
   `AgentRuntimeOutputSignal`.
7. Dart renders packets and sends the next typed intent. Dart does not infer
   durable runtime semantics.

The stable binding keeps packet payloads JSON-backed at the hub boundary.
Generated bindings carry:

- `request_id: String`;
- `packet_json: String` for Dart-to-Rust requests;
- `output_json: String` for Rust-to-Dart outputs;

This keeps generated Rinf schemas stable while the experimental projection
contract continues to evolve. Later, owner-approved implementation may promote
individual packet fields to generated signal fields only when the runtime
contract is stable enough to justify binding churn.

## Current affected files

Experiment-local files that remain the runtime source of truth:

- `backend/experiments/agent-runtime/crates/robdex-agent-runtime/src/rinf_transport.rs`
  - Request/output packet definitions.
  - Serialized action-loop ownership of `GuiBackendController`.
  - Transport-level tests and packet-shape proof.
- `backend/experiments/agent-runtime/crates/robdex-agent-runtime/src/gui_backend.rs`
  - Rust-owned operation dispatch and stream polling.
- `backend/experiments/agent-runtime/crates/robdex-agent-runtime/src/gui_sync.rs`
  - HTTP/WebSocket sync client.
- `backend/experiments/agent-runtime/crates/robdex-agent-runtime-projection/src/lib.rs`
  - Shared projection, reducer, local controller state, operation vocabulary,
    operation outcomes, and `ApiErrorPacket`.
- `backend/experiments/agent-runtime/RINF_TRANSPORT_BINDING_PLAN.md`
  - This implemented binding record.
- `backend/experiments/agent-runtime/README.md`
  - Runtime-side contract documentation.
- `backend/experiments/agent-runtime/ROADMAP.md`
  - Current roadmap/source-of-truth state.

Stable hub files changed by the implemented direct binding:

- `frontend/robdex_app/native/hub/Cargo.toml`;
- `frontend/robdex_app/native/hub/Cargo.lock`;
- `frontend/robdex_app/native/hub/src/runtime.rs`;
- `frontend/robdex_app/native/hub/src/signals/agent_runtime.rs`;
- `frontend/robdex_app/native/hub/src/signals/mod.rs`.

Generated Dart binding files changed by `rinf gen`:

- `frontend/robdex_app/lib/src/bindings/signals/agent_runtime_request_signal.dart`;
- `frontend/robdex_app/lib/src/bindings/signals/agent_runtime_output_signal.dart`;
- `frontend/robdex_app/lib/src/bindings/signals/signals.dart`;
- `frontend/robdex_app/lib/src/bindings/signals/signal_handlers.dart`.

Known stable app files intentionally not changed in this slice:

- `frontend/robdex_app/lib/src/app/robdex_app.dart`;
- `frontend/robdex_app/lib/src/core/state/workbench_controller.dart`;
- `frontend/robdex_app/packages/robdex_design_system/lib/robdex_design_system.dart`;
- `frontend/robdex_app/packages/design_lab/lib/main.dart`;
- stable Robdex backend, supervisor, bridge, database, and host service files.

## Generated-binding implications

The direct binding uses two generic generated signals:

- Dart to Rust: `AgentRuntimeRequestSignal { request_id, packet_json }`.
- Rust to Dart: `AgentRuntimeOutputSignal { request_id, output_json }`.

Reasons:

- `RuntimeProjection` and `RuntimeDelta` are intentionally still evolving.
- JSON-backed payloads avoid regenerating Dart bindings for every projection
  field addition.
- `GuiOperationRequest` already provides the typed operation vocabulary in Rust;
  Dart should send serialized intent packets, not duplicate operation logic.
- Generated Dart classes remain stable carriers rather than durable runtime
  model authorities.

If the owner later approves field-level generated bindings, the promotion order
is:

1. Request/output envelope metadata (`requestId`, `type`).
2. `GuiOperationRequest` intent fields that have stopped changing.
3. Stable local controller state fields.
4. Projection summaries after first GUI shell validation.

Do not generate Dart types that require Dart to calculate session lifecycle,
approval availability, command visibility, process status, WebSocket state, or
operation success.

## Packet ownership and lifetime rules

`GuiTransportHandle` ownership:

- One handle per hub runtime instance.
- The handle is created once during agent-runtime GUI backend initialization.
- The handle owns a single action channel to a single `GuiBackendController`.
- All Dart-originated packets enter that channel; direct controller access from
  Dart-facing code is forbidden.

Request packet lifetime:

- Dart assigns a request id before sending.
- Rust treats the request packet as an intent, not state.
- Rust emits one or more output packets with the same request id.
- Dart may correlate outputs to local UI affordances but must not decide
  semantic success independently of `GuiOperationResult` or `ApiErrorPacket`.

Projection and controller-state lifetime:

- Rust owns the current projection and controller state.
- Rust emits snapshot/controller packets after connect, hydrate, rehydrate,
  operation dispatch when available, and stream polling outcomes.
- Dart may cache the latest packet for rendering only.
- Dart must replace render state from incoming packets; it must not merge
  persistent runtime state using Dart-side reducers.

WebSocket stream lifetime:

- `GuiBackendController` owns the server WebSocket stream after connect,
  hydrate, rehydrate, and selected-session changes.
- Dart requests stream progress by sending `PollStreamOnce`.
- Rust reads one server message, applies reducer/resync/shutdown logic, and
  returns a typed `StreamOutcome`.
- Dart does not construct WebSocket URLs, choose watermarks, or decide selected
  session reconnect semantics.

Error lifetime:

- Rust maps transport, sync, HTTP, WebSocket, JSON, and protocol failures into
  `ApiErrorPacket`.
- Dart displays typed error packets and may let the user retry.
- Dart does not parse raw Rust error strings for runtime decisions.

Shutdown lifetime:

- When `serverShutdown` arrives, Rust emits `StreamOutcome::ServerShutdown`
  and updates controller state.
- Dart renders shutdown/disconnected state and waits for a user or controller
  reconnect intent.

## Service discovery bootstrap flow

GUI startup should use the canonical user-scoped local service discovery
contract when it needs to discover the resident server:

1. Dart sends `GuiTransportRequest::RefreshDiscovery` when it needs local
   service status. The request may carry an explicit discovery path; otherwise
   Rust reads the deterministic default
   `~/Library/Application Support/Robdex Agent Runtime/service/discovery.json`
   on macOS, or
   `${XDG_STATE_HOME:-~/.local/state}/robdex-agent-runtime/service/discovery.json`
   on non-macOS hosts. This is the same JSON packet produced by
   `scripts/agent-runtime-service.sh discover` / `json-status`.
2. Rust parses and classifies the packet inside `GuiTransportHandle`, then emits
   `AgentRuntimeControlTowerViewModel.discovery` with constructor-ready state,
   tone, title, message, URLs, runtime identity, connectability, and
   diagnostics. Dart renders those Rust-shaped fields and does not parse the
   discovery file or inspect pid/path/health internals.
3. If the file is absent, malformed, stopped, stale-pid, unhealthy,
   missing-config, or stale-discovery, Rust marks the discovery target as not
   connectable. Dart may show manual base URL input as fallback and may send a
   refresh intent, but it must not infer a runtime target from pid, log, config,
   health, or path fields.
4. If Rust classifies the packet as running/healthy and a `baseUrl` is present,
   Dart may send `GuiTransportRequest::ConnectDiscoveredRuntime`. Rust then
   refreshes discovery, verifies the target is still connectable, and dispatches
   the Rust-owned connect/hydrate/WebSocket path using the discovered `baseUrl`.
5. Rust resolves actual hydration, WebSocket connection, watermark, selected
   session, operation success, and errors through `GuiTransportHandle`.

The binding treats service discovery as bootstrap input only. The runtime
projection remains authoritative after connect.

## Remaining owner-approval gates

The owner has approved and the implementation has completed:

- direct dependency from `frontend/robdex_app/native/hub` to the experimental
  runtime crate;
- Rinf `DartSignal`/`RustSignal` carrier structs for JSON packets;
- generated Dart binding refresh for those carriers;
- minimal stable hub runtime forwarding to `GuiTransportHandle`.

The following still require explicit owner approval before implementation:

- adding root/system LaunchDaemons, sudo service installation, or host
  service-manager changes beyond the completed per-user LaunchAgent flow;
- changing stable Robdex bridge, supervisor, database, or production runtime
  behavior;
- adding remote/mDNS/iOS discovery;
- adding broader UI surfaces beyond the current control-tower shell.

## Dart thin-transport rule

Dart responsibilities:

- send user intents as `GuiTransportRequestPacket`;
- render `GuiTransportOutputPacket` values;
- manage widget-local facts such as focus, scroll, text editing, hover/press,
  animation, and layout;
- show typed diagnostics from `ApiErrorPacket` and discovery packets.

Dart forbidden responsibilities:

- session lifecycle decisions;
- approval availability or resumability;
- command visibility or policy;
- role status;
- process status;
- timeline interpretation;
- WebSocket URL construction, after-watermark selection, or stream continuity;
- operation semantic success;
- projection reducer application.

Rust responsibilities:

- own `GuiTransportHandle`;
- own `GuiBackendController`;
- hydrate snapshots and reconnect streams;
- apply all runtime deltas through `RuntimeProjection` reducer logic;
- map every failure to typed packets.

## Remaining decision gates

Next implementable decisions:

1. Whether to add remote/mDNS/iOS discovery.
2. Whether to add broader UI surfaces beyond the current control-tower shell.
3. Whether to add stable Robdex production service integration.

Remote discovery, broader UI, root/system service installation, or stable
production-service Requirements should not be set without explicit owner
approval for that slice.
