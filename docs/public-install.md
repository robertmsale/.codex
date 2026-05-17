# Robdex Public Install

Robdex is a local Codex control plane for agent orchestration, Requirements
review, bridge-backed communication, and optional GUI visibility. The public
install model is an overlay: it must not replace an existing `~/.codex` folder
or overwrite `config.toml`.

## Quickstart

```sh
git clone https://github.com/robertmsale/codex-robdex ~/.local/share/robdex
export ROBDEX_HOME="$HOME/.local/share/robdex"
export CODEX_HOME="${CODEX_HOME:-$HOME/.codex}"
export PATH="$ROBDEX_HOME/scripts:$PATH"
mkdir -p "$CODEX_HOME"
robdex doctor
robdex bootstrap plan --profile minimal
robdex setup orchestration --dry-run
robdex start --foreground
```

Use `robdex status` in another shell to inspect the local bridge. Use
`robdex stop` only for a background or service-managed stack.

## Environment

- `ROBDEX_HOME`: checkout/install root for this repository.
- `CODEX_HOME`: user's Codex home. Defaults to `~/.codex`.
- `ROBDEX_STATE_HOME`: bridge state root. Defaults to
  `$CODEX_HOME/robdex`.
- `ROBDEX_BRIDGE_BASE_URL`: bridge HTTP URL. Defaults to
  `http://127.0.0.1:42080`.
- `ROBDEX_BRIDGE_APP_SERVER_URL`: app-server websocket URL. Defaults to
  `ws://127.0.0.1:4200`.

## Start, Status, Stop

`robdex start` delegates to `scripts/robdex-service`.

Supported modes:

- no `ROBDEX_SERVICE_MANAGER`: starts app-server and bridge with local pid and
  log files under `ROBDEX_STATE_HOME`;
- `robdex start --foreground`: runs the two-process core stack in the current
  terminal;
- `ROBDEX_SERVICE_MANAGER=supervisor`: uses configured supervisor service
  names;
- `ROBDEX_SERVICE_MANAGER=systemd`: uses configured Linux user units;
- `ROBDEX_SERVICE_MANAGER=launchctl`: uses configured macOS launchd labels.

`robdex status` reports configured paths, local pid-file state when applicable,
and bridge health. `robdex stop` stops the configured service manager or the
pid-file fallback processes.

`robdex install-plan` prints a human-readable service plan. For structured
frontend/bootstrap output, use:

```sh
robdex bootstrap doctor
robdex bootstrap plan --profile minimal
robdex bootstrap apply --profile minimal
robdex bootstrap rollback --receipt "$CODEX_HOME/robdex-bootstrap/receipts/minimal.receipt"
robdex bootstrap uninstall --work-dir "$CODEX_HOME/robdex-bootstrap"
```

The `bootstrap` commands emit newline-delimited JSON events with status,
proposed changes, logs, and failures. `apply` writes only managed files under
`$CODEX_HOME/robdex-bootstrap`; it does not overwrite `config.toml`.

## Opt-In Orchestration

The core install should expose orchestration intentionally. Enable these pieces
only after `robdex doctor` and `robdex status` are healthy:

- `robdex` CLI on `PATH`;
- role files under `roles/`;
- skills needed for Robdex messaging, request-review, Requirements,
  communication/spawning, and safe project registration;
- optional hooks for project lifecycle events;

Flutter, simulators, design lab tooling, and device-driver services are not
required for the core orchestration path.

Privileged-exec policy, shell-wrapper behavior, custom Codex builds,
designer/QA runtime tooling, and Robert-specific workflows are advanced
features. Enable them only through the explicit `privileged-exec`, `gui`, or
`robertmsale.reference` profiles after reviewing their host assumptions.

## QA Runtime

The public bootstrap path does not start or require the old managed QA harness
or Flutter simulator broker. For QA validation, assign:

- a normal worktree path;
- a device UDID;
- the scenario to validate.

QA then uses the same direct runtime tools as designers:

```sh
designer-flutter-run --session qa-app --device-id <UDID> --workdir <worktree>
designer-drive hierarchy --device-id <UDID>
designer-drive screenshot --device-id <UDID> --out current.png
```

The old `flutter-sim reserve/reboot` broker flow is legacy/deprecated and is
not part of the default install.

## Config Safety

Do not clone this repository over `~/.codex`. Do not replace an existing
`config.toml`.

The intended config workflow is reversible:

```sh
robdex config diff --profile minimal
robdex config apply --profile orchestration
robdex config backup
robdex config restore
robdex config uninstall
```

These commands stage overlays under `$CODEX_HOME/robdex-config-overlays` and do
not overwrite `config.toml`.

Profiles:

- `minimal`: bridge and CLI basics;
- `orchestration`: bridge-backed communication/spawning, orchestrator/operator/
  worker roles, Requirements, request-review, and safe project registration;
- `privileged-exec`: sanctioned privileged execution policy;
- `gui`: optional Flutter UI integration;
- `robertmsale.reference`: Robert's personal setup as reference only.

## Orchestration Setup

Use the dry run before enabling the orchestration profile:

```sh
robdex setup orchestration --dry-run
```

It verifies the Robdex CLI wrapper, role files, core skills, bridge workspace,
public docs, optional hooks, and Robdex rules without changing config or state.

To stage linkable orchestration surfaces for a config overlay:

```sh
robdex setup orchestration --stage-dir "$HOME/.local/share/robdex-overlay/orchestration"
```

The stage command creates symlinks for the public `robdex` CLI, roles, core
skills, docs, and Robdex rules under the chosen staging directory. It still does
not edit `config.toml`, hooks, live Robdex state, or services.

## Service Templates

Generate service templates without installing them:

```sh
robdex service-template --platform macos --out-dir /tmp/robdex-services
robdex service-template --platform linux --out-dir /tmp/robdex-services
robdex service-template --platform supervisor --out-dir /tmp/robdex-services
```

Review generated files before installing them into launchd, systemd-user, or
supervisor.

## Windows

Native Windows bootstrap is not supported yet. Use WSL for the Linux-oriented
core path, or treat Windows support as future work. A native Windows design
still needs explicit handling for paths, shell behavior, service management,
PTY behavior, and privileged execution.

## GUI

The Flutter GUI is optional. It can connect to the bridge once the core stack is
running, but it is not required for Robdex communication, Requirements, or agent
orchestration. See [`gui-packaging.md`](gui-packaging.md).

## Uninstall and Rollback

For the foreground or pid-file fallback:

```sh
robdex stop
rm -rf "$ROBDEX_HOME"
```

For service-managed installs, stop and disable the user services first. Keep or
remove `ROBDEX_STATE_HOME` deliberately; it contains local Robdex state.
