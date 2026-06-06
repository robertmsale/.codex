import 'package:robdex_design_system/robdex_design_system.dart';

const int _maxCommandLength = 1200;
const int _maxOutputLength = 2400;
const int _maxRequirementKeyCount = 20;
const int _maxRequirementVerdictCount = 20;

class DomMirrorSnapshot {
  const DomMirrorSnapshot({
    required this.generatedAt,
    required this.connectionLabel,
    required this.statusHeadline,
    required this.statusDetail,
    required this.selection,
    required this.projects,
    required this.chatEntries,
    required this.pendingApprovals,
    required this.requirementsReview,
    required this.liveProcesses,
    required this.composerVisible,
  });

  final int generatedAt;
  final String connectionLabel;
  final String statusHeadline;
  final String statusDetail;
  final DomMirrorSelection selection;
  final List<DomMirrorProject> projects;
  final List<DomMirrorChatEntry> chatEntries;
  final List<DomMirrorPendingApproval> pendingApprovals;
  final DomMirrorRequirementsReview? requirementsReview;
  final List<DomMirrorLiveProcess> liveProcesses;
  final bool composerVisible;

  factory DomMirrorSnapshot.fromWorkbench(WorkbenchViewData view) {
    final threadsByProject = <String, List<DomMirrorThread>>{};
    for (final thread in view.threads) {
      final projectKey = thread.projectId.isNotEmpty
          ? thread.projectId
          : thread.projectRootPath.isNotEmpty
              ? thread.projectRootPath
              : thread.projectName;
      final bucket = threadsByProject.putIfAbsent(
        projectKey,
        () => <DomMirrorThread>[],
      );
      final requirements = thread.requirementReview?.displayStatus;
      bucket.add(
        DomMirrorThread(
          id: thread.id,
          title: thread.title,
          role: thread.role,
          isRunning: thread.isRunning,
          unreadCount: thread.unreadCount,
          requirementReviewStatus: requirements,
        ),
      );
    }
    for (final projectThreads in threadsByProject.values) {
      projectThreads.sort((left, right) {
        final roleCompare = _compareMirrorThreadRoles(left.role, right.role);
        if (roleCompare != 0) {
          return roleCompare;
        }
        return left.title.compareTo(right.title);
      });
    }

    final projects = view.projects
        .map(
          (project) => DomMirrorProject(
            id: project.id,
            name: project.name,
            rootPath: project.rootPath,
            threads: threadsByProject[project.id] ??
                threadsByProject[project.rootPath] ??
                threadsByProject[project.name] ??
                const <DomMirrorThread>[],
          ),
        )
        .toList(growable: false)
      ..sort((left, right) => left.name.compareTo(right.name));

    final selectedRequirementReview = view.requirementReview == null
        ? null
        : DomMirrorRequirementsReview.fromModel(view.requirementReview!);

    return DomMirrorSnapshot(
      generatedAt: DateTime.now().millisecondsSinceEpoch,
      connectionLabel: view.selection.connectionLabel,
      statusHeadline: view.statusHeadline,
      statusDetail: view.statusDetail,
      selection: DomMirrorSelection(
        projectName: view.selection.projectName,
        projectRootPath: view.selection.projectRootPath,
        threadId: view.selection.threadId,
        threadName: view.selection.threadName,
        threadRole: view.selection.threadRole,
        model: view.selection.model,
        approvalPolicy: view.selection.approvalPolicy,
        sandboxMode: view.selection.effectiveSandboxMode ?? view.selection.sandboxMode,
        networkAccess: view.selection.effectiveNetworkAccess ?? view.selection.networkAccess,
        serviceTier: view.selection.effectiveServiceTier ?? view.selection.serviceTier,
        reasoningEffort: view.selection.effectiveReasoningEffort ?? view.selection.reasoningEffort,
      ),
      projects: projects,
      chatEntries: view.chatEntries
          .map(
            (entry) => DomMirrorChatEntry(
              id: entry.id,
              author: entry.author,
              displayLabel: entry.displayLabel,
              kind: entry.kind,
              status: entry.status,
              timestamp: entry.timestamp,
              body: entry.body,
              command: _truncateText(entry.command, _maxCommandLength),
              outputPreview: _truncateText(entry.output, _maxOutputLength),
              isTool: entry.isTool,
              isStreaming: entry.isStreaming,
            ),
          )
          .toList(growable: false),
      pendingApprovals: view.pendingApprovals
          .map(
            (approval) => DomMirrorPendingApproval(
              id: approval.id,
              threadId: approval.threadId,
              kind: approval.kind,
              title: approval.title,
              detail: approval.detail,
              command: approval.command,
              commandCwd: approval.commandCwd,
              filePaths: approval.filePaths,
            ),
          )
          .toList(growable: false),
      requirementsReview: selectedRequirementReview,
      liveProcesses: view.liveProcesses
          .map(
            (process) => DomMirrorLiveProcess(
              processId: process.processId,
              pid: process.pid,
              processGroupId: process.processGroupId,
              command: process.command,
              cwd: process.cwd,
              startedAt: process.startedAt,
            ),
          )
          .toList(growable: false),
      composerVisible: view.selection.threadId != null,
    );
  }

  Map<String, Object?> toJson() {
    return {
      'generatedAt': generatedAt,
      'connectionLabel': connectionLabel,
      'statusHeadline': statusHeadline,
      'statusDetail': statusDetail,
      'selection': selection.toJson(),
      'projects': projects.map((project) => project.toJson()).toList(),
      'chatEntries': chatEntries.map((entry) => entry.toJson()).toList(),
      'pendingApprovals': pendingApprovals.map((approval) => approval.toJson()).toList(),
      'requirementsReview': requirementsReview?.toJson(),
      'liveProcesses': liveProcesses.map((process) => process.toJson()).toList(),
      'composerVisible': composerVisible,
    };
  }
}

class DomMirrorSelection {
  const DomMirrorSelection({
    required this.projectName,
    required this.projectRootPath,
    required this.threadId,
    required this.threadName,
    required this.threadRole,
    required this.model,
    required this.approvalPolicy,
    required this.sandboxMode,
    required this.networkAccess,
    required this.serviceTier,
    required this.reasoningEffort,
  });

  final String projectName;
  final String? projectRootPath;
  final String? threadId;
  final String threadName;
  final String? threadRole;
  final String? model;
  final String? approvalPolicy;
  final String? sandboxMode;
  final bool? networkAccess;
  final String? serviceTier;
  final String? reasoningEffort;

  Map<String, Object?> toJson() => {
        'projectName': projectName,
        'projectRootPath': projectRootPath,
        'threadId': threadId,
        'threadName': threadName,
        'threadRole': threadRole,
        'model': model,
        'approvalPolicy': approvalPolicy,
        'sandboxMode': sandboxMode,
        'networkAccess': networkAccess,
        'serviceTier': serviceTier,
        'reasoningEffort': reasoningEffort,
      };
}

class DomMirrorProject {
  const DomMirrorProject({
    required this.id,
    required this.name,
    required this.rootPath,
    required this.threads,
  });

  final String id;
  final String name;
  final String rootPath;
  final List<DomMirrorThread> threads;

  Map<String, Object?> toJson() => {
        'id': id,
        'name': name,
        'rootPath': rootPath,
        'threads': threads.map((thread) => thread.toJson()).toList(),
      };
}

class DomMirrorThread {
  const DomMirrorThread({
    required this.id,
    required this.title,
    required this.role,
    required this.isRunning,
    required this.unreadCount,
    required this.requirementReviewStatus,
  });

  final String id;
  final String title;
  final String role;
  final bool isRunning;
  final int unreadCount;
  final String? requirementReviewStatus;

  Map<String, Object?> toJson() => {
        'id': id,
        'title': title,
        'role': role,
        'isRunning': isRunning,
        'unreadCount': unreadCount,
        'requirementReviewStatus': requirementReviewStatus,
      };
}

class DomMirrorChatEntry {
  const DomMirrorChatEntry({
    required this.id,
    required this.author,
    required this.displayLabel,
    required this.kind,
    required this.status,
    required this.timestamp,
    required this.body,
    required this.command,
    required this.outputPreview,
    required this.isTool,
    required this.isStreaming,
  });

  final String id;
  final String author;
  final String displayLabel;
  final String? kind;
  final String? status;
  final int? timestamp;
  final String? body;
  final String? command;
  final String? outputPreview;
  final bool isTool;
  final bool isStreaming;

  Map<String, Object?> toJson() => {
        'id': id,
        'author': author,
        'displayLabel': displayLabel,
        'kind': kind,
        'status': status,
        'timestamp': timestamp,
        'body': body,
        'command': command,
        'outputPreview': outputPreview,
        'isTool': isTool,
        'isStreaming': isStreaming,
      };
}

class DomMirrorPendingApproval {
  const DomMirrorPendingApproval({
    required this.id,
    required this.threadId,
    required this.kind,
    required this.title,
    required this.detail,
    required this.command,
    required this.commandCwd,
    required this.filePaths,
  });

  final String id;
  final String threadId;
  final String kind;
  final String title;
  final String? detail;
  final String? command;
  final String? commandCwd;
  final List<String> filePaths;

  Map<String, Object?> toJson() => {
        'id': id,
        'threadId': threadId,
        'kind': kind,
        'title': title,
        'detail': detail,
        'command': command,
        'commandCwd': commandCwd,
        'filePaths': filePaths,
      };
}

class DomMirrorRequirementsReview {
  const DomMirrorRequirementsReview({
    required this.status,
    required this.reviewerThreadId,
    required this.activeRequirementCount,
    required this.passedCount,
    required this.failedCount,
    required this.blockedCount,
    required this.failedKeys,
    required this.blockedKeys,
    required this.failedVerdicts,
    required this.blockedVerdicts,
  });

  final String status;
  final String? reviewerThreadId;
  final int activeRequirementCount;
  final int passedCount;
  final int failedCount;
  final int blockedCount;
  final List<String> failedKeys;
  final List<String> blockedKeys;
  final List<DomMirrorRequirementVerdict> failedVerdicts;
  final List<DomMirrorRequirementVerdict> blockedVerdicts;

  factory DomMirrorRequirementsReview.fromModel(RequirementReviewSummary summary) {
    final failedKeys = <String>[];
    final blockedKeys = <String>[];
    final failedVerdicts = <DomMirrorRequirementVerdict>[];
    final blockedVerdicts = <DomMirrorRequirementVerdict>[];
    for (final verdict in summary.verdicts) {
      final normalized = (verdict.verdict ?? '').toLowerCase();
      final isFailed = switch (normalized) {
        'fail' || 'rejectedblocked' || 'blocked' => true,
        _ => false,
      };
      final isBlocked = switch (normalized) {
        'acceptedblocked' || 'rejectedblocked' || 'blocked' => true,
        _ => false,
      };
      if (isFailed) {
        if (failedKeys.length < _maxRequirementKeyCount) {
          failedKeys.add(verdict.key);
        }
        if (failedVerdicts.length < _maxRequirementVerdictCount) {
          failedVerdicts.add(
            DomMirrorRequirementVerdict(
              key: verdict.key,
              verdict: verdict.verdict,
              reason: verdict.reason,
              evidenceAssessment: verdict.evidenceAssessment,
              requiredCorrection: verdict.requiredCorrection,
            ),
          );
        }
      }
      if (isBlocked) {
        if (blockedKeys.length < _maxRequirementKeyCount) {
          blockedKeys.add(verdict.key);
        }
        if (blockedVerdicts.length < _maxRequirementVerdictCount) {
          blockedVerdicts.add(
            DomMirrorRequirementVerdict(
              key: verdict.key,
              verdict: verdict.verdict,
              reason: verdict.reason,
              evidenceAssessment: verdict.evidenceAssessment,
              requiredCorrection: verdict.requiredCorrection,
            ),
          );
        }
      }
    }
    return DomMirrorRequirementsReview(
      status: summary.displayStatus,
      reviewerThreadId: summary.reviewerThreadId,
      activeRequirementCount: summary.activeRequirementCount,
      passedCount: summary.passedCount,
      failedCount: summary.failedCount,
      blockedCount: summary.blockedCount,
      failedKeys: failedKeys,
      blockedKeys: blockedKeys,
      failedVerdicts: failedVerdicts,
      blockedVerdicts: blockedVerdicts,
    );
  }

  Map<String, Object?> toJson() => {
        'status': status,
        'reviewerThreadId': reviewerThreadId,
        'activeRequirementCount': activeRequirementCount,
        'passedCount': passedCount,
        'failedCount': failedCount,
        'blockedCount': blockedCount,
        'failedKeys': failedKeys,
        'blockedKeys': blockedKeys,
        'failedVerdicts': failedVerdicts.map((verdict) => verdict.toJson()).toList(),
        'blockedVerdicts': blockedVerdicts.map((verdict) => verdict.toJson()).toList(),
      };
}

class DomMirrorRequirementVerdict {
  const DomMirrorRequirementVerdict({
    required this.key,
    this.verdict,
    this.reason,
    this.evidenceAssessment,
    this.requiredCorrection,
  });

  final String key;
  final String? verdict;
  final String? reason;
  final String? evidenceAssessment;
  final String? requiredCorrection;

  Map<String, Object?> toJson() => {
        'key': key,
        'verdict': verdict,
        'reason': reason,
        'evidenceAssessment': evidenceAssessment,
        'requiredCorrection': requiredCorrection,
      };
}

int _compareMirrorThreadRoles(String leftRole, String rightRole) {
  const mirrorRoleOrder = <String, int>{
    'operator': 0,
    'orchestrator': 1,
    'worker': 2,
    'designer': 3,
    'qa': 4,
    'hidden': 5,
  };
  final leftNormalized = leftRole.toLowerCase();
  final rightNormalized = rightRole.toLowerCase();
  final leftOrder = mirrorRoleOrder[leftNormalized] ?? 6;
  final rightOrder = mirrorRoleOrder[rightNormalized] ?? 6;
  final roleCompare = leftOrder.compareTo(rightOrder);
  if (roleCompare != 0) {
    return roleCompare;
  }
  return leftNormalized.compareTo(rightNormalized);
}

class DomMirrorLiveProcess {
  const DomMirrorLiveProcess({
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

  Map<String, Object?> toJson() => {
        'processId': processId,
        'pid': pid,
        'processGroupId': processGroupId,
        'command': command,
        'cwd': cwd,
        'startedAt': startedAt,
      };
}

String? _truncateText(String? value, int maxLength) {
  if (value == null || value.isEmpty) {
    return null;
  }
  if (value.length <= maxLength) {
    return value;
  }
  return '${value.substring(0, maxLength)}…';
}
