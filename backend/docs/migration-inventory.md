# Migration Inventory

This document captures the current service surface that should eventually move
under `~/.codex/backend`.

## Current Supervisor Programs

In scope for this workspace:

1. `codex-aux-http`
2. `codex-flutter-sim-http`
4. `codex-flutter-http`

Out of scope for the first pass:

1. `robdex-app-server`
2. `robdex-bridge-deno`

## Current Source Locations

- `~/.codex/backend/crates/codex-aux-http`
- `~/.codex/backend/python/codex-services/src/codex_services_http/flutter_sim_server.py`
- `~/.codex/backend/python/codex-services/src/codex_services_http/flutter_exec_server.py`
- `~/.codex/backend/python/codex-services/supervisor/*.ini`
- `~/.codex/backend/bin/run-codex-*`

## Likely Migration Order

1. `codex-aux-http`
2. `codex-flutter-http`
3. `codex-flutter-sim-http`
Rationale:

- `codex-aux-http` is now narrowed to request-review support and remains the
  smallest service pattern-setter.
- `flutter-http` looks narrower than the sim broker.
- `flutter-sim-http` has the most subprocess and device-management complexity.

## Compatibility Rules For Rewrites

- keep existing port numbers until an intentional cutover
- prefer `CODEX_*` env names for backend-owned services
- use backend-local supervisor program names and launchers
- add parity tests before replacing live traffic
- prefer explicit failure over silent fallback when host tools are missing
