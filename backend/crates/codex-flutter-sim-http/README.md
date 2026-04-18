# codex-flutter-sim-http

Project-scoped Flutter simulator broker service.

This crate is the Rust replacement for the old Ezra-specific Python broker. It
does not hard-code a single project's launch process. Instead, it loads
project-owned lifecycle config and delegates startup, readiness, commands, and
teardown to hook programs.

## Purpose

Use this service when a project needs:

- a fixed set of manually assigned simulators
- lease-based ownership per device slot
- project-scoped runtime roots
- explicit lifecycle phases for source prep, dependencies, runtime start,
  readiness, commands, and teardown

The service keeps simulator ownership and phase state in Rust. Project-specific
behavior belongs in hook scripts and project config, not in the broker core.

## Config

Default config root:

```text
/Users/robertsale/.codex/backend/config/flutter-sim/projects
```

Default state root:

```text
/Users/robertsale/.codex/backend/state/flutter-sim
```

Override with:

- `CODEX_FLUTTER_SIM_CONFIG_DIR`
- `CODEX_FLUTTER_SIM_STATE_ROOT`

Each project config is a TOML file. The schema matches the generic QA harness
project model.

Example:

```toml
id = "ezra"
display_name = "Ezra"
repo_root = "/Users/robertsale/Code/ezra/ezra"
runtime_root = "/Users/robertsale/Code/ezra/qa"

[env]
EZRA_SOME_FLAG = "1"

[devices.primary]
type = "ios_sim"
device_id = "F8080594-26F7-4170-826A-452C15769215"
name = "iPad Pro 11-inch (M5)"
runtime_subdir = "F8080594-26F7-4170-826A-452C15769215"
boot_policy = "lazy"

[hooks]
prepare_source = "./hooks/prepare_source.sh"
start_dependencies = "./hooks/start_dependencies.sh"
start_runtime = "./hooks/start_runtime.sh"
check_readiness = "./hooks/check_readiness.sh"
teardown = "./hooks/teardown.sh"
command = "./hooks/command.sh"
```

Relative hook paths resolve from the config file directory.

## Lifecycle

The broker runs these phases in order:

1. boot simulator
2. `prepare_source`
3. `start_dependencies`
4. `start_runtime`
5. `check_readiness`
6. command execution
7. `teardown`

Hook contracts are JSON over stdin/stdout. The Rust broker owns:

- slot status
- slot phase
- lease ownership
- runtime artifact/process persistence

Projects own:

- how source is prepared
- how backend dependencies are started
- how Flutter is launched
- how readiness is checked
- how piloting commands are executed
- how teardown happens

## HTTP Surface

The service exposes the project-scoped broker interface:

- `GET /healthz`
- `GET /events`
- `GET /projects`
- `GET /projects/{project_id}/devices`
- `GET /projects/{project_id}/devices/{device_key}`
- `POST /projects/{project_id}/devices/{device_key}/lease`
- `DELETE /projects/{project_id}/devices/{device_key}/lease`
- `POST /projects/{project_id}/devices/{device_key}/start`
- `POST /projects/{project_id}/devices/{device_key}/restart`
- `POST /projects/{project_id}/devices/{device_key}/teardown`
- `POST /projects/{project_id}/devices/{device_key}/commands`
- `GET /projects/{project_id}/devices/{device_key}/simulator`

This is intentionally project-scoped. Device slots are not global, Ezra-only
resources anymore.
