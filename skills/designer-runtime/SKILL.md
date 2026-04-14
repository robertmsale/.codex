---
name: designer-runtime
description: Use this when a designer needs to pilot an ad-hoc iOS simulator directly, launch Flutter from their own worktree in tmux, or trigger hot reload without the broker/device-harness path.
---

# Designer Runtime

Use this skill for designers working directly from their own worktree and simulator.

Use only these scripts:

```sh
designer-drive ...
designer-flutter-run ...
designer-hot-reload ...
```

## What They Are For

- `designer-drive`
  - direct ad-hoc simulator inspection and interaction through plain `idb` commands plus small Python HID helpers for text editing cases
- `designer-flutter-run`
  - launches `flutter run -d <UDID>` in a tmux session from the designer worktree
- `designer-hot-reload`
  - sends `r` to the tmux session running Flutter

## Guardrails

- This skill is for designers, not QA broker flows.
- Do not use `flutter-sim` or the managed reservation path here.
- `designer-drive` must not rely on broker reservations, thread identity, mutagen lane setup, or managed runtime metadata.
- Work from the assigned designer worktree and keep controller/state logic intact unless the task explicitly says otherwise.
- Designers should focus on widget/interface work and avoid Rust changes unless explicitly reassigned.
- Use one simulator per active design loop and wait for each command to finish before issuing the next one.

## Typical Flow

1. Launch the app from the designer worktree:
   - `designer-flutter-run --session designer-app --device-id <UDID> --workdir <worktree_path>`
2. Inspect or interact with the running app:
   - `designer-drive hierarchy --device-id <UDID> --launch-path <worktree_path>`
   - `designer-drive command tapOn --device-id <UDID> --launch-path <worktree_path> --input '{"text":"Settings"}'`
3. After code changes, hot reload:
   - `designer-hot-reload --session designer-app`

## `designer-drive`

Commands:

```sh
designer-drive devices
designer-drive apps --device-id <UDID> --launch-path <path>
designer-drive hierarchy --device-id <UDID> --launch-path <path>
designer-drive screenshot --device-id <UDID> --launch-path <path> --out current.png
designer-drive command <name> --device-id <UDID> --launch-path <path> [--input <json>] [--label <text>] [--out <file>]
designer-drive flow --device-id <UDID> --launch-path <path> --input <json-array> [--label <text>]
```

Use `devices` to list booted simulators visible to idb. All other commands target the UDID you were given and the worktree launch path you are piloting from.
