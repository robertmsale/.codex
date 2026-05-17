---
name: flutter-driver-cli
description: Legacy compatibility for the old managed Flutter QA driver. Prefer designer-runtime for active QA/designer piloting. [skill-hash:3e0d6ea]
---

# Flutter Driver CLI

This is a legacy compatibility skill. The active Robdex QA/designer piloting
workflow uses `designer-runtime`: an assigned worktree plus an assigned device
UDID, launched with `designer-flutter-run` and piloted with `designer-drive`.

Use this skill only when an existing project still names the old wrappers:

```sh
flutter-sim ...
flutter-drive ...
flutter ...
```

What they do:

- `flutter-sim`
  - deprecated compatibility surface
  - `devices` delegates to `designer-drive devices`
  - `reserve`, `reboot`, and `dump-logs` print a deprecation error instead of managing a broker-owned runtime
- `flutter-drive`
  - UI inspection and interaction through the shared direct `idb` driver
  - kept as a direct-idb compatibility wrapper
- `flutter`
  - host-side Flutter commands routed through the sanctioned wrapper

## Guardrails

- Prefer `designer-runtime` for new QA and design work.
- Do not use the managed reservation path unless the operator explicitly revives it for a legacy project.
- Use the shared wrappers instead of ad hoc `idb` invocations.
- Do not issue parallel piloting commands against the same device.
- Always wait for one piloting command to finish before sending the next.
- Run commands plainly and sequentially.
- Do not combine `flutter-drive` commands with shell operators or wrappers.
- Use `--json` only when you need raw diagnostics instead of the compact human-readable output.

## `flutter-sim`

`flutter-sim` is deprecated. It no longer reserves or reboots managed runtime
slots in the active workflow.

Commands:

```sh
flutter-sim devices
flutter-sim reserve --device-id <udid>   # deprecated, exits with guidance
flutter-sim reboot --device-id <udid>    # deprecated, exits with guidance
flutter-sim dump-logs --device-id <udid> # deprecated, exits with guidance
```

What they are for:

- `devices`
  - lists booted simulators visible through the shared direct driver
- `reserve`, `reboot`, and `dump-logs`
  - retained only to produce clear deprecation output

## `flutter-drive`

Use `flutter-drive` only for compatibility with older instructions. New work
should use `designer-drive` directly.

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
- `clearAndInputText`
- `clearField`
- `swipe`
- `takeScreenshot`

Typical sequence:

```sh
designer-flutter-run --session qa-app --device-id <udid> --workdir <worktree_path>
designer-drive hierarchy --device-id <udid>
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
