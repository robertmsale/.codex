---
name: flutter-driver-cli
description: Use this skill when you need to drive a native Flutter app from the terminal via flutter_driver on macOS or iOS, including launching the app with flutter run in tmux, extracting the DTD URI from machine output, resolving the live app from DTD on each command, inspecting the widget tree, taking screenshots, and sending plaintext-first driver commands.
---

# Flutter Driver CLI

Use the local wrapper script:

```sh
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive ...
```

This skill is for native Flutter app driving only:
- macOS
- iOS

It is not for web automation, generic MCP server work, or broad Flutter CLI usage.

## Hard prohibitions

- Do not use `flutter devices` to list or discover devices as part of this skill.
- Do not use `xcrun` commands for simulator or device management.
- Do not restart, reboot, or otherwise reset the iOS Simulator.
- If a usable iOS device ID is not already known from context, stop and ask the user instead of improvising discovery or simulator control commands.

## Preferred workflow

Prefer the host-backed simulator broker:

```sh
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-sim devices
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-sim reserve --target lib/flutter_driver_pilot_main.dart
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-sim session
```

`reserve` and `restart` return the current DTD URI, so agents can hand it straight to `flutter-drive` without scraping logs.

Example:

```sh
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-sim reserve --target lib/flutter_driver_pilot_main.dart
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive widget-tree --dtd-uri ws://127.0.0.1:12344/efgh=
```

Use the legacy tmux flow only when the broker is unavailable.

## Legacy workflow

1. Launch the target app in a dedicated `tmux` session with `flutter run --machine --print-dtd`.
2. Wait for the build to settle.
3. Extract `app.debugPort` and `app.dtd` from the tmux pane using `rg`.
4. Use the DTD URI directly with `apps`, `widget-tree`, `screenshot`, and targeted `driver` commands.

Prefer this over scrolling terminal history manually.

## Launch in tmux

Example for macOS:

```sh
tmux new-session -d -s my-flutter-driver \
  'cd /absolute/path/to/app && flutter run --machine --print-dtd -d macos --target lib/flutter_driver_pilot_main.dart'
```

Example for iOS simulator or device:

```sh
tmux new-session -d -s my-flutter-driver \
  'cd /absolute/path/to/app && flutter run --machine --print-dtd -d <device-id> --target lib/flutter_driver_pilot_main.dart'
```

Then wait before scraping:

```sh
sleep 20
tmux capture-pane -pt my-flutter-driver:1 -S -200 | rg 'app.debugPort|app.dtd'
```

Use a longer wait on cold builds.

## Resolve and inspect

Use the DTD URI directly on each command. The CLI resolves the current app from DTD on demand.

Useful first commands:

```sh
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive apps --dtd-uri ws://127.0.0.1:12344/efgh=
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive widget-tree --dtd-uri ws://127.0.0.1:12344/efgh=
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive driver screenshot --dtd-uri ws://127.0.0.1:12344/efgh= --out /tmp/app.png
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive driver get_health --dtd-uri ws://127.0.0.1:12344/efgh=
```

## Finder guidance

Prefer selectors in this order:

1. `ByTooltipMessage` for icon buttons and header actions
2. `ByText` for visible text controls
3. `Ancestor` / `Descendant` when the visible text is only a child of the actual tappable widget
4. `ByType` only when the tree clearly shows a unique runtime type

Examples:

```sh
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive driver get_text \
  --dtd-uri ws://127.0.0.1:12344/efgh= \
  --arg finderType=ByText \
  --arg 'text=Sales'
```

```sh
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive driver tap \
  --dtd-uri ws://127.0.0.1:12344/efgh= \
  --arg finderType=ByTooltipMessage \
  --arg 'text=Close chat'
```

For nested finders, prefer `--input` with one JSON object:

```sh
/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive driver tap \
  --dtd-uri ws://127.0.0.1:12344/efgh= \
  --input '{"finderType":"Ancestor","of":{"finderType":"ByText","text":"Export"},"matching":{"finderType":"ByType","type":"OutlinedButton"},"firstMatchOnly":"true","matchRoot":"false"}'
```

## Important caveats

`flutter_driver` can resolve widgets that exist in the tree but are not actually tappable in the current foreground state.

This happens when:
- an overlay is above the target
- the target is outside the visible viewport
- the finder matched a non-interactive child instead of the owning control

Operational meaning:
- `get_text` succeeding does not prove `tap` will succeed
- a timed out `tap` or `waitForTappable` often means the widget is occluded, offscreen, or not the interactive node
- after closing overlays, previously timing-out taps may start succeeding immediately

Always cross-check with:
- `widget-tree`
- `driver screenshot`
- `driver get_offset`

## Output behavior

The CLI is plaintext-first by default.

Expect:
- `get_text` prints just the text
- `get_offset` prints `x,y`
- `screenshot` prints the output path
- some successful commands may print a compact JSON blob if the driver response has no obvious plaintext reduction

`driver get_offset` defaults `offsetType` to `center`, so callers usually only need to pass the finder payload.

Use `--json` only when raw structured output is actually needed.

## Preconditions

The target app must enable the Flutter driver extension in a dedicated entrypoint, typically something like:

```dart
import 'package:flutter_driver/driver_extension.dart';

import 'main.dart' as app;

Future<void> main() async {
  enableFlutterDriverExtension();
  await app.main();
}
```

If the extension is not enabled, driver commands will fail.

## Wrapper script

The wrapper script lives at:

[`/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive`](/Users/robertsale/.codex/skills/flutter-driver-cli/scripts/flutter-drive)

It executes the Dart entrypoint in:

[`/Users/robertsale/Code/flutter-driver-cli/bin/flutter_driver_cli.dart`](/Users/robertsale/Code/flutter-driver-cli/bin/flutter_driver_cli.dart)

If the repo moves, update the script.
