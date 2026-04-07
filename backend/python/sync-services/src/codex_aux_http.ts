type Json = null | boolean | number | string | Json[] | { [key: string]: Json };

type CommandParserRequest = {
  command: string[];
  output: string;
  includeWarnings?: boolean;
  additionalRequest?: string | null;
  profile?: string | null;
};

type RequestReviewRequest = {
  repo_path: string;
  title: string;
  profile: string;
  uncommitted: boolean;
  commit_ref?: string | null;
};

const DEFAULT_BIND = "127.0.0.1";
const DEFAULT_PORT = 8771;
const DEFAULT_COMMAND_PARSER_PROFILE = "command-parser";
const CODEX_BIN = "/usr/local/bin/codex";
const MAX_BODY_BYTES = 64 * 1024 * 1024;

function text(value: unknown): string | null {
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : null;
}

function stringArray(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : [];
}

function bool(value: unknown): boolean | null {
  return typeof value === "boolean" ? value : null;
}

function dirname(path: string): string {
  const index = path.lastIndexOf("/");
  return index <= 0 ? "." : path.slice(0, index);
}

async function ensureDir(path: string): Promise<void> {
  await Deno.mkdir(path, { recursive: true });
}

async function readJsonBody<T>(request: Request): Promise<T> {
  const raw = await request.text();
  if (raw.length > MAX_BODY_BYTES) {
    throw new Error("request body too large");
  }
  return JSON.parse(raw) as T;
}

function jsonResponse(value: unknown, status = 200): Response {
  return new Response(JSON.stringify(value), {
    status,
    headers: { "content-type": "application/json" },
  });
}

async function withTempDir<T>(prefix: string, run: (dir: string) => Promise<T>): Promise<T> {
  const root = `${Deno.env.get("TMPDIR") ?? "/tmp"}/codex-aux`;
  await ensureDir(root);
  const dir = await Deno.makeTempDir({ dir: root, prefix: `${prefix}-` });
  try {
    return await run(dir);
  } finally {
    await Deno.remove(dir, { recursive: true }).catch(() => {});
  }
}

function commandParserAgentsMd(): string {
  return `You are command-parser, a CLI output extraction agent.

Task:
- Read ./output.log and extract errors (and warnings only if requested).
- Prefer targeted search (rg, grep) before broad reads for huge files.
- Read ./command.txt to determine whether this command should have used command-parser.
- You cannot run commands. You cannot rerun commands. You cannot inspect anything outside the provided files.

Output rules:
- If there are no errors at all:
  - and no additional request: output exactly: No errors!
  - and additional request exists: output \`No errors!\` first, then \`## Requested Information\`
- Otherwise output:
  - ## Errors
  - one bullet per distinct error as: - <brief message> — <file:line(:col) when present>
- Special case — unit test failures:
  - Include failing test names and concise assertion/panic/trace lines that explain why a test failed.
  - Include expected vs actual snippets when present.
  - Do not include passing tests or non-error test noise.
- If warnings are requested and present, add:
  - ## Warnings
  - one bullet per distinct warning as: - <brief message> — <file:line(:col) when present>
- Additional request (optional):
  - Only if an additional request is provided, append:
    - ## Requested Information
    - concise bullets answering only that request, anchored to log lines/files when present
  - If the additional request asks you to run, rerun, retry, execute, invoke, or test a command, output exactly:
    - I cannot run commands, do not ask me again.
  - If requested information is not present, output: - Not found in output.
- Preserve file paths and coordinates exactly as shown.
- Do not include advice, fixes, commands, or extra headings.
`;
}

function commandParserPrompt(args: {
  command: string[];
  includeWarnings: boolean;
  additionalRequest: string | null;
}): string {
  const renderedCommand = args.command.map((part) => {
    if (/^[A-Za-z0-9_./:-]+$/.test(part)) return part;
    return JSON.stringify(part);
  }).join(" ");
  return `Parse ./output.log from this raw command:
${renderedCommand}

Include warnings: ${args.includeWarnings ? "yes" : "no"}

Additional request: ${args.additionalRequest?.trim() || "<none>"}

Return only the structured extraction format from AGENTS.md.`;
}

async function runCodexExec(args: {
  cwd: string;
  profile: string;
  prompt: string;
  sandbox: "read-only" | "danger-full-access";
  extraArgs?: string[];
  env?: Record<string, string>;
}): Promise<{ stdout: string; stderr: string; code: number }> {
  const cmd = new Deno.Command(CODEX_BIN, {
    cwd: args.cwd,
    args: [
      "exec",
      "--skip-git-repo-check",
      "--ephemeral",
      "--json",
      "-s",
      args.sandbox,
      "-C",
      args.cwd,
      "-p",
      args.profile,
      "-c",
      'web_search="disabled"',
      "-c",
      "features.unified_exec=true",
      "-c",
      "features.multi_agent=false",
      "-c",
      "features.steer=false",
      "-c",
      "features.skills=false",
      ...(args.extraArgs ?? []),
      args.prompt,
    ],
    env: args.env,
    stdout: "piped",
    stderr: "piped",
  });
  const output = await cmd.output();
  return {
    stdout: new TextDecoder().decode(output.stdout),
    stderr: new TextDecoder().decode(output.stderr),
    code: output.code,
  };
}

function extractParserResponse(eventsText: string): string {
  let lastCompletedMessage: string | null = null;
  let lastMessage: string | null = null;
  for (const rawLine of eventsText.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line) continue;
    try {
      const event = JSON.parse(line) as Record<string, unknown>;
      const eventType = event.type;
      if (eventType === "item.completed") {
        const item = event.item;
        if (item && typeof item === "object" && !Array.isArray(item)) {
          const record = item as Record<string, unknown>;
          const textValue = text(record.text);
          if (record.type === "agent_message" && textValue) {
            lastCompletedMessage = textValue;
            lastMessage = textValue;
          }
        }
      } else if (eventType === "agent_message") {
        const textValue = text(event.text);
        if (textValue) {
          lastMessage = textValue;
        }
      }
    } catch {
      // Ignore non-JSON noise.
    }
  }

  const finalMessage = lastCompletedMessage ?? lastMessage;
  if (!finalMessage) {
    throw new Error("missing parser response");
  }
  return finalMessage.trimEnd();
}

async function handleCommandParser(request: Request): Promise<Response> {
  const body = await readJsonBody<CommandParserRequest>(request);
  const command = stringArray(body.command);
  if (command.length === 0) {
    return jsonResponse({ error: "command is required" }, 400);
  }
  const output = typeof body.output === "string" ? body.output : "";
  const profile = text(body.profile) ?? DEFAULT_COMMAND_PARSER_PROFILE;

  try {
    const message = await withTempDir("command-parser", async (dir) => {
      await Deno.writeTextFile(`${dir}/output.log`, output);
      await Deno.writeTextFile(`${dir}/command.txt`, `${command.join(" ")}\n`);
      await Deno.writeTextFile(`${dir}/AGENTS.md`, commandParserAgentsMd());
      const prompt = commandParserPrompt({
        command,
        includeWarnings: bool(body.includeWarnings) ?? false,
        additionalRequest: text(body.additionalRequest),
      });
      const result = await runCodexExec({
        cwd: dir,
        profile,
        prompt,
        sandbox: "workspace-write",
      });
      if (result.code !== 0) {
        throw new Error(result.stderr.trim() || result.stdout.trim() || `codex exec failed with exit code ${result.code}`);
      }
      return extractParserResponse(result.stdout);
    });
    return jsonResponse({ ok: true, message });
  } catch (error) {
    return jsonResponse({ ok: false, error: error instanceof Error ? error.message : String(error) }, 500);
  }
}

async function handleRequestReview(request: Request): Promise<Response> {
  const body = await readJsonBody<RequestReviewRequest>(request);
  const repoPath = text(body.repo_path);
  const title = text(body.title);
  const profile = text(body.profile);
  const uncommitted = bool(body.uncommitted);
  const commitRef = text(body.commit_ref);

  if (!repoPath || !title || !profile || uncommitted === null) {
    return jsonResponse({ error: "repo_path, title, profile, and uncommitted are required" }, 400);
  }
  if (!uncommitted && !commitRef) {
    return jsonResponse({ error: "commit_ref is required when uncommitted is false" }, 400);
  }

  try {
    const outputPath = `${Deno.env.get("TMPDIR") ?? "/tmp"}/request-review-${crypto.randomUUID()}.log`;
    const args = [
      "exec",
      "--ephemeral",
      "-C",
      repoPath,
      "-s",
      "read-only",
      "-p",
      profile,
      "review",
      "--output-last-message",
      outputPath,
      "--title",
      title,
      ...(uncommitted ? ["--uncommitted"] : ["--commit", commitRef!]),
    ];

    const cmd = new Deno.Command(CODEX_BIN, {
      args,
      stdout: "null",
      stderr: "piped",
    });
    const result = await cmd.output();
    let message = "";
    try {
      message = (await Deno.readTextFile(outputPath)).trimEnd();
    } catch {
      message = new TextDecoder().decode(result.stderr).trim() || `request-review failed with exit code ${result.code}`;
    } finally {
      await Deno.remove(outputPath).catch(() => {});
    }

    return jsonResponse({
      result: {
        message,
        exit_code: result.code,
      },
    });
  } catch (error) {
    return jsonResponse({
      result: {
        message: error instanceof Error ? error.message : String(error),
        exit_code: 1,
      },
    });
  }
}

async function main(): Promise<void> {
  const bind = text(Deno.env.get("CODEX_AUX_HTTP_BIND")) ?? DEFAULT_BIND;
  const portValue = Number(Deno.env.get("CODEX_AUX_HTTP_PORT") ?? DEFAULT_PORT);
  const port = Number.isFinite(portValue) && portValue > 0 ? Math.floor(portValue) : DEFAULT_PORT;

  console.log(`codex-aux-http listening on http://${bind}:${port}`);
  Deno.serve({ hostname: bind, port }, async (request) => {
    const url = new URL(request.url);
    try {
      if (url.pathname === "/healthz" && request.method === "GET") {
        return jsonResponse({ ok: true, service: "codex-aux-http" });
      }
      if (url.pathname === "/v1/command-parser/parse" && request.method === "POST") {
        return await handleCommandParser(request);
      }
      if (url.pathname === "/v1/request-review/run" && request.method === "POST") {
        return await handleRequestReview(request);
      }
      return jsonResponse({ error: "not found" }, 404);
    } catch (error) {
      return jsonResponse({ error: error instanceof Error ? error.message : String(error) }, 500);
    }
  });
}

await main();
