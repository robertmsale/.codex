import * as http from 'http';
import * as https from 'https';
import * as vscode from 'vscode';

const DEFAULT_BRIDGE_BASE_URL = 'http://localhost:42080';
const ROBDEX_VIEW_ID = 'robdex.view';

let provider: RobdexViewProvider | undefined;

export function activate(context: vscode.ExtensionContext): void {
  const output = vscode.window.createOutputChannel('Robdex');
  provider = new RobdexViewProvider(output);
  context.subscriptions.push(
    output,
    vscode.window.registerWebviewViewProvider(ROBDEX_VIEW_ID, provider, {
      webviewOptions: {
        retainContextWhenHidden: true,
      },
    }),
    vscode.commands.registerCommand('robdex.open', async () => {
      await vscode.commands.executeCommand(`${ROBDEX_VIEW_ID}.focus`);
    }),
    vscode.commands.registerCommand('robdex.refresh', async () => {
      await provider?.refresh();
    }),
  );
}

export function deactivate(): void {
  provider = undefined;
}

class RobdexViewProvider implements vscode.WebviewViewProvider {
  private view: vscode.WebviewView | undefined;

  constructor(private readonly output: vscode.OutputChannel) {}

  async resolveWebviewView(webviewView: vscode.WebviewView): Promise<void> {
    this.view = webviewView;
    webviewView.webview.options = {
      enableScripts: true,
    };
    webviewView.webview.onDidReceiveMessage((message) => {
      void this.handleMessage(message);
    });
    await this.render();
  }

  async refresh(): Promise<void> {
    if (this.view === undefined) {
      await vscode.commands.executeCommand(`${ROBDEX_VIEW_ID}.focus`);
      return;
    }
    await this.render();
  }

  private async render(): Promise<void> {
    const view = this.view;
    if (view === undefined) {
      return;
    }

    const bridgeBaseUrl = getBridgeBaseUrl();
    const healthUrl = new URL('/healthz', bridgeBaseUrl).toString();
    this.log(`render bridgeBaseUrl=${bridgeBaseUrl}`);
    const health = await checkBridgeHealth(healthUrl);
    this.log(health.ok ? `health ok ${healthUrl}` : `health failed ${healthUrl}: ${health.message}`);

    if (health.ok) {
      view.webview.html = renderBridgeServedRobdexApp(view.webview, bridgeBaseUrl, healthUrl);
    } else {
      view.webview.html = renderBridgeFallback(view.webview, bridgeBaseUrl, healthUrl, health.message);
    }
  }

  private async handleMessage(message: unknown): Promise<void> {
    if (isRecord(message) && message.type === 'robdex.diagnostic') {
      const level = typeof message.level === 'string' ? message.level : 'info';
      const text = typeof message.message === 'string' ? message.message : JSON.stringify(message);
      this.log(`[webview:${level}] ${text}`);
      return;
    }
    if (!isRecord(message) || message.type !== 'robdex.openMentionedFile') {
      return;
    }
    const target = typeof message.target === 'string' ? message.target : '';
    this.log(`[webview:info] openMentionedFile target=${target}`);
    await openMentionedFile(target);
  }

  private log(message: string): void {
    this.output.appendLine(`[${new Date().toISOString()}] ${message}`);
  }
}

function getBridgeBaseUrl(): string {
  const configured = vscode.workspace
    .getConfiguration('robdex')
    .get<string>('bridgeBaseUrl', DEFAULT_BRIDGE_BASE_URL)
    .trim();
  return configured.length > 0 ? trimTrailingSlash(configured) : DEFAULT_BRIDGE_BASE_URL;
}

async function checkBridgeHealth(healthUrl: string): Promise<{ ok: true } | { ok: false; message: string }> {
  return new Promise((resolve) => {
    let parsed: URL;
    try {
      parsed = new URL(healthUrl);
    } catch (error) {
      resolve({ ok: false, message: `Invalid bridge health URL: ${String(error)}` });
      return;
    }

    const client = parsed.protocol === 'https:' ? https : http;
    const request = client.get(
      parsed,
      {
        timeout: 1500,
        headers: {
          Accept: 'application/json',
        },
      },
      (response) => {
        response.resume();
        if (response.statusCode !== undefined && response.statusCode >= 200 && response.statusCode < 300) {
          resolve({ ok: true });
        } else {
          resolve({ ok: false, message: `Health check returned HTTP ${response.statusCode ?? 'unknown'}.` });
        }
      },
    );

    request.on('timeout', () => {
      request.destroy(new Error('Health check timed out.'));
    });
    request.on('error', (error) => {
      resolve({ ok: false, message: error.message });
    });
  });
}

function renderBridgeServedRobdexApp(webview: vscode.Webview, bridgeBaseUrl: string, healthUrl: string): string {
  const cacheBuster = Date.now().toString();
  const bridgeOrigin = new URL(bridgeBaseUrl).origin;
  const bridgeSocketOrigin = bridgeOrigin.replace(/^http/, 'ws');
  const bridgeBase = `${trimTrailingSlash(bridgeBaseUrl)}/`;
  const flutterJsUrl = `${bridgeBase}flutter.js?v=${cacheBuster}`;
  const csp = [
    "default-src 'none'",
    `base-uri ${webview.cspSource}`,
    `script-src ${bridgeOrigin} 'unsafe-inline' 'unsafe-eval' 'wasm-unsafe-eval' blob:`,
    `worker-src ${bridgeOrigin} blob:`,
    `connect-src ${webview.cspSource} ${bridgeOrigin} ${bridgeSocketOrigin} https://fonts.gstatic.com blob:`,
    `img-src ${webview.cspSource} ${bridgeOrigin} data: blob:`,
    `font-src ${bridgeOrigin} https://fonts.gstatic.com data:`,
    `media-src ${webview.cspSource} ${bridgeOrigin} data: blob:`,
    `manifest-src ${bridgeOrigin}`,
    `style-src ${bridgeOrigin} 'unsafe-inline'`,
  ].join('; ');
  return htmlDocument(
    csp,
    `
      <base href="./">
      <main id="robdex-loading" class="loading">
        <strong>Loading Robdex</strong>
        <span data-detail>Bridge: ${escapeHtml(bridgeBaseUrl)}</span>
      </main>
      <script>
        const vscodeApi = typeof acquireVsCodeApi === 'function' ? acquireVsCodeApi() : undefined;
        const postDiagnostic = (level, message) => {
          try {
            vscodeApi?.postMessage({ type: 'robdex.diagnostic', level, message: String(message || '') });
          } catch (_) {
            // Diagnostics must never block Flutter startup.
          }
        };
        const loadingNode = () => document.getElementById('robdex-loading');
        const setLoadingDetail = (message) => {
          postDiagnostic('info', message);
          const node = loadingNode();
          if (!node) {
            return;
          }
          const detail = node.querySelector('[data-detail]');
          if (detail) {
            detail.textContent = String(message || '');
          }
        };
        const setLoadingError = (message) => {
          postDiagnostic('error', message);
          const node = loadingNode();
          if (!node) {
            return;
          }
          node.className = 'loading error';
          node.innerHTML = '<strong>Robdex failed to load</strong><span data-detail>' + String(message || 'Unknown webview error') + '</span>';
        };
        const originalConsoleError = console.error.bind(console);
        console.error = (...args) => {
          originalConsoleError(...args);
          setLoadingError(args.map((arg) => String(arg)).join(' '));
        };
        const originalConsoleWarn = console.warn.bind(console);
        console.warn = (...args) => {
          originalConsoleWarn(...args);
          setLoadingDetail(args.map((arg) => String(arg)).join(' '));
        };
        let appStartTimeout;
        const removeLoadingOverlay = () => {
          if (appStartTimeout) {
            window.clearTimeout(appStartTimeout);
          }
          const node = loadingNode();
          if (node) {
            node.remove();
          }
        };
        const originalElementAppend = Element.prototype.append;
        const rewriteRinfHubModuleScript = (node) => {
          if (!(node instanceof HTMLScriptElement) || node.type !== 'module') {
            return;
          }
          const text = node.innerHTML || node.textContent || '';
          if (!text.includes('pkg/hub.js')) {
            return;
          }
          postDiagnostic('info', 'Rewriting Rinf hub import to bridge URL.');
          node.textContent = text.replace(
            /from\\s+["'][^"']*pkg\\/hub\\.js["']/,
            ${JSON.stringify(`from "${bridgeBase}pkg/hub.js"`)},
          );
        };
        Element.prototype.append = function (...nodes) {
          for (const node of nodes) {
            rewriteRinfHubModuleScript(node);
          }
          return originalElementAppend.apply(this, nodes);
        };
        window.__ROBDEX_BRIDGE_BASE_URL__ = ${JSON.stringify(bridgeBaseUrl)};
        setLoadingDetail('Starting Flutter from bridge-served assets...');
        window.addEventListener('error', (event) => {
          setLoadingError(event.message || event.error || 'Unknown webview error');
        });
        window.addEventListener('unhandledrejection', (event) => {
          setLoadingError(event.reason || 'Unhandled promise rejection');
        });
        window.addEventListener('flutter-first-frame', () => {
          removeLoadingOverlay();
        });
        const flutterScript = document.createElement('script');
        flutterScript.src = ${JSON.stringify(flutterJsUrl)};
        flutterScript.async = true;
        flutterScript.onload = () => {
          setLoadingDetail('Flutter loader loaded; starting app...');
          window._flutter = window._flutter || {};
          window._flutter.buildConfig = {
            engineRevision: '425cfb54d01a9472b3e81d9e76fd63a4a44cfbcb',
            builds: [
              {
                compileTarget: 'dart2js',
                renderer: 'canvaskit',
                mainJsPath: 'main.dart.js',
              },
            ],
            useLocalCanvasKit: true,
          };
          const flutterConfig = {
            assetBase: ${JSON.stringify(bridgeBase)},
            canvasKitBaseUrl: ${JSON.stringify(`${bridgeBase}canvaskit/`)},
            canvasKitVariant: 'full',
            entrypointBaseUrl: ${JSON.stringify(bridgeBase)},
            renderer: 'canvaskit',
          };
          appStartTimeout = window.setTimeout(() => {
            const flutterNodes = Array.from(document.body.children)
              .map((child) => child.tagName.toLowerCase() + (child.id ? '#' + child.id : ''))
              .join(', ');
            setLoadingDetail('Still waiting for Flutter first frame after 15s. Body nodes: ' + flutterNodes);
          }, 15000);
          const markFlutterDomReady = () => {
            const flutterHost = document.querySelector('flt-glass-pane, flutter-view, flt-scene-host, canvas');
            if (!flutterHost) {
              return false;
            }
            removeLoadingOverlay();
            return true;
          };
          const domObserver = new MutationObserver(() => {
            if (markFlutterDomReady()) {
              domObserver.disconnect();
            }
          });
          domObserver.observe(document.body, { childList: true, subtree: true });
          window._flutter.loader.load({
            config: flutterConfig,
            onEntrypointLoaded: async (engineInitializer) => {
              try {
                setLoadingDetail('Dart entrypoint loaded; initializing Flutter engine...');
                const appRunner = await engineInitializer.initializeEngine(flutterConfig);
                setLoadingDetail('Flutter engine initialized; running Robdex...');
                await appRunner.runApp();
                setLoadingDetail('Robdex app started; waiting for first frame...');
                markFlutterDomReady();
              } catch (error) {
                window.clearTimeout(appStartTimeout);
                domObserver.disconnect();
                setLoadingError(error && error.stack ? error.stack : error);
              }
            },
          });
        };
        flutterScript.onerror = () => setLoadingError('Could not load flutter.js');
        document.body.appendChild(flutterScript);
      </script>
      <footer>Bridge healthy: ${escapeHtml(healthUrl)}</footer>
    `,
  );
}

async function openMentionedFile(target: string): Promise<void> {
  const parsed = parseMentionedFileTarget(target);
  if (parsed === undefined) {
    vscode.window.showWarningMessage(`Robdex could not open file link: ${target}`);
    return;
  }
  try {
    const document = await vscode.workspace.openTextDocument(parsed.uri);
    const editor = await vscode.window.showTextDocument(document, { preview: false });
    if (parsed.line !== undefined) {
      const position = new vscode.Position(parsed.line, parsed.character ?? 0);
      editor.selection = new vscode.Selection(position, position);
      editor.revealRange(new vscode.Range(position, position), vscode.TextEditorRevealType.InCenterIfOutsideViewport);
    }
  } catch (error) {
    vscode.window.showWarningMessage(`Robdex could not open ${target}: ${String(error)}`);
  }
}

function parseMentionedFileTarget(target: string): { uri: vscode.Uri; line?: number; character?: number } | undefined {
  const trimmed = target.trim();
  if (trimmed.length === 0) {
    return undefined;
  }
  if (/^https?:\/\//i.test(trimmed)) {
    return undefined;
  }

  let pathText = trimmed;
  if (trimmed.startsWith('file://')) {
    const uri = vscode.Uri.parse(trimmed);
    return { uri };
  }

  const match = /^(.*?)(?::(\d+))?(?::(\d+))?$/.exec(pathText);
  if (match !== null && match[1] !== undefined) {
    pathText = match[1];
    const line = match[2] === undefined ? undefined : Math.max(Number(match[2]) - 1, 0);
    const character = match[3] === undefined ? undefined : Math.max(Number(match[3]) - 1, 0);
    const uri = pathText.startsWith('/')
      ? vscode.Uri.file(pathText)
      : resolveWorkspaceRelativePath(pathText);
    return uri === undefined ? undefined : { uri, line, character };
  }
  return undefined;
}

function resolveWorkspaceRelativePath(pathText: string): vscode.Uri | undefined {
  const folders = vscode.workspace.workspaceFolders;
  if (folders === undefined || folders.length === 0) {
    return undefined;
  }
  return vscode.Uri.joinPath(folders[0].uri, pathText);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function renderBridgeFallback(
  webview: vscode.Webview,
  bridgeBaseUrl: string,
  healthUrl: string,
  message: string,
): string {
  const csp = [
    "default-src 'none'",
    `img-src ${webview.cspSource} data:`,
    `style-src ${webview.cspSource} 'unsafe-inline'`,
  ].join('; ');
  return htmlDocument(
    csp,
    `
      <main>
        <h1>Robdex bridge is not reachable</h1>
        <p>The extension checked:</p>
        <code>${escapeHtml(healthUrl)}</code>
        <p class="warning">${escapeHtml(message)}</p>
        <h2>Start or inspect Robdex</h2>
        <pre>robdex status
robdex start --foreground</pre>
        <h2>Bridge URL setting</h2>
        <pre>"robdex.bridgeBaseUrl": "${escapeHtml(bridgeBaseUrl)}"</pre>
        <p>After the bridge is healthy, run <strong>Robdex: Refresh</strong>.</p>
      </main>
    `,
  );
}

function htmlDocument(csp: string, body: string): string {
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta http-equiv="Content-Security-Policy" content="${escapeAttribute(csp)}">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <style>
    html, body {
      width: 100%;
      height: 100%;
      padding: 0;
      margin: 0;
      background: var(--vscode-editor-background);
      color: var(--vscode-editor-foreground);
      font-family: var(--vscode-font-family);
      font-size: var(--vscode-font-size);
    }
    footer {
      position: fixed;
      right: 8px;
      bottom: 6px;
      color: var(--vscode-descriptionForeground);
      font-size: 10px;
      pointer-events: none;
    }
    .loading {
      position: fixed;
      top: 12px;
      left: 12px;
      right: 12px;
      z-index: 1;
      display: flex;
      flex-direction: column;
      align-items: flex-start;
      gap: 4px;
      padding: 10px 12px;
      color: var(--vscode-descriptionForeground);
      background: rgba(17, 24, 32, 0.92);
      border: 1px solid var(--vscode-panel-border);
      border-radius: 6px;
      text-align: left;
      pointer-events: none;
    }
    .loading strong {
      color: var(--vscode-editor-foreground);
      font-size: 14px;
    }
    .loading.error span {
      max-width: min(720px, calc(100vw - 48px));
      color: var(--vscode-editorWarning-foreground);
      overflow-wrap: anywhere;
    }
    main {
      box-sizing: border-box;
      width: 100%;
      max-width: 680px;
      padding: 20px;
      line-height: 1.45;
    }
    h1 {
      margin: 0 0 14px;
      font-size: 17px;
    }
    h2 {
      margin: 22px 0 8px;
      font-size: 13px;
      color: var(--vscode-descriptionForeground);
    }
    code, pre {
      display: block;
      box-sizing: border-box;
      padding: 8px 10px;
      border: 1px solid var(--vscode-panel-border);
      border-radius: 4px;
      background: var(--vscode-textCodeBlock-background);
      color: var(--vscode-editor-foreground);
      overflow-x: auto;
    }
    .warning {
      color: var(--vscode-editorWarning-foreground);
    }
  </style>
</head>
<body>${body}</body>
</html>`;
}

function trimTrailingSlash(value: string): string {
  return value.replace(/\/+$/, '');
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

function escapeAttribute(value: string): string {
  return escapeHtml(value);
}
