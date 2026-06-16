# Agent Runtime Control Tower GUI Plan

This planning artifact defines the first shell for the experimental agent
runtime GUI. It is a backend/Rust planning source of truth only. It does not
implement Flutter UI, Rinf transport, production Robdex behavior, or any
fallback GUI state path.

## First shell implementation record

The first minimal Flutter-facing shell is now implemented. It stays intentionally
small: Dart sends JSON `GuiTransportRequestPacket` intents over
`AgentRuntimeRequestSignal`, consumes JSON `GuiTransportOutputPacket` outputs
from `AgentRuntimeOutputSignal`, and renders the Rust-owned
`AgentRuntimeControlTowerViewModel`. Dart does not derive session rows,
timeline rows, action rows, controller facts, labels, watermarks, operation
success, approval/command enablement, or durable state from raw projection or
controller JSON.

Reusable visual pieces live in
`frontend/robdex_app/packages/robdex_design_system` and minimal static
scenarios live in `frontend/robdex_app/packages/design_lab`. The initial shell
proves disconnected/error state, connect intent, projection/controller packet
receipt, selected-session timeline visibility when the Rust view model contains
it, action queue rendering from Rust-shaped action rows, and explicit stream
polling through the Rust-owned transport. The richer UX guidance below remains
the direction for subsequent slices.

## Richer UX implementation record

The mounted control tower has been tightened into an operations-first shell
without changing the transport boundary. Rust now shapes additional
`AgentRuntimeControlTowerViewModel` fields for the UI: status badges,
selected-session label, section titles, empty-state copy, session grouping,
row tones, action state text, and severity tones. The design-system widget
uses those Rust-shaped fields to render a clearer runtime status strip, denser
session rail, selected-session event stream, readable action queue, controller
detail panel, and explicit disconnected/connecting/connected/error/empty
states. Dart remains a thin renderer and may keep only widget-local pending
request ids, base URL text, scroll/focus/hover, and similar ephemeral facts.
Current action rows are limited to real attention items available in the
projection, currently approvals and approved resumable approvals. Installed or
enabled command registry entries are inventory, not action queue work; they may
appear as inventory counts/status detail, and actual command-registry requests
should enter the action queue only after a typed pending-request source is part
of the projection/control boundary.

## Direction

The first shell is an operations control tower, not a chat-first interface.
Conversation remains a detail inside a selected session. The top-level product
job is operational attention: the user must immediately see what is running,
what needs approval, what is blocked, what failed, what changed recently, and
what action is safe next.

The shell must optimize for command/control over runtime state:

- detect attention items before reading transcripts;
- show safe next actions before exposing raw detail;
- keep live process and approval state visible at all times;
- make resync, shutdown, and dependency failures explicit;
- avoid presenting model chat as the primary navigation model.

## First-shell information architecture

The first shell uses five stable regions:

1. **Top runtime status strip**
   - Runtime identity, database connectivity, server health, stream status,
     current selected session, latest watermark, resync/shutdown state, and
     compact counts for running, approvals, blocked, failed, and recent changes.
   - This strip is always visible and never replaced by session content.
2. **Left operational session rail**
   - Scannable session list grouped by operational state: running, blocked,
     needs approval, failed, open/idle, closed, archived when intentionally
     surfaced.
- Items expose typed state from `RuntimeProjection`; Dart does not infer
     status from raw events. The first Flutter shell receives these items as
     Rust-shaped control-tower session rows.
3. **Center selected-session event stream**
   - Ordered timeline for the selected session: user/model turns, tool calls,
     scripts, process events, approvals, command-registry changes, errors, and
     workflow-memory events.
   - This is not a chat transcript first. It is an operations event stream with
     compact progressive detail.
4. **Right action queue**
   - Pending approvals, resumable approvals, command-registry requests, blocked
     process/session actions, validation failures, and safe next operations.
   - Controls are enabled only by typed backend-derived fields such as
     `canDecide`, `canResume`, `canPreview`, `canApply`, and status summaries.
5. **On-demand detail drawer**
   - Detailed payload inspection, command policy detail, raw bounded event
     payloads, role/version detail, workflow memory detail, and diagnostic
     evidence.
   - It is opened intentionally and is not a permanent giant inspector pane.

## Screens and surfaces

### Disconnected setup

- Shows base URL, selected runtime target, last connection error, and connect
  action.
- No runtime state is invented. Before hydration, the shell displays only
  local controller state from `GuiControllerState`.

### Runtime overview

- Shows the top status strip, all sessions in the operational rail, the action
  queue, and an empty/detail-neutral center panel when no session is selected.
- Primary actions: create session, select session, inspect pending approvals,
  inspect command-registry requests, rehydrate when required.

### Session detail

- Requires selected-session hydration. Selecting a different session triggers
  rehydrate and WebSocket reconnect with the selected-session identifier.
- Center stream shows selected-session timeline. The rail and queue remain
  visible so the user does not lose operational context.

### Approval decision

- Appears in the right action queue and optional detail drawer.
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
  liveness from timeline prose.

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
- selected-session event stream row types;
- action queue item types for approvals, registry requests, process blockers,
  resync, shutdown, and typed API errors;
- approval decision panel;
- command-registry request review panel;
- process monitor row/detail;
- bounded payload/detail drawer;
- disconnected setup panel.

Design-system models must consume Rust/Rinf-provided control-tower view-model
packets. They must not duplicate runtime policy, synthesize lifecycle state, or
parse raw event payloads for control enablement.

## Dart/Rinf boundary

Rust owns runtime synchronization, operation dispatch, durable decisions, state
reduction, and control-tower view shaping. Dart receives
`AgentRuntimeControlTowerViewModel` packets plus typed result/error packets and
sends typed `GuiOperationRequest` intents.

Dart must not decide:

- session lifecycle status;
- approval availability;
- command visibility;
- command policy;
- role status;
- process status;
- timeline semantics;
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
values through `AgentRuntimeControlTowerViewModel`, `RuntimeProjection`,
`RuntimeDelta`, `GuiControllerState`, `GuiOperationRequest`,
`GuiOperationResult`, and `ApiErrorPacket`. The Flutter shell must not parse
raw `RuntimeProjection.sessions`, `RuntimeProjection.timeline`,
`RuntimeProjection.pendingApprovals`, or command-registry internals to build
control-tower rows.

## Visual risk controls

Avoid:

- nested panel stacks that hide the operational hierarchy;
- fake developer fan-fiction data;
- card, border, and prose overload;
- default AI-dashboard patterns that make everything look equally important;
- permanent giant inspector panes;
- chat-first composition that demotes approvals/processes/errors.

Prefer:

- compact scannability;
- strong hierarchy;
- operations-appropriate density;
- progressive detail;
- stable status language;
- clear safe-next-action affordances;
- explicit empty, resync, shutdown, blocked, and error states.

## Source-of-truth files for future implementers

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

These frontend paths are references only for this planning slice. They are not
modified here.

## Aesthetic context gate

No `.impeccable.md` design context exists at the repository root or under
`backend/experiments/agent-runtime` at the time this plan is written. Final
aesthetic direction must be owner-confirmed before any Flutter implementation
Requirements are set.
