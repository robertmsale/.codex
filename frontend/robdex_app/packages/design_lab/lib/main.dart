import 'dart:ui';

import 'package:flutter/material.dart';
import 'package:robdex_app/robdex_app.dart';
import 'package:robdex_design_system/robdex_design_system.dart';

void main() {
  runApp(const RobdexDesignLabApp());
}

class RobdexDesignLabApp extends StatelessWidget {
  const RobdexDesignLabApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      debugShowCheckedModeBanner: false,
      title: 'Robdex Design Lab',
      theme: buildRobdexTheme(),
      home: const RobdexDesignLabHome(),
    );
  }
}

class RobdexDesignLabHome extends StatelessWidget {
  const RobdexDesignLabHome({super.key});

  @override
  Widget build(BuildContext context) {
    const defaultSurface = String.fromEnvironment('DESIGN_LAB_INSPECTOR', defaultValue: 'shell');
    final surface = Uri.base.queryParameters['surface'] ?? defaultSurface;
    final workbench = mockWorkbenchData.copyWith(
      selection: const WorkspaceSelection(
        projectId: 'project-codex-home',
        projectRootPath: '/Users/robertsale/.codex',
        projectOrchestratorThreadId: null,
        projectOrchestratorName: null,
        threadId: 'config-operator',
        threadRole: 'operator',
        projectName: '.codex',
        threadName: 'Codex Config Operator',
        connectionLabel: 'Bridge Connected',
        sandboxMode: 'danger-full-access',
        networkAccess: true,
        approvalPolicy: 'on-request',
        model: 'gpt-5',
        reasoningEffort: 'medium',
        serviceTier: null,
        isRunning: false,
      ),
      workerMetadata: const WorkerMetadata(threadId: 'config-operator'),
    );
    final genericWorkbench = _cleanRobdexGenericWorkbench(workbench);
    if (surface == 'stats') {
      return const Scaffold(
        body: Center(
          child: ThreadStatsModalView(stats: _mockThreadStats),
        ),
      );
    }
    if (surface == 'weeklyStats') {
      return const Scaffold(
        body: Center(
          child: SizedBox(
            width: 920,
            child: PeriodStatsView(stats: _mockPeriodStats),
          ),
        ),
      );
    }
    if (surface == 'brushedMetalShader') {
      return const Scaffold(
        backgroundColor: Color(0xFF05090F),
        body: Center(
          child: SizedBox(
            width: 520,
            height: 520,
            child: _BrushedMetalShaderSpecimen(),
          ),
        ),
      );
    }
    if (surface == 'sidebar') {
      return Scaffold(
        body: SizedBox.expand(
          child: ThreadListPanel(
            selection: workbench.selection,
            projects: workbench.projects,
            threads: workbench.threads,
            pendingApprovals: workbench.pendingApprovals,
            onDisconnect: () {},
            onGlobalSettings: () {},
            onThreadSelected: (_) {},
            onCreateProject: () {},
            onProjectSettings: (_) {},
            onCreateThread: (_) {},
            onSpawnAgent: () {},
            onWeeklyStats: () {},
          ),
        ),
      );
    }
    if (surface == 'inspector') {
      return Scaffold(
        body: Align(
          alignment: Alignment.topCenter,
          child: SizedBox(
            width: 480,
            height: 1800,
            child: InspectorPanel(
              selection: workbench.selection,
              availableModels: workbench.availableModels,
              threadGroups: const [],
              workerMetadata: workbench.workerMetadata,
              requirementReview: workbench.requirementReview,
              loadRequirementComposables: null,
              setThreadRequirements: null,
              uploadImageBytes: null,
              onOpenThread: (_) {},
              onSettingsChanged: (_) {},
              onRunningStateChanged: (_) {},
              onRenameThread: (_) {},
              onArchiveThread: () {},
              onWarmHandoff: (_) {},
              onCreateThreadGroup: (_) {},
              onRenameThreadGroup: (_) async {},
              onDeleteThreadGroup: (_) {},
              onArchiveThreadGroup: (_) {},
              onMoveSelectedThreadToGroup: (_) {},
              onUpdateWorkerMetadata: (_) {},
            ),
          ),
        ),
      );
    }
    if (surface == 'agentRuntimeDisconnected') {
      return _AgentRuntimeScenario(data: mockAgentRuntimeDisconnected);
    }
    if (surface == 'robdexGenericConversationShell') {
      return ConversationShellScreen(
        data: workbenchConversationShellData(genericWorkbench),
        onSessionSelected: (_) {},
        onCreateSession: () {},
        onSendMessage: (_) {},
        onInterrupt: () {},
        onProjectSelected: (_) {},
        onSettings: () {},
      );
    }
    if (surface == 'agentRuntimeConnectedEmpty') {
      return _AgentRuntimeScenario(data: mockAgentRuntimeEmpty);
    }
    if (surface == 'agentRuntimeSelectedSessionTranscript') {
      return _AgentRuntimeScenario(data: mockAgentRuntimeConnected);
    }
    if (surface == 'agentRuntimeStarterKitImageEvidence') {
      return _AgentRuntimeScenario(data: _starterKitImageEvidenceData(), focusSurfaceId: 'imageArtifacts');
    }
    if (surface == 'agentRuntimeCreateSessionModal') {
      return Scaffold(
        backgroundColor: const Color(0xFF0E141B),
        body: Center(
          child: AgentRuntimeCreateSessionDialog(
            shell: agentRuntimeConversationShellData(mockAgentRuntimeConnected),
            data: mockAgentRuntimeConnected,
            initialProjectId: 'project-a',
            onCreate: ({required role, required project, required model, required workdir, required worktreeRoot, required title, required name}) {},
          ),
        ),
      );
    }
    if (surface == 'agentRuntimeCreateProjectModal') {
      return Scaffold(
        backgroundColor: const Color(0xFF0E141B),
        body: Center(
          child: AgentRuntimeCreateProjectDialog(
            data: mockAgentRuntimeConnected,
            existingProjectKeys: const ['project-a'],
            onCreate: ({
              required projectKey,
              required displayName,
              required defaultWorkdir,
              required defaultWorktreeRoot,
              required defaultRoleId,
              required defaultModel,
            }) {},
          ),
        ),
      );
    }
    if (surface == 'agentRuntimeProjectSettingsModal') {
      return Scaffold(
        backgroundColor: const Color(0xFF0E141B),
        body: Center(
          child: AgentRuntimeProjectSettingsDialog(
            data: mockAgentRuntimeConnected,
            projectId: 'project-a',
            project: const ConversationProject(
              id: 'project-a',
              title: 'Runtime',
              defaultWorkdir: '/Users/robertsale/.codex',
              defaultWorktreeRoot: '/Users/robertsale/.codex',
              defaultRoleId: 'runtime-allow',
              defaultModel: 'codex-live-model',
            ),
            onSave: ({
              required projectKey,
              required displayName,
              required defaultWorkdir,
              required defaultWorktreeRoot,
              required defaultRoleId,
              required defaultModel,
            }) {},
            onArchive: (_) {},
            onUnarchive: (_) {},
          ),
        ),
      );
    }
    if (surface == 'agentRuntimeSessionSettingsModal') {
      return AgentRuntimeSessionControlPlane(
        data: mockAgentRuntimeConnected,
        onClose: () {},
        onSave: ({
          required sessionId,
          required project,
          required role,
          required model,
          required workdir,
          required worktreeRoot,
          required title,
          required name,
          required tracked,
        }) {},
        onCloseSession: (_) {},
        onArchiveSession: (_) {},
        onForkSession: (_) {},
        onCompact: (_) {},
        onGrantGodMode: (_) {},
        onRevokeGodMode: (_) {},
        onTerminateProcess: (_) {},
        onFlushProcess: (_) {},
        onInputProcess: (_, _) {},
        onApprove: (_, _) {},
        onDeny: (_, _) {},
        onResumeApproval: (_) {},
        onPreviewCommandRequest: (_) {},
        onApproveCommandRequest: (_) {},
        onDenyCommandRequest: (_) {},
        onApplyCommandRequest: (_) {},
        onShowCommand: (_) {},
        onShowCommandRequest: (_) {},
        onSetRequirements: (_, {required title, required key, required statement}) {},
      );
    }
    if (surface == 'agentRuntimeGlobalSettingsModal') {
      return Scaffold(
        backgroundColor: const Color(0xFF0E141B),
        body: Center(
          child: AgentRuntimeGlobalSettingsDialog(
            data: mockAgentRuntimeConnected.copyWith(errorMessage: 'Connection failed. Check the runtime URL and try again.'),
            onConnectManual: (_) {},
            onRefreshDiscovery: () {},
            onConnectDiscovery: () {},
            onRefreshIcloud: () {},
            onConnectIcloud: () {},
            onImportProfile: () {},
            onRefreshImportedProfile: () {},
            onConnectImportedProfile: () {},
            onDisconnect: () {},
          ),
        ),
      );
    }
    if (surface == 'agentRuntimeRoleManagementDetail') {
      return Scaffold(
        backgroundColor: const Color(0xFF05090F),
        body: AgentRuntimeRoleManagerPage(
          data: mockAgentRuntimeConnected.roleAdmin,
          onClose: () {},
          onValidate: (_) {},
          onCreate: (_) {},
          onUpdate: (_) {},
          onExport: (_) {},
          onArchive: (_) {},
          onUnarchive: (_) {},
          onActivate: (_, _) {},
          onShowDetail: (_) {},
          onShowVersions: (_) {},
          onShowVersionData: (_) {},
        ),
      );
    }
    if (surface == 'agentRuntimeOperationsDetail') {
      return _AgentRuntimeScenario(data: mockAgentRuntimeConnected);
    }
    if (surface == 'agentRuntimeGodModeSessionSurface') {
      return _AgentRuntimeScenario(data: mockAgentRuntimeConnected, focusSurfaceId: 'session');
    }
    if (surface == 'agentRuntimeGodModeSessionDetailOnly') {
      return Scaffold(
        backgroundColor: const Color(0xFF0E141B),
        body: SafeArea(
          child: Center(
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 460),
              child: AgentRuntimeOperationsDetail(
                data: mockAgentRuntimeConnected,
                focusSurfaceId: 'session',
                onSessionClose: (_) {},
                onSessionArchive: (_) {},
                onSessionFork: (_) {},
                onGodModeGrant: (_) {},
                onGodModeRevoke: (_) {},
                onProcessTerminate: (_) {},
                onProcessInput: (_, _) {},
                onProcessFlush: (_) {},
                onCompactSession: (_) {},
                onApprovalApprove: (_, _) {},
                onApprovalDeny: (_, _) {},
                onApprovalResume: (_) {},
                onCommandRegistryApprove: (_, _) {},
                onCommandRegistryDeny: (_, _) {},
                onCommandRegistryPreview: (_, _) {},
                onCommandRegistryApply: (_) {},
                onCommandRegistryReview: (_) {},
                onCommandRegistryShowCommand: (_, _, _) {},
                onCommandRegistryListInstalled: (_, _) {},
                onCommandRegistryListRequests: () {},
              ),
            ),
          ),
        ),
      );
    }
    if (surface == 'agentRuntimeActiveResponse') {
      return _AgentRuntimeScenario(
        data: mockAgentRuntimeConnected.copyWith(
          timeline: const [
            AgentRuntimeTimelineItem(id: 'active-user', title: 'Owner', subtitle: 'Check the workspace status.', status: 'sent', tone: 'user'),
            AgentRuntimeTimelineItem(id: 'active-assistant', title: 'Assistant', subtitle: 'Reviewing the latest runtime state…', status: 'running', tone: 'success'),
          ],
        ),
      );
    }
    if (surface == 'agentRuntimeHistorySurface') {
      return _AgentRuntimeScenario(data: mockAgentRuntimeConnected, focusSurfaceId: 'history');
    }
    if (surface == 'agentRuntimeProcessManagerSurface') {
      return _AgentRuntimeScenario(data: mockAgentRuntimeConnected, focusSurfaceId: 'processManager');
    }
    if (surface == 'agentRuntimeCompactionSurface') {
      return _AgentRuntimeScenario(data: mockAgentRuntimeConnected, focusSurfaceId: 'compaction');
    }
    if (surface == 'agentRuntimeCompactionUnavailableSurface') {
      return _AgentRuntimeScenario(
        data: mockAgentRuntimeConnected.copyWith(
          operationSurfaces: const [
            AgentRuntimeOperationSurface(
              surfaceId: 'compaction',
              title: 'Compaction',
              subtitle: 'Checkpoint and context budget',
              rows: [
                AgentRuntimeFact(label: 'Checkpoints', value: 'No completed or failed compaction checkpoints'),
                AgentRuntimeFact(label: 'Current context estimate', value: 'Runtime projection supplies estimate data when available'),
                AgentRuntimeFact(label: 'Compaction thresholds', value: 'Runtime-owned budget thresholds apply'),
              ],
              actions: [
                AgentRuntimeActionItem(
                  id: 'compact-session-unavailable',
                  title: 'Compact selected session',
                  subtitle: 'Select a session before compacting history.',
                  kind: 'compactionUnavailable',
                  stateText: 'No selected session',
                  tone: 'muted',
                ),
              ],
            ),
          ],
        ),
        focusSurfaceId: 'compaction',
      );
    }
    if (surface == 'agentRuntimeStatisticsSurface') {
      return _AgentRuntimeScenario(data: mockAgentRuntimeConnected, focusSurfaceId: 'statistics');
    }
    if (surface == 'agentRuntimeWorkflowMemorySurface') {
      return _AgentRuntimeScenario(data: mockAgentRuntimeConnected, focusSurfaceId: 'workflowMemory');
    }
    if (surface == 'agentRuntimeApprovalsSurface') {
      return _AgentRuntimeScenario(data: mockAgentRuntimeConnected, focusSurfaceId: 'approvals');
    }
    if (surface == 'agentRuntimeCommandRegistrySurface') {
      return _AgentRuntimeScenario(data: mockAgentRuntimeConnected, focusSurfaceId: 'commandRegistry');
    }
    if (surface == 'agentRuntimeDynamicCustomRole') {
      return _AgentRuntimeScenario(
        data: mockAgentRuntimeConnected.copyWith(
          sessions: const [
            AgentRuntimeSessionItem(
              id: 'session-custom',
              title: 'Incident review',
              status: 'open',
              subtitle: 'Project workspace',
              groupLabel: 'Neon Incident Commander',
              tone: 'warning',
            ),
          ],
        ),
      );
    }
    if (surface == 'agentRuntimeLiveValidation') {
      return ConversationShellScreen(
        data: _agentRuntimeLiveValidationShellData(),
        onSessionSelected: (_) {},
        onCreateSession: () {},
        onSendMessage: (_) {},
        onInterrupt: () {},
        onCloseSession: (_) {},
        onArchiveSession: (_) {},
        onForkSession: (_) {},
        onProjectSelected: (_) {},
        onSettings: () {},
      );
    }
    if (surface == 'agentRuntimeCompactShell') {
      return _AgentRuntimeScenario(data: mockAgentRuntimeConnected);
    }
    if (surface == 'agentRuntimeConnecting') {
      return _AgentRuntimeScenario(data: mockAgentRuntimeConnecting);
    }
    if (surface == 'agentRuntimeConnected') {
      return _AgentRuntimeScenario(data: mockAgentRuntimeConnected);
    }
    if (surface == 'agentRuntimeError') {
      return _AgentRuntimeScenario(data: mockAgentRuntimeError);
    }
    if (surface == 'agentRuntimeEmpty') {
      return _AgentRuntimeScenario(data: mockAgentRuntimeEmpty);
    }
    return Scaffold(
      body: RobdexShellScreen(
        enableGraphics: true,
        workbench: workbench,
        onThreadSelected: (_) {},
        onProjectSelected: (_) {},
        onDisconnect: () {},
        onGlobalSettings: () {},
        onCreateProject: () {},
        onProjectSettings: (_) {},
        onCreateThread: (_) {},
        onSpawnAgent: () {},
        onSendMessage: (_) {},
        onOpenHistory: () {},
        onCompactThread: () {},
        onTerminateCommandExecution: (_) {},
        onInterruptThread: () {},
        onApprovalDecision: (_, _, _) async {},
        onSettingsChanged: (_) {},
        onRunningStateChanged: (_) {},
        onRenameThread: (_) {},
        onArchiveThread: () {},
        onWarmHandoff: (_) {},
        onSetProjectOrchestrator: () {},
        onCreateThreadGroup: (_) {},
        onRenameThreadGroup: (_) async {},
        onDeleteThreadGroup: (_) {},
        onArchiveThreadGroup: (_) {},
        onMoveSelectedThreadToGroup: (_) {},
        onUpdateWorkerMetadata: (_) {},
        loadThreadStats: (_) async => _mockThreadStats,
        loadPeriodStats: (_) async => _mockPeriodStats,
        loadFullSizeImage: (_) async => const FullSizeImageData(
          path: '/tmp/robdex-design-lab-static.png',
          bytesBase64: '',
          contentType: 'image/png',
        ),
      ),
    );
  }
}

WorkbenchViewData _cleanRobdexGenericWorkbench(WorkbenchViewData workbench) {
  return workbench.copyWith(
    selection: const WorkspaceSelection(
      projectId: 'project-codex-home',
      projectRootPath: null,
      projectOrchestratorThreadId: null,
      projectOrchestratorName: null,
      threadId: 'config-operator',
      threadRole: 'operator',
      projectName: '.codex',
      threadName: 'Codex Config Operator',
      connectionLabel: 'Bridge Connected',
      isRunning: false,
    ),
    threads: const [
      ThreadItem(
        id: 'config-operator',
        title: 'Config Operator',
        role: 'operator',
        projectName: '.codex',
        preview: 'Bridge connected. Ready for owner instructions.',
        isRunning: false,
        unreadCount: 0,
        requirementReview: null,
      ),
      ThreadItem(
        id: 'approval-worker',
        title: 'Approval Worker',
        role: 'worker',
        projectName: '.codex',
        preview: 'Waiting on owner approval.',
        isRunning: false,
        unreadCount: 1,
        requirementReview: null,
      ),
      ThreadItem(
        id: 'qa-review',
        title: 'QA Review',
        role: 'qa',
        projectName: '.codex',
        preview: 'Reviewing the latest bridge changes.',
        isRunning: false,
        unreadCount: 0,
        requirementReview: null,
      ),
    ],
    chatEntries: const [
      ChatEntry(
        id: 'owner-1',
        author: 'User',
        displayLabel: 'User',
        timestamp: null,
        body: 'Please keep the bridge stable while this conversation continues.',
      ),
      ChatEntry(
        id: 'operator-1',
        author: 'operator',
        displayLabel: 'Config Operator',
        timestamp: null,
        body: 'Bridge connection is stable. The selected thread and composer are ready.',
      ),
      ChatEntry(
        id: 'approval-1',
        author: 'system',
        displayLabel: 'Approval',
        timestamp: null,
        body: 'One command is waiting for owner approval.',
        status: 'pending',
        isTool: true,
      ),
    ],
    statusHeadline: 'Bridge connected',
    statusDetail: 'Live project and session state are synchronized.',
    composerHint: 'Message selected thread...',
  );
}

ConversationShellData _agentRuntimeLiveValidationShellData() {
  const sessionId = String.fromEnvironment('AGENT_RUNTIME_LIVE_VALIDATION_SESSION_ID', defaultValue: 'live-ui-validation-session');
  const turnId = String.fromEnvironment('AGENT_RUNTIME_LIVE_VALIDATION_TURN_ID', defaultValue: 'live-ui-validation-turn');
  const response = String.fromEnvironment('AGENT_RUNTIME_LIVE_VALIDATION_RESPONSE', defaultValue: 'Runtime response rendered.');
  return const ConversationShellData(
    appTitle: 'Agent Runtime',
    connectionLabel: 'Live validation connected',
    projects: [ConversationProject(id: 'runtime', title: 'Runtime', subtitle: 'Live validation')],
    sessions: [
      ConversationSession(
        id: sessionId,
        title: 'Live validation session',
        subtitle: 'Created and selected through runtime validation',
        role: 'Runtime',
        selected: true,
        rolePresentation: ConversationRolePresentation(
          roleId: 'runtime',
          displayLabel: 'Runtime',
          shortLabel: 'RT',
          iconKey: 'runtime',
          tone: 'success',
          statusLabel: 'Connected',
          description: 'Live runtime validation',
        ),
      ),
    ],
    selectedSessionId: sessionId,
    timelineTitle: 'Live validation session',
    entries: [
      ChatEntry(
        id: 'live-validation-user',
        author: 'User',
        displayLabel: 'User',
        timestamp: null,
        body: 'Check the runtime health and answer briefly.',
      ),
      ChatEntry(
        id: turnId,
        author: 'Assistant',
        displayLabel: 'Assistant',
        timestamp: null,
        body: response,
      ),
    ],
    composerEnabled: true,
    isRunning: false,
    detailTitle: 'Validation proof',
    detailSections: [
      ConversationDetailSection(
        title: 'Live path',
        rows: [
          ConversationDetailRow(label: 'Connect', value: 'Completed'),
          ConversationDetailRow(label: 'Session', value: 'Created and selected'),
          ConversationDetailRow(label: 'Send', value: 'Completed'),
          ConversationDetailRow(label: 'Response', value: 'Rendered in ChatTimeline'),
        ],
      ),
    ],
    emptyTitle: 'No session selected',
    emptyText: 'Create or select a session.',
    projectLabel: 'Projects',
    sessionLabel: 'Sessions',
    composerPlaceholder: 'Message selected session...',
    composerDisabledHint: 'Select a session to enable the composer.',
  );
}

class _AgentRuntimeScenario extends StatefulWidget {
  const _AgentRuntimeScenario({required this.data, this.focusSurfaceId});

  final AgentRuntimeWorkbenchData data;
  final String? focusSurfaceId;

  @override
  State<_AgentRuntimeScenario> createState() => _AgentRuntimeScenarioState();
}

class _AgentRuntimeScenarioState extends State<_AgentRuntimeScenario> {
  late final TextEditingController _baseUrlController;

  @override
  void initState() {
    super.initState();
    _baseUrlController = TextEditingController(text: widget.data.baseUrl);
  }

  @override
  void dispose() {
    _baseUrlController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (_hasConnectedRuntime(widget.data)) {
      final shell = agentRuntimeConversationShellData(widget.data);
      final narrow = MediaQuery.sizeOf(context).width < 560;
      final operations = AgentRuntimeOperationsDetail(
        data: widget.data,
        focusSurfaceId: widget.focusSurfaceId,
        onRoleValidate: (_) {},
        onRoleCreate: (_) {},
        onRoleUpdate: (_) {},
        onRoleExport: (_) {},
        onRoleArchive: (_) {},
        onRoleUnarchive: (_) {},
        onRoleActivate: (_, _) {},
        onWorkflowMemorySelect: (_) {},
        onWorkflowMemoryAttempted: (_) {},
        onWorkflowMemoryHelpful: (_) {},
        onWorkflowMemoryNotHelpful: (_) {},
        onSessionClose: (_) {},
        onSessionArchive: (_) {},
        onSessionFork: (_) {},
        onProcessTerminate: (_) {},
        onProcessInput: (_, _) {},
        onProcessFlush: (_) {},
        onCompactSession: (_) {},
        onGodModeGrant: (_) {},
        onGodModeRevoke: (_) {},
        onApprovalApprove: (_, _) {},
        onApprovalResume: (_) {},
        onCommandRegistryApprove: (_, _) {},
        onCommandRegistryApply: (_) {},
      );
      return Stack(
        children: [
          ConversationShellScreen(
            data: shell,
            onSessionSelected: (_) {},
            onCreateSession: () {},
            onSendMessage: (_) {},
            onInterrupt: () {},
            onCloseSession: (_) {},
            onArchiveSession: (_) {},
            onForkSession: (_) {},
            onProjectSelected: (_) {},
            onSettings: () {},
            showPermanentDetail: false,
            headerControls: Wrap(
              spacing: 6,
              children: [
                IconButton(
                  tooltip: 'Session settings',
                  onPressed: () {},
                  icon: const Icon(Icons.tune_rounded, size: 18),
                ),
                IconButton(
                  tooltip: 'Runtime operations',
                  onPressed: () {},
                  icon: const Icon(Icons.manage_history_rounded, size: 18),
                ),
                IconButton(
                  tooltip: 'Global settings',
                  onPressed: () {},
                  icon: const Icon(Icons.settings_rounded, size: 18),
                ),
                IconButton(
                  tooltip: 'Disconnect',
                  onPressed: () {},
                  icon: const Icon(Icons.link_off_rounded, size: 18),
                ),
              ],
            ),
          ),
          if (widget.focusSurfaceId != null)
            Positioned(
              right: narrow ? 12 : 24,
              left: narrow ? 12 : null,
              top: 72,
              bottom: 24,
              width: narrow ? null : 420,
              child: Material(
                elevation: 18,
                color: const Color(0xFF111820),
                borderRadius: BorderRadius.circular(18),
                clipBehavior: Clip.antiAlias,
                child: operations,
              ),
            ),
        ],
      );
    }
    return Scaffold(
      body: AgentRuntimeWorkbench(
        data: widget.data,
        baseUrlController: _baseUrlController,
        onConnect: () {},
        onRefreshDiscovery: () {},
        onConnectDiscovered: () {},
        onRefreshIcloudRemoteDiscovery: () {},
        onConnectIcloudRemote: () {},
        onImportRemoteProfile: () {},
        onRefreshImportedRemoteProfile: () {},
        onConnectImportedRemoteProfile: () {},
        onDisconnect: () {},
        onRoleValidate: (_) {},
        onRoleCreate: (_) {},
        onRoleUpdate: (_) {},
        onRoleExport: (_) {},
        onRoleArchive: (_) {},
        onRoleUnarchive: (_) {},
        onRoleActivate: (_, _) {},
        onWorkflowMemorySelect: (_) {},
        onWorkflowMemoryAttempted: (_) {},
        onWorkflowMemoryHelpful: (_) {},
        onWorkflowMemoryNotHelpful: (_) {},
        onSessionClose: (_) {},
        onSessionArchive: (_) {},
        onSessionFork: (_) {},
        onProcessTerminate: (_) {},
        onProcessInput: (_, _) {},
        onProcessFlush: (_) {},
      ),
    );
  }
}

bool _hasConnectedRuntime(AgentRuntimeWorkbenchData data) {
  return data.connectionState != 'disconnected' && data.connectionState != 'connecting' && data.connectionState != 'failed';
}

AgentRuntimeWorkbenchData _starterKitImageEvidenceData() {
  const previewPng =
      'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/p9sAAAAASUVORK5CYII=';
  return mockAgentRuntimeConnected.copyWith(
    selectedSessionLabel: 'Starter kit evidence',
    timelineTitle: 'Starter kit evidence',
    timelineSubtitle: 'Image artifact, managed server, and Requirements evidence projected from runtime state',
    detailTitle: 'Image artifacts',
    detailSubtitle: 'Runtime-owned evidence handles',
    sessions: const [
      AgentRuntimeSessionItem(
        id: 'starter-session',
        title: 'Starter kit evidence',
        status: 'open',
        subtitle: 'Worker tools · Project workspace',
        groupLabel: 'Open',
        tone: 'success',
      ),
      AgentRuntimeSessionItem(
        id: 'review-session',
        title: 'Requirements review',
        status: 'open',
        subtitle: 'Evidence ready for reviewer',
        groupLabel: 'Attention',
        tone: 'warning',
      ),
    ],
    timeline: const [
      AgentRuntimeTimelineItem(
        id: 'starter-file',
        title: 'File tools',
        subtitle: 'CWD-relative read and described patch completed',
        status: 'completed',
        tone: 'success',
      ),
      AgentRuntimeTimelineItem(
        id: 'starter-server',
        title: 'Managed server',
        subtitle: 'Runtime allocated PORT and projected the running URL',
        status: 'stopped',
        tone: 'info',
      ),
      AgentRuntimeTimelineItem(
        id: 'starter-image',
        title: 'Image artifact',
        subtitle: 'Screenshot evidence · PNG · attached to Requirements evidence',
        status: 'available',
        tone: 'success',
      ),
      AgentRuntimeTimelineItem(
        id: 'starter-requirements',
        title: 'Requirements evidence',
        subtitle: 'Reviewer receives artifact handle and thumbnail metadata, not a local path',
        status: 'ready',
        tone: 'warning',
      ),
    ],
    selectedConversation: const [
      ChatEntry(
        id: 'starter-user',
        author: 'User',
        displayLabel: 'Owner',
        timestamp: null,
        body: 'Capture starter-kit evidence.',
        status: 'sent',
      ),
      ChatEntry(
        id: 'starter-tool-image',
        author: 'Tool',
        displayLabel: 'Tool',
        timestamp: null,
        body: 'Image artifact captured',
        subtitle: 'Screenshot evidence · image/png · 1 KB',
        kind: 'image.capture_from_file',
        status: 'completed',
        output: 'Artifact handle stored; binary content stays outside transcript text.',
        imagePreviewBase64: previewPng,
        imagePreviewContentType: 'image/png',
        isTool: true,
      ),
      ChatEntry(
        id: 'starter-assistant',
        author: 'Assistant',
        displayLabel: 'Assistant',
        timestamp: null,
        body: 'Screenshot evidence is ready with reviewed viewport metadata.',
        status: 'completed',
      ),
    ],
    operationSurfaces: [
      ...mockAgentRuntimeConnected.operationSurfaces.where((surface) => surface.surfaceId != 'imageArtifacts'),
      const AgentRuntimeOperationSurface(
        surfaceId: 'imageArtifacts',
        title: 'Image artifacts',
        subtitle: 'Selected session evidence',
        rows: [
          AgentRuntimeFact(label: 'Artifact', value: 'Screenshot evidence image'),
          AgentRuntimeFact(label: 'Capture method', value: 'Design Lab Bun WebView'),
          AgentRuntimeFact(label: 'Viewport', value: '1366 × 1024'),
          AgentRuntimeFact(label: 'Reviewed flow', value: 'Selected session timeline and Requirements evidence'),
          AgentRuntimeFact(label: 'Reviewer delivery', value: 'Image artifact attachment'),
          AgentRuntimeFact(label: 'Transcript boundary', value: 'Binary bytes are not copied into chat text'),
        ],
        actions: [],
      ),
    ],
    controllerFacts: const [
      AgentRuntimeFact(label: 'Selected session', value: 'Starter kit evidence'),
      AgentRuntimeFact(label: 'Image artifacts', value: '1 available'),
      AgentRuntimeFact(label: 'Requirements evidence', value: 'Artifact attached'),
    ],
  );
}

class _BrushedMetalShaderSpecimen extends StatefulWidget {
  const _BrushedMetalShaderSpecimen();

  @override
  State<_BrushedMetalShaderSpecimen> createState() => _BrushedMetalShaderSpecimenState();
}

class _BrushedMetalShaderSpecimenState extends State<_BrushedMetalShaderSpecimen> {
  FragmentShader? _shader;

  @override
  void initState() {
    super.initState();
    _loadShader();
  }

  Future<void> _loadShader() async {
    try {
      final program = await FragmentProgram.fromAsset(
        'packages/robdex_design_system/shaders/brushed_metal_sidebar.frag',
      );
      if (!mounted) {
        return;
      }
      setState(() {
        _shader = program.fragmentShader();
      });
    } catch (_) {}
  }

  @override
  void dispose() {
    _shader?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(24),
        border: Border.all(color: Colors.white.withValues(alpha: 0.14)),
        boxShadow: [
          BoxShadow(
            color: Colors.black.withValues(alpha: 0.36),
            blurRadius: 36,
            offset: const Offset(0, 18),
          ),
        ],
      ),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(24),
        child: CustomPaint(
          painter: _BrushedMetalShaderSpecimenPainter(shader: _shader),
          child: const SizedBox.expand(),
        ),
      ),
    );
  }
}

class _BrushedMetalShaderSpecimenPainter extends CustomPainter {
  const _BrushedMetalShaderSpecimenPainter({required this.shader});

  final FragmentShader? shader;

  @override
  void paint(Canvas canvas, Size size) {
    final rect = Offset.zero & size;
    canvas.drawRect(
      rect,
      Paint()
        ..shader = const LinearGradient(
          begin: Alignment.topLeft,
          end: Alignment.bottomRight,
          colors: [
            Color(0xFF121923),
            Color(0xFF071018),
            Color(0xFF101821),
          ],
          stops: [0.0, 0.56, 1.0],
        ).createShader(rect),
    );

    final activeShader = shader;
    if (activeShader != null) {
      activeShader.setFloat(0, size.width);
      activeShader.setFloat(1, size.height);
      activeShader.setFloat(2, 0.0);
      canvas.drawRect(rect, Paint()..shader = activeShader);
    }
  }

  @override
  bool shouldRepaint(covariant _BrushedMetalShaderSpecimenPainter oldDelegate) {
    return oldDelegate.shader != shader;
  }
}

const _mockThreadStats = ThreadStatsData(
  threadId: 'config-operator',
  sessionPath: '/Users/robertsale/.codex/sessions/2026/05/30/rollout-config-operator.jsonl',
  generatedAtMs: 1780185600000,
  totals: TokenTotals(
    inputTokens: 1842000,
    uncachedInputTokens: 940000,
    outputTokens: 318000,
    cachedInputTokens: 902000,
    reasoningOutputTokens: 74500,
    totalTokens: 2160000,
  ),
  estimates: TokenEstimates(
    userMessageInputTokens: 128000,
    toolOutputInputTokens: 498000,
    toolCallOutputTokens: 62000,
    skillInstructionInputTokens: 39000,
  ),
  compactionCount: 7,
  timeline: [
    TokenTimelinePoint(index: 1, line: 32, inputTokens: 12000, uncachedInputTokens: 12000, outputTokens: 1800, cachedInputTokens: 0, reasoningOutputTokens: 420, totalTokens: 14220, deltaTokens: 14220),
    TokenTimelinePoint(index: 2, line: 104, inputTokens: 54000, uncachedInputTokens: 40000, outputTokens: 9200, cachedInputTokens: 14000, reasoningOutputTokens: 1800, totalTokens: 65220, deltaTokens: 51000),
    TokenTimelinePoint(index: 3, line: 226, inputTokens: 188000, uncachedInputTokens: 111000, outputTokens: 25500, cachedInputTokens: 77000, reasoningOutputTokens: 7200, totalTokens: 208920, deltaTokens: 143700),
    TokenTimelinePoint(index: 4, line: 488, inputTokens: 476000, uncachedInputTokens: 262000, outputTokens: 85000, cachedInputTokens: 214000, reasoningOutputTokens: 22100, totalTokens: 578020, deltaTokens: 369100),
    TokenTimelinePoint(index: 5, line: 772, inputTokens: 988000, uncachedInputTokens: 486000, outputTokens: 176000, cachedInputTokens: 502000, reasoningOutputTokens: 40100, totalTokens: 1280120, deltaTokens: 702100),
    TokenTimelinePoint(index: 6, line: 1008, inputTokens: 1842000, uncachedInputTokens: 940000, outputTokens: 318000, cachedInputTokens: 902000, reasoningOutputTokens: 74500, totalTokens: 2612620, deltaTokens: 1332500),
  ],
  categories: [
    TokenCategoryBreakdown(key: 'tool_output', label: 'Tool outputs', tokens: 498000, estimated: true),
    TokenCategoryBreakdown(key: 'user_message', label: 'User messages', tokens: 128000, estimated: true),
    TokenCategoryBreakdown(key: 'tool_call', label: 'Tool call inputs', tokens: 62000, estimated: true),
    TokenCategoryBreakdown(key: 'assistant_message', label: 'Assistant messages', tokens: 88000, estimated: true),
    TokenCategoryBreakdown(key: 'skill_instruction', label: 'Skill instructions', tokens: 39000, estimated: true),
  ],
  topItems: [
    TokenTopItem(label: 'Tool output cargo test', kind: 'tool_output', line: 772, tokens: 82000, estimated: true),
    TokenTopItem(label: 'Tool output flutter analyze', kind: 'tool_output', line: 1008, tokens: 58000, estimated: true),
    TokenTopItem(label: 'User message', kind: 'user_message', line: 16, tokens: 27000, estimated: true),
    TokenTopItem(label: 'Skill instructions', kind: 'skill_instruction', line: 2, tokens: 18000, estimated: true),
  ],
  warnings: [
    'line 702: token attribution is estimated for tool payloads.',
  ],
);

const _mockPeriodStats = PeriodStatsData(
  label: 'Weekly quota attribution',
  startMs: 1780446600000,
  endMs: 1781029200000,
  generatedAtMs: 1781029200000,
  sessionCount: 28,
  totals: TokenTotals(
    inputTokens: 4200000,
    uncachedInputTokens: 2180000,
    outputTokens: 720000,
    cachedInputTokens: 2020000,
    reasoningOutputTokens: 210000,
    totalTokens: 5130000,
  ),
  estimates: TokenEstimates(
    userMessageInputTokens: 260000,
    toolOutputInputTokens: 1460000,
    toolCallOutputTokens: 210000,
    skillInstructionInputTokens: 88000,
  ),
  compactionCount: 11,
  categories: [
    TokenCategoryBreakdown(key: 'tool_output', label: 'Tool outputs', tokens: 1460000, estimated: true),
    TokenCategoryBreakdown(key: 'user_message', label: 'User messages', tokens: 260000, estimated: true),
    TokenCategoryBreakdown(key: 'tool_call', label: 'Tool call inputs', tokens: 210000, estimated: true),
    TokenCategoryBreakdown(key: 'assistant_message', label: 'Assistant messages', tokens: 190000, estimated: true),
    TokenCategoryBreakdown(key: 'skill_instruction', label: 'Skill instructions', tokens: 88000, estimated: true),
  ],
  topItems: [
    TokenTopItem(label: 'worker-session.jsonl · Tool output flutter test', kind: 'tool_output', line: 1204, tokens: 142000, estimated: true),
    TokenTopItem(label: 'qa-session.jsonl · Tool output design lab sweep', kind: 'tool_output', line: 841, tokens: 118000, estimated: true),
  ],
  warnings: [],
  quota: WeeklyQuotaData(
    resetAtMs: 1781051400000,
    remainingPercent: 30,
    usedPercent: 70,
    inferredStartMs: 1780446600000,
  ),
);
