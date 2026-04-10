type HookInput = {
  session_id?: unknown;
  cwd?: unknown;
  transcript_path?: unknown;
  hook_event_name?: unknown;
  model?: unknown;
  prompt?: unknown;
  last_assistant_message?: unknown;
  source?: unknown;
};

type HookPayload = {
  event: "turnStarted" | "turnStopped";
  sessionId: string;
  turnId: string | null;
  cwd: string | null;
  transcriptPath: string | null;
  model: string | null;
  prompt: string | null;
  lastAssistantMessage: string | null;
  observedAt: string;
};

const DEFAULT_BRIDGE_URL = "http://127.0.0.1:42080/codex/hooks/turn-lifecycle";

function text(value: unknown): string | null {
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : null;
}

function outputAndExit(payload: Record<string, unknown> | null): never {
  if (payload) {
    console.log(JSON.stringify(payload));
  }
  Deno.exit(0);
}

const DEBUG_LOG_PATH = "/tmp/robdex-codex-turn-hook.log";

async function appendDebugLog(record: Record<string, unknown>): Promise<void> {
  try {
    await Deno.writeTextFile(DEBUG_LOG_PATH, `${JSON.stringify({ observedAt: new Date().toISOString(), ...record })}
`, {
      append: true,
      create: true,
    });
  } catch {
    // Debug logging must fail open.
  }
}

async function readStdinJson(): Promise<HookInput | null> {
  try {
    const raw = await new Response(Deno.stdin.readable).text();
    await appendDebugLog({ stage: "stdin", raw });
    if (!raw.trim()) return null;
    const parsed = JSON.parse(raw);
    return parsed && typeof parsed === "object" ? parsed as HookInput : null;
  } catch (error) {
    await appendDebugLog({ stage: "stdin-error", error: String(error) });
    return null;
  }
}

async function main(): Promise<void> {
  const input = await readStdinJson();
  const hookEventName = text(input?.hook_event_name);
  const isStopHook = hookEventName === "Stop";

  const sessionId = text(input?.session_id);
  if (!sessionId) {
    outputAndExit(isStopHook ? { continue: true } : null);
  }

  let event: HookPayload["event"] | null = null;
  if (hookEventName === "UserPromptSubmit") {
    event = "turnStarted";
  } else if (hookEventName === "Stop") {
    event = "turnStopped";
  }

  if (!event) {
    outputAndExit(isStopHook ? { continue: true } : null);
  }

  const payload: HookPayload = {
    event,
    sessionId,
    turnId: text(input?.turn_id),
    cwd: text(input?.cwd),
    transcriptPath: text(input?.transcript_path),
    model: text(input?.model),
    prompt: text(input?.prompt) ?? text(input?.source),
    lastAssistantMessage: text(input?.last_assistant_message),
    observedAt: new Date().toISOString(),
  };

  const bridgeUrl = text(Deno.env.get("ROBDEX_BRIDGE_HOOK_URL")) ?? DEFAULT_BRIDGE_URL;

  try {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 1500);
    try {
      const response = await fetch(bridgeUrl, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(payload),
        signal: controller.signal,
      });
      await appendDebugLog({
        stage: "fetch",
        bridgeUrl,
        payload,
        ok: response.ok,
        status: response.status,
      });
    } finally {
      clearTimeout(timeout);
    }
  } catch (error) {
    await appendDebugLog({ stage: "fetch-error", bridgeUrl, payload, error: String(error) });
    // Hooks must fail open. If the bridge is down or noisy, do nothing.
  }

  outputAndExit(isStopHook ? { continue: true } : null);
}

await main();
