import 'workbench_models.dart';

class AgentRuntimeWorkbenchData {
  const AgentRuntimeWorkbenchData({
    required this.connectionState,
    required this.discovery,
    required this.remoteDiscovery,
    required this.importedRemoteDiscovery,
    required this.connectionTone,
    required this.baseUrl,
    required this.statusLabel,
    required this.watermarkLabel,
    required this.statusBadges,
    this.modelOptions = const [],
    required this.selectedSessionLabel,
    required this.sessionsTitle,
    required this.sessionsSubtitle,
    required this.timelineTitle,
    required this.timelineSubtitle,
    required this.actionsTitle,
    required this.actionsSubtitle,
    required this.detailTitle,
    required this.detailSubtitle,
    required this.sessionsEmptyTitle,
    required this.sessionsEmptyText,
    required this.timelineEmptyTitle,
    required this.timelineEmptyText,
    required this.actionsEmptyTitle,
    required this.actionsEmptyText,
    required this.sessions,
    required this.timeline,
    this.selectedConversation = const [],
    required this.actions,
    required this.roleAdmin,
    required this.workflowMemory,
    required this.controllerFacts,
    this.operationSurfaces = const [],
    this.selectedSessionControlPlane,
    required this.outputLog,
    required this.pendingRequestCount,
    this.errorMessage,
  });

  final String connectionState;
  final AgentRuntimeDiscoveryInfo discovery;
  final AgentRuntimeDiscoveryInfo remoteDiscovery;
  final AgentRuntimeDiscoveryInfo importedRemoteDiscovery;
  final String connectionTone;
  final String baseUrl;
  final String statusLabel;
  final String watermarkLabel;
  final List<AgentRuntimeStatusBadge> statusBadges;
  final List<AgentRuntimeModelOption> modelOptions;
  final String selectedSessionLabel;
  final String sessionsTitle;
  final String sessionsSubtitle;
  final String timelineTitle;
  final String timelineSubtitle;
  final String actionsTitle;
  final String actionsSubtitle;
  final String detailTitle;
  final String detailSubtitle;
  final String sessionsEmptyTitle;
  final String sessionsEmptyText;
  final String timelineEmptyTitle;
  final String timelineEmptyText;
  final String actionsEmptyTitle;
  final String actionsEmptyText;
  final List<AgentRuntimeSessionItem> sessions;
  final List<AgentRuntimeTimelineItem> timeline;
  final List<ChatEntry> selectedConversation;
  final List<AgentRuntimeActionItem> actions;
  final AgentRuntimeRoleAdminData roleAdmin;
  final AgentRuntimeWorkflowMemoryData workflowMemory;
  final List<AgentRuntimeFact> controllerFacts;
  final List<AgentRuntimeOperationSurface> operationSurfaces;
  final AgentRuntimeSelectedSessionControlPlane? selectedSessionControlPlane;
  final List<String> outputLog;
  final int pendingRequestCount;
  final String? errorMessage;

  factory AgentRuntimeWorkbenchData.fromJson(Map<String, dynamic> json) {
    return AgentRuntimeWorkbenchData(
      connectionState: '${json['connectionState'] ?? 'disconnected'}',
      discovery: AgentRuntimeDiscoveryInfo.fromJson(
        Map<String, dynamic>.from((json['discovery'] as Map?) ?? const {}),
      ),
      remoteDiscovery: AgentRuntimeDiscoveryInfo.fromJson(
        Map<String, dynamic>.from((json['remoteDiscovery'] as Map?) ?? const {}),
      ),
      importedRemoteDiscovery: AgentRuntimeDiscoveryInfo.fromJson(
        Map<String, dynamic>.from((json['importedRemoteDiscovery'] as Map?) ?? const {}),
      ),
      connectionTone: '${json['connectionTone'] ?? 'muted'}',
      baseUrl: '${json['baseUrl'] ?? ''}',
      statusLabel: '${json['statusLabel'] ?? 'No projection packet'}',
      watermarkLabel: '${json['watermarkLabel'] ?? '—'}',
      statusBadges: _objects(json['statusBadges']).map(AgentRuntimeStatusBadge.fromJson).toList(growable: false),
      modelOptions: _objects(json['modelOptions']).map(AgentRuntimeModelOption.fromJson).toList(growable: false),
      selectedSessionLabel: '${json['selectedSessionLabel'] ?? 'none selected'}',
      sessionsTitle: '${json['sessionsTitle'] ?? 'Sessions'}',
      sessionsSubtitle: '${json['sessionsSubtitle'] ?? ''}',
      timelineTitle: '${json['timelineTitle'] ?? 'Selected session stream'}',
      timelineSubtitle: '${json['timelineSubtitle'] ?? ''}',
      actionsTitle: '${json['actionsTitle'] ?? 'Action queue'}',
      actionsSubtitle: '${json['actionsSubtitle'] ?? ''}',
      detailTitle: '${json['detailTitle'] ?? 'Controller detail'}',
      detailSubtitle: '${json['detailSubtitle'] ?? ''}',
      sessionsEmptyTitle: '${json['sessionsEmptyTitle'] ?? 'No sessions'}',
      sessionsEmptyText: '${json['sessionsEmptyText'] ?? 'No sessions are visible.'}',
      timelineEmptyTitle: '${json['timelineEmptyTitle'] ?? 'No timeline'}',
      timelineEmptyText: '${json['timelineEmptyText'] ?? 'Select a session to inspect its timeline.'}',
      actionsEmptyTitle: '${json['actionsEmptyTitle'] ?? 'No action required'}',
      actionsEmptyText: '${json['actionsEmptyText'] ?? 'No action items need attention.'}',
      sessions: _objects(json['sessions']).map(AgentRuntimeSessionItem.fromJson).toList(growable: false),
      timeline: _objects(json['timeline']).map(AgentRuntimeTimelineItem.fromJson).toList(growable: false),
      selectedConversation: _objects(json['selectedConversation']).map((entry) => ChatEntry.fromJson(entry)).toList(growable: false),
      actions: _objects(json['actions']).map(AgentRuntimeActionItem.fromJson).toList(growable: false),
      roleAdmin: AgentRuntimeRoleAdminData.fromJson(
        Map<String, dynamic>.from((json['roleAdmin'] as Map?) ?? const {}),
      ),
      workflowMemory: AgentRuntimeWorkflowMemoryData.fromJson(
        Map<String, dynamic>.from((json['workflowMemory'] as Map?) ?? const {}),
      ),
      controllerFacts: _objects(json['controllerFacts']).map(AgentRuntimeFact.fromJson).toList(growable: false),
      operationSurfaces: _objects(json['operationSurfaces']).map(AgentRuntimeOperationSurface.fromJson).toList(growable: false),
      selectedSessionControlPlane: json['selectedSessionControlPlane'] is Map ? AgentRuntimeSelectedSessionControlPlane.fromJson(Map<String, dynamic>.from(json['selectedSessionControlPlane'] as Map)) : null,
      outputLog: (json['outputLog'] as List<dynamic>? ?? const []).map((value) => '$value').toList(growable: false),
      pendingRequestCount: (json['pendingRequestCount'] as num?)?.toInt() ?? 0,
      errorMessage: json['errorMessage'] as String?,
    );
  }

  AgentRuntimeWorkbenchData copyWith({
    String? connectionState,
    AgentRuntimeDiscoveryInfo? discovery,
    AgentRuntimeDiscoveryInfo? remoteDiscovery,
    AgentRuntimeDiscoveryInfo? importedRemoteDiscovery,
    String? connectionTone,
    String? baseUrl,
    String? statusLabel,
    String? watermarkLabel,
    List<AgentRuntimeStatusBadge>? statusBadges,
    List<AgentRuntimeModelOption>? modelOptions,
    String? selectedSessionLabel,
    String? sessionsTitle,
    String? sessionsSubtitle,
    String? timelineTitle,
    String? timelineSubtitle,
    String? actionsTitle,
    String? actionsSubtitle,
    String? detailTitle,
    String? detailSubtitle,
    String? sessionsEmptyTitle,
    String? sessionsEmptyText,
    String? timelineEmptyTitle,
    String? timelineEmptyText,
    String? actionsEmptyTitle,
    String? actionsEmptyText,
    List<AgentRuntimeSessionItem>? sessions,
    List<AgentRuntimeTimelineItem>? timeline,
    List<ChatEntry>? selectedConversation,
    List<AgentRuntimeActionItem>? actions,
    AgentRuntimeRoleAdminData? roleAdmin,
    AgentRuntimeWorkflowMemoryData? workflowMemory,
    List<AgentRuntimeFact>? controllerFacts,
    List<AgentRuntimeOperationSurface>? operationSurfaces,
    AgentRuntimeSelectedSessionControlPlane? selectedSessionControlPlane,
    List<String>? outputLog,
    int? pendingRequestCount,
    String? errorMessage,
  }) {
    return AgentRuntimeWorkbenchData(
      connectionState: connectionState ?? this.connectionState,
      discovery: discovery ?? this.discovery,
      remoteDiscovery: remoteDiscovery ?? this.remoteDiscovery,
      importedRemoteDiscovery: importedRemoteDiscovery ?? this.importedRemoteDiscovery,
      connectionTone: connectionTone ?? this.connectionTone,
      baseUrl: baseUrl ?? this.baseUrl,
      statusLabel: statusLabel ?? this.statusLabel,
      watermarkLabel: watermarkLabel ?? this.watermarkLabel,
      statusBadges: statusBadges ?? this.statusBadges,
      modelOptions: modelOptions ?? this.modelOptions,
      selectedSessionLabel: selectedSessionLabel ?? this.selectedSessionLabel,
      sessionsTitle: sessionsTitle ?? this.sessionsTitle,
      sessionsSubtitle: sessionsSubtitle ?? this.sessionsSubtitle,
      timelineTitle: timelineTitle ?? this.timelineTitle,
      timelineSubtitle: timelineSubtitle ?? this.timelineSubtitle,
      actionsTitle: actionsTitle ?? this.actionsTitle,
      actionsSubtitle: actionsSubtitle ?? this.actionsSubtitle,
      detailTitle: detailTitle ?? this.detailTitle,
      detailSubtitle: detailSubtitle ?? this.detailSubtitle,
      sessionsEmptyTitle: sessionsEmptyTitle ?? this.sessionsEmptyTitle,
      sessionsEmptyText: sessionsEmptyText ?? this.sessionsEmptyText,
      timelineEmptyTitle: timelineEmptyTitle ?? this.timelineEmptyTitle,
      timelineEmptyText: timelineEmptyText ?? this.timelineEmptyText,
      actionsEmptyTitle: actionsEmptyTitle ?? this.actionsEmptyTitle,
      actionsEmptyText: actionsEmptyText ?? this.actionsEmptyText,
      sessions: sessions ?? this.sessions,
      timeline: timeline ?? this.timeline,
      selectedConversation: selectedConversation ?? this.selectedConversation,
      actions: actions ?? this.actions,
      roleAdmin: roleAdmin ?? this.roleAdmin,
      workflowMemory: workflowMemory ?? this.workflowMemory,
      controllerFacts: controllerFacts ?? this.controllerFacts,
      operationSurfaces: operationSurfaces ?? this.operationSurfaces,
      selectedSessionControlPlane: selectedSessionControlPlane ?? this.selectedSessionControlPlane,
      outputLog: outputLog ?? this.outputLog,
      pendingRequestCount: pendingRequestCount ?? this.pendingRequestCount,
      errorMessage: errorMessage ?? this.errorMessage,
    );
  }
}

class AgentRuntimeModelOption {
  const AgentRuntimeModelOption({
    required this.id,
    required this.displayLabel,
    required this.source,
    this.isDefault = false,
  });

  final String id;
  final String displayLabel;
  final String source;
  final bool isDefault;

  factory AgentRuntimeModelOption.fromJson(Map<String, dynamic> json) {
    return AgentRuntimeModelOption(
      id: '${json['id'] ?? ''}',
      displayLabel: '${json['displayLabel'] ?? json['id'] ?? ''}',
      source: '${json['source'] ?? ''}',
      isDefault: json['isDefault'] == true,
    );
  }
}

class AgentRuntimeDiscoveryInfo {
  const AgentRuntimeDiscoveryInfo({
    required this.state,
    required this.tone,
    required this.title,
    required this.message,
    required this.discoveryPath,
    required this.connectable,
    this.sourceType = 'localServiceFile',
    this.sourcePath = '',
    this.lastImportedAt,
    this.baseUrl,
    this.healthUrl,
    this.webSocketUrl,
    this.runtimeIdentity,
    this.serviceState,
    this.diagnostics = const [],
  });

  final String state;
  final String tone;
  final String title;
  final String message;
  final String sourceType;
  final String sourcePath;
  final String? lastImportedAt;
  final String discoveryPath;
  final bool connectable;
  final String? baseUrl;
  final String? healthUrl;
  final String? webSocketUrl;
  final String? runtimeIdentity;
  final String? serviceState;
  final List<String> diagnostics;

  factory AgentRuntimeDiscoveryInfo.fromJson(Map<String, dynamic> json) {
    return AgentRuntimeDiscoveryInfo(
      state: '${json['state'] ?? 'notLoaded'}',
      tone: '${json['tone'] ?? 'muted'}',
      title: '${json['title'] ?? 'Discovery not loaded'}',
      message: '${json['message'] ?? ''}',
      sourceType: '${json['sourceType'] ?? 'localServiceFile'}',
      sourcePath: '${json['sourcePath'] ?? json['discoveryPath'] ?? ''}',
      lastImportedAt: json['lastImportedAt'] as String?,
      discoveryPath: '${json['discoveryPath'] ?? ''}',
      connectable: json['connectable'] == true,
      baseUrl: json['baseUrl'] as String?,
      healthUrl: json['healthUrl'] as String?,
      webSocketUrl: json['webSocketUrl'] as String?,
      runtimeIdentity: json['runtimeIdentity'] as String?,
      serviceState: json['serviceState'] as String?,
      diagnostics: (json['diagnostics'] as List<dynamic>? ?? const []).map((value) => '$value').toList(growable: false),
    );
  }
}

class AgentRuntimeStatusBadge {
  const AgentRuntimeStatusBadge({
    required this.label,
    required this.value,
    required this.tone,
  });

  final String label;
  final String value;
  final String tone;

  factory AgentRuntimeStatusBadge.fromJson(Map<String, dynamic> json) {
    return AgentRuntimeStatusBadge(
      label: '${json['label'] ?? ''}',
      value: '${json['value'] ?? ''}',
      tone: '${json['tone'] ?? 'info'}',
    );
  }
}

class AgentRuntimeSessionItem {
  const AgentRuntimeSessionItem({
    required this.id,
    required this.title,
    required this.status,
    required this.subtitle,
    required this.groupLabel,
    required this.tone,
  });

  final String id;
  final String title;
  final String status;
  final String subtitle;
  final String groupLabel;
  final String tone;

  factory AgentRuntimeSessionItem.fromJson(Map<String, dynamic> json) {
    return AgentRuntimeSessionItem(
      id: '${json['id'] ?? 'session'}',
      title: '${json['title'] ?? json['id'] ?? 'Session'}',
      status: '${json['status'] ?? 'unknown'}',
      subtitle: '${json['subtitle'] ?? ''}',
      groupLabel: '${json['groupLabel'] ?? 'Sessions'}',
      tone: '${json['tone'] ?? 'info'}',
    );
  }
}

class AgentRuntimeTimelineItem {
  const AgentRuntimeTimelineItem({
    required this.id,
    required this.title,
    required this.subtitle,
    required this.status,
    required this.tone,
  });

  final String id;
  final String title;
  final String subtitle;
  final String status;
  final String tone;

  factory AgentRuntimeTimelineItem.fromJson(Map<String, dynamic> json) {
    return AgentRuntimeTimelineItem(
      id: '${json['id'] ?? 'event'}',
      title: '${json['title'] ?? 'event'}',
      subtitle: '${json['subtitle'] ?? ''}',
      status: '${json['status'] ?? ''}',
      tone: '${json['tone'] ?? 'info'}',
    );
  }
}

class AgentRuntimeActionItem {
  const AgentRuntimeActionItem({
    required this.id,
    required this.title,
    required this.subtitle,
    required this.kind,
    required this.stateText,
    required this.tone,
  });

  final String id;
  final String title;
  final String subtitle;
  final String kind;
  final String stateText;
  final String tone;

  factory AgentRuntimeActionItem.fromJson(Map<String, dynamic> json) {
    return AgentRuntimeActionItem(
      id: '${json['id'] ?? 'action'}',
      title: '${json['title'] ?? 'Action'}',
      subtitle: '${json['subtitle'] ?? ''}',
      kind: '${json['kind'] ?? 'action'}',
      stateText: '${json['stateText'] ?? json['kind'] ?? 'Action'}',
      tone: '${json['tone'] ?? 'info'}',
    );
  }
}

class AgentRuntimeFact {
  const AgentRuntimeFact({
    required this.label,
    required this.value,
  });

  final String label;
  final String value;

  factory AgentRuntimeFact.fromJson(Map<String, dynamic> json) {
    return AgentRuntimeFact(
      label: '${json['label'] ?? ''}',
      value: '${json['value'] ?? ''}',
    );
  }
}

class AgentRuntimeOperationSurface {
  const AgentRuntimeOperationSurface({
    required this.surfaceId,
    required this.title,
    required this.subtitle,
    required this.rows,
    required this.actions,
  });

  final String surfaceId;
  final String title;
  final String subtitle;
  final List<AgentRuntimeFact> rows;
  final List<AgentRuntimeActionItem> actions;

  factory AgentRuntimeOperationSurface.fromJson(Map<String, dynamic> json) {
    return AgentRuntimeOperationSurface(
      surfaceId: '${json['surfaceId'] ?? ''}',
      title: '${json['title'] ?? 'Detail'}',
      subtitle: '${json['subtitle'] ?? ''}',
      rows: _objects(json['rows']).map(AgentRuntimeFact.fromJson).toList(growable: false),
      actions: _objects(json['actions']).map(AgentRuntimeActionItem.fromJson).toList(growable: false),
    );
  }
}

class AgentRuntimeSelectedSessionControlPlane {
  const AgentRuntimeSelectedSessionControlPlane({
    required this.sessionId,
    required this.title,
    required this.name,
    required this.status,
    required this.roleId,
    required this.projectKey,
    required this.activeModel,
    required this.workdir,
    required this.worktreeRoot,
    required this.tracked,
    required this.modelOptions,
    required this.godMode,
    required this.managedProcesses,
    required this.approvals,
    required this.commandRequests,
    required this.requirementsReview,
    required this.runningServers,
    required this.imageArtifacts,
    required this.quickActions,
  });

  final String sessionId;
  final String title;
  final String name;
  final String status;
  final String roleId;
  final String projectKey;
  final String activeModel;
  final String workdir;
  final String worktreeRoot;
  final bool tracked;
  final List<AgentRuntimeModelOption> modelOptions;
  final AgentRuntimeGodModeState godMode;
  final List<AgentRuntimeManagedProcessRow> managedProcesses;
  final List<AgentRuntimeApprovalCard> approvals;
  final List<AgentRuntimeCommandRequestCard> commandRequests;
  final AgentRuntimeRequirementsReviewPanel requirementsReview;
  final List<AgentRuntimeFact> runningServers;
  final List<AgentRuntimeFact> imageArtifacts;
  final List<AgentRuntimeActionAvailability> quickActions;

  factory AgentRuntimeSelectedSessionControlPlane.fromJson(Map<String, dynamic> json) {
    return AgentRuntimeSelectedSessionControlPlane(
      sessionId: '${json['sessionId'] ?? ''}',
      title: '${json['title'] ?? ''}',
      name: '${json['name'] ?? ''}',
      status: '${json['status'] ?? ''}',
      roleId: '${json['roleId'] ?? ''}',
      projectKey: '${json['projectKey'] ?? ''}',
      activeModel: '${json['activeModel'] ?? ''}',
      workdir: '${json['workdir'] ?? ''}',
      worktreeRoot: '${json['worktreeRoot'] ?? ''}',
      tracked: json['tracked'] == true,
      modelOptions: _objects(json['modelOptions']).map(AgentRuntimeModelOption.fromJson).toList(growable: false),
      godMode: AgentRuntimeGodModeState.fromJson(Map<String, dynamic>.from((json['godMode'] as Map?) ?? const {})),
      managedProcesses: _objects(json['managedProcesses']).map(AgentRuntimeManagedProcessRow.fromJson).toList(growable: false),
      approvals: _objects(json['approvals']).map(AgentRuntimeApprovalCard.fromJson).toList(growable: false),
      commandRequests: _objects(json['commandRequests']).map(AgentRuntimeCommandRequestCard.fromJson).toList(growable: false),
      requirementsReview: AgentRuntimeRequirementsReviewPanel.fromJson(Map<String, dynamic>.from((json['requirementsReview'] as Map?) ?? const {})),
      runningServers: _objects(json['runningServers']).map(AgentRuntimeFact.fromJson).toList(growable: false),
      imageArtifacts: _objects(json['imageArtifacts']).map(AgentRuntimeFact.fromJson).toList(growable: false),
      quickActions: _objects(json['quickActions']).map(AgentRuntimeActionAvailability.fromJson).toList(growable: false),
    );
  }
}

class AgentRuntimeGodModeState {
  const AgentRuntimeGodModeState({required this.active, required this.reason, required this.grantedBy, required this.grantedAt});
  final bool active;
  final String reason;
  final String grantedBy;
  final String grantedAt;
  factory AgentRuntimeGodModeState.fromJson(Map<String, dynamic> json) => AgentRuntimeGodModeState(active: json['active'] == true, reason: '${json['reason'] ?? ''}', grantedBy: '${json['grantedBy'] ?? ''}', grantedAt: '${json['grantedAt'] ?? ''}');
}

class AgentRuntimeManagedProcessRow {
  const AgentRuntimeManagedProcessRow({required this.id, required this.handle, required this.command, required this.status, required this.startedAt, required this.endedAt, required this.cwd, required this.pid, required this.stdinPolicy, required this.endOfTurnBehavior, required this.endOfSessionBehavior, required this.latestOutputSummary, required this.canTerminate, required this.canFlush, required this.canInput});
  final String id, handle, command, status, startedAt, endedAt, cwd, pid, stdinPolicy, endOfTurnBehavior, endOfSessionBehavior, latestOutputSummary;
  final bool canTerminate, canFlush, canInput;
  factory AgentRuntimeManagedProcessRow.fromJson(Map<String, dynamic> json) => AgentRuntimeManagedProcessRow(id: '${json['id'] ?? ''}', handle: '${json['handle'] ?? ''}', command: '${json['command'] ?? ''}', status: '${json['status'] ?? ''}', startedAt: '${json['startedAt'] ?? ''}', endedAt: '${json['endedAt'] ?? ''}', cwd: '${json['cwd'] ?? ''}', pid: '${json['pid'] ?? ''}', stdinPolicy: '${json['stdinPolicy'] ?? ''}', endOfTurnBehavior: '${json['endOfTurnBehavior'] ?? ''}', endOfSessionBehavior: '${json['endOfSessionBehavior'] ?? ''}', latestOutputSummary: '${json['latestOutputSummary'] ?? ''}', canTerminate: json['canTerminate'] == true, canFlush: json['canFlush'] == true, canInput: json['canInput'] == true);
}

class AgentRuntimeApprovalCard {
  const AgentRuntimeApprovalCard({required this.id, required this.title, required this.status, required this.requiredApprover, required this.requestedAt, required this.contextSummary, required this.canDecide, required this.canResume, required this.decisionSummary});
  final String id, title, status, requiredApprover, requestedAt, contextSummary, decisionSummary;
  final bool canDecide, canResume;
  factory AgentRuntimeApprovalCard.fromJson(Map<String, dynamic> json) => AgentRuntimeApprovalCard(id: '${json['id'] ?? ''}', title: '${json['title'] ?? ''}', status: '${json['status'] ?? ''}', requiredApprover: '${json['requiredApprover'] ?? ''}', requestedAt: '${json['requestedAt'] ?? ''}', contextSummary: '${json['contextSummary'] ?? ''}', canDecide: json['canDecide'] == true, canResume: json['canResume'] == true, decisionSummary: '${json['decisionSummary'] ?? ''}');
}

class AgentRuntimeCommandRequestCard {
  const AgentRuntimeCommandRequestCard({required this.id, required this.title, required this.operation, required this.status, required this.scopeSummary, required this.policySummary, required this.previewStatus, required this.applyStatus, required this.canPreview, required this.canDecide, required this.canApply, required this.commandSummary});
  final String id, title, operation, status, scopeSummary, policySummary, previewStatus, applyStatus, commandSummary;
  final bool canPreview, canDecide, canApply;
  factory AgentRuntimeCommandRequestCard.fromJson(Map<String, dynamic> json) => AgentRuntimeCommandRequestCard(id: '${json['id'] ?? ''}', title: '${json['title'] ?? ''}', operation: '${json['operation'] ?? ''}', status: '${json['status'] ?? ''}', scopeSummary: '${json['scopeSummary'] ?? ''}', policySummary: '${json['policySummary'] ?? ''}', previewStatus: '${json['previewStatus'] ?? ''}', applyStatus: '${json['applyStatus'] ?? ''}', canPreview: json['canPreview'] == true, canDecide: json['canDecide'] == true, canApply: json['canApply'] == true, commandSummary: '${json['commandSummary'] ?? ''}');
}

class AgentRuntimeRequirementsReviewPanel {
  const AgentRuntimeRequirementsReviewPanel({required this.active, required this.status, required this.progressSummary, required this.reviewerStatus, required this.ownerActionStatus, required this.latestPacketStatus});
  final bool active;
  final String status, progressSummary, reviewerStatus, ownerActionStatus, latestPacketStatus;
  factory AgentRuntimeRequirementsReviewPanel.fromJson(Map<String, dynamic> json) => AgentRuntimeRequirementsReviewPanel(active: json['active'] == true, status: '${json['status'] ?? ''}', progressSummary: '${json['progressSummary'] ?? ''}', reviewerStatus: '${json['reviewerStatus'] ?? ''}', ownerActionStatus: '${json['ownerActionStatus'] ?? ''}', latestPacketStatus: '${json['latestPacketStatus'] ?? ''}');
}

class AgentRuntimeActionAvailability {
  const AgentRuntimeActionAvailability({required this.id, required this.label, required this.available, required this.reason});
  final String id, label, reason;
  final bool available;
  factory AgentRuntimeActionAvailability.fromJson(Map<String, dynamic> json) => AgentRuntimeActionAvailability(id: '${json['id'] ?? ''}', label: '${json['label'] ?? ''}', available: json['available'] == true, reason: '${json['reason'] ?? ''}');
}

class AgentRuntimeCommandRegistryDecisionDraft {
  const AgentRuntimeCommandRegistryDecisionDraft({
    required this.status,
    required this.scopeType,
    required this.projectKey,
    required this.policyDecision,
    required this.policyReason,
    required this.actionId,
    required this.displayName,
    required this.binaryName,
    required this.argvTemplate,
    required this.defaultCwd,
    required this.cwdPolicy,
    required this.envPolicy,
    required this.stdinPolicy,
    required this.syncAllowed,
    required this.asyncAllowed,
    required this.maxRuntimeMs,
    required this.endOfTurnBehavior,
    required this.endOfSessionBehavior,
    required this.mutationClass,
    required this.modelDescription,
    required this.allowCwdArg,
    required this.allowArgsArg,
    required this.forbiddenArgs,
    required this.executionPolicy,
  });

  const AgentRuntimeCommandRegistryDecisionDraft.empty()
      : status = '',
        scopeType = '',
        projectKey = '',
        policyDecision = '',
        policyReason = '',
        actionId = '',
        displayName = '',
        binaryName = '',
        argvTemplate = const [],
        defaultCwd = '',
        cwdPolicy = '',
        envPolicy = '',
        stdinPolicy = '',
        syncAllowed = false,
        asyncAllowed = false,
        maxRuntimeMs = null,
        endOfTurnBehavior = '',
        endOfSessionBehavior = '',
        mutationClass = '',
        modelDescription = '',
        allowCwdArg = false,
        allowArgsArg = false,
        forbiddenArgs = const [],
        executionPolicy = '';

  final String status;
  final String scopeType;
  final String projectKey;
  final String policyDecision;
  final String policyReason;
  final String actionId;
  final String displayName;
  final String binaryName;
  final List<String> argvTemplate;
  final String defaultCwd;
  final String cwdPolicy;
  final String envPolicy;
  final String stdinPolicy;
  final bool syncAllowed;
  final bool asyncAllowed;
  final int? maxRuntimeMs;
  final String endOfTurnBehavior;
  final String endOfSessionBehavior;
  final String mutationClass;
  final String modelDescription;
  final bool allowCwdArg;
  final bool allowArgsArg;
  final List<String> forbiddenArgs;
  final String executionPolicy;

  AgentRuntimeCommandRegistryDecisionDraft copyWith({
    String? status,
    String? scopeType,
    String? projectKey,
    String? policyDecision,
    String? policyReason,
    String? actionId,
    String? displayName,
    String? binaryName,
    List<String>? argvTemplate,
    String? defaultCwd,
    String? cwdPolicy,
    String? envPolicy,
    String? stdinPolicy,
    bool? syncAllowed,
    bool? asyncAllowed,
    int? maxRuntimeMs,
    String? endOfTurnBehavior,
    String? endOfSessionBehavior,
    String? mutationClass,
    String? modelDescription,
    bool? allowCwdArg,
    bool? allowArgsArg,
    List<String>? forbiddenArgs,
    String? executionPolicy,
  }) {
    return AgentRuntimeCommandRegistryDecisionDraft(
      status: status ?? this.status,
      scopeType: scopeType ?? this.scopeType,
      projectKey: projectKey ?? this.projectKey,
      policyDecision: policyDecision ?? this.policyDecision,
      policyReason: policyReason ?? this.policyReason,
      actionId: actionId ?? this.actionId,
      displayName: displayName ?? this.displayName,
      binaryName: binaryName ?? this.binaryName,
      argvTemplate: argvTemplate ?? this.argvTemplate,
      defaultCwd: defaultCwd ?? this.defaultCwd,
      cwdPolicy: cwdPolicy ?? this.cwdPolicy,
      envPolicy: envPolicy ?? this.envPolicy,
      stdinPolicy: stdinPolicy ?? this.stdinPolicy,
      syncAllowed: syncAllowed ?? this.syncAllowed,
      asyncAllowed: asyncAllowed ?? this.asyncAllowed,
      maxRuntimeMs: maxRuntimeMs ?? this.maxRuntimeMs,
      endOfTurnBehavior: endOfTurnBehavior ?? this.endOfTurnBehavior,
      endOfSessionBehavior: endOfSessionBehavior ?? this.endOfSessionBehavior,
      mutationClass: mutationClass ?? this.mutationClass,
      modelDescription: modelDescription ?? this.modelDescription,
      allowCwdArg: allowCwdArg ?? this.allowCwdArg,
      allowArgsArg: allowArgsArg ?? this.allowArgsArg,
      forbiddenArgs: forbiddenArgs ?? this.forbiddenArgs,
      executionPolicy: executionPolicy ?? this.executionPolicy,
    );
  }
}

class AgentRuntimeWorkflowMemoryData {
  const AgentRuntimeWorkflowMemoryData({
    required this.title,
    required this.subtitle,
    required this.emptyTitle,
    required this.emptyText,
    required this.rows,
    required this.recentEvents,
    required this.feedbackActions,
    this.selectedMemoryId,
    this.selectedDetail,
  });

  final String title;
  final String subtitle;
  final String emptyTitle;
  final String emptyText;
  final String? selectedMemoryId;
  final List<AgentRuntimeWorkflowMemoryRow> rows;
  final AgentRuntimeWorkflowMemoryDetail? selectedDetail;
  final List<AgentRuntimeWorkflowMemoryEventRow> recentEvents;
  final List<AgentRuntimeActionItem> feedbackActions;

  factory AgentRuntimeWorkflowMemoryData.fromJson(Map<String, dynamic> json) {
    final selectedDetail = json['selectedDetail'];
    return AgentRuntimeWorkflowMemoryData(
      title: '${json['title'] ?? 'Workflow Memory'}',
      subtitle: '${json['subtitle'] ?? 'execute_code/Starlark memories'}',
      emptyTitle: '${json['emptyTitle'] ?? 'No workflow memories'}',
      emptyText: '${json['emptyText'] ?? 'No visible workflow memories are projected.'}',
      selectedMemoryId: json['selectedMemoryId'] as String?,
      rows: _objects(json['rows']).map(AgentRuntimeWorkflowMemoryRow.fromJson).toList(growable: false),
      selectedDetail: selectedDetail is Map ? AgentRuntimeWorkflowMemoryDetail.fromJson(Map<String, dynamic>.from(selectedDetail)) : null,
      recentEvents: _objects(json['recentEvents']).map(AgentRuntimeWorkflowMemoryEventRow.fromJson).toList(growable: false),
      feedbackActions: _objects(json['feedbackActions']).map(AgentRuntimeActionItem.fromJson).toList(growable: false),
    );
  }
}

class AgentRuntimeWorkflowMemoryRow {
  const AgentRuntimeWorkflowMemoryRow({
    required this.id,
    required this.title,
    required this.subtitle,
    required this.scopeType,
    required this.helpfulScore,
    required this.sourceSessionId,
    required this.tone,
    required this.selected,
    this.projectKey,
    this.promotedAt,
  });

  final String id;
  final String title;
  final String subtitle;
  final String scopeType;
  final String? projectKey;
  final double helpfulScore;
  final String? promotedAt;
  final String sourceSessionId;
  final String tone;
  final bool selected;

  factory AgentRuntimeWorkflowMemoryRow.fromJson(Map<String, dynamic> json) {
    return AgentRuntimeWorkflowMemoryRow(
      id: '${json['id'] ?? ''}',
      title: '${json['title'] ?? json['id'] ?? 'Workflow memory'}',
      subtitle: '${json['subtitle'] ?? ''}',
      scopeType: '${json['scopeType'] ?? 'project'}',
      projectKey: json['projectKey'] as String?,
      helpfulScore: (json['helpfulScore'] as num?)?.toDouble() ?? 0,
      promotedAt: json['promotedAt'] as String?,
      sourceSessionId: '${json['sourceSessionId'] ?? ''}',
      tone: '${json['tone'] ?? 'info'}',
      selected: json['selected'] == true,
    );
  }
}

class AgentRuntimeWorkflowMemoryDetail {
  const AgentRuntimeWorkflowMemoryDetail({
    required this.id,
    required this.title,
    required this.reason,
    required this.summary,
    required this.sourceSessionId,
    required this.sourceStarlark,
    required this.sourcePreview,
    required this.helpfulScore,
    required this.scopeLabel,
    required this.feedbackEnabled,
    this.sourceScriptRunId,
    this.provider,
    this.model,
    this.dimensions,
    this.storageType,
    this.sourceHash,
    this.commandFingerprint,
    this.feedbackSessionId,
  });

  final String id;
  final String title;
  final String reason;
  final String summary;
  final String sourceSessionId;
  final String? sourceScriptRunId;
  final String sourceStarlark;
  final String sourcePreview;
  final String? provider;
  final String? model;
  final int? dimensions;
  final String? storageType;
  final String? sourceHash;
  final String? commandFingerprint;
  final double helpfulScore;
  final String scopeLabel;
  final String? feedbackSessionId;
  final bool feedbackEnabled;

  factory AgentRuntimeWorkflowMemoryDetail.fromJson(Map<String, dynamic> json) {
    return AgentRuntimeWorkflowMemoryDetail(
      id: '${json['id'] ?? ''}',
      title: '${json['title'] ?? 'Workflow memory'}',
      reason: '${json['reason'] ?? ''}',
      summary: '${json['summary'] ?? ''}',
      sourceSessionId: '${json['sourceSessionId'] ?? ''}',
      sourceScriptRunId: json['sourceScriptRunId'] as String?,
      sourceStarlark: '${json['sourceStarlark'] ?? json['sourcePreview'] ?? ''}',
      sourcePreview: '${json['sourcePreview'] ?? ''}',
      provider: json['provider'] as String?,
      model: json['model'] as String?,
      dimensions: (json['dimensions'] as num?)?.toInt(),
      storageType: json['storageType'] as String?,
      sourceHash: json['sourceHash'] as String?,
      commandFingerprint: json['commandFingerprint'] as String?,
      helpfulScore: (json['helpfulScore'] as num?)?.toDouble() ?? 0,
      scopeLabel: '${json['scopeLabel'] ?? ''}',
      feedbackSessionId: json['feedbackSessionId'] as String?,
      feedbackEnabled: json['feedbackEnabled'] == true,
    );
  }
}

class AgentRuntimeWorkflowMemoryEventRow {
  const AgentRuntimeWorkflowMemoryEventRow({
    required this.id,
    required this.title,
    required this.subtitle,
    required this.tone,
    this.createdAt,
  });

  final String id;
  final String title;
  final String subtitle;
  final String? createdAt;
  final String tone;

  factory AgentRuntimeWorkflowMemoryEventRow.fromJson(Map<String, dynamic> json) {
    return AgentRuntimeWorkflowMemoryEventRow(
      id: '${json['id'] ?? ''}',
      title: '${json['title'] ?? ''}',
      subtitle: '${json['subtitle'] ?? ''}',
      createdAt: json['createdAt'] as String?,
      tone: '${json['tone'] ?? 'info'}',
    );
  }
}

class AgentRuntimeRoleAdminData {
  const AgentRuntimeRoleAdminData({
    required this.title,
    required this.subtitle,
    required this.emptyTitle,
    required this.emptyText,
    required this.rows,
    required this.versionRows,
    required this.validationErrors,
    required this.actionStates,
    required this.editorOptions,
    this.selectedDetail,
    this.editorDraft,
  });

  final String title;
  final String subtitle;
  final String emptyTitle;
  final String emptyText;
  final List<AgentRuntimeRoleRow> rows;
  final AgentRuntimeRoleDetail? selectedDetail;
  final List<AgentRuntimeRoleVersionRow> versionRows;
  final AgentRuntimeRoleEditorDraft? editorDraft;
  final List<String> validationErrors;
  final List<AgentRuntimeActionItem> actionStates;
  final AgentRuntimeRoleEditorOptions editorOptions;

  factory AgentRuntimeRoleAdminData.fromJson(Map<String, dynamic> json) {
    final selectedDetail = json['selectedDetail'];
    final editorDraft = json['editorDraft'];
    return AgentRuntimeRoleAdminData(
      title: '${json['title'] ?? 'Role Admin'}',
      subtitle: '${json['subtitle'] ?? 'Immutable role versions'}',
      emptyTitle: '${json['emptyTitle'] ?? 'No roles projected'}',
      emptyText: '${json['emptyText'] ?? 'Hydrate the runtime projection to inspect roles.'}',
      rows: _objects(json['rows']).map(AgentRuntimeRoleRow.fromJson).toList(growable: false),
      selectedDetail: selectedDetail is Map ? AgentRuntimeRoleDetail.fromJson(Map<String, dynamic>.from(selectedDetail)) : null,
      versionRows: _objects(json['versionRows']).map(AgentRuntimeRoleVersionRow.fromJson).toList(growable: false),
      editorDraft: editorDraft is Map ? AgentRuntimeRoleEditorDraft.fromJson(Map<String, dynamic>.from(editorDraft)) : null,
      validationErrors: (json['validationErrors'] as List<dynamic>? ?? const []).map((value) => '$value').toList(growable: false),
      actionStates: _objects(json['actionStates']).map(AgentRuntimeActionItem.fromJson).toList(growable: false),
      editorOptions: json['editorOptions'] is Map ? AgentRuntimeRoleEditorOptions.fromJson(Map<String, dynamic>.from(json['editorOptions'] as Map)) : AgentRuntimeRoleEditorOptions.empty(),
    );
  }
}

class AgentRuntimeRoleEditorOptions {
  const AgentRuntimeRoleEditorOptions({
    required this.models,
    required this.reasoningEfforts,
    required this.capabilities,
    required this.policyActions,
    required this.policyDecisions,
    required this.routingModes,
    required this.recipients,
    required this.reservedActions,
  });

  final List<String> models;
  final List<String> reasoningEfforts;
  final List<String> capabilities;
  final List<String> policyActions;
  final List<String> policyDecisions;
  final List<String> routingModes;
  final List<String> recipients;
  final List<String> reservedActions;

  bool get isCompleteForPrimaryAuthoring {
    return models.isNotEmpty &&
        reasoningEfforts.isNotEmpty &&
        capabilities.isNotEmpty &&
        policyActions.isNotEmpty &&
        policyDecisions.isNotEmpty &&
        routingModes.isNotEmpty &&
        recipients.isNotEmpty &&
        reservedActions.isNotEmpty;
  }

  factory AgentRuntimeRoleEditorOptions.fromJson(Map<String, dynamic> json) {
    return AgentRuntimeRoleEditorOptions(
      models: _strings(json['models']),
      reasoningEfforts: _strings(json['reasoningEfforts']),
      capabilities: _strings(json['capabilities']),
      policyActions: _strings(json['policyActions']),
      policyDecisions: _strings(json['policyDecisions']),
      routingModes: _strings(json['routingModes']),
      recipients: _strings(json['recipients']),
      reservedActions: _strings(json['reservedActions']),
    );
  }

  factory AgentRuntimeRoleEditorOptions.empty() {
    return const AgentRuntimeRoleEditorOptions(
      models: [],
      reasoningEfforts: [],
      capabilities: [],
      policyActions: [],
      policyDecisions: [],
      routingModes: [],
      recipients: [],
      reservedActions: [],
    );
  }
}

class AgentRuntimeRoleRow {
  const AgentRuntimeRoleRow({
    required this.id,
    required this.title,
    required this.subtitle,
    required this.status,
    required this.tone,
    this.currentVersionId,
  });

  final String id;
  final String title;
  final String subtitle;
  final String status;
  final String tone;
  final String? currentVersionId;

  factory AgentRuntimeRoleRow.fromJson(Map<String, dynamic> json) {
    return AgentRuntimeRoleRow(
      id: '${json['id'] ?? ''}',
      title: '${json['title'] ?? json['id'] ?? 'Role'}',
      subtitle: '${json['subtitle'] ?? ''}',
      status: '${json['status'] ?? 'unknown'}',
      tone: '${json['tone'] ?? 'info'}',
      currentVersionId: json['currentVersionId'] as String?,
    );
  }
}

class AgentRuntimeRoleDetail {
  const AgentRuntimeRoleDetail({
    required this.id,
    required this.displayName,
    required this.version,
    required this.model,
    required this.status,
    required this.instructionText,
    required this.capabilities,
    required this.policy,
    required this.routing,
    required this.visibility,
    required this.lifecycleAuthority,
  });

  final String id;
  final String displayName;
  final String version;
  final String model;
  final String status;
  final String instructionText;
  final List<String> capabilities;
  final List<AgentRuntimeRolePolicyRow> policy;
  final List<AgentRuntimeFact> routing;
  final List<AgentRuntimeFact> visibility;
  final List<AgentRuntimeFact> lifecycleAuthority;

  factory AgentRuntimeRoleDetail.fromJson(Map<String, dynamic> json) {
    return AgentRuntimeRoleDetail(
      id: '${json['id'] ?? ''}',
      displayName: '${json['displayName'] ?? 'Role'}',
      version: '${json['version'] ?? 'unknown'}',
      model: '${json['model'] ?? 'model unknown'}',
      status: '${json['status'] ?? 'unknown'}',
      instructionText: '${json['instructionText'] ?? ''}',
      capabilities: (json['capabilities'] as List<dynamic>? ?? const []).map((value) => '$value').toList(growable: false),
      policy: _objects(json['policy']).map(AgentRuntimeRolePolicyRow.fromJson).toList(growable: false),
      routing: _objects(json['routing']).map(AgentRuntimeFact.fromJson).toList(growable: false),
      visibility: _objects(json['visibility']).map(AgentRuntimeFact.fromJson).toList(growable: false),
      lifecycleAuthority: _objects(json['lifecycleAuthority']).map(AgentRuntimeFact.fromJson).toList(growable: false),
    );
  }
}

class AgentRuntimeRolePolicyRow {
  const AgentRuntimeRolePolicyRow({required this.action, required this.decision});

  final String action;
  final String decision;

  factory AgentRuntimeRolePolicyRow.fromJson(Map<String, dynamic> json) {
    return AgentRuntimeRolePolicyRow(
      action: '${json['action'] ?? ''}',
      decision: '${json['decision'] ?? ''}',
    );
  }
}

class AgentRuntimeRoleVersionRow {
  const AgentRuntimeRoleVersionRow({
    required this.versionId,
    required this.version,
    required this.status,
    this.createdAt,
  });

  final String versionId;
  final String version;
  final String status;
  final String? createdAt;

  factory AgentRuntimeRoleVersionRow.fromJson(Map<String, dynamic> json) {
    return AgentRuntimeRoleVersionRow(
      versionId: '${json['versionId'] ?? ''}',
      version: '${json['version'] ?? 'unknown'}',
      status: '${json['status'] ?? 'unknown'}',
      createdAt: json['createdAt'] as String?,
    );
  }
}

class AgentRuntimeRoleEditorDraft {
  const AgentRuntimeRoleEditorDraft({
    required this.roleId,
    required this.version,
    required this.displayName,
    required this.model,
    required this.reasoningEffort,
    required this.instructionText,
    required this.capabilities,
    required this.policy,
    required this.routingMode,
    required this.routingReservedActions,
    required this.allowedRecipients,
    required this.listed,
    required this.ownerVisible,
    required this.canSpawnAgents,
    required this.canArchiveAgents,
    required this.lifecycleReservedActions,
    this.defaultRecipient,
  });

  final String roleId;
  final String version;
  final String displayName;
  final String model;
  final String reasoningEffort;
  final String instructionText;
  final List<String> capabilities;
  final List<AgentRuntimeRolePolicyRow> policy;
  final String routingMode;
  final List<String> routingReservedActions;
  final String? defaultRecipient;
  final List<String> allowedRecipients;
  final bool listed;
  final bool ownerVisible;
  final bool canSpawnAgents;
  final bool canArchiveAgents;
  final List<String> lifecycleReservedActions;

  factory AgentRuntimeRoleEditorDraft.fromJson(Map<String, dynamic> json) {
    return AgentRuntimeRoleEditorDraft(
      roleId: '${json['roleId'] ?? ''}',
      version: '${json['version'] ?? ''}',
      displayName: '${json['displayName'] ?? ''}',
      model: '${json['model'] ?? ''}',
      reasoningEffort: '${json['reasoningEffort'] ?? ''}',
      instructionText: '${json['instructionText'] ?? ''}',
      capabilities: (json['capabilities'] as List<dynamic>? ?? const []).map((value) => '$value').toList(growable: false),
      policy: _objects(json['policy']).map(AgentRuntimeRolePolicyRow.fromJson).toList(growable: false),
      routingMode: '${json['routingMode'] ?? 'direct'}',
      routingReservedActions: (json['routingReservedActions'] as List<dynamic>? ?? const []).map((value) => '$value').toList(growable: false),
      defaultRecipient: json['defaultRecipient'] as String?,
      allowedRecipients: (json['allowedRecipients'] as List<dynamic>? ?? const []).map((value) => '$value').toList(growable: false),
      listed: json['listed'] == true,
      ownerVisible: json['ownerVisible'] == true,
      canSpawnAgents: json['canSpawnAgents'] == true,
      canArchiveAgents: json['canArchiveAgents'] == true,
      lifecycleReservedActions: (json['lifecycleReservedActions'] as List<dynamic>? ?? const []).map((value) => '$value').toList(growable: false),
    );
  }

  Map<String, Object?> toDraftJson() {
    return {
      'id': roleId,
      'version': version,
      'displayName': displayName,
      'modelDefaults': {
        'model': model,
        'reasoningEffort': reasoningEffort,
      },
      'instructionText': instructionText,
      'capabilities': capabilities,
      'policy': {for (final row in policy) row.action: row.decision},
      'routing': {
        'mode': routingMode,
        'defaultRecipient': defaultRecipient,
        'allowedRecipients': allowedRecipients,
        'reservedActions': routingReservedActions,
      },
      'visibility': {
        'listed': listed,
        'ownerVisible': ownerVisible,
      },
      'lifecycleAuthority': {
        'canSpawnAgents': canSpawnAgents,
        'canArchiveAgents': canArchiveAgents,
        'reservedActions': lifecycleReservedActions,
      },
    };
  }
}

Iterable<Map<String, dynamic>> _objects(Object? value) {
  return (value as List<dynamic>? ?? const [])
      .whereType<Map>()
      .map((item) => Map<String, dynamic>.from(item));
}

List<String> _strings(Object? value) {
  return (value as List<dynamic>? ?? const []).map((item) => '$item').where((item) => item.isNotEmpty).toList(growable: false);
}
