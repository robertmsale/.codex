class WorkspaceSelection {
  const WorkspaceSelection({
    required this.projectId,
    required this.projectRootPath,
    required this.projectOrchestratorThreadId,
    required this.projectOrchestratorName,
    required this.threadId,
    required this.threadRole,
    required this.projectName,
    required this.threadName,
    required this.connectionLabel,
    this.sandboxMode,
    this.networkAccess,
    this.approvalPolicy,
    this.model,
    this.reasoningEffort,
    this.serviceTier,
    this.effectiveSandboxMode,
    this.effectiveNetworkAccess,
    this.effectiveApprovalPolicy,
    this.effectiveModel,
    this.effectiveReasoningEffort,
    this.effectiveServiceTier,
    this.isRunning = false,
  });

  final String? projectId;
  final String? projectRootPath;
  final String? projectOrchestratorThreadId;
  final String? projectOrchestratorName;
  final String? threadId;
  final String? threadRole;
  final String projectName;
  final String threadName;
  final String connectionLabel;
  final String? sandboxMode;
  final bool? networkAccess;
  final String? approvalPolicy;
  final String? model;
  final String? reasoningEffort;
  final String? serviceTier;
  final String? effectiveSandboxMode;
  final bool? effectiveNetworkAccess;
  final String? effectiveApprovalPolicy;
  final String? effectiveModel;
  final String? effectiveReasoningEffort;
  final String? effectiveServiceTier;
  final bool isRunning;

  factory WorkspaceSelection.fromJson(Map<String, dynamic> json) {
    return WorkspaceSelection(
      projectId: json['projectId'] as String?,
      projectRootPath: json['projectRootPath'] as String?,
      projectOrchestratorThreadId: json['projectOrchestratorThreadId'] as String?,
      projectOrchestratorName: json['projectOrchestratorName'] as String?,
      threadId: json['threadId'] as String?,
      threadRole: json['threadRole'] as String?,
      projectName: (json['projectName'] as String?) ?? 'No Project',
      threadName: (json['threadName'] as String?) ?? 'No Thread Selected',
      connectionLabel: (json['connectionLabel'] as String?) ?? 'Bridge Unknown',
      sandboxMode: json['sandboxMode'] as String?,
      networkAccess: json['networkAccess'] as bool?,
      approvalPolicy: json['approvalPolicy'] as String?,
      model: json['model'] as String?,
      reasoningEffort: json['reasoningEffort'] as String?,
      serviceTier: json['serviceTier'] as String?,
      effectiveSandboxMode: json['effectiveSandboxMode'] as String?,
      effectiveNetworkAccess: json['effectiveNetworkAccess'] as bool?,
      effectiveApprovalPolicy: json['effectiveApprovalPolicy'] as String?,
      effectiveModel: json['effectiveModel'] as String?,
      effectiveReasoningEffort: json['effectiveReasoningEffort'] as String?,
      effectiveServiceTier: json['effectiveServiceTier'] as String?,
      isRunning: json['isRunning'] as bool? ?? false,
    );
  }
}

class ThreadGroupItem {
  const ThreadGroupItem({
    required this.id,
    required this.title,
    required this.threadIds,
    required this.isCollapsed,
  });

  final String id;
  final String title;
  final List<String> threadIds;
  final bool isCollapsed;

  factory ThreadGroupItem.fromJson(Map<String, dynamic> json) {
    return ThreadGroupItem(
      id: json['id'] as String? ?? '',
      title: json['title'] as String? ?? 'Group',
      threadIds: (json['threadIds'] as List<dynamic>? ?? const [])
          .whereType<String>()
          .toList(growable: false),
      isCollapsed: json['isCollapsed'] as bool? ?? false,
    );
  }
}

class WorkerMetadata {
  const WorkerMetadata({
    required this.threadId,
    this.issueNumber,
    this.pullRequestNumber,
    this.blockedReason,
    this.unblockWhen,
  });

  final String threadId;
  final int? issueNumber;
  final int? pullRequestNumber;
  final String? blockedReason;
  final String? unblockWhen;

  factory WorkerMetadata.fromJson(Map<String, dynamic> json) {
    return WorkerMetadata(
      threadId: json['threadId'] as String? ?? '',
      issueNumber: json['issueNumber'] as int?,
      pullRequestNumber: json['pullRequestNumber'] as int?,
      blockedReason: json['blockedReason'] as String?,
      unblockWhen: json['unblockWhen'] as String?,
    );
  }
}

class ProjectItem {
  const ProjectItem({
    required this.id,
    required this.name,
    required this.rootPath,
    required this.defaultCwd,
    required this.autoRouteReplies,
    required this.routeApprovalRequests,
    required this.preferredModelProvider,
    required this.orchestratorDefaultModel,
    required this.orchestratorDefaultReasoningEffort,
    required this.workerDefaultModel,
    required this.workerDefaultReasoningEffort,
    required this.qaDefaultModel,
    required this.qaDefaultReasoningEffort,
    required this.orchestratorDeveloperInstructions,
    required this.workerDeveloperInstructions,
    required this.qaDeveloperInstructions,
    required this.operatorDeveloperInstructions,
    required this.hiddenDeveloperInstructions,
    required this.isSelected,
  });

  final String id;
  final String name;
  final String rootPath;
  final String defaultCwd;
  final bool autoRouteReplies;
  final bool routeApprovalRequests;
  final String? preferredModelProvider;
  final String? orchestratorDefaultModel;
  final String? orchestratorDefaultReasoningEffort;
  final String? workerDefaultModel;
  final String? workerDefaultReasoningEffort;
  final String? qaDefaultModel;
  final String? qaDefaultReasoningEffort;
  final String? orchestratorDeveloperInstructions;
  final String? workerDeveloperInstructions;
  final String? qaDeveloperInstructions;
  final String? operatorDeveloperInstructions;
  final String? hiddenDeveloperInstructions;
  final bool isSelected;

  factory ProjectItem.fromJson(Map<String, dynamic> json) {
    return ProjectItem(
      id: json['id'] as String? ?? '',
      name: json['name'] as String? ?? '',
      rootPath: json['rootPath'] as String? ?? '',
      defaultCwd: json['defaultCwd'] as String? ?? '',
      autoRouteReplies: json['autoRouteReplies'] as bool? ?? false,
      routeApprovalRequests: json['routeApprovalRequests'] as bool? ?? false,
      preferredModelProvider: json['preferredModelProvider'] as String?,
      orchestratorDefaultModel: json['orchestratorDefaultModel'] as String?,
      orchestratorDefaultReasoningEffort:
          json['orchestratorDefaultReasoningEffort'] as String?,
      workerDefaultModel: json['workerDefaultModel'] as String?,
      workerDefaultReasoningEffort: json['workerDefaultReasoningEffort'] as String?,
      qaDefaultModel: json['qaDefaultModel'] as String?,
      qaDefaultReasoningEffort: json['qaDefaultReasoningEffort'] as String?,
      orchestratorDeveloperInstructions: json['orchestratorDeveloperInstructions'] as String?,
      workerDeveloperInstructions: json['workerDeveloperInstructions'] as String?,
      qaDeveloperInstructions: json['qaDeveloperInstructions'] as String?,
      operatorDeveloperInstructions: json['operatorDeveloperInstructions'] as String?,
      hiddenDeveloperInstructions: json['hiddenDeveloperInstructions'] as String?,
      isSelected: json['isSelected'] as bool? ?? false,
    );
  }
}

class ThreadItem {
  const ThreadItem({
    required this.id,
    required this.title,
    required this.role,
    required this.projectName,
    required this.preview,
    required this.isRunning,
    required this.unreadCount,
  });

  final String id;
  final String title;
  final String role;
  final String projectName;
  final String preview;
  final bool isRunning;
  final int unreadCount;

  factory ThreadItem.fromJson(Map<String, dynamic> json) {
    return ThreadItem(
      id: json['id'] as String? ?? '',
      title: json['title'] as String? ?? '',
      role: json['role'] as String? ?? 'worker',
      projectName: json['projectName'] as String? ?? '',
      preview: json['preview'] as String? ?? '',
      isRunning: json['isRunning'] as bool? ?? false,
      unreadCount: json['unreadCount'] as int? ?? 0,
    );
  }
}

class ModelItem {
  const ModelItem({
    required this.id,
    required this.name,
    required this.hidden,
  });

  final String id;
  final String? name;
  final bool hidden;

  factory ModelItem.fromJson(Map<String, dynamic> json) {
    return ModelItem(
      id: json['id'] as String? ?? '',
      name: json['name'] as String?,
      hidden: json['hidden'] as bool? ?? false,
    );
  }
}

class LiveProcessItem {
  const LiveProcessItem({
    required this.processId,
    required this.pid,
    required this.processGroupId,
    required this.command,
    required this.cwd,
    required this.startedAt,
  });

  final String processId;
  final int? pid;
  final int? processGroupId;
  final String command;
  final String? cwd;
  final int? startedAt;

  factory LiveProcessItem.fromJson(Map<String, dynamic> json) {
    int? parseInt(dynamic value) {
      if (value is int) {
        return value;
      }
      if (value is String) {
        return int.tryParse(value);
      }
      return null;
    }

    return LiveProcessItem(
      processId: json['processId'] as String? ?? '',
      pid: parseInt(json['pid']),
      processGroupId: parseInt(json['processGroupId']),
      command: json['command'] as String? ?? '',
      cwd: json['cwd'] as String?,
      startedAt: parseInt(json['startedAt']),
    );
  }
}

class ChatEntry {
  const ChatEntry({
    required this.id,
    required this.author,
    required this.displayLabel,
    required this.timestampLabel,
    required this.body,
    this.subtitle,
    this.kind,
    this.status,
    this.processId,
    this.command,
    this.output,
    this.deliveryState,
    this.planItems = const <PlanChecklistItem>[],
    this.isStreaming = false,
    this.isTool = false,
  });

  final String id;
  final String author;
  final String displayLabel;
  final String timestampLabel;
  final String body;
  final String? subtitle;
  final String? kind;
  final String? status;
  final String? processId;
  final String? command;
  final String? output;
  final String? deliveryState;
  final List<PlanChecklistItem> planItems;
  final bool isStreaming;
  final bool isTool;

  bool get hasPlanItems => planItems.isNotEmpty;

  factory ChatEntry.fromJson(Map<String, dynamic> json) {
    return ChatEntry(
      id: json['id'] as String? ?? '',
      author: json['author'] as String? ?? 'Unknown',
      displayLabel: json['displayLabel'] as String? ?? json['author'] as String? ?? 'Unknown',
      timestampLabel: json['timestampLabel'] as String? ?? 'now',
      body: json['body'] as String? ?? '',
      subtitle: json['subtitle'] as String?,
      kind: json['kind'] as String?,
      status: json['status'] as String?,
      processId: json['processId'] as String?,
      command: json['command'] as String?,
      output: json['output'] as String?,
      deliveryState: json['deliveryState'] as String?,
      planItems: (json['planItems'] as List<dynamic>? ?? const [])
          .whereType<Map<String, dynamic>>()
          .map(PlanChecklistItem.fromJson)
          .toList(growable: false),
      isStreaming: json['isStreaming'] as bool? ?? false,
      isTool: json['isTool'] as bool? ?? false,
    );
  }
}

class PlanChecklistItem {
  const PlanChecklistItem({
    required this.text,
    required this.completed,
    this.status,
  });

  final String text;
  final bool completed;
  final String? status;

  bool get isInProgress => status?.toLowerCase() == 'in_progress';

  factory PlanChecklistItem.fromJson(Map<String, dynamic> json) {
    return PlanChecklistItem(
      text: json['text'] as String? ?? '',
      completed: json['completed'] as bool? ?? false,
      status: json['status'] as String?,
    );
  }
}

class WorkspaceFile {
  const WorkspaceFile({
    required this.path,
    required this.kind,
    required this.status,
  });

  final String path;
  final String kind;
  final String status;

  factory WorkspaceFile.fromJson(Map<String, dynamic> json) {
    return WorkspaceFile(
      path: json['path'] as String? ?? '',
      kind: json['kind'] as String? ?? '',
      status: json['status'] as String? ?? '',
    );
  }
}

class InspectorFact {
  const InspectorFact({
    required this.label,
    required this.value,
  });

  final String label;
  final String value;

  factory InspectorFact.fromJson(Map<String, dynamic> json) {
    return InspectorFact(
      label: json['label'] as String? ?? '',
      value: json['value'] as String? ?? '',
    );
  }
}

class PendingApprovalItem {
  const PendingApprovalItem({
    required this.id,
    required this.threadId,
    required this.kind,
    required this.title,
    required this.detail,
    this.command,
    this.commandCwd,
    this.filePaths = const <String>[],
  });

  final String id;
  final String threadId;
  final String kind;
  final String title;
  final String? detail;
  final String? command;
  final String? commandCwd;
  final List<String> filePaths;

  factory PendingApprovalItem.fromJson(Map<String, dynamic> json) {
    final filePaths = (json['filePaths'] as List<dynamic>? ?? const [])
        .whereType<String>()
        .toList(growable: false);
    return PendingApprovalItem(
      id: json['id'] as String? ?? '',
      threadId: json['threadId'] as String? ?? '',
      kind: json['kind'] as String? ?? 'approval',
      title: json['title'] as String? ?? 'Approval Request',
      detail: json['detail'] as String?,
      command: json['command'] as String?,
      commandCwd: json['commandCwd'] as String?,
      filePaths: filePaths,
    );
  }
}

class WorkbenchData {
  const WorkbenchData({
    required this.selection,
    required this.threads,
    required this.threadGroups,
    required this.chatEntries,
    required this.workspaceFiles,
    required this.inspectorFacts,
  });

  final WorkspaceSelection selection;
  final List<ThreadItem> threads;
  final List<ThreadGroupItem> threadGroups;
  final List<ChatEntry> chatEntries;
  final List<WorkspaceFile> workspaceFiles;
  final List<InspectorFact> inspectorFacts;
}
