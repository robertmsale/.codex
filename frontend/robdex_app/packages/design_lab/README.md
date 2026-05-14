# Robdex Design Lab

Mocked Robdex UI surface for screenshot-driven design work.

Run it directly with:

```sh
flutter run -d web-server --web-hostname 127.0.0.1 --web-port 43110
```

Or use the helper scripts:

```sh
scripts/design-lab-run
scripts/design-lab-hot-reload
scripts/design-lab-screenshot http://127.0.0.1:43110 /tmp/robdex-design-lab.png
```

For reliable headless screenshots, serve the release build:

```sh
scripts/design-lab-build-serve
scripts/design-lab-screenshot http://127.0.0.1:43111 /tmp/robdex-design-lab.png
```

`flutter run -d web-server` is useful for hot reload, but Flutter's debug web-server output can render blank in headless Chrome without the Dart debug extension. Use the release helper for automated screenshot review.

This package renders `mockWorkbenchData` through `robdex_design_system`, so design changes can be reviewed without the bridge, native hub, app-server, or live agent state.
