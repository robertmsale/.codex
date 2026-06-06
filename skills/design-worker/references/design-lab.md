# Design Lab Workflow

Use this reference when a Flutter project provides or needs `clients/design_lab` for screenshot-driven design proof.

## Contract

Design Lab is a visual proof surface. It must render shared design-system/source components used by the client application where applicable. It must not become a forked product implementation.

Do not add product networking, auth, Rinf/backend bridges, persistence, app singleton state, or production service calls to Design Lab. Use fixture models, no-op actions, and local render configuration.

## Capture

```sh
design-lab-capture \
  --workdir /path/to/project \
  --story salesDashboard \
  --shell none \
  --fixture reference \
  --viewport iPadLandscape \
  --out /tmp/sales-dashboard.png \
  --width 1366 \
  --height 1024
```

`design-lab-capture` owns build, ephemeral serving, screenshot capture, cleanup, and log reporting. Workers do not manage ports, tmux sessions, hot reload, or manual teardown for design proof.

Do not pass wrapper-owned or readiness-bypass options through the capture command. If readiness fails, fix the readiness signal or report a blocker.

## Requirements Evidence

Use captured screenshot paths in Requirements claims. Include viewport/device, story/shell/fixture, reference image path when applicable, scope contract, and anti-slop self-review. `flutter test` remains useful for behavior, logic, state, and widget contracts; it is not visual proof for Design Lab work.

## Design Lab Shape

Recommended layout:

```text
clients/design_lab/
  AGENTS.md
  package.json
  pubspec.yaml
  lib/
    main.dart
    design_lab_registry.dart
    design_lab_fixtures.dart
  tools/
    bun_shot.ts
```

The project should provide `npm run bun:shot -- --url <url> --out <path>` and a generic readiness signal such as `window.__designLabReady = { ready: true }` after the first stable frame.
