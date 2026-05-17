# Robdex VS Code/VSCodium Extension

This extension runs Robdex directly inside a VSCodium webview. It does not
reimplement the Robdex GUI in TypeScript and it does not iframe the bridge app.
Instead, it constructs a webview document that loads the Flutter web runtime and
assets directly from the configured local Robdex bridge.

## Prerequisites

- VSCodium at `/Applications/VSCodium.app`
- Node.js and npm
- The Robdex bridge serving runtime APIs and WebSockets, normally at
  `http://localhost:42080`

Start or inspect the core stack from `/Users/robertsale/.codex`:

```sh
robdex status
robdex start --foreground
```

## Develop

```sh
cd /Users/robertsale/.codex/extensions/robdex-vscode
npm install
npm run compile
/Applications/VSCodium.app/Contents/Resources/app/bin/codium \
  --extensionDevelopmentPath=/Users/robertsale/.codex/extensions/robdex-vscode
```

In the launched Extension Development Host, run `Robdex: Open` from the command
palette.

## Configuration

`robdex.bridgeBaseUrl` defaults to:

```json
"http://localhost:42080"
```

The extension probes `${robdex.bridgeBaseUrl}/healthz`. If the bridge is
healthy, it sets the webview `<base>` to the bridge URL, loads `flutter.js` from
that origin, and lets the Flutter/Rinf runtime resolve normal browser-relative
assets such as `main.dart.js`, `pkg/hub.js`, and `pkg/hub_bg.wasm`. If the
bridge is unavailable, it renders a lightweight fallback with the checked URL
and start/status guidance.

Markdown links in Robdex chat can ask the extension to open mentioned files in
the current VSCodium workspace. Absolute paths and workspace-relative paths with
optional `:line:column` suffixes are supported.

## Manual Smoke Test

The owner should perform this UI smoke test in VSCodium:

1. Ensure `curl -fsS http://localhost:42080/healthz` succeeds.
2. Run `npm run compile` in this extension directory.
3. Launch VSCodium with `--extensionDevelopmentPath` as shown above.
4. Run `Robdex: Open`.
5. Confirm the bridge-served Robdex Flutter UI appears in the Robdex activity view.
6. Change `robdex.bridgeBaseUrl` to an unused local port, run
   `Robdex: Refresh`, and confirm the fallback is clear and actionable.
7. Restore `robdex.bridgeBaseUrl` and run `Robdex: Refresh`.

Automated validation covers TypeScript compilation and static extension
surface. Live VSCodium webview rendering is intentionally deferred to the owner
for this first slice.
