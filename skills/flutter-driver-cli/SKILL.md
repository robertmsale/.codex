---
name: flutter-driver-cli
description: Use this skill when you need to drive a native Flutter app on iOS through the simulator broker, including reserving a runtime, connecting with the returned DTD/App URIs, inspecting the widget tree, taking screenshots, and rebooting the runtime when a new build is needed. [skill-hash:6cc4fd4]
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
- If you do not already know the target simulator device ID, ask the user or use `flutter-sim devices`.

## Standard workflow

1. Check devices:

```sh
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-sim devices
```

2. Reserve the target runtime:

```sh
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-sim reserve --device-id <id>
```

`reserve` is the main entrypoint. It waits for the broker to make the runtime ready and returns the connection details you need:
- DTD URI
- App URI
- connection domain
- API base URL

If the runtime is already up, `reserve` returns the existing session details.

3. Drive the app with the returned connection details:

```sh
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive widget-tree \
  --dtd-uri <dtd-uri> \
  --app-uri <app-uri>
```

4. If fixes land and you need a fresh runtime, reboot it:

```sh
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-sim reboot --device-id <id>
```

Then reconnect using the new DTD/App URIs returned by `reboot`.

## Session checks

Use `session` when you need to inspect the current runtime state for one device:

```sh
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-sim session --device-id <id>
```

Use this to confirm:
- current state
- DTD URI
- App URI
- connection domain
- API base URL
- last error

## Common driver commands

```sh
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive apps \
  --dtd-uri <dtd-uri>

/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive widget-tree \
  --dtd-uri <dtd-uri> \
  --app-uri <app-uri>

/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive driver screenshot \
  --dtd-uri <dtd-uri> \
  --app-uri <app-uri> \
  --out /tmp/app.png

/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive driver get_health \
  --dtd-uri <dtd-uri> \
  --app-uri <app-uri>
```

Use both the DTD URI and the App URI returned by the broker. Do not assume the raw loopback app URI reported by DTD is sufficient on its own.

## Finder guidance

Prefer selectors in this order:

1. `ByTooltipMessage` for icon buttons and header actions
2. `ByText` for visible text controls
3. `Ancestor` or `Descendant` when the visible text is only a child of the tappable widget
4. `ByType` only when the tree clearly shows a unique runtime type

Prefer `fill_text` over separate tap plus type flows for text fields.

Examples:

```sh
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive driver get_text \
  --dtd-uri <dtd-uri> \
  --arg finderType=ByText \
  --arg 'text=Sales'
```

```sh
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive driver fill_text \
  --dtd-uri <dtd-uri> \
  --app-uri <app-uri> \
  --arg finderType=ByValueKey \
  --arg keyValueString=domainField \
  --arg keyValueType=String \
  --arg 'text=https://example.test'
```

```sh
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive driver tap \
  --dtd-uri <dtd-uri> \
  --app-uri <app-uri> \
  --arg finderType=ByTooltipMessage \
  --arg 'text=Close chat'
```

For nested finders, prefer `--input` with one JSON object:

```sh
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive driver tap \
  --dtd-uri <dtd-uri> \
  --app-uri <app-uri> \
  --input '{"finderType":"Ancestor","of":{"finderType":"ByText","text":"Export"},"matching":{"finderType":"ByType","type":"OutlinedButton"},"firstMatchOnly":"true","matchRoot":"false"}'
```

## Caveats

`flutter_driver` can resolve widgets that are present in the tree but not actually tappable.

If a tap times out:
- check `widget-tree`
- take a screenshot
- verify whether an overlay or offscreen state is blocking the target
