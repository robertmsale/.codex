# Codex Home 🛠️

This is the live `~/.codex` control plane for a heavily customized Codex setup.
It is not just dotfiles. It is the runtime home for:

- agent config and profiles
- local skills and workflow wrappers
- local MCP servers
- the Robdex bridge/runtime state
- backend services that support Codex operations

Think of this directory as operational infrastructure 🚦

## What Lives Here

### Core config ⚙️

- [`config.toml`](~/.codex/config.toml)
  Main Codex configuration. Profiles, model defaults, sandbox defaults, MCP registration, and runtime behavior start here.

- [`AGENTS.md`](~/.codex/AGENTS.md)
  Global operating rules for agents working in this home directory.

- [`roles/`](~/.codex/roles)
  Base instructions and role-specific prompt files.

- [`configs/`](~/.codex/configs)
  Extra config fragments and local overrides.

### Skills 🧰

- [`skills/`](~/.codex/skills)
  Local skill library. Each skill usually contains a `SKILL.md` plus scripts and supporting assets.

Important skills in active use:

- `command-execution`
  Job-based execution model with stable `job_id`s.

- `command-parser`
  Wrapper for noisy commands like builds, tests, and lint-like tooling.

- `request-review`
  Review wrapper for code changes.

- `robdex-orchestrator`
  Robdex messaging and worker orchestration surface.

- `gh-version-control-workflow`
  Script-first worktree and publish workflow.

- `safe-delete`
  Non-destructive delete flow.

### Backend services 🚀

- [`backend/`](~/.codex/backend)
  Codex-owned service workspace.

Current layout:

- Rust workspace for high-performance local services
- Python staging area for sync/flutter helper services
- vendored Codex source used by the Rust Robdex bridge adapter
- supervisor templates and launch scripts

Notable pieces:

- Rust Robdex bridge
- Rust aux HTTP server for `command-parser` and `request-review`
- Python sync services for gitops/flutter helpers

See [`backend/README.md`](~/.codex/backend/README.md) for the service-level breakdown.

### MCP servers 🔌

- [`mcp/`](~/.codex/mcp)
  Local MCP server implementations used by this environment.

These are live behavior surfaces, not placeholders.

### Robdex state 🧭

- [`robdex/`](~/.codex/robdex)
  Live Robdex bridge state and caches.

Important files:

- [`robdex/robdex.json`](~/.codex/robdex/robdex.json)
  Project, agent, and orchestration state.

- [`robdex/robdex.sqlite`](~/.codex/robdex/robdex.sqlite)
  Thread cache and bridge-side persisted data.

- [`robdex/migration-backups/`](~/.codex/robdex/migration-backups)
  Manual backups taken before risky bridge cutovers.

### Runtime artifacts 📦

These are generated/runtime-owned, not hand-maintained source:

- [`sessions/`](~/.codex/sessions)
- [`archived_sessions/`](~/.codex/archived_sessions)
- [`sqlite/`](~/.codex/sqlite)
- [`shell_snapshots/`](~/.codex/shell_snapshots)
- [`history.jsonl`](~/.codex/history.jsonl)
- [`tmp/`](~/.codex/tmp)

## Current Operating Model 🧠

### Command execution

- Commands are job-based.
- Long-running work should be awaited via the job workflow, not rerun.

### Noisy commands

- Use `command-parser` when coverage exists.
- Treat raw noisy output as a last resort.

### Robdex

- The Rust bridge is the live bridge implementation.
- Bridge state is owned here in `~/.codex`.
- The GUI should receive snapshots/events and send narrow intent commands.

### Services

- Supervisor-managed support services are part of this environment.
- Rust and Python services both live under `backend/`.
- Restart-sensitive changes should be treated like production ops 🔥

## Files To Treat Carefully ⚠️

- [`config.toml`](~/.codex/config.toml)
- [`AGENTS.md`](~/.codex/AGENTS.md)
- [`robdex/robdex.json`](~/.codex/robdex/robdex.json)
- [`robdex/robdex.sqlite`](~/.codex/robdex/robdex.sqlite)
- [`rules/`](~/.codex/rules)

Changes here can affect active agents, approvals, orchestration, or bridge behavior immediately.

## Validation Guide ✅

Use the real surface you changed.

Examples:

- shell scripts: `bash -n ...`
- Rust services: `cargo check` or targeted tests
- Python services: `python3 -m py_compile ...`
- bridge/service changes: hit the live HTTP/websocket surface after restart
- workflow wrappers: run the actual wrapper, not just unit tests

## Short version ✨

`~/.codex` is the live home directory for Codex, Robdex, local skills, bridge state, MCP servers, and service infrastructure. Treat it like a small local platform, not a passive config folder.
