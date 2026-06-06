# Optional Robdex GUI Packaging

The Flutter GUI is optional. It is a convenience layer over the core Robdex
bridge and is not required for CLI communication, Requirements, or orchestration.

## Prerequisites

Common:

- Flutter SDK on `PATH`
- Rust bridge built and reachable
- `ROBDEX_BRIDGE_BASE_URL`, defaulting to `http://127.0.0.1:42080`

macOS:

- Xcode command line tools
- Flutter macOS desktop support enabled

Linux:

- Flutter Linux desktop support enabled
- GTK and standard Flutter Linux build dependencies for the host distribution
- Rust via `rustup` for the native rinf hub build

## Build

```sh
cd "$ROBDEX_HOME/frontend/robdex_app"
flutter pub get
flutter build macos   # macOS
flutter build linux   # Linux, on a Linux host
flutter build web     # optional web smoke build
```

For isolated Linux build validation from macOS, a disposable Docker container is
acceptable as long as the repository is mounted read-only and copied to the
container filesystem before building. The container needs Flutter, CMake,
Ninja, GTK development headers, Clang/LLD, and Rust via rustup.

## Run

Start the core stack first:

```sh
robdex doctor
ROBDEX_SERVICE_MANAGER=supervisor robdex start
```

Then launch the GUI from Flutter or the platform build artifact. Connect to the
bridge host shown on the login screen. For local development the default is
`127.0.0.1:42080`.

## Limitations

- GUI packaging is not part of the headless bootstrap requirement.
- Linux packaging is validated by `flutter build linux`; drag-and-drop images
  and desktop integrations still need a focused runtime smoke test on a real
  Linux desktop session.
- Integrated terminal support is macOS-focused unless separately validated.
- If an older GUI build does not understand a newer schema, update the GUI
  rather than adding backend normalization for local-only deployments.

## Validation

For shared GUI changes, run focused widget tests and `flutter analyze`. For
packaging changes, build the affected platform target. A macOS host can validate
macOS and web builds; Linux packaging must be validated on Linux or in an
isolated Linux container with the required desktop build packages installed.
