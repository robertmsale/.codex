---
name: flutter-driver-cli
description: Use this skill when you need to drive a managed iOS device slot through the shared local wrappers. Use it for device lifecycle, hierarchy inspection, taps, text entry, screenshots, and scripted UI flows. [skill-hash:3e0d6ea]
---

# Flutter Driver CLI

Use this skill when a project provides a managed iOS QA/runtime device and the sanctioned way to interact with it is through the shared wrappers below.

Use only these scripts:

```sh
flutter-sim ...
flutter-drive ...
flutter ...
```

What they do:

- `flutter-sim`
  - lifecycle and device-slot management
- `flutter-drive`
  - UI inspection and interaction against the managed runtime
- `flutter`
  - host-side Flutter commands routed through the sanctioned wrapper

## Guardrails

- Use the shared wrappers instead of `xcrun`, `simctl`, `osascript`, or ad hoc `idb` invocations.
- Do not launch the app manually.
- Do not issue parallel piloting commands against the same device slot.
- Always wait for one piloting command to finish before sending the next.
- Use `--json` only when you need raw diagnostics instead of the compact human-readable output.

## `flutter-sim`

Use `flutter-sim` when you need the managed device/runtime lifecycle surface.

Commands:

```sh
flutter-sim devices
flutter-sim reserve --device-id <udid>
flutter-sim reboot --device-id <udid>
flutter-sim dump-logs --device-id <udid>
```

What they are for:

- `devices`
  - lists the currently known managed device slots
- `reserve`
  - waits for the selected device slot to be ready and returns the reservation/runtime details
- `reboot`
  - rebuilds or restarts the managed runtime for that device slot
- `dump-logs`
  - writes broker/runtime/driver artifacts to `/tmp/flutter-driver-screenshots/<udid>/logs`

## `flutter-drive`

Use `flutter-drive` when you need to inspect the screen or interact with it.

Commands:

```sh
flutter-drive hierarchy --device-id <udid>
flutter-drive screenshot --device-id <udid> --out current.png
flutter-drive command <name> --device-id <udid> [--input <json>] [--label <text>] [--out <file>]
flutter-drive flow --device-id <udid> --input <json-array> [--label <text>]
```

Common commands:

- `tapOn`
- `tapPoint`
- `longPressOn`
- `inputText`
- `swipe`
- `takeScreenshot`
- `clearField`

Typical sequence:

```sh
flutter-sim reserve --device-id <udid>
flutter-drive hierarchy --device-id <udid>
flutter-drive command tapOn --device-id <udid> --input '{"text":"Search"}'
flutter-drive command inputText --device-id <udid> --input '"query"'
flutter-drive screenshot --device-id <udid> --out result.png
```

## Hierarchy

`flutter-drive hierarchy` prints a compact accessibility listing derived from the current UI tree.

Each line may include:

- visible label
- `id=...` when present
- `value=...` when present
- role
- bounds

Use it to:

- see what is on screen
- choose a selector for `tapOn`
- choose an accessibility-space point for `tapPoint`
- verify field contents or visible state

`tapOn`, `tapPoint`, and `longPressOn` already return:

- a short description of what was hit
- the post-action hierarchy

So you usually do not need an immediate extra `hierarchy` call after those commands.

## Text fields

Preferred pattern:

1. `tapOn` the field
2. `inputText`
3. verify with `hierarchy`

To clear a field:

```sh
flutter-drive command clearField \
  --device-id <udid> \
  --input '{"text":"Search"}'
```

`clearField` focuses the control, attempts hardware-keyboard select-all, then deletes.

## Notes

- The lifecycle layer serializes work per device slot.
- Different device slots can be used independently.
- Coordinate transforms for taps and swipes are handled inside the sanctioned wrapper.
