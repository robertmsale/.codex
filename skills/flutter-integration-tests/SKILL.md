---
name: flutter-integration-tests
description: Author reliable Flutter integration tests that run with `flutter test -d flutter-tester`, especially in apps using GetX navigation and rinf-style async bridges. Use this when creating or editing `integration_test/*.dart` and avoiding deadlocks matters. [skill-hash:9c7d2f1]
---

# Flutter Integration Tests

Use this skill when authoring or editing `integration_test/*.dart` for Flutter apps that:
- run tests with `flutter test`, not `flutter drive`
- use `flutter-tester`
- use GetX navigation
- have async app/bootstrap work, bridge/runtime initialization, or long-lived background activity

## Run Pattern

Run a single integration test file with:

```bash
command-parser flutter test \
  --dart-define=TEST_BASE_URL=http://localhost:<PORT> \
  -d flutter-tester \
  integration_test/<test_file>.dart
```

Rules:
- Prefer `--dart-define=...` over prefixing the command with env vars.
- Prefer one test file at a time while authoring.
- Match the repo's actual define names if they differ from `TEST_BASE_URL`.

## Core Model

Keep three categories separate:

1. External async work
- login through repos/APIs
- bridge/runtime initialization
- connection/bootstrap configuration
- data seeding

2. UI-driving work
- `Get.toNamed(...)`
- route helpers like `goToRoute(...)`
- `tester.tap(...)`
- `tester.enterText(...)`
- `tester.drag(...)`
- `tester.pump(...)`

3. Observation
- bounded waits for route, widget, text, or controller/store state

Most deadlocks happen when category 2 is awaited like category 1.

## Hard Rules

### Do not await route pushes

Do not `await Get.toNamed(...)` or a route helper that forwards that future unless the test explicitly needs the pop result.

Bad:

```dart
await goToRoute(AppRoutes.settings);
```

Good:

```dart
goToRoute(AppRoutes.settings);
await tester.pump();
await _pumpUntil(
  tester,
  condition: () => Get.currentRoute == AppRoutes.settings,
  timeout: const Duration(seconds: 2),
  reason: 'settings route did not open',
);
```

Why:
- `Get.toNamed(...)` resolves when the pushed route is popped, not when it appears.

### Do not put UI navigation in `tester.runAsync(...)`

Use `tester.runAsync(...)` only for external async work that does not depend on pumping frames.

Good uses:
- auth/login helpers
- repo/API calls
- bridge/runtime calls
- seeding

Bad uses:
- navigation
- taps
- text entry
- scrolling
- widget waits

### Prefer bounded waits over `pumpAndSettle()`

Use explicit bounded waits for:
- current route
- widget presence
- text presence
- controller/store values

Avoid defaulting to `pumpAndSettle()` in apps with:
- streams
- timers
- shell/bootstrap activity
- bridge-driven updates

## Safe Patterns

### External async first, then UI

```dart
await tester.runAsync(() async {
  await ensureBridgeInitialized();
  await configureConnectionSettings();
  await loginAsTestUser();
});

await tester.pumpWidget(const MyApp());
await tester.pump();
```

### Safe navigation

```dart
goToRoute(detailRoute);
await tester.pump();

await _pumpUntil(
  tester,
  condition: () => Get.currentRoute == detailRoute,
  timeout: const Duration(seconds: 2),
  reason: 'detail route did not open',
);
```

### Safe UI action

```dart
await tester.tap(find.text('Save'));
await tester.pump();

await _pumpUntil(
  tester,
  condition: () => find.text('Saved').evaluate().isNotEmpty,
  timeout: const Duration(seconds: 2),
  reason: 'save confirmation did not appear',
);
```

### Safe async future that still needs pumping

If a future may only complete while the app keeps processing frames, use a helper that pumps while waiting, such as the repo's `pumpUntilDone(...)` / `pumpUntilValue(...)` style helpers.

## GetX Navigation Guidance

If the repo has helpers like `goToRoute(...)`, inspect what they return.

If they forward `Get.toNamed(...)`, treat them as push-result futures:
- call them
- do not await them for arrival
- pump once
- wait on route/UI state instead

Only await them when asserting the pop result.

## rinf / Bridge Guidance

For apps with rinf-style runtime setup:
- initialize the runtime before pumping the app if the app depends on it
- configure connection settings before login/bootstrap
- use the repo's existing harness helpers instead of inventing new bootstrap code
- if the repo already has helpers that pump while bridge work completes, prefer those

If a future depends on bridge messages or UI-driven listeners, a naked `await` may deadlock. Use a helper that keeps pumping the test loop.

If signals change, `rinf gen` needs to run in order to generate the bindings. These must be committed to source control if they change.

Changes to rust code require `cargo build` inside the `<flutter project root>/native/hub` before running tests so the 

## Setup / Teardown

Reset state aggressively between tests.

Typical expectations:
- clear auth/session state in `setUp`
- `Get.reset()` in `setUp` or `tearDown` as the repo pattern requires
- dispose app scope / controllers / subscriptions created by the test
- finalize or reset bridge/runtime state if the repo requires it

Attach cleanup close to allocation with `addTearDown(...)` when the test creates one-off controllers or scopes.

## Common Deadlock Smells

If a test hangs, check these first:

1. `await goToRoute(...)` or `await Get.toNamed(...)`
2. navigation inside `tester.runAsync(...)`
3. naked `await` on a future that needs pumping to complete
4. `pumpAndSettle()` on a screen with ongoing timers/streams
5. leaked GetX or bridge state from a previous test

## Authoring Checklist

- Use `flutter test -d flutter-tester`, not `flutter drive`.
- Use `--dart-define=...` instead of prefixing env vars.
- Keep external async inside `tester.runAsync(...)`.
- Keep navigation and widget interaction outside `tester.runAsync(...)`.
- Do not await route-push futures unless asserting the pop result.
- Pump once after each major UI trigger.
- Use bounded waits for route/widget/text/state.
- Reset auth/GetX/bridge state according to repo harness rules.
- Prefer existing harness helpers over ad hoc waits.

## Minimal Review Heuristic

Before finishing a new integration test, scan for:

- `await goToRoute(`
- `await Get.toNamed(`
- `tester.runAsync(() async { ... navigation ... })`
- unbounded `pumpAndSettle()` in shell/bootstrap-heavy flows

If any of those are present, the test likely needs restructuring.
