# Codex Backend Workspace

This workspace is the future home for codex-owned auxiliary servers and their
supervisor/runtime configuration.

Current goal:

- create a stable Rust development area under `~/.codex/backend`
- keep codex-owned Python service code under the same root when Python is the
  right implementation choice
- mirror the currently running service surface where it remains active
- keep existing Python services in place until each Rust replacement is
  implemented and cut over deliberately

Live service inventory at scaffold time:

- `codex-aux-http`
- `codex-flutter-sim-http`
- `codex-flutter-http`
- `robdex-app-server`
- `robdex-bridge-deno`

Only the first five are in scope for this workspace right now. Robdex services
remain external until you decide to migrate them.

## Workspace Layout

- `crates/codex-backend-core`: shared runtime/config helpers
- `crates/codex-aux-http`: request-review support HTTP service
- `crates/codex-flutter-sim-http`: project-scoped Flutter simulator broker/reservation service
- `crates/codex-flutter-http`: generic Flutter execution service
- `crates/codex-supervisor`: supervisor config inventory and future management tooling
- `python/codex-services`: backend-local Python service code for the current
  Codex auxiliary servers
- `docs/`: migration notes and implementation sequencing
- `supervisor/`: codex-owned supervisor templates and inventories

## Migration Note

The Rust `codex-aux-http` service owns the request-review support surface. The
backend-local Python services own the remaining Flutter broker lanes.
Historical references to the old
`~/Code/parallels-sync` home remain only for provenance.

## Next Step

Keep the remaining auxiliary services moving toward the same local-script or
Rust-owned cutover model where it reduces operational complexity.
