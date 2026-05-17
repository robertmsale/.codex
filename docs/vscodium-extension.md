# Robdex VSCodium Extension

The Robdex VSCodium extension bundles the Flutter web app and runs it directly
inside a VSCodium webview. The extension does not duplicate Robdex GUI logic in
JavaScript; TypeScript provides the editor shell and VS Code API bridge while
Flutter remains the UI.

## First Slice

- Extension path: `extensions/robdex-vscode`
- Default bridge/API URL: `http://localhost:42080`
- Commands:
  - `Robdex: Open`
  - `Robdex: Refresh`
- Configuration:
  - `robdex.bridgeBaseUrl`

## Development

```sh
cd /Users/robertsale/.codex/extensions/robdex-vscode
npm install
npm run compile
(cd /Users/robertsale/.codex/frontend/robdex_app && flutter build web --release --no-web-resources-cdn)
mkdir -p media/robdex-web
cp -R /Users/robertsale/.codex/frontend/robdex_app/build/web/. media/robdex-web/
/Applications/VSCodium.app/Contents/Resources/app/bin/codium \
  --extensionDevelopmentPath=/Users/robertsale/.codex/extensions/robdex-vscode
```

In the launched Extension Development Host, run `Robdex: Open`.

## Bridge Behavior

The extension checks:

```text
${robdex.bridgeBaseUrl}/healthz
```

If the bridge is healthy, the webview loads the bundled Flutter app from
`extensions/robdex-vscode/media/robdex-web` and passes
`${robdex.bridgeBaseUrl}` to the app for API and WebSocket traffic.

If health is unreachable, the webview renders a small fallback with the checked
URL and guidance to run:

```sh
robdex status
robdex start --foreground
```

## Manual Smoke

The live VSCodium webview smoke test is owner-run for this slice:

1. Verify `curl -fsS http://localhost:42080/healthz`.
2. Compile the extension with `npm run compile`.
3. Launch `/Applications/VSCodium.app` with `--extensionDevelopmentPath`.
4. Run `Robdex: Open`.
5. Confirm the bundled Flutter UI loads in the Robdex activity view.
6. Set `robdex.bridgeBaseUrl` to an unused port and run `Robdex: Refresh`.
7. Confirm the fallback shows the checked URL and start/status guidance.

## Editor Bridge

The bundled Flutter app can communicate with the extension host through VS Code
webview `postMessage`. The first supported affordance is opening mentioned file
links from chat markdown. Absolute paths and workspace-relative links with
optional `:line:column` suffixes are opened through VSCodium.
