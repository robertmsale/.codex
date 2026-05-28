# Design Lab Workflow

Use this reference when a Flutter project provides, or needs to create,
`clients/design_lab` for screenshot-driven design work.

## Contract

Design Lab is the merge-grade visual confirmation path for design-review work.
The sanctioned proof path builds the Flutter Web artifact, serves that build
through an ephemeral local static server, captures pixels through the project's
Bun/WebView screenshot command, and tears the server down automatically.

Design Lab is for visual confirmation only. It must not include product
networking, auth, Rinf/backend bridges, persistence, app singleton state, or
production service calls. It should consume shared design-system widgets,
fixture models, no-op actions, and local render configuration.

`flutter test` is useful for behavior, logic, state, and widget contract
assertions. It is not acceptable visual proof for Design Lab work because it
does not render through the same real app/WebView surface and encourages
accommodations to a rendering target the app will never ship.

## Required Project Shape

Recommended layout:

```text
clients/design_lab/
  AGENTS.md
  README.md
  package.json
  pubspec.yaml
  lib/
    main.dart
    config.example.dart
    design_lab_registry.dart
    design_lab_fixtures.dart
  tools/
    bun_shot.ts
```

The project should provide:

- `npm run bun:shot -- --url <url> --out <path>` for screenshots.
- A fullscreen Flutter Web app with no catalog/sidebar chrome unless that chrome
  is the product surface under review.
- Story/session selection through `--dart-define` values such as story, shell,
  fixture, viewport, theme, or inspector mode.
- A generic readiness signal for screenshot tooling, preferably
  `window.__designLabReady = { ready: true, ... }` after the first stable frame.
  Do not use project-branded global names for reusable Design Lab readiness
  contracts.
- Fixture states for ready, empty/unavailable, overflow/stress, and important
  interaction modes.

## Package Boundaries

Design Lab should import shared UI packages, not the production app client.

Good:

- shared design-system widgets
- route/page body renderers extracted into shared UI packages
- deterministic fixtures
- local no-op callbacks and intent sinks

Bad:

- imports from `clients/app`
- product auth/session setup
- Rinf/backend bridges
- API clients, databases, caches, or service locators
- production networking hidden behind fixture setup

If a page only exists inside app code, first extract the page body/renderer
contract into a shared design-system or UI package, then wire a Design Lab story.
Do not fall back to Flutter tester screenshots for visual design proof.

## Operating Commands

Capture a merge-grade screenshot:

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

`design-lab-capture` runs `flutter build web --release --no-wasm-dry-run`,
starts a private localhost server for `build/web`, runs `npm run bun:shot`, and
cleans up before returning. Workers do not manage ports, tmux sessions, hot
reload, or manual teardown for merge-grade proof.

`design-lab-capture` owns `--port`, `--out`, and screenshot backend selection
when invoking the project screenshot script. Workers must not pass those through
after `--`, and must not pass readiness bypasses such as `--skipReady`; if the
capture cannot reach readiness, fix the Design Lab readiness signal or report a
tooling blocker.

Alternate `--url` values are allowed after `--` for sanctioned web-client
capture when the target page exposes the same generic readiness signal. This is
for capturing a real web client route with Design Lab screenshot tooling, not
for bypassing readiness or switching browser backends.

Then run design review against the reference and the Design Lab screenshot:

```sh
design-review /tmp/reference.png /tmp/sales-dashboard.png \
  "Scope: page content surface from Design Lab Bun/WebView capture. Grade visual fidelity, surface treatment, spacing, hierarchy, chart/data fidelity, copy/data quality, and production polish."
```

## Project AGENTS.md Rules

Each Design Lab should include local instructions equivalent to:

```markdown
# AGENTS: Project Design Lab

- Design Lab is for visual confirmation only.
- Do not add product networking, auth, backend bridges, persistence, app
  singleton state, or production service calls.
- Use `design-lab-capture` for merge-grade screenshots.
- Do not use Flutter tester screenshots for Design Lab visual proof.
- If a page needs app code to render, extract the page body/renderer into a
  shared UI package first, then wire a Design Lab story.
```

## Review Standard

Design review must compare the reference image against the Design Lab
Bun/WebView screenshot unless the task explicitly says it is not a Design Lab
task and accepts widget-rendered artifacts. A visually degraded implementation
must fail even when the same sections are present.
