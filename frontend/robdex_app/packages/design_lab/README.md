# Robdex Design Lab

Mocked Robdex UI surface for screenshot-driven design work.

Use the global sanctioned capture command:

```sh
design-lab-capture \
  --workdir /Users/robertsale/.codex \
  --lab-dir frontend/robdex_app/packages/design_lab \
  --out /tmp/robdex-design-lab.png
```

`design-lab-capture` builds the Flutter Web artifact, serves `build/web`
ephemerally, captures a Bun/WebView screenshot through `npm run bun:shot`, and
tears down the server automatically.

This package renders `mockWorkbenchData` through `robdex_design_system`, so
design changes can be reviewed without the bridge, native hub, app-server, or
live agent state.
