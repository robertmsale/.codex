---
name: flutter-driver-cli
description: Use this skill when you need to drive a native Flutter app on iOS through the simulator broker, including reserving a runtime, inspecting apps and widget trees, sending driver commands, taking screenshots, and rebooting a runtime when a fresh build is needed. [skill-hash:3e0d6ea]
---

# Flutter Driver CLI

Use the local wrappers:

```sh
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-sim ...
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive ...
```

This skill is for native Flutter driving on iOS through the simulator broker.

## Hard requirements

- Do not use `flutter devices` for discovery as part of this skill.
- Do not use `xcrun` for simulator management as part of this skill.
- Do not use `osascript` as part of this skill.
- Do not reboot, reset, or otherwise manage simulators directly as part of this skill.
- Do not launch `flutter run` manually as part of this skill.
- Do not run multiple commands from this skill in parallel.
- Run every command in strict sequence and wait for each one to finish before starting the next one.
- Parallel driver calls make state observation unreliable and can invalidate QA results by racing taps, text entry, screenshots, and scroll state against each other.
- If you do not already know the target simulator device ID, ask the user or use `flutter-sim devices`.

## Standard workflow

1. Check devices:

```sh
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-sim devices
```

Skip step 1 if the orchestrator provides you with a device directly.

2. Reserve the target runtime:

```sh
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-sim reserve --device-id <id>
```

`reserve` waits for the broker to make the runtime ready and returns the information needed to drive the app.

3. Drive the app by device ID:

```sh
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive widget-tree \
  --device-id <id>
```

4. If fixes land and you need a fresh runtime, reboot it:

```sh
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-sim reboot --device-id <id>
```

Then continue using the same device ID with `flutter-drive`.

## Common driver commands

```sh
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive apps \
  --device-id <id>

/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive widget-tree \
  --device-id <id>

/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive driver screenshot \
  --device-id <id> \
  --out app.png

/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive driver get_health \
  --device-id <id>
```

## Finder guidance

Prefer selectors in this order:

1. `ByTooltipMessage` for icon buttons and header actions
2. `ByText` for visible text controls
3. `Ancestor` or `Descendant` when the visible text is only a child of the tappable widget
4. `ByType` only when the tree clearly shows a unique runtime type

```sh
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive driver get_text \
  --device-id <id> \
  --arg finderType=ByText \
  --arg 'text=Sales'
```

```sh
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive driver tap \
  --device-id <id> \
  --arg finderType=ByTooltipMessage \
  --arg 'text=Close chat'
```

For nested finders, prefer `--input` with one JSON object:

```sh
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive driver tap \
  --device-id <id> \
  --input '{"finderType":"Ancestor","of":{"finderType":"ByText","text":"Export"},"matching":{"finderType":"ByType","type":"OutlinedButton"},"firstMatchOnly":"true","matchRoot":"false"}'
```

For screenshots:

- Provide only the image name with `--out`
- The wrapper writes the file under `/tmp/flutter-driver-screenshots/<device-id>/<image-name>`
- The command prints the absolute path to the created screenshot
- For text entry, tap the field first and then run `enter_text` as a separate command

## Caveats

`flutter_driver` can resolve widgets that are present in the tree but not actually tappable.

If a tap times out:
- check `widget-tree`
- take a screenshot
- verify whether an overlay or offscreen state is blocking the target
