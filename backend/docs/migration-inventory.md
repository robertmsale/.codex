# Migration Inventory

This document captures the current service surface that should eventually move
under `~/.codex/backend`.

## Current Supervisor Programs

In scope for this workspace:

1. `codex-aux-http`
2. `parallels-sync-gitops-http`
3. `parallels-sync-flutter-sim-http`
4. `parallels-sync-flutter-drive-http`
5. `parallels-sync-flutter-http`

Out of scope for the first pass:

1. `robdex-app-server`
2. `robdex-bridge-deno`

## Current Source Locations

- `~/Code/parallels-sync/src/codex_aux_http.ts`
- `~/Code/parallels-sync/src/parallels_sync_gitops_http/server.py`
- `~/Code/parallels-sync/src/parallels_sync_gitops_http/flutter_sim_server.py`
- `~/Code/parallels-sync/src/parallels_sync_gitops_http/flutter_drive_server.py`
- `~/Code/parallels-sync/src/parallels_sync_gitops_http/flutter_exec_server.py`
- `~/Code/parallels-sync/supervisor/*.ini`
- `~/Code/parallels-sync/scripts/run-*`

## Likely Migration Order

1. `codex-aux-http`
2. `parallels-sync-flutter-http`
3. `parallels-sync-gitops-http`
4. `parallels-sync-flutter-sim-http`
5. `parallels-sync-flutter-drive-http`

Rationale:

- `codex-aux-http` is probably the smallest service and a good pattern-setter.
- `flutter-http` looks narrower than the sim/drive brokers.
- `gitops-http` has important behavior but clearer request/response contracts.
- `flutter-sim-http` and `flutter-drive-http` have the most subprocess and
  device-management complexity.

## Compatibility Rules For Rewrites

- keep existing port numbers until an intentional cutover
- preserve current env var names where practical
- preserve current supervisor program names unless there is a strong reason to
  rename them
- add parity tests before replacing live traffic
- prefer explicit failure over silent fallback when host tools are missing

