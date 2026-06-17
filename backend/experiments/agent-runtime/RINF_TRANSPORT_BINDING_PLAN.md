# Agent Runtime Rinf Transport Binding Plan

This is the binding record for the Agent Runtime Control Tower Rinf boundary. The generic JSON-string carrier has been replaced by generated typed Rinf request and output signals. Dart sends typed generated request variants; the stable hub maps them to the existing Rust-owned `GuiTransportHandle`/`GuiBackendController` path; Rust maps internal outputs back to typed generated output variants.

## Implemented typed binding

Stable hub files:

- `frontend/robdex_app/native/hub/src/signals/agent_runtime.rs`
  - Defines `AgentRuntimeRequestSignal { request_id, request }`.
  - Defines typed `AgentRuntimeRequest` variants for connect, disconnect, hydrate, rehydrate, local discovery refresh/connect, iCloud profile refresh/connect, imported profile import/refresh/connect, stream polling, and typed GUI operation dispatch.
  - Defines typed `AgentRuntimeGuiOperation` variants for session operations, approval operations, command-registry operations, Role Admin operations, and Workflow Memory selection/feedback.
  - Defines `AgentRuntimeOutputSignal { request_id, output }` plus typed output variants for projection snapshot, controller state, operation result, stream outcome, typed error, and Control Tower view model.
- `frontend/robdex_app/native/hub/src/runtime.rs`
  - Owns one long-lived `GuiTransportHandle`.
  - Maps generated typed request variants into internal `GuiTransportRequest`/`GuiOperationRequest` values.
  - Maps internal `GuiTransportOutput` values into generated typed output variants.
  - Spawns Agent Runtime work off the main workbench loop and preserves request-id correlation.
- `frontend/robdex_app/lib/src/agent_runtime/agent_runtime_control_tower_controller.dart`
  - Calls generated typed Rinf signal constructors.
  - Renders Rust-shaped `AgentRuntimeControlTowerViewModel` output data.
  - Keeps only widget-local state such as text input, pending request ids, and latest received view model.

Generated Rinf artifacts changed by `rinf gen` under `frontend/robdex_app/lib/src/bindings/signals/` include the Agent Runtime request/output signal files, request/output enum files, nested Control Tower view-model files, nested Role Admin files, nested Workflow Memory files, and signal handler/barrel updates.

## Boundary rules

Dart must not parse profile files, construct runtime URLs from discovery fields, probe health, apply reducers, calculate stream watermarks, infer operation success, infer approval/command/process enablement, or own durable runtime state. Those responsibilities remain in Rust:

- `backend/experiments/agent-runtime/crates/robdex-agent-runtime/src/rinf_transport.rs` owns the experiment-local transport runner and internal packet/action loop.
- `backend/experiments/agent-runtime/crates/robdex-agent-runtime/src/gui_backend.rs` owns `RuntimeSyncClient`, projection/controller state, selected-session reconnect semantics, operation dispatch, and stream polling.
- `backend/experiments/agent-runtime/crates/robdex-agent-runtime-projection/src/lib.rs` owns `RuntimeProjection`, `RuntimeDelta`, `GuiControllerState`, `GuiOperationRequest`, `GuiOperationResult`, and `ApiErrorPacket`.

The stable hub is an adapter. It does not duplicate runtime state and does not add a second discovery, profile, reducer, or operation vocabulary.

## Discovery and service bootstrap

Local file discovery, iCloud profile discovery, and imported profile discovery remain Rust-owned bootstrap providers. Generated typed request variants trigger refresh/connect intents. Rust reads the canonical per-user discovery/profile paths, validates/parses files, probes `/health`, classifies connectability, and emits typed Control Tower discovery view fields.

## Remaining gates

The typed Rinf boundary is implemented. Remaining gates are outside this binding plan: remote/mDNS discovery beyond the profile sentinel/import flows, iOS remote profile sync beyond document import, and future UX expansion that preserves Rust-owned view-model semantics.
