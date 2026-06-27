# Agent Runtime Shared Shell GUI Plan

## Workbench-compatible shell contract

The connected Agent Runtime product surface now uses the canonical Robdex
Workbench pattern as a chat-centered product shell. The shell
presents a brushed-metal left project/session rail, the shared `ChatTimeline` in
the center, the shared `ComposerPanel` for selected-session messages, and
toolbar-opened modal or sheet surfaces for active runtime operations: Compaction,
Process Manager, Role Admin, Workflow Memory, Requirements Review, Approvals,
and global Command Registry inventory. The removed Statistics, Image artifacts,
Settings, History, and Diagnostics sheets are not part of the GUI operation
surface.
Selected-session settings and controls use the full-screen shared design-system
`AgentRuntimeSessionControlPlane`. That control plane folds in selected-session
identity/settings, lifecycle, compaction, God Mode, current managed processes,
selected-session approvals, selected-session command requests, and Requirements
Review actions. The connected layout must not mount a permanent operations
operations pane. The login/setup screen remains the disconnected entry
point. Runtime projection, discovery, project, role, model, workflow-memory, and
operation decisions remain Rust-owned; Dart assembles the shared shell, sends
typed Rinf intents, and keeps only widget-local draft, selection, and connection
state.

Operational state is retained in typed Rust/PostgreSQL projections and logs
rather than a generic diagnostics modal or sheet. New visual work must extend
the Robdex Workbench conversation-shell contract, not rebuild a parallel
dashboard, permanent operations pane, or diagnostics-first workflow.

This plan records the current Workbench-compatible Agent Runtime GUI contract. It began as the backend/Rust planning source of truth and now also
records the implemented Flutter-facing shared shell, Rust/Rinf transport
boundary, and service discovery behavior. It does not define production Robdex
behavior or any fallback GUI state path.

## First shell implementation record

The first minimal Flutter-facing shell is now implemented. It stays intentionally
small: Dart sends generated typed Agent Runtime request variants over
`AgentRuntimeRequestSignal`, consumes generated typed Agent Runtime output
variants from `AgentRuntimeOutputSignal`, and renders the Rust-owned
`AgentRuntimeWorkbenchViewModel`. Dart does not derive session rows,
chat rows and separate history rows, action rows, runtime facts, labels, watermarks, operation
success, approval/command enablement, or durable state from raw projection or
controller JSON.

Reusable visual pieces live in
`frontend/robdex_app/packages/robdex_design_system` and minimal static
scenarios live in `frontend/robdex_app/packages/design_lab`. The initial shell
proves disconnected/error state, connect intent, runtime packet
receipt, selected-session chat visibility when the Rust view model contains
it, attention-item rendering from Rust-shaped action rows, and Rust-owned
selected-session stream subscription. Dart issues user and lifecycle intents;
Rust owns WebSocket stream consumption, state reduction, and typed output
signals. The richer UX guidance below remains the direction for subsequent
slices.

## Richer UX implementation record

The mounted Agent Runtime UI has been tightened into an Workbench-compatible chat shell
without changing the transport boundary. Rust now shapes additional
`AgentRuntimeWorkbenchViewModel` fields for the UI: status badges,
selected-session label, section titles, empty-state copy, session grouping,
row tones, action state text, and severity tones. The design-system widget
uses those Rust-shaped fields to render a clearer runtime status strip, denser
session rail, selected-session chat transcript, readable attention list, runtime
detail panel, and explicit disconnected/connecting/connected/error/empty
states. Dart remains a thin renderer and may keep only widget-local pending
request ids, base URL text, scroll/focus/hover, and similar ephemeral facts.
Current action rows are limited to real attention items available in the
projection: approvals, approved resumable approvals, and typed pending/actionable
command-registry request summaries. Installed or enabled command registry
entries are inventory, not attention work; they may appear as inventory
counts/status detail, but they are not counted as required attention.

## File bootstrap discovery implementation record

The Agent Runtime UI now receives Rust-shaped local discovery fields from the
experimental transport. Rust reads the canonical user-scoped discovery packet
by default: `~/Library/Application Support/Robdex Agent Runtime/service/discovery.json`
on macOS, or
`${XDG_STATE_HOME:-~/.local/state}/robdex-agent-runtime/service/discovery.json`
on non-macOS hosts. The packet uses the same JSON contract emitted by
`scripts/agent-runtime-service.sh discover` / `json-status`. Rust parses and
classifies the packet, decides whether the local runtime target is connectable,
and emits constructor-ready discovery state on
`AgentRuntimeWorkbenchViewModel`.

Dart may render the discovered target and send refresh/connect-discovered
intents. Dart must not read the discovery file, interpret pid/path/health
fields, decide service health, construct runtime URLs, or apply fallback
discovery logic. Running and healthy discovery enables a one-step connect using
the Rust-selected `baseUrl`; stopped, stale-pid, unhealthy, missing-config,
stale-discovery, missing-file, and parse-error states remain diagnostics and do
not pretend to be connected. Manual base URL entry remains a fallback input.

iCloud remote profile discovery is implemented as a second Rust-owned bootstrap
provider beside the local service file. It reads a sync-safe profile sentinel
from `~/Library/Mobile Documents/com~apple~CloudDocs/Robdex Agent Runtime/remote-profile.json`
by default, or from `ROBDEX_AGENT_RUNTIME_ICLOUD_REMOTE_PROFILE_PATH` for tests
and development. The profile supplies only a host/port/scheme candidate
(`robertmsale._peer.internal:8765` by default); Rust probes `/health` before
the Agent Runtime UI marks it connectable. The UI shows local discovery and iCloud
remote profile discovery distinctly. mDNS/Bonjour discovery and iOS profile-sync
UX remain separate owner-approved slices. The current service packaging
affordance includes per-user script-based packaging and macOS LaunchAgent
install/load/unload/status commands.

Document import is implemented as a practical iPhone/macOS bootstrap path
without Apple iCloud container entitlements. The Agent Runtime UI shows an
`Import profile` affordance beside iCloud remote discovery. Dart sends an import
intent only; Rust validates the selected JSON profile, writes a sanitized
app-local copy, probes `/health`, and exposes `importedRemoteDiscovery` with
clear no-file, stale/malformed, healthy, and unreachable states. `Refresh
imported` and `Connect imported` operate on the Rust-owned app-local copy.


## Role Admin implementation record

Runtime Operations launches Role Admin as a full-screen Role Manager page. Rust owns role draft semantics, validation dispatch, operation dispatch, projection reduction, and view-model shaping. The `roleAdmin` view-model section contains role rows, selected-role detail, version rows, editable draft content, validation errors, and role operation action states. Dart renders these Rust-shaped values and may keep only widget-local editor text/connection state; validation/create/update/export/activate/archive/unarchive are sent back as typed Rust GUI operations, and version rows expose activation only for non-current immutable versions.

Role create/update uses inline editor `instructionText` and persists it to immutable `role_versions.instruction_text`; the UI never creates prompt files. Server validation reuses canonical role manifest validation, DB routing validation, and command-policy validation. The shared Role Manager has one role-authority editor where each action row carries its policy decision; validate/save drafts derive capabilities and policy from those rows before dispatch so the backend policy-key invariant remains the source of truth. Create/update/activate/archive/unarchive mutations wait for projection/delta evidence, while metadata/options, validation, detail, version, and export operations return direct typed results.

## Direction

The connected shell is a chat-first Robdex Workbench product using the shared conversation primitives.
Conversation is the center workflow once a session is selected. Operational details
remain available through modal toolbar surfaces so the user can inspect what is
running, what needs approval, what is blocked, what failed, what changed recently,
and what action is safe next without replacing the center transcript.

The shell must optimize for command/control over runtime state:

- detect attention items before reading transcripts;
- show safe next actions before exposing raw detail;
- keep live process and approval state visible at all times;
- make resync, shutdown, and dependency failures explicit;
- avoid presenting model chat as the primary navigation model.

## First-shell information architecture

The shell uses stable regions:

1. **Top runtime status strip**
   - Runtime identity, database connectivity, server health, stream status,
     current selected session, latest watermark, resync/shutdown state, and
     compact counts for running, approvals, blocked, failed, and recent changes.
   - This strip is always visible and never replaced by session content.
2. **Left operational session rail**
   - Scannable session list grouped by operational state: running, blocked,
     needs approval, failed, stopped/idle, archived when intentionally
     surfaced.
- Items expose typed state from `RuntimeProjection`; Dart does not infer
     status from raw events. The first Flutter shell receives these items as
     Rust-shaped workbench-shell session rows.
3. **Center selected-session chat transcript**
   - Product-shaped chat for the selected session: user messages, assistant responses, compact tool/result summaries, and stored image previews from selected-session image artifacts.
   - Runtime audit events, process events, approvals, command-registry changes, errors, and workflow-memory events remain in PostgreSQL/server history or the relevant active modal operations surface, not in removed History/Diagnostics sheets.
4. **Right attention and operations detail**
   - Pending approvals, resumable approvals, command-registry requests, blocked
     process/session actions, validation failures, and safe next operations.
   - Controls are enabled only by typed backend-derived fields such as
     `canDecide`, `canResume`, `canPreview`, `canApply`, and status summaries.
5. **On-demand detail drawer**
   - Detailed payload inspection, command policy detail, raw bounded event
     payloads, role/version detail, workflow memory detail, and diagnostic
     evidence.
   - Workflow Memory detail is inspection plus feedback only: Starlark source is
     read-only, row selection is a Rust-owned controller intent with
     deterministic fallback to the first visible memory, visibility and feedback
     authority are Rust-owned, and no memory editing/curation controls are
     present.
   - It is opened intentionally and is not a permanent giant inspector pane.

## Screens and surfaces

### Disconnected setup

- Shows base URL, selected runtime target, last connection error, and connect
  action.
- No runtime state is invented. Before hydration, the shell displays only
  local connection state from `GuiControllerState`.

### Runtime overview

- Shows the top status strip, all sessions in the operational rail, the action
  queue, and an empty/detail-neutral center panel when no session is selected.
- Primary actions: create session, select session, inspect pending approvals,
  inspect command-registry requests, rehydrate when required.

### Session detail

- Requires selected-session hydration. Selecting a different session triggers
  rehydrate and WebSocket reconnect with the selected-session identifier.
- Center panel shows product-shaped selected-session chat. The rail and queue remain
  visible so the user does not lose operational context.

### Approval decision

- Appears in toolbar-opened operations surfaces.
- Uses typed approval fields: status, approver kind, resumable state,
  `canDecide`, `canResume`, reason requirement, and typed result/error packets.
- Dart must not infer approval availability from status strings or raw
  `inputContext`.

### Command-registry request review

- Shows request summary, requested operation, proposed command, current status,
  and typed control fields `canPreview`, `canDecide`, and `canApply`.
- Preview/decide/apply use typed `GuiOperationRequest` shapes aligned to server
  JSON. Dart must not construct ad hoc registry payloads.

### Process monitor

- Shows live/continuing managed process rows, terminal/lost states, max runtime,
  stdin policy, end-of-turn/end-of-session behavior, and output policy.
- Process status comes from projection/delta state. Dart must not infer process
  liveness from typed runtime state.

### Error, resync, and shutdown handling

- Resync required is a first-class blocking state. The shell shows the reason,
  disables unsafe operations, and offers rehydrate.
- Server shutdown is distinct from network error. The shell shows shutdown
  outcome and waits for explicit reconnect.
- Dependency unavailable and typed API errors display stable code, message, and
  details from `ApiErrorPacket`.

## Required runtime states and mock scenarios

The first Design Lab pass must cover these scenarios with deterministic,
constructor-ready data from the Rust contract. Mock data must be structurally
valid and must not be developer fan fiction that contradicts runtime policy.

| Scenario | Required visible evidence | Primary safe action |
| --- | --- | --- |
| Disconnected | local connection state, base URL, last typed error if any | Connect |
| Empty runtime | healthy server, zero sessions, zero action items | Create session |
| Connected/no sessions | connected stream, hydrated watermark, role availability summary | Create/select workflow |
| Active running session | running turn/tool/script/process summary and recent event | Monitor or open detail |
| Pending approvals | approval count, `canDecide=true`, required reason | Decide approval |
| Resumable approval | approved resumable row, `canResume=true` | Resume approval |
| Command-registry request pending | request summary, `canPreview`/`canDecide`/`canApply` | Preview or decide |
| Live or continuing process running | process id/status/policy/output summary | Monitor; terminate only if a typed operation exists later |
| Blocked/error state | typed error code/message/details and blocked entity | Resolve named blocker |
| Resync required | resync flag, reason, last watermark | Rehydrate and reconnect |
| Server shutdown | shutdown outcome, disconnected stream state | Reconnect after server returns |

## Design-system contract for later implementation

Runtime-specific models and widgets belong later in:

- `frontend/robdex_app/packages/robdex_design_system`

Design Lab scenarios belong later in:

- `frontend/robdex_app/packages/design_lab`

No widgets are implemented in this slice. Future design-system work should add
constructor-ready models and widgets for:

- runtime status strip;
- operational session rail item and grouped rail;
- selected-session chat entry types;
- attention item types for approvals, registry requests, process blockers,
  resync, shutdown, and typed API errors;
- approval decision panel;
- command-registry request review panel;
- process monitor row/detail;
- bounded payload/detail drawer;
- disconnected setup panel.

Design-system models must consume Rust/Rinf-provided workbench-shell view-model
packets. They must not duplicate runtime policy, synthesize lifecycle state, or
parse raw event payloads for control enablement.

## Dart/Rinf boundary

Rust owns runtime synchronization, operation dispatch, durable decisions, state
reduction, and workbench-shell view shaping. Dart receives
`AgentRuntimeWorkbenchViewModel` packets plus typed result/error packets and
sends typed `GuiOperationRequest` intents.

Dart must not decide:

- session lifecycle status;
- approval availability;
- command visibility;
- command policy;
- role status;
- process status;
- chat and history semantics;
- WebSocket state;
- operation success.

Dart may own widget-local ephemeral facts:

- text field editing mechanics;
- focus;
- scroll position;
- hover/press state;
- animations;
- local layout.

The Rust boundary must provide constructor-ready or near-constructor-ready
values through `AgentRuntimeWorkbenchViewModel`, `RuntimeProjection`,
`RuntimeDelta`, `GuiControllerState`, `GuiOperationRequest`,
`GuiOperationResult`, and `ApiErrorPacket`. The Flutter shell must not parse
raw `RuntimeProjection.sessions`, `RuntimeProjection chat/history fields`,
`RuntimeProjection.pendingApprovals`, or command-registry internals to build
workbench-shell rows.

## Visual risk controls

Avoid:

- nested panel stacks that hide the operational hierarchy;
- fake developer fan-fiction data;
- card, border, and prose overload;
- default metric-board patterns that make everything look equally important;
- permanent giant inspector panes;
- hiding approvals, processes, or errors from the modal operational surfaces.

Prefer:

- compact scannability;
- strong hierarchy;
- operations-appropriate density;
- progressive detail;
- stable status language;
- clear safe-next-action affordances;
- explicit empty, resync, shutdown, blocked, and error states.

### Emergency presentation correction record

The mounted Agent Runtime presentation has been corrected away from the broken
one-row discovery/status/control strip. Connection and discovery now live in a
setup screen shown only before a runtime is connected. Once connected, the
Agent Runtime shell shows a compact operations bar and hides the manual URL/profile
setup affordances until the user disconnects. The setup screen presents one
state-appropriate primary action, compact local/iCloud/imported discovery
controls that wrap at narrow widths, and concise bridge loading/failure copy
instead of raw Flutter/Rust crash cards. Agent Runtime surfaces use restrained
radii no larger than 8 and avoid status-chip spam, giant empty rectangles, and
Connect/Disconnect as simultaneous peer actions for the same target. The
design-system package owns the CodeForge boundary: native app surfaces use
`code_forge`, Design Lab/web surfaces use `code_forge_web`, and both clients
render the same design-system Agent Runtime component.

## Source-of-truth files for implementers

Runtime/backend source of truth:

- Projection/reducer and GUI contract:
  `backend/experiments/agent-runtime/crates/robdex-agent-runtime-projection/src/lib.rs`
- Rust GUI sync client:
  `backend/experiments/agent-runtime/crates/robdex-agent-runtime/src/gui_sync.rs`
- Rust/Rinf GUI backend controller boundary:
  `backend/experiments/agent-runtime/crates/robdex-agent-runtime/src/gui_backend.rs`
- Server routes and WebSocket endpoint:
  `backend/experiments/agent-runtime/crates/robdex-agent-runtime/src/server.rs`
- Snapshot/delta adapters:
  `backend/experiments/agent-runtime/crates/robdex-agent-runtime/src/projection.rs`
- Runtime send/model boundary:
  `backend/experiments/agent-runtime/crates/robdex-agent-runtime/src/runtime.rs`

Existing app and future UI integration references:

- Existing Flutter app entry:
  `frontend/robdex_app/lib/main.dart`
- Existing Flutter app shell:
  `frontend/robdex_app/lib/src/app/robdex_app.dart`
- Existing Rinf hub:
  `frontend/robdex_app/native/hub/src/lib.rs`
- Existing Rinf runtime bridge:
  `frontend/robdex_app/native/hub/src/runtime.rs`
- Design system package:
  `frontend/robdex_app/packages/robdex_design_system`
- Design Lab package:
  `frontend/robdex_app/packages/design_lab`

These frontend paths identify the implemented shell and its source-of-truth
boundaries.

## Aesthetic context gate

No `.impeccable.md` design context exists at the repository root or under
`backend/experiments/agent-runtime` at the time this plan is maintained. Future
visual expansion beyond the current workbench-shell shell requires
owner-confirmed aesthetic direction before implementation Requirements are set.
