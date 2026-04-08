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
  Main Codex configuration. Profiles, model defaults, sandbox defaults, MCP registration, and runtime behavior start here. `danger-full-access` is set, but is not used in practice.

- [`AGENTS.md`](~/.codex/AGENTS.md)
  Global operating rules for agents working in this home directory.

- [`roles/`](~/.codex/roles)
  Base instructions and role-specific prompt files.

### Skills 🧰

- [`skills/`](~/.codex/skills)
  Local skill library. Each skill usually contains a `SKILL.md` plus scripts and supporting assets.

Important skills in active use:

- `command-execution`
  Job-based execution model with stable `job_id`s. This allows agents to run long-running commands without constantly polling, wasting tokens, and potentially terminating a good running process that just takes more than 60 seconds to complete.

- `command-parser`
  Wrapper for noisy commands like builds, tests, and lint-like tooling. Uses a smaller, cheaper model to parse tool call outputs for a coding agent so their context does not fill with garbage.

- `request-review`
  Review wrapper for code changes.

- `robdex-orchestrator`
  Robdex messaging and worker orchestration surface.

- `gh-version-control-workflow`
  Script-first worktree and publish workflow. All of these scripts are designed around additive commands that are not destructive.

- `safe-delete`
  Non-destructive delete flow. Throws items into `/tmp` to be reconciled manually or on reboot.

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

Rationale:

- Having privileged services expose CLI tooling allows for workspace-write sandbox with sensible escape hatches.

See [`backend/README.md`](~/.codex/backend/README.md) for the service-level breakdown.

### MCP servers 🔌

- [`mcp/`](~/.codex/mcp)
  Local MCP server implementations used by this environment.

- [`mcp/command-execution`](~/.codex/mcp/command-execution)
  The one and only MCP server. This is the most important one. It allows agents to await long-running commands without polling stdin. Designed to be used in conjunction with the skill script.

### Robdex state 🧭

- [`robdex/`](~/.codex/robdex)
  Live Robdex bridge state and caches. These are intentionally ignored, but generated when running the backend and using it.

Important files:

- [`robdex/robdex.json`](~/.codex/robdex/robdex.json)
  Project, agent, and orchestration state.

- [`robdex/robdex.sqlite`](~/.codex/robdex/robdex.sqlite)
  Thread cache and bridge-side persisted data.

- [`robdex/migration-backups/`](~/.codex/robdex/migration-backups)
  Manual backups taken before risky bridge cutovers.

## Current Operating Model 🧠

### Command execution

- Commands are job-based.
- Long-running work should be awaited via the job workflow, not rerun.
- `launch-job` script is symlinked as `zsh` somewhere.
- `/bin/zsh` is symlinked as `hsz` somewhere and `chsh .../hsz`
- Codex launches, detects `hsz` is the login shell, reacts by fetching `launch-job` as `zsh` from modified PATH.
- All commands now produce a stable job_id that the agents must await using the MCP tool.

### Noisy commands

- Use `command-parser` when coverage exists.
- Treat raw noisy output as a last resort.
- Positive and Negative rulesets prevent command parser from being used for trivial commands like `ls`, while preventing noisy commands from being ran directly.
- Rules have justifications pointing to this tool.

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

## Short version ✨

`~/.codex` is the live home directory for Codex, Robdex, local skills, bridge state, MCP servers, and service infrastructure. Treat it like a small local platform, not a passive config folder. This is an operating system for agentic work.
