---
name: flutter-driver-cli
description: Use this skill when you need to drive broker-managed iOS QA simulators through the local wrapper scripts. The broker owns runtime lifecycle and uses idb for UI interaction. [skill-hash:3e0d6ea]
---

# Flutter Driver CLI

Use only these scripts:

```sh
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-sim ...
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive ...
```

`flutter-sim` talks to the broker lifecycle server.
`flutter-drive` talks to the separate command server.

## Rules

- Do not use `flutter devices`.
- Do not use `xcrun`, `simctl`, or `osascript` for simulator management.
- Do not launch the app manually.
- Do not issue parallel commands against the same device.
- Keep simulators in portrait or landscape. The command server resolves tap coordinates by probing the matching rotation automatically.

## `flutter-sim`

Broker lifecycle commands:

```sh
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-sim devices
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-sim reserve --device-id <udid>
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-sim reboot --device-id <udid>
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-sim dump-logs --device-id <udid>
```

Use them like this:

1. `devices`
   - lists broker-known booted simulators
2. `reserve`
   - blocks until the runtime is ready
   - returns API host plus login credentials
3. `reboot`
   - rebuilds the runtime on that simulator
4. `dump-logs`
   - snapshots broker, API, runtime, and driver artifacts into `/tmp/flutter-driver-screenshots/<udid>/logs`

## `flutter-drive`

UI interaction commands:

```sh
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive hierarchy --device-id <udid>
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive screenshot --device-id <udid> --out current.png
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive command <name> --device-id <udid> [--input <json>] [--label <text>] [--out <file>]
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive flow --device-id <udid> --input <json-array> [--label <text>]
```

Supported practical commands:

- `tapOn`
- `longPressOn`
- `inputText`
- `swipe`
- `takeScreenshot`
- `clearField`

## Typical flow

```sh
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-sim reserve --device-id <udid>
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive hierarchy --device-id <udid>
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive command tapOn --device-id <udid> --input '{"text":"Search"}'
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive command inputText --device-id <udid> --input '"query"'
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive screenshot --device-id <udid> --out result.png
```

## Hierarchy

`hierarchy` prints a compact accessibility listing derived from `idb`.

Each line may include:

- visible label
- `id=...` when present
- `value=...` when present
- role
- bounds

Use it to:

- see what is on screen
- choose a selector for `tapOn`
- verify text field contents
- inspect the current interactable screen state

Use `--json` only for raw diagnostics.

`tapOn` and `longPressOn` already return:

- a short description of what was tapped
- the post-tap hierarchy

So agents usually do not need a separate `hierarchy` call immediately after tapping.

## Text fields

Preferred pattern:

1. `tapOn` the field
2. `inputText`
3. verify with `hierarchy`

To clear a field:

```sh
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive command clearField \
  --device-id <udid> \
  --input '{"text":"Search customers, locations, jobs, estimates…"}'
```

`clearField` tries:

1. focus
2. hardware-keyboard `Cmd+A`
3. backspace

It does not use long-press or `Select All`.

## Notes

- The broker uses per-device serialization only.
- Different devices can be driven concurrently.
- Coordinate transforms for brokered taps and swipes are handled internally for the supported orientations.
