export type ProjectItem = {
  id: string;
  name: string;
  rootPath: string;
  defaultCwd: string;
  isSelected: boolean;
};

export type ThreadItem = {
  id: string;
  title: string;
  role: string;
  projectName: string;
  preview: string;
  isRunning: boolean;
  unreadCount: number;
};

export type ChatEntry = {
  id: string;
  author: string;
  displayLabel: string;
  timestamp: number | null;
  body: string;
  subtitle: string | null;
  kind: string | null;
  status: string | null;
  processId: string | null;
  command: string | null;
  output: string | null;
  deliveryState: string | null;
  isStreaming: boolean;
  isTool: boolean;
};

export type PendingApprovalItem = {
  id: string;
  threadId: string;
  kind: string;
  title: string;
  detail: string | null;
  command: string | null;
  commandCwd: string | null;
  filePaths: string[];
};

export type LiveProcessItem = {
  processId: string;
  pid: number | null;
  processGroupId: number | null;
  command: string;
  cwd: string | null;
  startedAt: number | null;
};

export type WorkspaceSelection = {
  projectId: string | null;
  projectRootPath: string | null;
  projectOrchestratorThreadId: string | null;
  projectOrchestratorName: string | null;
  threadId: string | null;
  threadRole: string | null;
  projectName: string;
  threadName: string;
  connectionLabel: string;
  sandboxMode: string | null;
  networkAccess: boolean | null;
  approvalPolicy: string | null;
  model: string | null;
  reasoningEffort: string | null;
  serviceTier: string | null;
  effectiveSandboxMode: string | null;
  effectiveNetworkAccess: boolean | null;
  effectiveApprovalPolicy: string | null;
  effectiveModel: string | null;
  effectiveReasoningEffort: string | null;
  effectiveServiceTier: string | null;
  isRunning: boolean;
};

export type WorkbenchViewData = {
  projects: ProjectItem[];
  selection: WorkspaceSelection;
  threads: ThreadItem[];
  liveProcesses: LiveProcessItem[];
  chatEntries: ChatEntry[];
  contextWindowRemainingPercent: number | null;
  pendingApprovals: PendingApprovalItem[];
  statusHeadline: string;
  statusDetail: string;
  composerHint: string;
};

export type ThreadMessagesPayload = {
  threadId?: string;
  threadID?: string;
  messages?: unknown[];
  contextWindowRemainingPercent?: number | null;
  contextWindowStatus?: {
    remainingPercent?: number | null;
  } | null;
};

export type WorkbenchEventEnvelope = {
  type: string;
  payload?: {
    sequence?: number | null;
    event?: {
      name?: string;
      data?: unknown;
      message?: string;
    };
  };
};
