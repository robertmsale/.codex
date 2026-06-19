# Agent Runtime Roadmap

This roadmap is a planner-facing source of truth for the experimental agent
runtime in `backend/experiments/agent-runtime`. It is intentionally lightweight:
use it to avoid rediscovering settled decisions after compaction, not as a full
product specification.

## Status

- **Current milestone:** Build an alternative, PostgreSQL-backed agent runtime
  that can eventually power a macOS/iOS GUI without depending on stable Robdex
  bridge internals.
- **Current active slice:** Per-user launchd/autostart support is implemented for the experimental Agent Runtime service; stable backend/supervisor integration has not started.
- **Current implementation owner:** Codex Config Operator.
- **Planner stance:** Rust owns GUI runtime synchronization, operation dispatch,
  durable state decisions, and workbench-shell view shaping; Flutter remains a
  thin renderer/intent sender.

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
- Rust-owned Agent Runtime view model:
  `crates/robdex-agent-runtime/src/rinf_transport.rs`
- Rinf transport binding plan: `RINF_TRANSPORT_BINDING_PLAN.md`
- Stable hub direct binding files:
  `frontend/robdex_app/native/hub/Cargo.toml`,
  `frontend/robdex_app/native/hub/src/signals/agent_runtime.rs`,
  `frontend/robdex_app/native/hub/src/signals/mod.rs`, and
  `frontend/robdex_app/native/hub/src/runtime.rs`
- First-shell GUI planning artifact: `WORKBENCH_GUI_PLAN.md`
- Local service scripts: `scripts/agent-runtime-service.sh` and
  `scripts/validate-local-service.sh`
- Local service discovery file:
  `~/Library/Application Support/Robdex Agent Runtime/service/discovery.json`
  on macOS, or
  `${XDG_STATE_HOME:-~/.local/state}/robdex-agent-runtime/service/discovery.json`
  on non-macOS hosts, unless `ROBDEX_AGENT_RUNTIME_SERVICE_STATE_DIR` is set.
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
21. Resident server validation harness.
22. Runtime operations hardening and local service wrapper.
23. GUI integration foundation via shared projection and sync client prototype.
24. Cache-stable command discovery with synthetic runtime command context and
    Starlark `cmd.describe()` affordances.
25. Validation-script cutover for explicit CLI binary selection and sanitized
    model request evidence.
26. GUI contract + proof: typed local controller state, operation vocabulary,
    Dart responsibility boundary, approval/command control enablement, and
    projection/reducer proof tests.
27. GUI API gap audit: every `GuiOperationRequest` had a documented and
    code-backed route/local-action mapping; approval and command-registry
    request shape mismatches were resolved. Role-admin GUI operation intents
    are now implemented by the structured Role Admin editor slice. Workflow
    Memory operations-detail inspection/feedback is implemented as an inspector
    surface rather than a memory-editing UI.
28. Rust/Rinf GUI backend controller boundary: a Rust-owned dispatcher owns
    `RuntimeSyncClient`, owned WebSocket stream handle, `RuntimeProjection`,
    `GuiControllerState`, selected session, connection/resync state, transient
    errors, operation dispatch, and typed `GuiOperationResult` emission for a future thin Rinf layer. The controller also exposes a public owned-stream polling method for consuming one WebSocket server message at a time through the shared reducer.
29. Workbench shell GUI plan: the current direction is documented as a
    Robdex Workbench-compatible chat product with modal operational surfaces;
    the artifact defines selected conversation, composer, toolbar modal
    surfaces, runtime states, Dart/Rinf boundaries, visual controls,
    design-system handoff contract, and source-of-truth files. Later slices
    implemented the Flutter-facing shell against this plan.
30. Experimental Rinf transport proof: an experiment-local Rust module defines
    stable Dart-to-Rust request envelopes and Rust-to-Dart output envelopes for
    driving `GuiBackendController`, owns exactly one controller through a
    serialized async action loop, uses JSON-backed projection/controller
    payloads where schemas should remain stable, maps errors to
    `ApiErrorPacket`, and proves connect/hydrate, operation dispatch, owned
    stream polling, typed errors, and disconnect without modifying
    `frontend/robdex_app` or the stable Rinf hub.
31. Local discovery service packaging: the Agent Runtime service wrapper exposes
    a JSON `discover`/`json-status` contract and persists the same redacted
    discovery packet to the service state directory for GUI/Rinf bootstrap.
    The packet covers service state, base/health/WebSocket URLs,
    runtime identity when known, pid/liveness, paths, policies, health,
    diagnostics, and timestamps without adding launchd, supervisor,
    stable hub, or production service integration.
32. Rinf transport binding plan: the experiment-local planning artifact
    prepared the direct-binding decision by documenting packet ownership,
    generated-binding implications, service discovery bootstrap, and Dart
    thin-transport responsibilities. Slice 33 supersedes the plan's former
    stable-hub decision gate with the implemented direct dependency binding.
33. Direct stable hub Rinf transport binding: owner selected the direct
    dependency strategy. The stable hub now depends on the existing
    experimental runtime crate, exposes generated typed
    `AgentRuntimeRequestSignal`/`AgentRuntimeOutputSignal` variants, maps
    Dart-originated typed intents to one long-lived Rust-owned
    `GuiTransportHandle`, emits typed output variants back to Dart with
    request-id correlation, and keeps Dart as a thin transport with no runtime
    decisions. Generated Rinf Dart carriers were refreshed for the typed
    Agent Runtime request/output schema. Later slices mounted the
    Flutter-facing Workbench shell on this transport. Stable backend/supervisor
    changes remain out of scope.
34. Flutter-facing Workbench shell first shell: the app now exposes a minimal
    Agent Runtime Workbench shell that sends generated typed intents through
    `AgentRuntimeRequestSignal` and renders generated typed outputs from
    `AgentRuntimeOutputSignal`.
    Reusable shell visuals live in the design-system package with minimal
    Design Lab scenarios for disconnected, connecting, connected, error, and
    empty states. Dart remains a thin renderer/intent sender; Rust owns
    connection semantics, WebSocket URLs, watermarks, reducer application,
    enablement, and operation outcomes.
35. Rust-owned Agent Runtime view model: the transport now emits an
    `AgentRuntimeWorkbenchViewModel` with constructor-ready connection
    labels, base URL, watermark/status labels, session rows, product chat rows and separate history rows,
    action rows, controller facts, recent output log, pending-request slot, and
    typed error display text. Dart decodes the Rust-shaped view packet and no
    longer interprets raw `RuntimeProjection` or `GuiControllerState` JSON to
    derive workbench-shell rows, labels, facts, or enablement text.
36. Richer Workbench chat-shell UX: the Rust-owned view model now
    carries status badges, selected-session label, section titles, empty-state
    copy, session group labels, row tones, action state text, and
    action/timeline/session severity tones. The design-system Workbench shell
    renders a clearer runtime status strip, better session rail,
    selected-session product chat transcript, readable attention list, runtime/error
    detail visibility, and disconnected/connecting/connected/error/empty
    states without adding Dart-side runtime decisions or a Dart network client.
    Action rows are real attention items, currently approvals/resumable
    approvals; command registry inventory is surfaced as inventory status/count
    and is not counted as action-queue work.
37. Registry request projection/action rows: `RuntimeProjection` now includes
    typed pending/actionable command-registry request summaries sourced from
    `command_registry_requests`, separate from installed command inventory.
    Snapshot and event deltas upsert/remove those request rows as lifecycle
    events occur, and the operations attention list includes registry request
    rows alongside approval/resume rows. Installed/enabled command inventory
    remains inventory status/count detail, not action work.
38. File bootstrap discovery: the Rust transport reads the service discovery
    packet by default, using the same JSON contract produced by
    `scripts/agent-runtime-service.sh discover` / `json-status`. Rust classifies
    no-file, stopped, stale-pid, unhealthy, missing-config, stale-discovery,
    running/healthy, and parse-error states, exposes constructor-ready
    discovery fields on `AgentRuntimeWorkbenchViewModel`, and connects to a
    running/healthy target through a Rust-owned connect-discovered intent. Dart
    renders the Rust-shaped state and sends refresh/connect intents only.
39. Service packaging: the wrapper and Rust bootstrap default now use a
    canonical user-scoped state directory instead of the experiment workspace:
    `~/Library/Application Support/Robdex Agent Runtime/service` on macOS, or
    `${XDG_STATE_HOME:-~/.local/state}/robdex-agent-runtime/service` on
    non-macOS hosts. The explicit
    `ROBDEX_AGENT_RUNTIME_SERVICE_STATE_DIR` override remains for validation and
    development. The script also exposes `default-state-dir`,
    `install-user-service`, `package-status`, and `uninstall-user-service` as a
    per-user script-based packaging contract that preserves the existing
    resident server binary/path and discovery packet contracts. Launchd/autostart
    remained deferred until slice 40.
40. Launchd autostart: the same service wrapper now supports per-user macOS
    launchd lifecycle commands: `install-launchd`, `load-launchd`,
    `unload-launchd`, `uninstall-launchd`, and `launchd-status`. The generated
    plist lives under `~/Library/LaunchAgents`, targets the existing
    `scripts/agent-runtime-service.sh start` flow, preserves the canonical or
    explicitly overridden service state directory, writes launchd logs under the
    state directory, and keeps the existing discovery/config/pid/package
    contract authoritative. `launchd-status` and `package-status` distinguish
    not installed, installed/unloaded, loaded/running, loaded/unhealthy, and
    stale/unknown from user-domain launchctl state plus service health. No
    LaunchDaemons, sudo, `/Library/LaunchDaemons`, or root-owned `/var`
    installation exists.

## Active slice

Role Admin UI structured editor is implemented: DB-backed role draft validation, create/update immutable versions, activate/archive/unarchive/export routes, role projection/view-model fields, Rust GUI operations, and the design-system Role Admin panel are now in place. The compaction kernel is implemented on top of PostgreSQL-backed output artifacts. Workflow Memory inspection plus feedback is implemented inside Agent Runtime modal operations surfaces: Rust-owned workflow-memory rows/detail/events/source metadata, Rust-owned row selection with deterministic fallback, and session-scoped attempted/helpful/not-helpful actions. iCloud remote profile discovery is implemented as a sync-safe sentinel transport with Rust-owned profile parsing and /health connectability checks. Document-import remote profile UX is implemented as an app-local profile acquisition path: Dart sends import/refresh/connect intents, Rust validates and stores a sanitized profile copy, and /health remains the connectability authority. The current shell slice keeps the connected Agent Runtime UI in the canonical Robdex Workbench pattern with brushed-metal left project/session rail, center real chat transcript in `ChatTimeline`, shared `ComposerPanel`, toolbar modal/sheet operations surfaces, typed Rinf operations, table-derived stats, project-aware/model-aware creation, selected-chat deltas, and lifecycle reconciliation. Connected Agent Runtime must not regress to a dashboard shell, permanent operations pane or diagnostics-first interaction model. Remaining gates are mDNS/Bonjour discovery, native iOS file-picker polish beyond the typed import boundary, broader execution expansion after GUI/runtime lifecycle is stable, workflow-memory editing/curation if the owner explicitly scopes it later, and any owner-approved production Robdex integration. Tokenizer-based accounting remains intentionally out of scope.

## Validation baseline

Run from `backend/experiments/agent-runtime` unless noted otherwise:

```bash
cargo check
cargo test
scripts/validate-resident-server.sh
scripts/validate-local-service.sh
scripts/validate-launchd-packaging.sh
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
scripts/validate-live-server-gpt54mini.sh
scripts/validate-lmstudio-embeddings.sh
```

## Deferred / next likely slices

1. mDNS/Bonjour discovery and native iOS file-picker/profile-sync polish beyond the implemented iCloud profile sentinel plus app-local document-import path.
2. Workflow-memory inspection operation intents if projection/detail state is
   insufficient for the first GUI shell.
3. Broader execution expansion only after GUI/runtime lifecycle is stable.

## Current known risks

- The cumulative experimental runtime diff is large; review by slice and rely on
  validation evidence.
- Projection types can become too raw if future GUI slices add controls without
  typed backend-derived summaries.
- Operation contracts must not leak durable decision-making into Dart.
- Command/runtime context must remain cache-stable at the model schema layer.
- Validation scripts must track intentionally removed fields such as full
  `requestShape` persistence.
