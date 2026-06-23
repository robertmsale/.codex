# Robdex Public Bootstrap Roadmap

This roadmap tracks the path from Robert's live `.codex` control plane to a
public, reversible Robdex install path. The target model is an overlay/tooling
package, not a clone-over-`~/.codex` dotfiles install.

## Finish Line

Robdex should be installable without replacing a user's existing Codex home:

```sh
git clone https://github.com/robertmsale/codex-robdex ~/.local/share/robdex
export ROBDEX_HOME="$HOME/.local/share/robdex"
export PATH="$ROBDEX_HOME/scripts:$PATH"
robdex doctor
robdex bootstrap plan --profile minimal
robdex setup orchestration --dry-run
robdex start
```

The installer and docs must make the core orchestration path easy to reach while
keeping advanced local workflow pieces opt-in.

## Stages

### 1. Relocatable Core

Goal: core Robdex scripts and runners derive paths from environment variables
instead of Robert-specific absolute paths.

Canonical variables:

- `ROBDEX_HOME`: checkout/install root for this repo.
- `CODEX_HOME`: user's Codex home, defaulting to `~/.codex`.
- `ROBDEX_STATE_HOME`: Robdex bridge state root, defaulting under
  `CODEX_HOME/robdex` for current compatibility.
- `ROBDEX_BRIDGE_BASE_URL`: bridge HTTP base URL.

Remaining blockers:

- `config.toml` is a personal live config, not an install template.
- generated supervisor/systemd/launchd templates are not implemented yet.
- host-specific helper services may still encode local paths outside the active
  local worker/QA worktree workflow.

### 2. Doctor Diagnostics

Goal: a read-only command reports whether a host can run the core Robdex path.

The doctor checks:

- OS support classification.
- `CODEX_HOME` and `ROBDEX_HOME` assumptions.
- `codex` availability.
- Rust toolchain availability.
- bridge release/debug build status.
- safe app-server and bridge health reachability.
- required Robdex scripts on `PATH`.
- available service managers.

This slice adds the first doctor implementation at `scripts/robdex-doctor`.
`robdex doctor` is also available through the public Robdex wrapper.

### 3. Safe Config Overlay and Profiles

Goal: public setup must never overwrite `~/.codex/config.toml`.

Implemented public commands:

- `robdex config diff --profile minimal`
- `robdex config apply --profile orchestration`
- `robdex config backup`
- `robdex config restore`
- `robdex config uninstall`
- `robdex bootstrap doctor`
- `robdex bootstrap plan --profile minimal`
- `robdex bootstrap apply --profile minimal`
- `robdex bootstrap rollback --receipt PATH`
- `robdex bootstrap uninstall --work-dir PATH`

Profiles should live as templates, not applied live by default:

- `minimal`: core bridge/CLI basics.
- `orchestration`: bridge-backed communication/spawning,
  orchestrator/operator/worker roles, Requirements, and safe project
  registration.
- `privileged-exec`: sanctioned privileged execution policy; advanced opt-in.
- `gui`: optional Flutter UI integration.
- `robertmsale.reference`: personal reference profile only.

### 4. Opt-In Orchestration Setup

Goal: the core value path remains headless and explicit.

Setup should install or link:

- `robdex` CLI surface.
- roles and skill docs needed for orchestrator/worker/QA flows.
- Requirements workflow support.
- bridge-backed metadata and communication.
- optional hooks and privileged-exec policy.

It should not require Flutter, iOS simulators, design lab tooling, or device
driver services.

### 5. Service Start and Status

Goal: one command starts the core local stack or gives exact remediation.

Support order:

- macOS service manager integration when configured.
- Linux `systemd --user` or supervisor integration when configured.

`supervisorctl` should remain an implementation option, not a public
prerequisite.

This slice adds `scripts/robdex-service` and `robdex start/status/stop`
delegation. It supports configured `supervisor`, `systemd --user`, or
`launchctl` service-manager paths. It intentionally does not support pid-file or
foreground fallback ownership for normal service control.

### 6. Optional GUI Setup

Goal: GUI remains a convenience layer over the core.

The Flutter GUI may be installed and built separately. It must not become a
prerequisite for bridge, Robdex communication, Requirements, or orchestration.
Packaging notes live in `docs/gui-packaging.md`.

### 7. Platform Support

Supported target:

- macOS: primary.
- Linux: intended for the core bridge/CLI/orchestration path.

Explicit limitation:

- Windows is not supported yet. A real design is needed for shell behavior,
  path handling, service management, PTY behavior, and privileged execution.

## Completed In This Slice

- Added this roadmap.
- Added a read-only bootstrap doctor entrypoint.
- Documented the overlay model and staged config profile plan.
- Added config overlay profile templates and reversible
  `robdex config diff/apply/backup/restore/uninstall` commands.
- Made the app-server runner and bridge defaults derive from public variables.
- Added public `robdex doctor/start/status/stop` delegation.
- Added `robdex setup orchestration --dry-run` and `--stage-dir` for opt-in
  setup inspection and safe link staging.
- Added service template generation for launchd, systemd-user, and supervisor.
- Added public install and Linux support docs.
- Added advanced orchestration and GUI packaging docs.
- Simplified QA direction: QA uses assigned worktrees and device UDIDs with
  designer-runtime tooling.

## Remaining Work

- Validate the core path on a Linux host.
- Expand config profiles as real-world installs report gaps.
- Add optional GUI packaging automation if manual Flutter packaging is not
  enough.
