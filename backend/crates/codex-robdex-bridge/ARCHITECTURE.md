# codex-robdex-bridge

Robdex Bridge is the long-running control plane between the Robdex frontend and
the Codex app-server / project-local lifecycle environment.

This document is the canonical bridge architecture reference for:

- current runtime behavior
- persisted state shape and responsibilities
- HTTP and event surfaces
- integration boundaries with external services
- planned project hook system for agent lifecycle automation
- live process tracking for shell-buffered command termination

## Purpose

The bridge exists to own Robdex-specific orchestration concerns that do not
belong in the app-server:

- persisted Robdex app state
- frontend-facing HTTP and websocket surfaces
- thread and message bootstrap shaping
- orchestrator and worker coordination commands
- bridge-owned recovery logic around transport and app-server quirks
- local service integration points

The bridge is not the place for device runtime management or generic QA runtime
ownership. That belongs in `codex-qa-harness`.

Current role boundary notes:

- orchestrators may spawn only subordinate `worker` and `qa` agents
- administrators may directly create additional thread roles through the
  generic thread-create surface
- `designer` is an administrator-only design role and intentionally does not
  inherit QA harness or spawn-hook coupling

## Current Components

- `config.rs`
  - bridge CLI args and resolved runtime settings
- `runtime.rs`
  - long-lived runtime, state loading, event fanout, thread cache persistence,
    transport lifecycle, workbench snapshot assembly
- `http.rs`
  - frontend-facing HTTP API, websocket upgrades, replay/bootstrap routes,
    bridge-owned service proxy routes
- `commands.rs`
  - imperative bridge command handlers and mutating orchestration actions
- `store.rs`
  - sqlite-backed persistence for bridge-owned state
- `transport.rs`
  - app-server transport loop and reconnect behavior
- `upstream.rs`
  - reduction of upstream transport events into bridge state
- `transforms.rs`
  - view/message shaping helpers
- `models.rs`
  - wire models and event payload definitions

## Runtime Model

`BridgeRuntime` is the authoritative in-memory object for the bridge process.

It owns:

- resolved settings
- persisted state document (`robdex.json` + sqlite mirror)
- thread cache payload
- connection status
- event log and broadcast bus
- pending approvals
- running-state reduction
- app-server transport control channel
- thread-cache flush coordination
- serialized state mutation lock

The runtime boot sequence is:

1. resolve settings and paths
2. ensure state root exists
3. connect sqlite store
4. load persisted state JSON
5. load thread cache payload
6. create upstream + transport channels
7. create event bus
8. spawn upstream reducer worker
9. publish initial disconnected state

## Persistence

The bridge persists two main artifacts:

- `robdex.json`
  - bridge-owned application state document
- `robdex.sqlite`
  - durable local cache for thread/message/runtime-supporting data

The bridge treats the state document as the primary project/thread/orchestrator
control structure, while sqlite supports the faster-changing cache layers.

Current important persisted concerns include:

- selected project
- project definitions
- agent/thread records
- worker metadata
- thread grouping
- archived thread handling
- thread message cache
- running thread ids
- context window metadata
- hook lifecycle metadata and hook telemetry
- shell-reported live process registry per thread

## Event Surfaces

The bridge currently exposes:

- HTTP bootstrap routes
- websocket live event transport
- event replay endpoint for reconnect/bootstrap repair

Current bridge event model includes:

- connection status changes
- app state snapshots
- thread message changes
- hook failure notices

The frontend workbench is built from:

- `/workbench/bootstrap`
- `/events/replay`
- bridge websocket follow stream

## Current HTTP Surface

Major route groups:

- health/info
  - `/health`
  - `/healthz`
  - `/info`
- workbench/bootstrap/state
  - `/state/app`
  - `/state/snapshot`
  - `/workbench/bootstrap`
- project/thread mutation
  - `/projects`
  - `/threads`
  - thread interrupt / terminate / running-state routes
  - thread live-process register / complete routes
- orchestrator control
  - worker spawn
  - warm handoff
  - archive / rename / lookup / approvals
- event transport
  - `/events/replay`
  - `/ws`
  - `/workbench/ws`
- bridge-owned external service summary/proxy
  - `/services/qa-harness/summary`

## External Service Boundaries

The bridge currently talks to:

- Codex app-server
  - primary upstream transport target
- codex-qa-harness
  - currently summarized through a lightweight health/projects proxy route

Rules:

- the bridge may proxy summaries or targeted actions for local services
- the bridge should not absorb the full runtime logic of those services
- when a service owns lifecycle state, the bridge should consume it instead of
  reimplementing it

## Existing Behavioral Constraints

- warm handoff backend state is bridge-owned
- tracked thread pruning is bridge-owned when app-server resumes dead rollouts
- command execution termination is bridge-owned from the Robdex side
- shell-buffered local command termination is bridge-owned through a thread PID registry
- state mutations should remain serialized and persisted deliberately
- direct app-server fetches should stay conservative

## Live Process Tracking

The current shell model buffers command output until completion, which means the
GUI cannot infer a killable OS process from streamed command rows alone.

To support terminate in the buffered shell model, the bridge now owns a
lightweight live-process registry.

### Source Of Truth

The shell wrapper reports command lifecycle to the bridge with:

- thread id
- pid
- process group id
- command text
- cwd
- started-at timestamp

Current routes:

- `POST /threads/{thread_id}/processes/register`
- `POST /threads/{thread_id}/processes/{process_id}/complete`

The bridge persists those records under thread metadata as
`robdexLiveProcesses`.

### UI Reduction

The frontend reduces `robdexLiveProcesses` into synthetic in-progress
`commandExecution` rows for the selected thread. This keeps the existing
terminate control usable without depending on streamed stdout/stderr.

### Termination Model

`commandExecutionTerminate` now resolves only through the thread live-process
registry. Termination prefers the reported command-local process group and
falls back to PID-only signaling if no process group was reported.

Each shim-launched command runs in its own isolated process group. Process
groups are not shared across an entire `CODEX_THREAD_ID`, because that would
make one terminate action capable of killing unrelated later commands from the
same thread.

### Constraints

- registration is best-effort and thread-scoped
- completion is best-effort and idempotent
- unknown-thread registration is rejected
- missing-process completion is ignored
- stale live-process entries are removed when terminate sees `ESRCH`

## Planned Project Hook System

The next major bridge-owned feature is project lifecycle hooks.

The goal is to make worker/QA setup and teardown deterministic and bridge-owned,
instead of pushing worktree/branch/stack hygiene onto agents.

### Why Hooks Belong In The Bridge

Hooks should live in the bridge, not the QA harness, because they are about
agent lifecycle rather than device/runtime lifecycle.

Bridge-owned examples:

- worker creation
- worker archive
- prompt mutation for spawned agents
- worktree and branch allocation
- stack allocation for worker-local testing
- cleanup on archive or handoff

QA harness-owned examples:

- device slot lifecycle
- simulator boot and readiness
- runtime command execution against a managed QA environment

Designer-specific constraint:

- designers run directly from their own dedicated worktree and debug runtime
  rather than through the QA harness path

### Hook Discovery

At project root:

`<project>/.codex/robdex-hooks.json`

Example:

```json
{
  "version": 1,
  "hooks": {
    "onWorkerCreate": "./.codex/hooks/on-worker-create",
    "onWorkerArchive": "./.codex/hooks/on-worker-archive",
    "onQaCreate": "./.codex/hooks/on-qa-create",
    "onQaArchive": "./.codex/hooks/on-qa-archive",
    "onWarmHandoff": "./.codex/hooks/on-warm-handoff"
  }
}
```

Current implementation status:

- `onWorkerCreate`
- `onWorkerArchive`
- `onQaCreate`
- `onQaArchive`

are now wired in the bridge as opt-in hooks. If a project has no
`.codex/robdex-hooks.json`, bridge behavior remains unchanged.

Create-hook artifacts such as `worktreePath`, `branchName`, and `baseUrl` are
persisted as bridge-owned lifecycle metadata and may be shown in prompt/UI
surfaces, but they do not mutate the spawned thread's base `cwd`. Thread spawn
context remains the project-configured path unless the spawn request itself
explicitly overrides it.

Hook paths should resolve relative to project root unless explicitly absolute.

### Initial Hook Events

V1 should start with:

- `onWorkerCreate`
- `onWorkerArchive`

Likely next:

- `onQaCreate`
- `onQaArchive`
- `onWarmHandoff`

### Hook Execution Contract

Each hook is any shebang-executable script.

The bridge supplies:

- environment variables
- JSON stdin payload
- strict stdout JSON response parsing

Suggested env:

- `ROBDEX_HOOK_EVENT`
- `ROBDEX_PROJECT_ROOT`
- `ROBDEX_PROJECT_ID`
- `ROBDEX_THREAD_ID`
- `ROBDEX_AGENT_NAME`
- `ROBDEX_AGENT_ROLE`

Example `onWorkerCreate` stdin:

```json
{
  "event": "onWorkerCreate",
  "projectRoot": "/Users/robertsale/Code/ezra/ezra",
  "threadId": "019...",
  "agent": {
    "name": "Worker QA Workflow Print View",
    "role": "worker"
  },
  "defaults": {
    "branchName": "codex/worker-qa-workflow-print-view",
    "worktreePath": "/Users/robertsale/Code/ezra/ezra/.worktrees/worker-qa-workflow-print-view"
  }
}
```

Example stdout:

```json
{
  "ok": true,
  "artifacts": {
    "worktreePath": "/Users/robertsale/Code/ezra/ezra/.worktrees/worker-qa-workflow-print-view",
    "branchName": "codex/worker-qa-workflow-print-view",
    "stackName": "worker-qa-workflow-print-view",
    "baseUrl": "http://127.0.0.1:54136"
  },
  "promptAppend": [
    "Your worktree is located at /Users/robertsale/Code/ezra/ezra/.worktrees/worker-qa-workflow-print-view.",
    "Use branch codex/worker-qa-workflow-print-view."
  ],
  "cleanup": {
    "onArchive": true
  }
}
```

### Hook Output Responsibilities

The bridge should accept:

- `artifacts`
  - structured lifecycle outputs such as worktree path, branch name, stack name,
    base URL, allocated ports, docker network, subnet, etc.
- `promptAppend`
  - append-only text injected into the spawned agent’s initial prompt
- `cleanup`
  - cleanup intent/policy metadata for archive time
- `metadata`
  - optional project-defined opaque lifecycle state

The bridge should reject:

- invalid JSON
- missing required fields for the specific hook
- hook paths outside allowed resolution policy

### Administrator vs Orchestrator Authority

These surfaces are intentionally different and must not drift together:

- Administrator lifecycle surface
  - `thread/start`, `thread/resume`, `thread/fork`
  - may set explicit role, cwd, sandbox, model, reasoning, and other session overrides
  - this is the GUI/admin authority surface

- Orchestrator subordinate-spawn surface
  - `orchestrator/spawn-agent`
  - is not administrator authority
  - may choose subordinate role plus display name and prompt only
  - must derive `cwd`, approval policy, sandbox, and network settings from authoritative bridge project/global state
  - must derive role-specific model/instructions from the target role, not the orchestrator thread
  - must reject `orchestrator`, `operator`, and `hidden` target roles

Bridge tests should treat this split as invariant. If a change causes orchestrator spawn to inherit role/session settings from the wrong authority, that is a regression.

### Current Hook Input Shape

Create hooks now receive bridge-owned context beyond just the raw project root:

- `project`
  - `id`
  - `name`
  - `root`
- `requestedCwd`
- `agent`
  - `name`
  - `role`
- `parentThreadId` when applicable
- `spawn`
  - `approvalPolicy`
  - `sandboxMode`
  - `networkAccess`
  - `modelID`
  - `modelProvider`
  - `reasoningEffort`
  - `serviceTier`
  - `serviceName`
  - `ephemeral`

Worker create hooks also receive bridge-suggested defaults:

- `defaults.branchName`
- `defaults.worktreePath`

Archive hooks receive the same `project` and `agent` context plus:

- `threadId`
- `requestedCwd`
- `lifecycle`

### Bridge-Owned Hook State

The bridge must persist hook lifecycle artifacts per thread.

Minimum shape:

- thread id
- project id
- hook event name
- worktree path
- branch name
- stack/runtime identifiers
- cleanup policy
- prompt append content used
- last hook status
- opaque metadata blob

Current bridge shape now persists the common lifecycle fields explicitly:

- `branchName`
- `worktreePath`
- `baseUrl`
- `stackName`

while also retaining the original `artifacts` map for project-defined extensible
state and backward compatibility.

This state is what makes cleanup deterministic during:

- archive
- warm handoff
- bridge restart

### Prompt Mutation Rules

V1 should allow append-only prompt mutation.

That means hooks may return:

- `promptAppend`

But not:

- full prompt replacement
- arbitrary overwrite of base developer instructions

This keeps hook behavior auditable and limits blast radius.

### Guardrails

- hooks are loaded only from the selected project root
- relative hook paths resolve under project root
- hook output is schema-validated
- create/archive hook failures and timeouts now fall back to "no hook applied"
  rather than blocking thread lifecycle
- failure/time-out telemetry is persisted in bridge state for inspection
- bridge persists hook results before considering setup complete

## Proposed Worker Lifecycle With Hooks

1. orchestrator spawns worker
2. bridge loads `.codex/robdex-hooks.json`
3. bridge derives default slug / branch / worktree suggestions
4. bridge runs `onWorkerCreate`
5. hook creates or validates worktree / branch / stack
6. bridge persists returned lifecycle artifacts
7. bridge appends hook-provided prompt guidance to the initial worker prompt
8. worker begins from bridge-owned prepared context

Archive path:

1. worker archived
2. bridge loads persisted lifecycle artifacts
3. bridge runs `onWorkerArchive`
4. hook cleans up worktree / stack / branch-local resources as configured
5. bridge records cleanup outcome

## Non-Goals

- full project automation policy engine in v1
- arbitrary hook-triggered mutation of bridge global state
- putting QA harness runtime management inside bridge hooks
- allowing agents themselves to become the source of truth for setup/teardown

## Implementation Checklist

- [ ] Add bridge-local `ARCHITECTURE.md`
- [ ] Define `robdex-hooks.json` schema
- [ ] Add hook config loader under project root
- [ ] Add executable hook runner with JSON stdin/stdout contract
- [ ] Add persisted lifecycle artifact state per thread
- [ ] Add `onWorkerCreate` execution during worker spawn
- [ ] Add prompt append support from hook results
- [ ] Add `onWorkerArchive` execution during archive
- [ ] Add cleanup failure persistence and UI visibility
- [ ] Add unit tests for hook config resolution, hook output validation, and prompt append behavior

## Current Stopping Point

As of this document version:

- bridge talks to the empty `codex-qa-harness` service
- frontend surfaces harness-empty status through workbench status text
- project lifecycle hooks are designed but not implemented
