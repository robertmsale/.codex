import 'workbench_models.dart';

class WorkbenchViewData {
  const WorkbenchViewData({
    required this.projects,
    required this.selection,
    required this.threads,
    required this.availableModels,
    required this.threadGroups,
    required this.liveProcesses,
    required this.chatEntries,
    required this.contextWindowRemainingPercent,
    required this.workspaceFiles,
    required this.inspectorFacts,
    required this.pendingApprovals,
    required this.workerMetadata,
    required this.statusHeadline,
    required this.statusDetail,
    required this.composerHint,
  });

  final List<ProjectItem> projects;
  final WorkspaceSelection selection;
  final List<ThreadItem> threads;
  final List<ModelItem> availableModels;
  final List<ThreadGroupItem> threadGroups;
  final List<LiveProcessItem> liveProcesses;
  final List<ChatEntry> chatEntries;
  final int? contextWindowRemainingPercent;
  final List<WorkspaceFile> workspaceFiles;
  final List<InspectorFact> inspectorFacts;
  final List<PendingApprovalItem> pendingApprovals;
  final WorkerMetadata? workerMetadata;
  final String statusHeadline;
  final String statusDetail;
  final String composerHint;

  factory WorkbenchViewData.fromJson(Map<String, dynamic> json) {
    List<T> decodeList<T>(
      String key,
      T Function(Map<String, dynamic>) decode,
    ) {
      return (json[key] as List<dynamic>? ?? const [])
          .whereType<Map<String, dynamic>>()
          .map(decode)
          .toList(growable: false);
    }

    return WorkbenchViewData(
      projects: decodeList('projects', ProjectItem.fromJson),
      selection: WorkspaceSelection.fromJson(
        (json['selection'] as Map<String, dynamic>? ?? const {}),
      ),
      threads: decodeList('threads', ThreadItem.fromJson),
      availableModels: decodeList('availableModels', ModelItem.fromJson),
      threadGroups: decodeList('threadGroups', ThreadGroupItem.fromJson),
      liveProcesses: decodeList('liveProcesses', LiveProcessItem.fromJson),
      chatEntries: decodeList('chatEntries', ChatEntry.fromJson),
      contextWindowRemainingPercent: json['contextWindowRemainingPercent'] as int?,
      workspaceFiles: decodeList('workspaceFiles', WorkspaceFile.fromJson),
      inspectorFacts: decodeList('inspectorFacts', InspectorFact.fromJson),
      pendingApprovals:
          decodeList('pendingApprovals', PendingApprovalItem.fromJson),
      workerMetadata: (json['workerMetadata'] as Map<String, dynamic>?) == null
          ? null
          : WorkerMetadata.fromJson(
              json['workerMetadata'] as Map<String, dynamic>,
            ),
      statusHeadline: (json['statusHeadline'] as String?) ?? 'Bridge Unknown',
      statusDetail: (json['statusDetail'] as String?) ?? '',
      composerHint: (json['composerHint'] as String?) ?? '',
    );
  }

  WorkbenchViewData copyWith({
    List<ProjectItem>? projects,
    WorkspaceSelection? selection,
    List<ThreadItem>? threads,
    List<ModelItem>? availableModels,
    List<ThreadGroupItem>? threadGroups,
    List<LiveProcessItem>? liveProcesses,
    List<ChatEntry>? chatEntries,
    int? contextWindowRemainingPercent,
    List<WorkspaceFile>? workspaceFiles,
    List<InspectorFact>? inspectorFacts,
    List<PendingApprovalItem>? pendingApprovals,
    WorkerMetadata? workerMetadata,
    String? statusHeadline,
    String? statusDetail,
    String? composerHint,
  }) {
    return WorkbenchViewData(
      projects: projects ?? this.projects,
      selection: selection ?? this.selection,
      threads: threads ?? this.threads,
      availableModels: availableModels ?? this.availableModels,
      threadGroups: threadGroups ?? this.threadGroups,
      liveProcesses: liveProcesses ?? this.liveProcesses,
      chatEntries: chatEntries ?? this.chatEntries,
      contextWindowRemainingPercent:
          contextWindowRemainingPercent ?? this.contextWindowRemainingPercent,
      workspaceFiles: workspaceFiles ?? this.workspaceFiles,
      inspectorFacts: inspectorFacts ?? this.inspectorFacts,
      pendingApprovals: pendingApprovals ?? this.pendingApprovals,
      workerMetadata: workerMetadata ?? this.workerMetadata,
      statusHeadline: statusHeadline ?? this.statusHeadline,
      statusDetail: statusDetail ?? this.statusDetail,
      composerHint: composerHint ?? this.composerHint,
    );
  }
}
