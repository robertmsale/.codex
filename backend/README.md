# Codex Backend Workspace

This workspace is the future home for codex-owned auxiliary servers and their
supervisor/runtime configuration.

Current goal:

- create a stable Rust development area under `~/.codex/backend`
- keep codex-owned Python service code under the same root when Python is the
  right implementation choice
- mirror the currently running service surface
- keep existing Python and Deno services in place until each Rust replacement is
  implemented and cut over deliberately

Live service inventory at scaffold time:

- `codex-aux-http`
- `sync-gitops-http`
- `sync-flutter-sim-http`
- `sync-flutter-drive-http`
- `sync-flutter-http`
- `robdex-app-server`
- `robdex-bridge-deno`

Only the first five are in scope for this workspace right now. Robdex services
remain external until you decide to migrate them.

## Workspace Layout

- `crates/codex-backend-core`: shared runtime/config helpers
- `crates/codex-aux-http`: replacement for the current Deno aux server
- `crates/codex-gitops-http`: replacement for the current Python gitops bridge
- `crates/codex-flutter-sim-http`: simulator broker/reservation service
- `crates/codex-flutter-drive-http`: driver/control service
- `crates/codex-flutter-http`: generic Flutter execution service
- `crates/codex-supervisor`: supervisor config inventory and future management tooling
- `python/sync-services`: staged Python service code copied from the
  old `~/Code/parallels-sync` home so it is now within codex backend scope
- `docs/`: migration notes and implementation sequencing
- `supervisor/`: codex-owned supervisor templates and inventories

## Non-Goals

- no live service cutover
- no supervisor restart
- no edits to the currently running services in `~/Code/parallels-sync`

## Next Step

Implement one service at a time behind the existing ports and env contracts,
starting with the smallest/highest-confidence server.
