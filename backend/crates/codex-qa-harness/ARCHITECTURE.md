# codex-qa-harness

Generic project-scoped QA harness daemon for Robdex integration.

## V1 Scope

- iOS simulators only
- no Flutter-specific core assumptions
- per-project lifecycle hooks
- per-device hidden runtime roots
- lease-based ownership
- persisted state and event-oriented HTTP service

## Core Model

- `ProjectConfig`
  - repo root, runtime root, devices, hooks, timeouts, env
- `DeviceConfig`
  - `ios_sim` only in v1, keyed by simulator UDID
- `LeaseRecord`
  - slot ownership and intent
- `SlotRuntimeState`
  - current status, phase, artifacts, tracked processes, last error

## Planned Lifecycle

1. boot simulator on demand
2. `prepare_source`
3. `start_dependencies`
4. `start_runtime`
5. `check_readiness`
6. serve driver commands
7. `teardown`

## HTTP Surface

- `GET /healthz`
- `GET /projects`
- `GET /projects/:project_id/devices`
- `GET /projects/:project_id/devices/:device_id`
- `POST /projects/:project_id/devices/:device_id/lease`
- `DELETE /projects/:project_id/devices/:device_id/lease`
- `POST /projects/:project_id/devices/:device_id/start`
- `POST /projects/:project_id/devices/:device_id/restart`
- `POST /projects/:project_id/devices/:device_id/teardown`
- `POST /projects/:project_id/devices/:device_id/commands`

## Implementation Checklist

- [x] Add workspace crate and executable entrypoint
- [x] Define v1 config schema and loader
- [x] Define runtime state, lease, and command models
- [x] Add persistent state store scaffold under `~/.qa-harness`
- [x] Add HTTP route scaffold for project/device/lease/runtime operations
- [x] Add iOS simulator helper surface for lazy boot checks
- [x] Add unit-testable runtime construction and behavioral tests for lease/lifecycle rules
- [x] Implement hook runner with JSON stdin/stdout contract
- [ ] Implement per-slot serialized state machine transitions
- [ ] Persist and reconcile tracked child processes across daemon restarts
- [ ] Add SSE event stream for Robdex follow mode
- [x] Implement simulator boot/check using `simctl`
- [x] Implement hook-backed lifecycle phases
- [x] Implement hook-backed generic command execution
- [ ] Add supervised runner wiring and service config
- [ ] Add Robdex integration path

## Notes

- The daemon should own lifecycle state; Robdex should consume it.
- Project-specific behavior belongs in hook scripts and project config, not in the Rust core.
- No infinite waits should exist in the daemon core. Every phase should eventually gain a deadline.
