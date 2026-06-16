# Agent Runtime Roadmap

This roadmap is a planner-facing source of truth for the experimental agent
runtime in `backend/experiments/agent-runtime`. It is intentionally lightweight:
use it to avoid rediscovering settled decisions after compaction, not as a full
product specification.

## Status

- **Current milestone:** Build an alternative, PostgreSQL-backed agent runtime
  that can eventually power a macOS/iOS GUI without depending on stable Robdex
  bridge internals.
- **Current active slice:** Rust-owned control-tower view shaping is completed; launchd and stable backend/supervisor integration have not started.
- **Current implementation owner:** Codex Config Operator.
- **Planner stance:** Rust owns GUI runtime synchronization, operation dispatch, and durable state decisions; Flutter UI remains out of scope until explicitly assigned.

## Owner principles

- PostgreSQL is the canonical runtime source of truth.
- JSON/Markdown files are seed/import/export artifacts unless explicitly stated
  otherwise.
- No legacy tombstones, fallback tooling, or deprecated duplicate paths.
- Tool/function schemas must remain cache-stable; dynamic context belongs later
  in request input or runtime-managed context, not schema text.
- Roles are DB-backed, hot-updatable runtime policy/config records.
- Command registry entries are live DB data, not const-compiled app behavior.
- Workers request command-registry changes; approvers choose final scope and
  policy.
- Durable state belongs in Rust/Postgres. Dart may own only widget-local
  ephemeral state.
- Requirements should be task-specific. Project-wide invariants belong in
  composables or role/project docs.
- Validation scripts must use isolated validation databases and must not mutate
  the normal runtime database by default.

## Source of truth map

- Workspace root: `backend/experiments/agent-runtime/`
- Shared projection/reducer types:
  `crates/robdex-agent-runtime-projection/src/lib.rs`
- Runtime DB/schema/helpers: `crates/robdex-agent-runtime/src/db.rs`
- Server/API/WebSocket routes: `crates/robdex-agent-runtime/src/server.rs`
- Snapshot/delta adapters: `crates/robdex-agent-runtime/src/projection.rs`
- Runtime send loop/model boundary: `crates/robdex-agent-runtime/src/runtime.rs`
- Model adapter: `crates/robdex-agent-runtime/src/model/`
- Starlark execution host: `crates/robdex-agent-runtime/src/starlark_host/`
- Command registry: `crates/robdex-agent-runtime/src/command_registry.rs`
- Workflow memory: `crates/robdex-agent-runtime/src/workflow_memory.rs`
- Resident operations/startup/shutdown: `crates/robdex-agent-runtime/src/operations.rs`
- GUI sync client prototype: `crates/robdex-agent-runtime/src/gui_sync.rs`
- Rust/Rinf GUI backend controller: `crates/robdex-agent-runtime/src/gui_backend.rs`
- Experimental Rinf-shaped transport proof:
  `crates/robdex-agent-runtime/src/rinf_transport.rs`
- Rust-owned control-tower view model:
  `crates/robdex-agent-runtime/src/rinf_transport.rs`
- Rinf transport binding plan: `RINF_TRANSPORT_BINDING_PLAN.md`
- Stable hub direct binding files:
  `frontend/robdex_app/native/hub/Cargo.toml`,
  `frontend/robdex_app/native/hub/src/signals/agent_runtime.rs`,
  `frontend/robdex_app/native/hub/src/signals/mod.rs`, and
  `frontend/robdex_app/native/hub/src/runtime.rs`
- First-shell GUI planning artifact: `CONTROL_TOWER_GUI_PLAN.md`
- Local service scripts: `scripts/agent-runtime-service.sh` and
  `scripts/validate-local-service.sh`
- Local service discovery file: `.runtime-service/discovery.json` by default
  when the experiment-local service wrapper is used.
- README/user docs: `README.md`

## Completed slices

1. Nested experimental runtime workspace isolated from stable Robdex.
2. Host PostgreSQL-backed state and schema initialization.
3. Starlark `execute_code` runtime surface.
4. Kernel hardening around filesystem, git protection, mutation actions, and
   execution roots.
5. DB-backed role policy system with import/export/archive/version behavior.
6. Approvals and role routing.
7. Action-only approval resume.
8. Test hygiene and isolated validation DB helpers.
9. DB-backed typed command registry.
10. Agent-facing scoped command-registry requests.
11. Session-only async process manager.
12. Persistent session lifecycle: metadata, archive, close, fork, history.
13. Approver ergonomics for command-registry review/preview/decision flow.
14. Role admin backend routes/CLI.
15. Agent-led workflow memory using pgvector/LM Studio embeddings.
16. Projection-first snapshot/reducer foundation.
17. Resident server MVP.
18. Semantic WebSocket deltas.
19. Core admin APIs.
20. Typed API error shape and error mapping polish.
21. Resident server smoke harness.
22. Runtime operations hardening and local service wrapper.
23. GUI integration foundation via shared projection and sync client prototype.
24. Cache-stable command discovery with synthetic runtime command context and
    Starlark `cmd.describe()` affordances.
25. Validation-script cutover for explicit CLI binary selection and sanitized
    model request evidence.
26. GUI contract + proof: typed local controller state, operation vocabulary,
    Dart responsibility boundary, approval/command control enablement, and
    projection/reducer proof tests.
27. GUI API gap audit: every `GuiOperationRequest` has a documented and
    code-backed route/local-action mapping; approval and command-registry
    request shape mismatches are resolved; role-admin mutations and
    workflow-memory inspection operation intents are explicitly deferred.
28. Rust/Rinf GUI backend controller boundary: a Rust-owned dispatcher owns
    `RuntimeSyncClient`, owned WebSocket stream handle, `RuntimeProjection`,
    `GuiControllerState`, selected session, connection/resync state, transient
    errors, operation dispatch, and typed `GuiOperationResult` emission for a future thin Rinf layer. The controller also exposes a public owned-stream polling method for consuming one WebSocket server message at a time through the shared reducer.
29. Control tower GUI plan: first-shell direction is documented as an
    operations control tower, not a chat-first interface; the artifact defines
    operational attention jobs, information architecture, screens, runtime
    states, Dart/Rinf boundaries, visual risk controls, design-system handoff
    contract, and source-of-truth files. This is planning only and does not
    start Flutter implementation.
30. Experimental Rinf transport proof: an experiment-local Rust module defines
    stable Dart-to-Rust request envelopes and Rust-to-Dart output envelopes for
    driving `GuiBackendController`, owns exactly one controller through a
    serialized async action loop, uses JSON-backed projection/controller
    payloads where schemas should remain stable, maps errors to
    `ApiErrorPacket`, and proves connect/hydrate, operation dispatch, owned
    stream polling, typed errors, and disconnect without modifying
    `frontend/robdex_app` or the stable Rinf hub.
31. Local discovery service packaging: the experiment-local service wrapper
    exposes a JSON `discover`/`json-status` contract and persists the same
    redacted discovery packet to the service state directory for future GUI/Rinf
    clients. The packet covers service state, base/health/WebSocket URLs,
    runtime identity when known, pid/liveness, paths, policies, health,
    diagnostics, and timestamps without adding launchd, supervisor, Flutter,
    stable hub, or production service integration.
32. Rinf transport binding plan: the experiment-local planning artifact
    prepared the direct-binding decision by documenting packet ownership,
    generated-binding implications, service discovery bootstrap, and Dart
    thin-transport responsibilities. Slice 33 supersedes the plan's former
    stable-hub decision gate with the implemented direct dependency binding;
    remaining gates are Flutter UI, design-system/Design Lab work, and service
    packaging beyond experiment-local scripts.
33. Direct stable hub Rinf transport binding: owner selected the direct
    dependency strategy. The stable hub now depends on the existing
    experimental runtime crate, exposes JSON-backed
    `AgentRuntimeRequestSignal`/`AgentRuntimeOutputSignal` carriers, forwards
    Dart-originated packets to one long-lived Rust-owned `GuiTransportHandle`,
    emits every `GuiTransportOutputPacket` back to Dart with request-id
    correlation, and keeps Dart as a thin transport with no runtime decisions.
    Generated Rinf Dart carriers were refreshed for the two stable packet
    signals. Flutter UI, launchd/system service installation, and stable
    backend/supervisor changes remain out of scope.
34. Flutter-facing control tower first shell: the app now exposes a minimal
    Agent Runtime control tower that sends JSON `GuiTransportRequestPacket`
    intents through `AgentRuntimeRequestSignal` and renders JSON
    `GuiTransportOutputPacket` outputs from `AgentRuntimeOutputSignal`.
    Reusable shell visuals live in the design-system package with minimal
    Design Lab scenarios for disconnected, connecting, connected, error, and
    empty states. Dart remains a thin renderer/intent sender; Rust owns
    connection semantics, WebSocket URLs, watermarks, reducer application,
    enablement, and operation outcomes.
35. Rust-owned control-tower view model: the transport now emits an
    `AgentRuntimeControlTowerViewModel` with constructor-ready connection
    labels, base URL, watermark/status labels, session rows, timeline rows,
    action rows, controller facts, recent output log, pending-request slot, and
    typed error display text. Dart decodes the Rust-shaped view packet and no
    longer interprets raw `RuntimeProjection` or `GuiControllerState` JSON to
    derive control-tower rows, labels, facts, or enablement text.

## Active slice

No active implementation slice is recorded after completing Rust-owned
control-tower view shaping. The next implementable gates are richer
control-tower UX, service packaging beyond experiment-local scripts, and
deferred role/workflow GUI operation intents if owner brings them into scope.

Standing non-goals until reassigned:

- No additional Flutter UI implementation beyond the minimal control-tower
  shell until owner assigns it.
- No launchd/system service installation.
- No stable Robdex production-path changes.
- No fallback state path parallel to PostgreSQL + projection/deltas.

## Validation baseline

Run from `backend/experiments/agent-runtime` unless noted otherwise:

```bash
cargo check
cargo test
scripts/smoke-resident-server.sh
scripts/validate-local-service.sh
```

Run from repo root for whitespace validation:

```bash
git diff --check -- backend/experiments/agent-runtime
```

Common targeted scripts:

```bash
scripts/validate-command-registry.sh
scripts/validate-scoped-command-requests.sh
scripts/validate-session-lifecycle.sh
scripts/validate-db-canonical-roles.sh
scripts/validate-process-manager.sh
scripts/validate-action-resume.sh
scripts/validate-approvals-routing.sh
scripts/validate-approver-ergonomics.sh
scripts/validate-role-admin-ux.sh
scripts/validate-mutation-actions.sh
scripts/validate-workflow-memory.sh
```

Live-model or external-service tests are opt-in only:

```bash
scripts/smoke-live-server-gpt54mini.sh
scripts/smoke-lmstudio-embeddings.sh
```

## Deferred / next likely slices

1. Richer control-tower UX beyond the first packet-rendering shell.
2. Service packaging beyond local scripts.
3. Deferred role-admin GUI operation intents if owner moves role mutation into
   the first GUI shell.
4. Deferred workflow-memory inspection operation intents if projection/detail
   state is insufficient for the first GUI shell.
5. Broader execution expansion only after GUI/runtime lifecycle is stable.

## Current known risks

- The cumulative experimental runtime diff is large; review by slice and rely on
  validation evidence.
- Projection types can become too raw if future GUI slices add controls without
  typed backend-derived summaries.
- Operation contracts must not leak durable decision-making into Dart.
- Command/runtime context must remain cache-stable at the model schema layer.
- Validation scripts must track intentionally removed fields such as full
  `requestShape` persistence.
