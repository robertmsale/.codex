---
name: designer-runtime
description: Use this when a designer needs to pilot an ad-hoc iOS simulator directly, launch Flutter from their own worktree in tmux, or trigger hot reload without the broker/device-harness path.
---

# Designer Runtime

Use this skill for designers working directly from their own worktree and simulator.

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
- `designer-flutter-run`
  - launches `flutter run -d <UDID>` in a tmux session from the designer worktree
- `designer-hot-reload`
  - sends `r` to the tmux session running Flutter
- `designer-crop-screenshot`
  - crops screenshots for pixel-level review using exact boxes, percentage+anchor crops, or named presets

## Guardrails

- This skill is for designers, not QA broker flows.
- Do not use `flutter-sim` or the managed reservation path here.
- `designer-drive` must not rely on broker reservations, thread identity, mutagen lane setup, or managed runtime metadata.
- Work from the assigned designer worktree and keep controller/state logic intact unless the task explicitly says otherwise.
- Designers should focus on widget/interface work and avoid Rust changes unless explicitly reassigned.
- Use one simulator per active design loop and wait for each command to finish before issuing the next one.
- Run commands plainly and sequentially.
- Do not combine designer runtime commands with shell operators or wrappers.
- Do not use compound commands with `&&`, `||`, `;`, pipes, command substitution, or inline env wrappers when operating this tooling.
- When reviewing a small UI region, take the screenshot first and crop it as a separate command.

## Typical Flow

1. Launch the app from the designer worktree:
   - `designer-flutter-run --session designer-app --device-id <UDID> --workdir <worktree_path>`
2. Inspect or interact with the running app:
   - `designer-drive --launch-path <worktree_path> hierarchy --device-id <UDID>`
   - `designer-drive --launch-path <worktree_path> command tapOn --device-id <UDID> --input '{"text":"Settings"}'`
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
- In practice, that means `--launch-path`, `--app-id`, `--runtime-label`, and `--json` go before `apps`, `hierarchy`, `command`, `flow`, and so on.
- Use a single command at a time. Wait for it to complete, inspect the result, then run the next command.

Commands:

```sh
designer-drive devices
designer-drive --launch-path <path> apps --device-id <UDID>
designer-drive --launch-path <path> hierarchy --device-id <UDID>
designer-drive --launch-path <path> screenshot --device-id <UDID> --out current.png
designer-drive --launch-path <path> command <name> --device-id <UDID> [--input <json>] [--label <text>] [--out <file>]
designer-drive --launch-path <path> flow --device-id <UDID> --input <json-array> [--label <text>]
```

Use `devices` to list booted simulators visible to idb. All other commands target the UDID you were given and the worktree launch path you are piloting from.

Common examples:

```sh
designer-drive devices
designer-drive --launch-path /path/to/worktree hierarchy --device-id <UDID>
designer-drive --launch-path /path/to/worktree command tapOn --device-id <UDID> --input '{"text":"Continue"}'
designer-drive --launch-path /path/to/worktree command clearAndInputText --device-id <UDID> --input 'New title'
designer-drive --launch-path /path/to/worktree command takeScreenshot --device-id <UDID> --out current.png
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
3. Use `designer-drive --launch-path <path> hierarchy --device-id <UDID>` to inspect the current screen.
4. Run one interaction command.
5. Re-run `hierarchy` or `takeScreenshot` to verify the result.
6. If you need pixel-level review, crop the screenshot to the exact region you are inspecting.
7. After code edits, run `designer-hot-reload --session <session>`.

## `designer-crop-screenshot`

Use this when you need a stable screenshot region for bottom safe-area checks, header blur comparisons, or before/after diffs.

Usage shape:

```sh
designer-crop-screenshot --input <image.png> --out <crop.png> [crop-mode-options]
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
- Use percentage + anchor mode when the same region should adapt across screen sizes.
- Presets are shortcuts and can be used for quick review loops.
- Cropping is a separate command from screenshot capture on purpose; keep the steps plain and sequential.
