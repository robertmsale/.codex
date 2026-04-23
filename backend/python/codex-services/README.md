# codex-services

Host-side support services and sync definitions for Robdex and Ezra.

Current direction:

- Codex agents run on macOS
- the Robdex Rust bridge and Codex app-server run on macOS
- Ezra QA uses one mirrored repo at `/Users/robertsale/Code/ezra/qa/repo`
- the Flutter simulator broker stays host-side

## Mutagen

The active Mutagen project is intentionally narrow:

- `ezra-qa-repo`: `/Users/robertsale/Code/ezra/ezra` -> `/Users/robertsale/Code/ezra/qa/repo`

Ignored paths:

- `.git`
- `.worktrees`

Project file:

- [mutagen.yml](/Users/robertsale/.codex/backend/python/codex-services/mutagen.yml)

## Robdex Host Runtime

Supervisor units:

- [robdex-app-server.ini](/Users/robertsale/.codex/backend/python/codex-services/supervisor/robdex-app-server.ini)
- [robdex-bridge.ini](/Users/robertsale/.codex/backend/supervisor/templates/robdex-bridge.ini)
- [codex-aux-http.ini](/Users/robertsale/.codex/backend/python/codex-services/supervisor/codex-aux-http.ini)
- [codex-flutter-sim-http.ini](/Users/robertsale/.codex/backend/python/codex-services/supervisor/codex-flutter-sim-http.ini)

Launchers:

- [run-robdex-app-server](/Users/robertsale/.codex/backend/python/codex-services/scripts/run-robdex-app-server)
- [run-codex-robdex-bridge](/Users/robertsale/.codex/backend/bin/run-codex-robdex-bridge)
- [run-codex-aux-http](/Users/robertsale/.codex/backend/python/codex-services/scripts/run-codex-aux-http)
- [run-codex-flutter-sim-http](/Users/robertsale/.codex/backend/python/codex-services/scripts/run-codex-flutter-sim-http)

Robdex host-local transport:

- Codex app-server: `ws://127.0.0.1:4200`
- Rust bridge bind: `127.0.0.1:42080`
- Rust bridge app-server target: `ws://127.0.0.1:4200`
- Bridge state JSON: `~/.codex/robdex/robdex.json`
- Bridge thread cache SQLite: `~/.codex/robdex/robdex.sqlite`

## Codex Hooks

The host-global Codex hooks file is:

- `/Users/robertsale/.codex/hooks.json`

Current hook wiring points at an external host-global launcher under `/Users/robertsale/Code/parallels-sync/scripts/`.
There is no in-repo turn-lifecycle hook launcher here anymore.

## Codex Aux HTTP

Host-side helper service for sandbox-safe parser/review execution.

Bind:

- `http://127.0.0.1:8771`

Endpoints:

- `GET /healthz`
- `POST /v1/command-parser/parse`
- `POST /v1/request-review/run`

Used by:

- `~/.codex/skills/command-parser/scripts/command-parser`
- `~/.codex/skills/request-review/scripts/request-review`

## Flutter Simulator HTTP Bridge

Host-side Flutter broker for iOS simulators.

Run manually:

```sh
uv run codex-flutter-sim-http --host 0.0.0.0 --port 8767
```

API:

- `GET /healthz`
- `GET /devices`
- `GET /session?path=/Users/robertsale/...&device_id=optional`
- `POST /reserve` with JSON body `{"path":"...","target":"lib/flutter_driver_pilot_main.dart","device_id":"optional"}`
- `POST /restart` with JSON body `{"device_id":"...","path":"optional","target":"optional"}`
- `POST /release` with JSON body `{"device_id":"...","path":"optional"}`

The broker already supports:

- per-device reservations
- same-path launch serialization
- host-side `flutter run --machine --print-dtd`

## Flutter Drive CLI

`flutter-drive` is now a local wrapper under:

- [flutter-drive](/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive)

It no longer depends on a dedicated host HTTP service. The local helper uses the shared direct-idb driver path and keeps the `flow` path local.

## Ezra Host QA Planning

Architecture note:

- [ezra-host-native-qa.md](/Users/robertsale/.codex/backend/python/codex-services/docs/ezra-host-native-qa.md)

Planning helper:

- [ezra_host_qa.py](/Users/robertsale/.codex/backend/python/codex-services/src/codex_services_http/ezra_host_qa.py)
