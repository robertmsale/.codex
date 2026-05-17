---
name: designer-runtime
description: Use this when a designer needs to pilot an ad-hoc iOS simulator directly, launch Flutter from their own worktree in tmux, or trigger hot reload without the broker/device-harness path.
---

# Designer Runtime

Use this skill for designers and QA agents working directly from an assigned
worktree and simulator.

Use only these scripts:

```sh
designer-drive ...
designer-crop-screenshot ...
designer-flutter-run ...
designer-hot-reload ...
```

## What They Are For

- `designer-drive`
  - direct ad-hoc simulator inspection and interaction through plain `idb` commands plus small Python HID helpers for text editing cases
  - screenshot capture stays on the same `designer-drive screenshot` surface, falls back to simulator-native capture if `idb` bitmap capture is unavailable, normalizes the saved image to the live UI orientation, and can crop directly to a live accessibility selector
  - when the app exports `automation.orientationBeacon`, tap, swipe, and screenshot orientation use that accessibility sentinel instead of probe-based orientation guessing
- `designer-flutter-run`
  - launches `flutter run -d <UDID>` in a tmux session from the designer worktree
  - mirrors Flutter output to a log file so immediate tmux exits are diagnosable
- `designer-hot-reload`
  - sends `r` to the tmux session running Flutter
- `designer-crop-screenshot`
  - crops screenshots for pixel-level review using exact boxes, percentage+anchor crops, named presets, or live hierarchy selectors

## Guardrails

- This skill is for direct designer/QA piloting, not QA broker flows.
- Do not use `flutter-sim` or the managed reservation path here.
- `designer-drive` must not rely on broker reservations, thread identity, mutagen lane setup, or managed runtime metadata.
- Work from the assigned designer worktree and keep controller/state logic intact unless the task explicitly says otherwise.
- Designers should focus on widget/interface work and avoid Rust changes unless explicitly reassigned.
- Use one simulator per active design loop and wait for each command to finish before issuing the next one.
- Run commands plainly and sequentially.
- Do not combine designer runtime commands with shell operators or wrappers.
- Do not use compound commands with `&&`, `||`, `;`, pipes, command substitution, or inline env wrappers when operating this tooling.
- When reviewing a small UI region, prefer `designer-drive screenshot --selector ...` if a live accessibility frame is enough.
- Use `designer-crop-screenshot` when you need manual boxes, presets, percentages, or to crop an existing saved image.

## Typical Flow

1. Launch the app from the designer worktree:
   - `designer-flutter-run --session designer-app --device-id <UDID> --workdir <worktree_path>`
   - If the tmux session exits immediately, inspect the printed log file path.
2. Inspect or interact with the running app:
   - `designer-drive hierarchy --device-id <UDID>`
   - `designer-drive command tapOn --device-id <UDID> --input '{"text":"Settings"}'`
   - `designer-drive screenshot --device-id <UDID> --selector '{"id":"node-style-minimal-flat"}' --out minimal-flat.png`
   - `designer-crop-screenshot --input current.png --preset bottom_safe_area --out current-bottomsafe.png`
3. After code changes, hot reload:
   - `designer-hot-reload --session designer-app`

## `designer-drive`

Usage shape:

```sh
designer-drive [global-options] <subcommand> [subcommand-options]
```

Important:

- Global flags must come before the subcommand.
- In practice, that means `--app-id` and `--json` go before `apps`, `hierarchy`, `command`, `flow`, and so on.
- Use a single command at a time. Wait for it to complete, inspect the result, then run the next command.
- For screenshot selector cropping, pass selector JSON such as `--selector '{"id":"node-style-minimal-flat"}'`.
- Fresh screenshots from `designer-drive screenshot` are normalized to the live UI orientation before they are saved or cropped.
- Flutter apps can expose a hidden accessibility sentinel with id `automation.orientationBeacon` and value `<orientation>|<width>|<height>`, for example `landscapeLeft|1366|1024`.
- Supported sentinel orientations are `portraitUp`, `portraitDown`, `landscapeLeft`, and `landscapeRight`.

Commands:

```sh
designer-drive devices
designer-drive apps --device-id <UDID>
designer-drive hierarchy --device-id <UDID>
designer-drive screenshot --device-id <UDID> --out current.png
designer-drive screenshot --device-id <UDID> --selector '{"id":"node-style-minimal-flat"}' --out minimal-flat.png
designer-drive command <name> --device-id <UDID> [--input <json>] [--label <text>] [--out <file>]
designer-drive flow --device-id <UDID> --input <json-array> [--label <text>]
```

Use `devices` to list booted simulators visible to idb. All other commands target the UDID you were given. `designer-drive` uses the current working directory internally; designers should not need to pass a worktree path just to pilot a device.

Common examples:

```sh
designer-drive devices
designer-drive hierarchy --device-id <UDID>
designer-drive command tapOn --device-id <UDID> --input '{"text":"Continue"}'
designer-drive screenshot --device-id <UDID> --selector '{"text":"Close"}' --out close-button.png
designer-drive command clearAndInputText --device-id <UDID> --input 'New title'
designer-drive command takeScreenshot --device-id <UDID> --out current.png
designer-hot-reload --session designer-app
```

Useful command names:

- `tapOn`
- `longPressOn`
- `inputText`
- `clearAndInputText`
- `eraseText`
- `forwardEraseText`
- `hideKeyboard`
- `swipe`
- `takeScreenshot`

Recommended operating pattern:

1. Start the app with `designer-flutter-run`.
2. Run `designer-drive devices` and confirm the target UDID.
3. Use `designer-drive hierarchy --device-id <UDID>` to inspect the current screen.
4. Run one interaction command.
5. Re-run `hierarchy` or `takeScreenshot` to verify the result.
6. If you want a one-step cropped screenshot, use `designer-drive screenshot --selector ...`.
7. If you need a manual or preset crop flow, use `designer-crop-screenshot`.
8. After code edits, run `designer-hot-reload --session <session>`.

## `designer-flutter-run`

Usage shape:

```sh
designer-flutter-run --session <tmux-session> --device-id <UDID> [--workdir <path>] [--target <dart-file>] [--flavor <flavor>] [--log <path>] [-- <flutter args>...]
```

Examples:

```sh
designer-flutter-run --session designer-app --device-id <UDID> --workdir <worktree_path>
designer-flutter-run --session designer-app --device-id <UDID> --workdir <worktree_path> --log /tmp/designer-app.log
designer-flutter-run --session designer-app --device-id <UDID> --workdir <worktree_path> -- --dart-define=FOO=bar
```

Notes:

- `--help` prints the supported flags.
- The default log is `/tmp/designer-flutter-run/<session>.log`.
- The script prints the log path after launch.
- `designer-flutter-run` is the sanctioned path for Flutter debug launching; raw `flutter run` remains blocked by the Flutter shim.
- If `tmux list-sessions` no longer shows the session, inspect the log before retrying.
- Do not replace this with raw `flutter run` unless the operator explicitly asks for a one-off diagnostic path.

## `designer-crop-screenshot`

Use this when you need a stable screenshot region for bottom safe-area checks, header blur comparisons, or before/after diffs.
If a single live selector crop is enough, prefer `designer-drive screenshot --selector ...` first.
Selector mode uses selector JSON, for example `--selector '{"text":"Close"}'`.

Usage shape:

```sh
designer-crop-screenshot --input <image.png> --out <crop.png> [crop-mode-options]
```

Selector mode from live hierarchy:

```sh
designer-crop-screenshot --input shot.png --device-id <UDID> --selector '{"id":"node-style-minimal-flat"}' --out minimal-flat.png
designer-crop-screenshot --input shot.png --device-id <UDID> --selector '{"text":"Close"}' --out close-button.png
```

Exact pixel box:

```sh
designer-crop-screenshot --input shot.png --x 0 --y 2400 --width 2048 --height 332 --out bottom-strip.png
```

Percentage + anchor:

```sh
designer-crop-screenshot --input shot.png --anchor bottom_center --width-pct 100 --height-pct 12 --out bottom-safe.png
designer-crop-screenshot --input shot.png --anchor top_right --width-pct 25 --height-pct 20 --offset-x -24 --out top-right.png
```

Preset mode:

```sh
designer-crop-screenshot --input shot.png --preset header --out header.png
designer-crop-screenshot --input shot.png --preset bottom_right --out bottom-right.png
designer-crop-screenshot --input shot.png --preset bottom_safe_area --out bottom-safe.png
```

Supported anchors:

- `top_left`
- `top_center`
- `top_right`
- `center_left`
- `center`
- `center_right`
- `bottom_left`
- `bottom_center`
- `bottom_right`

Supported presets:

- `bottom_right`
- `header`
- `center`
- `top_right`
- `bottom_safe_area`

Notes:

- Use exact pixel mode when you need repeatable box-for-box comparisons.
- Use selector mode when you want the crop region to come directly from the live accessibility frame in `designer-drive hierarchy`.
- Selector-mode crops automatically scale hierarchy coordinates into screenshot pixel coordinates.
- Use percentage + anchor mode when the same region should adapt across screen sizes.
- Presets are shortcuts and can be used for quick review loops.
- Prefer one-step selector crops from `designer-drive screenshot --selector ...` when that covers the review.
- Use `designer-crop-screenshot` when you need a second explicit crop step.
