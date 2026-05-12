import React, { FormEvent, useEffect, useMemo, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import {
  Bot,
  Circle,
  FolderPlus,
  History,
  ImagePlus,
  Link2Off,
  Loader2,
  Pause,
  Plus,
  Settings2,
  Send,
  Shield,
  Sparkles,
  Square,
  TerminalSquare,
  UserRound,
  Wifi,
  Wrench,
} from "lucide-react";
import {
  bridgeWsUrl,
  chatEntriesFromPayload,
  compactThread,
  createThread,
  decideApproval,
  fetchThreadMessages,
  fetchWorkbench,
  imageUrl,
  interruptThread,
  sendThreadMessage,
  THREAD_MESSAGE_LIMIT,
  terminateCommand,
  thumbnailUrl,
  updateProject,
  updateMessagesFromPayload,
  updateThreadMetadata,
  uploadImage,
  workbenchViewFromRaw,
} from "./api";
import type { ChatEntry, PendingApprovalItem, ProjectItem, ThreadItem, WorkbenchEventEnvelope, WorkbenchViewData } from "./types";
import "./styles.css";

function App() {
  const [view, setView] = useState<WorkbenchViewData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [connection, setConnection] = useState("connecting");
  const selectedThreadId = view?.selection.threadId ?? null;
  const selectedThreadRef = useRef<string | null>(null);
  const wsRef = useRef<WebSocket | null>(null);

  useEffect(() => {
    let cancelled = false;
    fetchWorkbench()
      .then((next) => {
        if (!cancelled) {
          setView(next);
          selectedThreadRef.current = next.selection.threadId;
        }
      })
      .catch((err: unknown) => setError(String(err)));
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let disposed = false;
    let reconnect: number | undefined;
    function connect() {
      const ws = new WebSocket(bridgeWsUrl());
      wsRef.current = ws;
      setConnection("connecting");
      ws.onopen = () => {
        setConnection("connected");
        const threadId = selectedThreadRef.current;
        if (threadId) {
          sendThreadSelection(ws, threadId);
        }
      };
      ws.onmessage = (event) => {
        handleEnvelope(event.data, selectedThreadRef.current, setView, setConnection, setError);
      };
      ws.onerror = () => setConnection("error");
      ws.onclose = () => {
        if (!disposed) {
          setConnection("disconnected");
          reconnect = window.setTimeout(connect, 1200);
        }
      };
    }
    connect();
    return () => {
      disposed = true;
      if (reconnect !== undefined) {
        window.clearTimeout(reconnect);
      }
      wsRef.current?.close();
    };
  }, []);

  useEffect(() => {
    selectedThreadRef.current = selectedThreadId;
    const ws = wsRef.current;
    if (selectedThreadId && ws?.readyState === WebSocket.OPEN) {
      sendThreadSelection(ws, selectedThreadId);
    }
  }, [selectedThreadId]);

  useEffect(() => {
    if (!selectedThreadId) {
      return;
    }
    let cancelled = false;
    fetchThreadMessages(selectedThreadId, THREAD_MESSAGE_LIMIT)
      .then((messages) => {
        if (!cancelled && selectedThreadRef.current === selectedThreadId) {
          setView((current) => current ? { ...current, chatEntries: messages } : current);
        }
      })
      .catch((err: unknown) => setError(String(err)));
    return () => {
      cancelled = true;
    };
  }, [selectedThreadId]);

  function selectThread(threadId: string) {
    setView((current) => current ? {
      ...current,
      selection: {
        ...current.selection,
        threadId,
        threadName: current.threads.find((thread) => thread.id === threadId)?.title ?? current.selection.threadName,
        threadRole: current.threads.find((thread) => thread.id === threadId)?.role ?? current.selection.threadRole,
      },
      chatEntries: [],
    } : current);
    selectedThreadRef.current = threadId;
  }

  async function refreshHistory() {
    if (!selectedThreadId) {
      return;
    }
    const messages = await fetchThreadMessages(selectedThreadId, THREAD_MESSAGE_LIMIT);
    setView((current) => current ? { ...current, chatEntries: messages } : current);
  }

  async function refreshWorkbench(selectThreadId?: string | null) {
    const next = await fetchWorkbench();
    if (selectThreadId) {
      next.selection.threadId = selectThreadId;
      const selected = next.threads.find((thread) => thread.id === selectThreadId);
      if (selected) {
        next.selection.threadName = selected.title;
        next.selection.threadRole = selected.role;
        next.selection.projectName = selected.projectName;
      }
    }
    setView(next);
    selectedThreadRef.current = next.selection.threadId;
  }

  async function handleNewThread(project: ProjectItem) {
    const title = window.prompt(`New thread name for ${project.name}`, "New Thread");
    if (!title?.trim()) {
      return;
    }
    try {
      const created = await createThread(project.id, title.trim(), "worker");
      await refreshWorkbench(created.threadId);
    } catch (err) {
      setError(String(err));
    }
  }

  async function handleProjectSettings(project: ProjectItem) {
    const name = window.prompt("Project name", project.name);
    if (name == null) {
      return;
    }
    const defaultCwd = window.prompt("Default CWD", project.defaultCwd || project.rootPath);
    if (defaultCwd == null) {
      return;
    }
    try {
      await updateProject(project, { name: name.trim(), defaultCwd: defaultCwd.trim() });
      await refreshWorkbench(selectedThreadRef.current);
    } catch (err) {
      setError(String(err));
    }
  }

  if (!view) {
    return (
      <main className="boot" aria-live="polite">
        <h1>Robdex</h1>
        <p>{error ?? "Connecting to bridge..."}</p>
      </main>
    );
  }

  const selectedThread = view.threads.find((thread) => thread.id === selectedThreadId) ?? null;

  return (
    <main className="appShell">
      <Sidebar
        projects={view.projects}
        threads={view.threads}
        selectedThreadId={selectedThreadId}
        onSelectThread={selectThread}
        onProjectSettings={handleProjectSettings}
        onNewThread={handleNewThread}
      />
      <section className="workspace" aria-label="Selected thread workspace">
        <ThreadHeader
          view={view}
          selectedThread={selectedThread}
          connection={connection}
          onRefreshHistory={refreshHistory}
          onMetadataChange={(metadata) => selectedThreadId && updateThreadMetadata(selectedThreadId, metadata)}
          onCompact={() => selectedThreadId && compactThread(selectedThreadId)}
          onInterrupt={() => selectedThreadId && interruptThread(selectedThreadId)}
        />
        <ApprovalStrip
          approvals={view.pendingApprovals.filter((approval) => approval.threadId === selectedThreadId)}
          senderThreadId={selectedThreadId}
        />
        <ChatTimeline
          threadId={selectedThreadId}
          entries={view.chatEntries}
          onTerminate={(processId) => selectedThreadId && terminateCommand(selectedThreadId, processId)}
        />
        <Composer
          disabled={!selectedThreadId}
          running={view.selection.isRunning}
          onSend={async (text, imagePaths) => {
            if (!selectedThreadId) {
              return;
            }
            await sendThreadMessage(selectedThreadId, text, imagePaths);
            const messages = await fetchThreadMessages(selectedThreadId, THREAD_MESSAGE_LIMIT);
            setView((current) => current && current.selection.threadId === selectedThreadId
              ? { ...current, chatEntries: messages }
              : current);
          }}
        />
      </section>
    </main>
  );
}

function Sidebar(props: {
  projects: WorkbenchViewData["projects"];
  threads: ThreadItem[];
  selectedThreadId: string | null;
  onSelectThread: (threadId: string) => void;
  onProjectSettings: (project: ProjectItem) => void;
  onNewThread: (project: ProjectItem) => void;
}) {
  const byProject = useMemo(() => {
    const groups = new Map<string, ThreadItem[]>();
    for (const project of props.projects) {
      groups.set(project.name, []);
    }
    for (const thread of props.threads) {
      const group = groups.get(thread.projectName) ?? [];
      group.push(thread);
      groups.set(thread.projectName, group);
    }
    for (const [project, threads] of groups.entries()) {
      groups.set(project, [...threads].sort(compareThreadsForSidebar));
    }
    return groups;
  }, [props.projects, props.threads]);

  return (
    <aside className="sidebar" aria-label="Projects and threads">
      <div className="sidebarTitle">
        <div>
          <h1>Threads</h1>
          <span>{props.threads.length}</span>
        </div>
        <div className="sidebarTopActions" aria-hidden="true">
          <FolderPlus size={18} />
          <Link2Off size={18} />
        </div>
      </div>
      {Array.from(byProject.entries()).map(([projectName, threads]) => (
        <section className="projectGroup" key={projectName} aria-label={projectName}>
          <div className="projectHeader">
            <h2>{projectName}</h2>
            <span>
              <button
                type="button"
                className="projectAction"
                aria-label={`Edit ${projectName} project settings`}
                title="Project settings"
                onClick={() => {
                  const project = props.projects.find((item) => item.name === projectName);
                  if (project) {
                    props.onProjectSettings(project);
                  }
                }}
              >
                <Settings2 size={16} />
              </button>
              <button
                type="button"
                className="projectAction"
                aria-label={`Create new thread in ${projectName}`}
                title="New thread"
                onClick={() => {
                  const project = props.projects.find((item) => item.name === projectName);
                  if (project) {
                    props.onNewThread(project);
                  }
                }}
              >
                <Plus size={16} />
              </button>
            </span>
          </div>
          <div className="threadList" role="list">
            {threads.map((thread) => (
              <button
                key={thread.id}
                type="button"
                className={`threadItem ${thread.id === props.selectedThreadId ? "selected" : ""}`}
                onClick={() => props.onSelectThread(thread.id)}
                aria-current={thread.id === props.selectedThreadId ? "true" : undefined}
              >
                <span className="threadTitle">{thread.title}</span>
                <span className={`roleBadge ${thread.role}`} aria-label={`Role ${thread.role}`}>{roleIcon(thread.role)}</span>
                {thread.isRunning && <span className="runningDot" aria-label="Running" />}
                {thread.unreadCount > 0 && <span className="unreadBadge" aria-label={`${thread.unreadCount} unread`}>{thread.unreadCount}</span>}
              </button>
            ))}
          </div>
        </section>
      ))}
    </aside>
  );
}

function ThreadHeader(props: {
  view: WorkbenchViewData;
  selectedThread: ThreadItem | null;
  connection: string;
  onMetadataChange: (metadata: Record<string, unknown>) => void;
  onRefreshHistory: () => void;
  onCompact: () => void;
  onInterrupt: () => void;
}) {
  const selection = props.view.selection;
  return (
    <header className="threadHeader">
      <div className="threadIdentity">
        <p className="eyebrow">{props.view.selection.projectName}</p>
        <h2>{props.selectedThread?.title ?? "No thread selected"}</h2>
        <div className="statusLine">
          <span>{props.view.contextWindowRemainingPercent ?? "--"}% remaining</span>
          {props.connection !== "connected" && <span className="connectionProblem">{props.connection}</span>}
        </div>
        <div className="threadControls" aria-label="Thread controls">
          <CompactSelect
            label="Model"
            icon={<Bot size={13} />}
            value={selection.model ?? ""}
            options={["", "gpt-5.5", "gpt-5.4", "gpt-5.4-mini", "gpt-5.4-nano", "gpt-5"]}
            optionLabel={(value) => value || inherited(selection.effectiveModel)}
            onChange={(modelID) => props.onMetadataChange({ modelID })}
          />
          <CompactSelect
            label="Role"
            icon={<Settings2 size={13} />}
            value={selection.threadRole ?? ""}
            options={["worker", "designer", "qa", "operator", "orchestrator", "hidden"]}
            onChange={(role) => props.onMetadataChange({ role })}
          />
          <CompactSelect
            label="Approval"
            icon={<Shield size={13} />}
            value={selection.approvalPolicy ?? ""}
            options={["", "untrusted", "on-failure", "on-request", "never"]}
            optionLabel={(value) => value || inherited(selection.effectiveApprovalPolicy)}
            onChange={(approvalPolicy) => props.onMetadataChange({ approvalPolicy })}
          />
          <CompactSelect
            label="Sandbox"
            icon={<TerminalSquare size={13} />}
            value={selection.sandboxMode ?? ""}
            options={["", "workspace-write", "danger-full-access"]}
            optionLabel={(value) => value || inherited(selection.effectiveSandboxMode)}
            onChange={(sandboxMode) => props.onMetadataChange({ sandboxMode })}
          />
          <CompactSelect
            label="Network"
            icon={<Wifi size={13} />}
            value={networkMode(selection.networkAccess)}
            options={["default", "enabled", "disabled"]}
            optionLabel={(value) => value === "default" ? inherited(networkLabel(selection.effectiveNetworkAccess)) : value}
            onChange={(value) => props.onMetadataChange({ networkAccess: value === "default" ? null : value === "enabled" })}
          />
          <CompactSelect
            label="Service"
            icon={<Sparkles size={13} />}
            value={selection.serviceTier ?? ""}
            options={["", "fast", "flex"]}
            optionLabel={(value) => value || inherited(selection.effectiveServiceTier)}
            onChange={(serviceTier) => props.onMetadataChange({ serviceTier })}
          />
          <CompactSelect
            label="Reasoning"
            icon={<Circle size={10} fill="currentColor" />}
            value={selection.reasoningEffort ?? ""}
            options={["", "low", "medium", "high"]}
            optionLabel={(value) => value || inherited(selection.effectiveReasoningEffort)}
            onChange={(reasoningEffort) => props.onMetadataChange({ reasoningEffort })}
          />
        </div>
      </div>
      <nav className="headerActions" aria-label="Thread actions">
        <button type="button" aria-label="Open history" title="History" onClick={props.onRefreshHistory}><History size={17} /></button>
        <button type="button" aria-label="Compact thread" title="Compact" onClick={props.onCompact}><Sparkles size={17} /></button>
        <button type="button" aria-label="Interrupt thread" title="Interrupt" onClick={props.onInterrupt}><Pause size={17} /></button>
      </nav>
    </header>
  );
}

function CompactSelect(props: {
  label: string;
  icon: React.ReactNode;
  value: string;
  options: string[];
  optionLabel?: (value: string) => string;
  onChange: (value: string) => void;
}) {
  return (
    <label className="compactSelect" title={props.label}>
      <span aria-hidden="true">{props.icon}</span>
      <span className="visuallyHidden">{props.label}</span>
      <select value={props.value} onChange={(event) => props.onChange(event.target.value)} aria-label={props.label}>
        {props.options.map((option) => (
          <option key={option || "default"} value={option}>{props.optionLabel ? props.optionLabel(option) : option}</option>
        ))}
      </select>
    </label>
  );
}

function ApprovalStrip(props: { approvals: PendingApprovalItem[]; senderThreadId: string | null }) {
  if (props.approvals.length === 0 || !props.senderThreadId) {
    return null;
  }
  return (
    <section className="approvalStrip" aria-label="Pending approvals">
      {props.approvals.map((approval) => (
        <article className="approvalCard" key={approval.id}>
          <h3>{approval.title}</h3>
          {approval.command && <pre>{approval.command}</pre>}
          {approval.detail && <p>{approval.detail}</p>}
          <div className="approvalActions">
            <button type="button" onClick={() => decideApproval(props.senderThreadId!, approval.id, "approved")}>Approve</button>
            <button type="button" onClick={() => decideApproval(props.senderThreadId!, approval.id, "denied")}>Deny</button>
          </div>
        </article>
      ))}
    </section>
  );
}

function ChatTimeline(props: {
  threadId: string | null;
  entries: ChatEntry[];
  onTerminate: (processId: string) => void;
}) {
  const endRef = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    endRef.current?.scrollIntoView({ block: "end" });
  }, [props.entries.length, props.threadId]);

  return (
    <section className="timeline" aria-label="Chat timeline">
      {props.entries.map((entry) => (
        <ChatBubble key={`${entry.id}-${entry.status ?? ""}-${entry.isStreaming}`} entry={entry} onTerminate={props.onTerminate} />
      ))}
      <div ref={endRef} />
    </section>
  );
}

function ChatBubble(props: { entry: ChatEntry; onTerminate: (processId: string) => void }) {
  const entry = props.entry;
  const [expanded, setExpanded] = useState(false);
  const timestamp = entry.timestamp ? new Date(entry.timestamp * 1000).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" }) : "";
  const isImage = entry.kind === "imageView" && entry.output;
  const isCommand = entry.kind === "commandExecution";
  const isToolEvent = entry.isTool || Boolean(entry.kind);
  const canTerminate = entry.processId && (entry.isStreaming || entry.status?.toLowerCase().includes("running"));

  if (isCommand) {
    return (
      <CommandEventRow
        entry={entry}
        timestamp={timestamp}
        expanded={expanded}
        onExpandedChanged={setExpanded}
        onTerminate={props.onTerminate}
      />
    );
  }

  if (isToolEvent && !isImage) {
    return (
      <ToolEventRow
        entry={entry}
        timestamp={timestamp}
        expanded={expanded}
        onExpandedChanged={setExpanded}
      />
    );
  }

  return (
    <article className={`chatBubble ${entry.isTool ? "tool" : ""} ${entry.author.toLowerCase()}`} aria-label={`${entry.displayLabel} message`}>
      <header>
        <strong>{entry.displayLabel || entry.author}</strong>
        <span>{timestamp}</span>
      </header>
      {isImage ? (
        <a href={imageUrl(entry.output!)} target="_blank" rel="noreferrer" className="imagePreview">
          <img src={thumbnailUrl(entry.output!)} alt="Generated or viewed image" />
          <span>{entry.output}</span>
        </a>
      ) : (
        <>
          {entry.command && <pre className="commandText">{entry.command}</pre>}
          {entry.body && <div className="messageText">{entry.body}</div>}
          {entry.output && <pre className="outputText">{entry.output}</pre>}
        </>
      )}
      {canTerminate && (
        <button className="terminateButton" type="button" onClick={() => props.onTerminate(entry.processId!)}>
          <Square size={14} /> Terminate command
        </button>
      )}
    </article>
  );
}

function CommandEventRow(props: {
  entry: ChatEntry;
  timestamp: string;
  expanded: boolean;
  onExpandedChanged: (expanded: boolean) => void;
  onTerminate: (processId: string) => void;
}) {
  const command = props.entry.command?.trim() || props.entry.body.trim();
  const canExpand = Boolean(props.entry.output?.trim() || props.entry.body.trim() || props.entry.processId);
  const canTerminate = props.entry.processId && (props.entry.isStreaming || isInProgressStatus(props.entry.status));
  return (
    <article className="eventRow commandEvent" aria-label="Command event">
      <div className="eventHeader">
        <strong>Command</strong>
        <StatusBadge status={props.entry.status} streaming={props.entry.isStreaming} />
        {canExpand && (
          <button
            type="button"
            className="eventExpand"
            aria-label={props.expanded ? "Collapse command" : "Expand command"}
            onClick={() => props.onExpandedChanged(!props.expanded)}
          >
            {props.expanded ? "⌃" : "⌄"}
          </button>
        )}
        <span className="eventTime">{props.timestamp}</span>
        {canTerminate && (
          <button className="eventTerminate" type="button" onClick={() => props.onTerminate(props.entry.processId!)}>
            <Square size={14} />
          </button>
        )}
      </div>
      <code className="eventPreview">{props.expanded ? command : compactPreview(command)}</code>
      {props.expanded && props.entry.processId && (
        <EventSection label="PID" value={props.entry.processId} />
      )}
      {props.expanded && props.entry.output?.trim() && (
        <EventSection label="Output" value={props.entry.output} />
      )}
    </article>
  );
}

function ToolEventRow(props: {
  entry: ChatEntry;
  timestamp: string;
  expanded: boolean;
  onExpandedChanged: (expanded: boolean) => void;
}) {
  const title = props.entry.subtitle?.trim() || props.entry.displayLabel || "Tool";
  const preview = props.entry.command?.trim() || props.entry.body.trim();
  const canExpand = Boolean(props.entry.output?.trim() || needsExpansion(props.entry.command) || needsExpansion(props.entry.body));
  return (
    <article className="eventRow toolEvent" aria-label={`${title} event`}>
      <div className="eventHeader">
        <StatusBadge status={props.entry.status} streaming={props.entry.isStreaming} tool />
        <strong>{title}</strong>
        {canExpand && (
          <button
            type="button"
            className="eventExpand"
            aria-label={props.expanded ? `Collapse ${title}` : `Expand ${title}`}
            onClick={() => props.onExpandedChanged(!props.expanded)}
          >
            {props.expanded ? "⌃" : "⌄"}
          </button>
        )}
        <span className="eventTime">{props.timestamp}</span>
      </div>
      {preview && <code className="eventPreview muted">{props.expanded ? preview : compactPreview(preview)}</code>}
      {props.expanded && props.entry.output?.trim() && (
        <EventSection label="Output" value={props.entry.output} />
      )}
    </article>
  );
}

function StatusBadge(props: { status: string | null; streaming: boolean; tool?: boolean }) {
  const state = statusState(props.status, props.streaming);
  return (
    <span className={`statusBadge ${state}`} title={props.status ?? state}>
      {props.tool ? <Wrench size={13} /> : statusIcon(state)}
      <span>{stateLabel(state)}</span>
    </span>
  );
}

function EventSection(props: { label: string; value: string }) {
  return (
    <div className="eventSection">
      <span>{props.label}</span>
      <pre>{props.value}</pre>
    </div>
  );
}

function Composer(props: {
  disabled: boolean;
  running: boolean;
  onSend: (text: string, imagePaths: string[]) => Promise<void> | void;
}) {
  const [text, setText] = useState("");
  const [images, setImages] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const fileInputRef = useRef<HTMLInputElement | null>(null);

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    if (props.disabled || (!text.trim() && images.length === 0)) {
      return;
    }
    setBusy(true);
    try {
      await props.onSend(text.trim(), images);
      setText("");
      setImages([]);
    } finally {
      setBusy(false);
    }
  }

  async function handleFiles(files: FileList | null) {
    if (!files) {
      return;
    }
    setBusy(true);
    try {
      const uploaded: string[] = [];
      for (const file of Array.from(files)) {
        if (file.type.startsWith("image/")) {
          uploaded.push(await uploadImage(file));
        }
      }
      setImages((current) => [...current, ...uploaded]);
    } finally {
      setBusy(false);
      if (fileInputRef.current) {
        fileInputRef.current.value = "";
      }
    }
  }

  return (
    <form className="composer" onSubmit={handleSubmit} aria-label="Message composer">
      {images.length > 0 && (
        <div className="attachmentTray" aria-label="Image attachments">
          {images.map((path) => (
            <button key={path} type="button" className="attachmentChip" onClick={() => setImages((current) => current.filter((item) => item !== path))}>
              <img src={thumbnailUrl(path)} alt="" />
              <span>{fileName(path)}</span>
              <span aria-hidden="true">×</span>
            </button>
          ))}
        </div>
      )}
      <input
        ref={fileInputRef}
        type="file"
        accept="image/*"
        multiple
        className="visuallyHidden"
        onChange={(event) => handleFiles(event.target.files)}
      />
      <button type="button" disabled={props.disabled || busy} onClick={() => fileInputRef.current?.click()}>
        <ImagePlus size={18} />
      </button>
      <label className="visuallyHidden" htmlFor="composer-input">Chat message input</label>
      <textarea
        id="composer-input"
        value={text}
        disabled={props.disabled || busy}
        placeholder="Send a message to the selected thread"
        onChange={(event) => setText(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" && !event.shiftKey) {
            event.preventDefault();
            event.currentTarget.form?.requestSubmit();
          }
        }}
      />
      <button type="submit" disabled={props.disabled || busy}>
        {busy ? <Loader2 className="spin" size={18} /> : props.running && !text.trim() ? <Square size={18} /> : <Send size={18} />}
      </button>
    </form>
  );
}

function handleEnvelope(
  raw: string,
  selectedThreadId: string | null,
  setView: React.Dispatch<React.SetStateAction<WorkbenchViewData | null>>,
  setConnection: React.Dispatch<React.SetStateAction<string>>,
  setError: React.Dispatch<React.SetStateAction<string | null>>,
) {
  try {
    const envelope = JSON.parse(raw) as WorkbenchEventEnvelope;
    if (envelope.type === "helloAck") {
      setConnection("connected");
      return;
    }
    const event = envelope.payload?.event;
    if (!event?.name) {
      return;
    }
    if (event.name === "appStateSnapshot" && event.data) {
      setView((current) => {
        const next = workbenchViewFromRaw(event.data, current);
        return current ? { ...next, chatEntries: current.chatEntries } : next;
      });
    } else if (event.name === "threadMessagesChanged" && event.data) {
      const payload = event.data as { threadId?: string; threadID?: string };
      const payloadThreadId = payload.threadId ?? payload.threadID ?? null;
      if (payloadThreadId && selectedThreadId && payloadThreadId !== selectedThreadId) {
        return;
      }
      setView((current) => current ? updateMessagesFromPayload(current, event.data as never) : current);
    } else if (event.name === "liveProcessesChanged" && event.data) {
      const payload = event.data as { threadId?: string; threadID?: string; processes?: never[] };
      const payloadThreadId = payload.threadId ?? payload.threadID ?? null;
      if (payloadThreadId && selectedThreadId && payloadThreadId !== selectedThreadId) {
        return;
      }
      setView((current) => current ? { ...current, liveProcesses: (payload.processes ?? []) as never } : current);
    } else if (event.name === "connectionStatus") {
      const message = typeof event.message === "string"
        ? event.message
        : typeof (event.data as { message?: unknown } | null)?.message === "string"
          ? String((event.data as { message?: unknown }).message)
          : "connected";
      setConnection(message);
    } else if (event.name === "commandResult" && event.data) {
      const errorMessage = (event.data as { errorMessage?: unknown }).errorMessage;
      if (typeof errorMessage === "string" && errorMessage.trim()) {
        setError(errorMessage);
      }
    }
  } catch (err) {
    setError(String(err));
  }
}

function sendThreadSelection(ws: WebSocket, threadId: string) {
  ws.send(JSON.stringify({
    type: "command",
    payload: {
      id: `thread-select-${threadId}`,
      command: {
        name: "threadSelectionSet",
        payload: { threadId },
      },
    },
  }));
}

function compareThreadsForSidebar(a: ThreadItem, b: ThreadItem): number {
  const roleDelta = roleSortRank(a.role) - roleSortRank(b.role);
  if (roleDelta !== 0) {
    return roleDelta;
  }
  const runningDelta = Number(b.isRunning) - Number(a.isRunning);
  if (runningDelta !== 0) {
    return runningDelta;
  }
  return a.title.localeCompare(b.title, undefined, { sensitivity: "base" });
}

function roleSortRank(role: string): number {
  switch (role) {
    case "operator":
      return 0;
    case "orchestrator":
      return 1;
    case "designer":
      return 2;
    case "qa":
      return 3;
    case "worker":
      return 4;
    case "hidden":
      return 6;
    default:
      return 5;
  }
}

function roleIcon(role: string): React.ReactNode {
  switch (role) {
    case "orchestrator":
      return <TerminalSquare size={13} />;
    case "qa":
      return <Wrench size={13} />;
    case "designer":
      return <Sparkles size={13} />;
    case "operator":
      return <UserRound size={13} />;
    case "hidden":
      return <Bot size={13} />;
    default:
      return <Circle size={9} fill="currentColor" />;
  }
}

function statusState(status: string | null, streaming: boolean): string {
  const normalized = status?.toLowerCase() ?? "";
  if (normalized.includes("fail") || normalized.includes("error") || normalized.includes("reject")) {
    return "failed";
  }
  if (streaming || normalized.includes("progress") || normalized.includes("pending") || normalized.includes("running")) {
    return "in-progress";
  }
  if (normalized.includes("complete") || normalized.includes("success") || normalized.includes("approved")) {
    return "completed";
  }
  return streaming ? "in-progress" : "completed";
}

function statusIcon(state: string): React.ReactNode {
  if (state === "failed") {
    return <Circle size={11} fill="currentColor" />;
  }
  if (state === "in-progress") {
    return <Loader2 className="spin" size={13} />;
  }
  return <Circle size={11} fill="currentColor" />;
}

function stateLabel(state: string): string {
  if (state === "in-progress") {
    return "in progress";
  }
  return state;
}

function isInProgressStatus(status: string | null): boolean {
  return statusState(status, false) === "in-progress";
}

function needsExpansion(value: string | null): boolean {
  const trimmed = value?.trim() ?? "";
  return trimmed.includes("\n") || trimmed.length > 160;
}

function compactPreview(value: string): string {
  const normalized = value.replace(/\s+/g, " ").trim();
  return normalized.length > 160 ? `${normalized.slice(0, 157)}...` : normalized;
}

function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

function inherited(value: string | null | undefined): string {
  return value?.trim() ? `(${value})` : "(system)";
}

function networkMode(value: boolean | null): string {
  if (value === true) {
    return "enabled";
  }
  if (value === false) {
    return "disabled";
  }
  return "default";
}

function networkLabel(value: boolean | null): string {
  if (value === true) {
    return "enabled";
  }
  if (value === false) {
    return "disabled";
  }
  return "system";
}

createRoot(document.getElementById("root")!).render(<App />);
