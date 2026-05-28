type Args = Record<string, string>;

const args = parseArgs(Bun.argv.slice(2));

if (args.help) {
  console.log(`Usage:
  npm run bun:shot -- --url http://127.0.0.1:47080 --out /tmp/design-lab.png

Options:
  --url <url>              Design Lab URL.
  --out <path>             PNG output path.
  --width <px>             Viewport width. Default: 1366
  --height <px>            Viewport height. Default: 1024
  --timeout <ms>           Ready timeout. Default: 30000
  --settle <ms>            Extra settle delay before screenshot. Default: 180
  --backend <webkit|chrome> Bun WebView backend. Default: webkit
  --skipReady              Do not wait for window.__designLabReady.
`);
  process.exit(0);
}

const url = args.url ?? 'http://127.0.0.1:47080';
const out = args.out ?? `/tmp/design-lab-bun-${Date.now()}.png`;
const width = Number(args.width ?? 1366);
const height = Number(args.height ?? 1024);
const timeoutMs = Number(args.timeout ?? 30000);
const settleMs = Number(args.settle ?? 180);
const backend = (args.backend ?? 'webkit') as 'webkit' | 'chrome';

const hardTimeout = setTimeout(() => {
  console.error(JSON.stringify({
    ok: false,
    error: `Design Lab Bun screenshot watchdog timed out after ${timeoutMs + 15000}ms`,
    url,
    out,
    backend,
  }, null, 2));
  process.exit(124);
}, timeoutMs + 15000);

const view = new Bun.WebView({
  width,
  height,
  url,
  backend,
  console,
});

try {
  const state = await waitForReady(view, timeoutMs);
  await Bun.sleep(settleMs);
  const screenshot = await view.screenshot({ encoding: 'buffer' });
  await Bun.write(out, screenshot);
  const title = await evaluateSafely<string>(view, 'document.title');
  clearTimeout(hardTimeout);
  console.log(JSON.stringify({
    ok: true,
    out,
    backend,
    url,
    width,
    height,
    state,
    title,
    screenshot: {
      bytes: screenshot.byteLength,
    },
  }, null, 2));
} catch (error) {
  clearTimeout(hardTimeout);
  console.error(JSON.stringify({
    ok: false,
    error: error instanceof Error ? error.message : String(error),
    backend,
    url,
    out,
  }, null, 2));
  process.exitCode = 1;
} finally {
  try {
    view.close();
  } catch {
    // The WebView can close itself after a failed navigation/backend startup.
  }
}

async function waitForReady(view: Bun.WebView, timeoutMs: number) {
  if (truthy(args.skipReady)) return null;
  const deadline = Date.now() + timeoutMs;
  let lastError = '';
  while (Date.now() < deadline) {
    try {
      const state = await view.evaluate('window.__designLabReady || window.__ezraDesignLabReady || null');
      if (state?.ready) return state;
    } catch (error) {
      lastError = error instanceof Error ? error.message : String(error);
    }
    await Bun.sleep(100);
  }
  throw new Error(
    `Timed out waiting for window.__designLabReady after ${timeoutMs}ms` +
      (lastError ? `; last eval error: ${lastError}` : ''),
  );
}

async function evaluateSafely<T = unknown>(view: Bun.WebView, expression: string): Promise<T | null> {
  try {
    return await view.evaluate(expression) as T;
  } catch {
    return null;
  }
}

function parseArgs(rawArgs: string[]): Args {
  const parsed: Args = {};
  for (let index = 0; index < rawArgs.length; index += 1) {
    const arg = rawArgs[index];
    if (!arg.startsWith('--')) continue;
    const key = arg.slice(2);
    const next = rawArgs[index + 1];
    if (!next || next.startsWith('--')) {
      parsed[key] = 'true';
    } else {
      parsed[key] = next;
      index += 1;
    }
  }
  return parsed;
}

function truthy(value: string | undefined): boolean {
  if (!value) return false;
  const normalized = value.trim().toLowerCase();
  return normalized === 'true' || normalized === '1' || normalized === 'yes';
}
