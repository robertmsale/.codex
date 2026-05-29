# Codex Home

This is the live `~/.codex` control plane for a customized Codex setup.
It is the runtime home for:

- agent config and profiles
- local skills and workflow wrappers
- Robdex bridge/runtime state
- backend services that support Codex operations

For public bootstrap/install guidance, start with
[`docs/public-install.md`](docs/public-install.md). The public model is an
overlay/tooling install; do not clone this repository over an existing
`~/.codex`.

QA now follows the simplified direct runtime model: assign a QA agent a normal
worktree and device UDID, then use `designer-flutter-run`, `designer-drive`, and
`designer-crop-screenshot` for launch, piloting, and evidence. The older managed
QA harness / Flutter simulator broker stack is legacy/deprecated.

## What Lives Here

### Core config

- [`config.toml`](~/.codex/config.toml)
  Main Codex configuration. Profiles, model defaults, sandbox defaults, MCP registration, and runtime behavior start here. `danger-full-access` is set, but is not used in practice.

- [`AGENTS.md`](~/.codex/AGENTS.md)
  Global operating rules for agents working in this home directory.

- [`roles/`](~/.codex/roles)
  Base instructions and role-specific prompt files.

### Skills

- [`skills/`](~/.codex/skills)
  Local skill library. Each skill usually contains a `SKILL.md` plus scripts and supporting assets.

Important skills in active use:

- `robdex-orchestrator`
  Robdex messaging and worker orchestration surface.

- `gh-version-control-workflow`
  Script-first worktree and publish workflow for mutating git/gh operations.

- `safe-delete`
  Non-destructive delete flow. Throws items into `/tmp` to be reconciled manually or on reboot.

Retired workflows:

- `command-parser`
  Removed from the active workflow. It previously compacted noisy shell output,
  but now adds overhead. See [`docs/command-parser-decommission.md`](~/.codex/docs/command-parser-decommission.md).

### Backend services

- [`backend/`](~/.codex/backend)
  Codex-owned service workspace.

Current layout:

- Rust workspace for high-performance local services
- Python staging area for sync/flutter helper services
- vendored Codex source used by the Rust Robdex bridge adapter
- supervisor templates and launch scripts

Notable pieces:

- Rust Robdex bridge
- legacy simulator broker and Flutter helper services

See [`backend/README.md`](~/.codex/backend/README.md) for the service-level breakdown.

### Robdex state

- [`robdex/`](~/.codex/robdex)
  Live Robdex bridge state and caches. These are intentionally ignored, but generated when running the backend and using it.

Important files:

- [`robdex/robdex.json`](~/.codex/robdex/robdex.json)
  Project, agent, and orchestration state.

- [`robdex/robdex.sqlite`](~/.codex/robdex/robdex.sqlite)
  Thread cache and bridge-side persisted data.

- [`robdex/migration-backups/`](~/.codex/robdex/migration-backups)
  Manual backups taken before risky bridge cutovers.

## Current Operating Model

### Command execution

- Agents run through the configured `zsh` wrapper at [`scripts/zsh`](scripts/zsh).
- The wrapper reconstructs `PATH`, applies the dynamic privileged-exec policy, reports live process state, and preserves synchronous command execution.
- Long-running commands are synchronous. There is no command-execution job/MCP wait path anymore.

### Requirements

- Requirements are an active completion-contract mechanism.
- When active, requirement claims are schema-gated and routed to a requirements reviewer.
- Passing review clears the active Requirements; failed review routes corrections back to the source agent.

### Robdex

- The Rust bridge is the live bridge implementation.
- Bridge state is owned here in `~/.codex`.
- The GUI should receive snapshots/events and send narrow intent commands.

### Services

- Supervisor-managed support services are part of this environment.
- Rust and Python services both live under `backend/`.
- Restart-sensitive changes should be treated like production ops.
- Public bootstrap uses `robdex doctor`, `robdex start`, `robdex status`, and
  `robdex stop` through the relocatable service wrapper.

## Files To Treat Carefully

- [`config.toml`](~/.codex/config.toml)
- [`AGENTS.md`](~/.codex/AGENTS.md)
- [`robdex/robdex.json`](~/.codex/robdex/robdex.json)
- [`robdex/robdex.sqlite`](~/.codex/robdex/robdex.sqlite)
- [`rules/`](~/.codex/rules)

Changes here can affect active agents, approvals, orchestration, shell execution, or bridge behavior immediately.

## Short version ✨

`~/.codex` is the live home directory for Codex, Robdex, local skills, bridge state, and service infrastructure. Treat it like a small local platform, not a passive config folder.
