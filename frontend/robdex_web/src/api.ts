import type { ChatEntry, ProjectItem, ThreadMessagesPayload, WorkbenchViewData } from "./types";

export const THREAD_MESSAGE_LIMIT = 50;

export function bridgeBase(): string {
  return window.location.origin;
}

export function bridgeWsUrl(): string {
  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  return `${protocol}//${window.location.host}/workbench/ws`;
}

export async function fetchWorkbench(): Promise<WorkbenchViewData> {
  const response = await fetch("/workbench/bootstrap");
  if (!response.ok) {
    throw new Error(`Workbench bootstrap failed: ${response.status}`);
  }
  return workbenchViewFromRaw(await response.json());
}

export async function fetchThreadMessages(threadId: string, limit = THREAD_MESSAGE_LIMIT): Promise<ChatEntry[]> {
  const url = new URL("/threads/messages", bridgeBase());
  url.searchParams.set("threadId", threadId);
  if (limit != null) {
    url.searchParams.set("limit", String(limit));
  }
  const response = await fetch(url);
  if (response.status === 404) {
    return [];
  }
  if (!response.ok) {
    throw new Error(`Thread history failed: ${response.status}`);
  }
  const payload = await response.json() as { messages?: unknown[] };
  return chatEntriesFromPayload(payload);
}

export async function sendThreadMessage(threadId: string, text: string, localImagePaths: string[]): Promise<void> {
  const response = await fetch(`/threads/${encodeURIComponent(threadId)}/messages`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ text, localImagePaths }),
  });
  if (!response.ok) {
    throw new Error(`Send failed: ${response.status}`);
  }
}

export async function createThread(projectId: string, title: string, role: string): Promise<{ threadId: string | null }> {
  const response = await fetch("/threads", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ projectId, title, role }),
  });
  if (!response.ok) {
    throw new Error(`Thread create failed: ${response.status} ${await response.text()}`);
  }
  const payload = await response.json() as { threadId?: string };
  return { threadId: payload.threadId ?? null };
}

export async function updateProject(project: ProjectItem, changes: { name?: string; defaultCwd?: string }): Promise<void> {
  const response = await fetch(`/projects/${encodeURIComponent(project.id)}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      name: changes.name ?? project.name,
      defaultCWD: changes.defaultCwd ?? project.defaultCwd,
    }),
  });
  if (!response.ok) {
    throw new Error(`Project update failed: ${response.status} ${await response.text()}`);
  }
}

export async function interruptThread(threadId: string): Promise<void> {
  await postEmpty(`/threads/${encodeURIComponent(threadId)}/interrupt`);
}

export async function compactThread(threadId: string): Promise<void> {
  await postEmpty(`/threads/${encodeURIComponent(threadId)}/compact`);
}

export async function terminateCommand(threadId: string, processId: string): Promise<void> {
  const response = await fetch(`/threads/${encodeURIComponent(threadId)}/commands/terminate`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ processId }),
  });
  if (!response.ok) {
    throw new Error(`Terminate failed: ${response.status}`);
  }
}

export async function updateThreadMetadata(threadId: string, metadata: Record<string, unknown>): Promise<void> {
  const response = await fetch(`/threads/${encodeURIComponent(threadId)}/metadata`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(metadata),
  });
  if (!response.ok) {
    throw new Error(`Thread metadata update failed: ${response.status}`);
  }
}

export async function decideApproval(senderThreadId: string, approvalId: string, decision: string, message?: string): Promise<void> {
  const response = await fetch("/orchestrator/approval-decision", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      senderThreadId,
      approvalId,
      decision,
      message: message?.trim() ? message : null,
    }),
  });
  if (!response.ok) {
    throw new Error(`Approval decision failed: ${response.status}`);
  }
}

export async function uploadImage(file: File): Promise<string> {
  const url = new URL("/uploads/images/instant", bridgeBase());
  url.searchParams.set("filename", file.name || "image");
  const response = await fetch(url, {
    method: "POST",
    headers: { "content-type": file.type || "application/octet-stream" },
    body: file,
  });
  if (!response.ok) {
    throw new Error(`Upload failed: ${response.status}`);
  }
  const payload = await response.json() as { path?: string };
  if (!payload.path) {
    throw new Error("Upload response missing path");
  }
  return payload.path;
}

export function thumbnailUrl(savedPath: string): string {
  const url = new URL("/images/thumbnail", bridgeBase());
  url.searchParams.set("saved_path", savedPath);
  return url.toString();
}

export function imageUrl(savedPath: string): string {
  const url = new URL("/images/image", bridgeBase());
  url.searchParams.set("saved_path", savedPath);
  return url.toString();
}

export function chatEntriesFromPayload(payload: unknown): ChatEntry[] {
  const messages = typeof payload === "object" && payload !== null && "messages" in payload
    ? (payload as { messages?: unknown[] }).messages
    : undefined;
  return (messages ?? []).map(chatEntryFromUnknown).filter((entry): entry is ChatEntry => entry != null);
}

export function updateMessagesFromPayload(view: WorkbenchViewData, payload: ThreadMessagesPayload): WorkbenchViewData {
  const remainingPercent = typeof payload.contextWindowRemainingPercent === "number"
    ? payload.contextWindowRemainingPercent
    : typeof payload.contextWindowStatus?.remainingPercent === "number"
      ? payload.contextWindowStatus.remainingPercent
      : view.contextWindowRemainingPercent;
  return {
    ...view,
    chatEntries: chatEntriesFromPayload(payload),
    contextWindowRemainingPercent: remainingPercent,
  };
}

export function workbenchViewFromRaw(raw: unknown, previous?: WorkbenchViewData | null): WorkbenchViewData {
  if (isWorkbenchView(raw)) {
    return raw;
  }
  const root = record(raw);
  const state = record(root.state);
  const projectsRecord = record(state.projects);
  const selectedProjectId = stringOrNull(state.selectedProjectID) ?? stringOrNull(state.selectedProjectId);
  const contextByThread = record(record(root.threadCache).contextWindowStatusByThreadID);
  const runningThreadIds = new Set(array(record(root.threadCache).runningThreadIDs).filter((value): value is string => typeof value === "string"));
  const liveProcessesByThread = record(root.liveProcessesByThreadID);

  const projects = Object.entries(projectsRecord)
    .map(([id, value]) => {
      const project = record(value);
      return {
        id: stringValue(project.id, id),
        name: stringValue(project.name, id),
        rootPath: stringValue(project.projectRoot, stringValue(project.rootPath)),
        defaultCwd: stringValue(project.cwd, stringValue(project.defaultCwd)),
        isSelected: (stringValue(project.id, id) === selectedProjectId) || id === selectedProjectId,
      };
    })
    .sort((a, b) => a.name.localeCompare(b.name));

  const threads: WorkbenchViewData["threads"] = [];
  for (const projectValue of Object.values(projectsRecord)) {
    const project = record(projectValue);
    const projectName = stringValue(project.name, "Project");
    for (const [threadId, agentValue] of Object.entries(record(project.agents))) {
      const agent = record(agentValue);
      if (agent.archived === true) {
        continue;
      }
      const role = stringValue(agent.role, "worker");
      threads.push({
        id: threadId,
        title: stringValue(agent.displayName, threadId),
        role,
        projectName,
        preview: previewForAgent(agent),
        isRunning: runningThreadIds.has(threadId),
        unreadCount: numberValue(agent.unreadCount) ?? 0,
      });
    }
  }
  const selectedThreadId = previous?.selection.threadId && threads.some((thread) => thread.id === previous.selection.threadId)
    ? previous.selection.threadId
    : threads[0]?.id ?? null;
  const selectedThread = threads.find((thread) => thread.id === selectedThreadId) ?? null;
  const selectedProject = projects.find((project) => project.name === selectedThread?.projectName) ?? projects.find((project) => project.isSelected) ?? projects[0] ?? null;
  const selectedAgent = selectedThreadId ? findAgent(projectsRecord, selectedThreadId) : null;
  const selectedContext = selectedThreadId ? record(contextByThread[selectedThreadId]) : {};
  const liveProcesses = selectedThreadId
    ? ((array(liveProcessesByThread[selectedThreadId]).length > 0 ? array(liveProcessesByThread[selectedThreadId]) : array(record(selectedAgent).robdexLiveProcesses)) as unknown[]).map(liveProcessFromUnknown)
    : [];

  return {
    projects,
    threads,
    selection: {
      projectId: selectedProject?.id ?? null,
      projectRootPath: selectedProject?.rootPath ?? null,
      projectOrchestratorThreadId: null,
      projectOrchestratorName: null,
      threadId: selectedThreadId,
      threadRole: selectedThread?.role ?? null,
      projectName: selectedProject?.name ?? "No Project",
      threadName: selectedThread?.title ?? "No Thread Selected",
      connectionLabel: stringValue(root.connectionStatus, "connected"),
      sandboxMode: nullableString(record(selectedAgent).sandboxMode),
      networkAccess: booleanOrNull(record(selectedAgent).networkAccess),
      approvalPolicy: nullableString(record(selectedAgent).approvalPolicy),
      model: nullableString(record(selectedAgent).model),
      reasoningEffort: nullableString(record(selectedAgent).reasoningEffort),
      serviceTier: nullableString(record(selectedAgent).serviceTier),
      effectiveSandboxMode: nullableString(record(selectedAgent).sandboxMode),
      effectiveNetworkAccess: booleanOrNull(record(selectedAgent).networkAccess),
      effectiveApprovalPolicy: nullableString(record(selectedAgent).approvalPolicy),
      effectiveModel: nullableString(record(selectedAgent).model),
      effectiveReasoningEffort: nullableString(record(selectedAgent).reasoningEffort),
      effectiveServiceTier: nullableString(record(selectedAgent).serviceTier),
      isRunning: selectedThread?.isRunning ?? false,
    },
    liveProcesses,
    chatEntries: previous?.selection.threadId === selectedThreadId ? previous.chatEntries : [],
    contextWindowRemainingPercent: numberValue(selectedContext.remainingPercent),
    pendingApprovals: array(root.pendingApprovals).map(approvalFromUnknown),
    statusHeadline: "Bridge Connected",
    statusDetail: "",
    composerHint: "",
  };
}

async function postEmpty(path: string): Promise<void> {
  const response = await fetch(path, { method: "POST" });
  if (!response.ok) {
    throw new Error(`Request failed: ${response.status}`);
  }
}

function chatEntryFromUnknown(value: unknown): ChatEntry | null {
  if (typeof value !== "object" || value === null) {
    return null;
  }
  const item = value as Record<string, unknown>;
  const toolMetadata = record(item.toolMetadata);
  const role = stringValue(item.author, stringValue(item.role));
  const kind = nullableString(item.kind) ?? nullableString(toolMetadata.kind);
  const command = nullableString(item.command) ?? nullableString(toolMetadata.command);
  const output = nullableString(item.output) ?? nullableString(toolMetadata.output);
  const status = nullableString(item.status) ?? nullableString(toolMetadata.status);
  const body = stringValue(item.body, stringValue(item.text));
  return {
    id: stringValue(item.id),
    author: role,
    displayLabel: stringValue(item.displayLabel, displayLabelForMessage(role, kind)),
    timestamp: numberValue(item.timestamp) ?? numberValue(item.createdAt),
    body,
    subtitle: nullableString(item.subtitle),
    kind,
    status,
    processId: nullableString(item.processId) ?? nullableString(toolMetadata.processId),
    command,
    output,
    deliveryState: nullableString(item.deliveryState),
    isStreaming: Boolean(item.isStreaming),
    isTool: Boolean(item.isTool) || role === "tool" || toolMetadata.kind != null,
  };
}

function displayLabelForMessage(role: string, kind: string | null): string {
  if (kind === "commandExecution") {
    return "Command";
  }
  if (kind === "imageView") {
    return "Image";
  }
  if (!role) {
    return "Message";
  }
  return role.charAt(0).toUpperCase() + role.slice(1);
}

function isWorkbenchView(value: unknown): value is WorkbenchViewData {
  const item = record(value);
  return Array.isArray(item.projects) &&
    typeof item.selection === "object" &&
    Array.isArray(item.threads) &&
    Array.isArray(item.chatEntries);
}

function findAgent(projects: Record<string, unknown>, threadId: string): Record<string, unknown> | null {
  for (const projectValue of Object.values(projects)) {
    const agent = record(record(projectValue).agents)[threadId];
    if (agent) {
      return record(agent);
    }
  }
  return null;
}

function liveProcessFromUnknown(value: unknown) {
  const item = record(value);
  return {
    processId: stringValue(item.processId),
    pid: numberValue(item.pid),
    processGroupId: numberValue(item.processGroupId),
    command: stringValue(item.command),
    cwd: nullableString(item.cwd),
    startedAt: numberValue(item.startedAt),
  };
}

function approvalFromUnknown(value: unknown) {
  const item = record(value);
  return {
    id: stringValue(item.id),
    threadId: stringValue(item.threadId, stringValue(item.threadID)),
    kind: stringValue(item.kind),
    title: stringValue(item.title, "Approval request"),
    detail: nullableString(item.detail),
    command: nullableString(item.command),
    commandCwd: nullableString(item.commandCwd),
    filePaths: array(item.filePaths).filter((path): path is string => typeof path === "string"),
  };
}

function previewForAgent(agent: Record<string, unknown>): string {
  const blocked = nullableString(agent.blockedReason);
  if (blocked) {
    return blocked;
  }
  const issue = numberValue(agent.issueNumber);
  const pr = numberValue(agent.pullRequestNumber);
  if (pr != null) {
    return `PR #${pr}`;
  }
  if (issue != null) {
    return `Issue #${issue}`;
  }
  return stringValue(agent.cwd);
}

function record(value: unknown): Record<string, unknown> {
  return typeof value === "object" && value !== null ? value as Record<string, unknown> : {};
}

function array(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

function stringValue(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback;
}

function nullableString(value: unknown): string | null {
  return typeof value === "string" ? value : null;
}

function numberValue(value: unknown): number | null {
  return typeof value === "number" ? value : null;
}

function stringOrNull(value: unknown): string | null {
  return typeof value === "string" ? value : null;
}

function booleanOrNull(value: unknown): boolean | null {
  return typeof value === "boolean" ? value : null;
}
