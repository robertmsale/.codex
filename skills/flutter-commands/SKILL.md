---
name: flutter-commands
description: Read this before running Flutter commands. Use when working in Flutter or Dart projects that may invoke `flutter build`, `flutter test`, integration tests, macOS desktop builds, Widgetbook flows, or design-system-driven UI work. Enforces local command safety rules, bans `flutter analyze`, and documents the preferred validation/build path.
---

# Flutter Commands

Read this before running Flutter commands.

## Hard Rules

- Never run `flutter analyze`.
- Never run multiple Flutter build or test commands concurrently or in the background.
- Treat overlapping Flutter builds/tests as a tooling hazard, not as a speed optimization.

## Validation Priority

- Prefer `flutter build macos` as the default validation path when the target app supports it.
- Use build-first validation because it catches many real compile/link/runtime-surface issues that `flutter analyze` misses.
- Run one Flutter build or test command at a time and wait for it to finish before starting another.

## Integration Tests With Rust

- When Flutter integration tests depend on Rust dylibs, move the dylib to a sandbox-readable location under `/tmp`.
- Point the test/app runtime at that copied dylib with environment variables rather than loading it from the original build location.
- Use this pattern to avoid false sandbox failures that look like product bugs but are really file-access restrictions.

## UI Architecture Preference

- Prefer building reusable UI in the design system first.
- Keep the design-system layer on abstract interfaces and mock data.
- Put mocks and showcase coverage in Widgetbook.
- Keep real app controllers, bindings, and client-specific orchestration in the client app rather than the design-system package.

## Guardrails

- If a Flutter command is noisy, use the `command-parser` skill instead of dumping raw output.
- If a build directory or lockfile error appears after overlapping commands, first suspect command concurrency before changing code.
