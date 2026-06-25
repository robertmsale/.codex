import 'dart:convert';

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

class GlobalSettings {
  const GlobalSettings({
    this.approvalPolicy,
    this.sandboxMode,
    this.networkAccess,
  });

  final String? approvalPolicy;
  final String? sandboxMode;
  final bool? networkAccess;

  factory GlobalSettings.fromJson(Map<String, dynamic> json) {
    return GlobalSettings(
      approvalPolicy: json['approvalPolicy'] as String?,
      sandboxMode: json['sandboxMode'] as String?,
      networkAccess: json['networkAccess'] as bool?,
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
    this.defaultModel,
    this.defaultReasoningEffort,
    this.defaultSandboxMode,
    this.defaultApprovalPolicy,
    this.defaultNetworkAccess,
    this.globalDefaultSandboxMode,
    this.globalDefaultApprovalPolicy,
    this.globalDefaultNetworkAccess,
    required this.roleRuntimeDefaults,
    required this.orchestratorDefaultModel,
    required this.orchestratorDefaultReasoningEffort,
    required this.workerDefaultModel,
    required this.workerDefaultReasoningEffort,
    required this.qaDefaultModel,
    required this.qaDefaultReasoningEffort,
    required this.designerDefaultModel,
    required this.designerDefaultReasoningEffort,
    this.plannerDefaultModel,
    this.plannerDefaultReasoningEffort,
    required this.requirementsReviewerDefaultModel,
    required this.requirementsReviewerDefaultReasoningEffort,
    required this.orchestratorDeveloperInstructions,
    required this.workerDeveloperInstructions,
    required this.qaDeveloperInstructions,
    required this.designerDeveloperInstructions,
    required this.operatorDeveloperInstructions,
    required this.hiddenDeveloperInstructions,
    required this.permanentRequirementComposables,
    required this.manifestRuns,
    required this.isSelected,
  });

  final String id;
  final String name;
  final String rootPath;
  final String defaultCwd;
  final bool autoRouteReplies;
  final bool routeApprovalRequests;
  final String? preferredModelProvider;
  final String? defaultModel;
  final String? defaultReasoningEffort;
  final String? defaultSandboxMode;
  final String? defaultApprovalPolicy;
  final bool? defaultNetworkAccess;
  final String? globalDefaultSandboxMode;
  final String? globalDefaultApprovalPolicy;
  final bool? globalDefaultNetworkAccess;
  final Map<String, RoleRuntimeDefaults> roleRuntimeDefaults;
  final String? orchestratorDefaultModel;
  final String? orchestratorDefaultReasoningEffort;
  final String? workerDefaultModel;
  final String? workerDefaultReasoningEffort;
  final String? qaDefaultModel;
  final String? qaDefaultReasoningEffort;
  final String? designerDefaultModel;
  final String? designerDefaultReasoningEffort;
  final String? plannerDefaultModel;
  final String? plannerDefaultReasoningEffort;
  final String? requirementsReviewerDefaultModel;
  final String? requirementsReviewerDefaultReasoningEffort;
  final String? orchestratorDeveloperInstructions;
  final String? workerDeveloperInstructions;
  final String? qaDeveloperInstructions;
  final String? designerDeveloperInstructions;
  final String? operatorDeveloperInstructions;
  final String? hiddenDeveloperInstructions;
  final List<String> permanentRequirementComposables;
  final List<ManifestRunSummary> manifestRuns;
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
      defaultModel: json['defaultModel'] as String?,
      defaultReasoningEffort: json['defaultReasoningEffort'] as String?,
      defaultSandboxMode: json['defaultSandboxMode'] as String?,
      defaultApprovalPolicy: json['defaultApprovalPolicy'] as String?,
      defaultNetworkAccess: json['defaultNetworkAccess'] as bool?,
      globalDefaultSandboxMode: json['globalDefaultSandboxMode'] as String?,
      globalDefaultApprovalPolicy: json['globalDefaultApprovalPolicy'] as String?,
      globalDefaultNetworkAccess: json['globalDefaultNetworkAccess'] as bool?,
      roleRuntimeDefaults: _roleRuntimeDefaultsFromJson(json['roleRuntimeDefaults']),
      orchestratorDefaultModel: json['orchestratorDefaultModel'] as String?,
      orchestratorDefaultReasoningEffort:
          json['orchestratorDefaultReasoningEffort'] as String?,
      workerDefaultModel: json['workerDefaultModel'] as String?,
      workerDefaultReasoningEffort: json['workerDefaultReasoningEffort'] as String?,
      qaDefaultModel: json['qaDefaultModel'] as String?,
      qaDefaultReasoningEffort: json['qaDefaultReasoningEffort'] as String?,
      designerDefaultModel: json['designerDefaultModel'] as String?,
      designerDefaultReasoningEffort:
          json['designerDefaultReasoningEffort'] as String?,
      plannerDefaultModel: json['plannerDefaultModel'] as String?,
      plannerDefaultReasoningEffort:
          json['plannerDefaultReasoningEffort'] as String?,
      requirementsReviewerDefaultModel:
          json['requirementsReviewerDefaultModel'] as String?,
      requirementsReviewerDefaultReasoningEffort:
          json['requirementsReviewerDefaultReasoningEffort'] as String?,
      orchestratorDeveloperInstructions: json['orchestratorDeveloperInstructions'] as String?,
      workerDeveloperInstructions: json['workerDeveloperInstructions'] as String?,
      qaDeveloperInstructions: json['qaDeveloperInstructions'] as String?,
      designerDeveloperInstructions: json['designerDeveloperInstructions'] as String?,
      operatorDeveloperInstructions: json['operatorDeveloperInstructions'] as String?,
      hiddenDeveloperInstructions: json['hiddenDeveloperInstructions'] as String?,
      permanentRequirementComposables:
          (json['permanentRequirementComposables'] as List<dynamic>? ?? const [])
              .whereType<String>()
              .toList(growable: false),
      manifestRuns: (json['manifestRuns'] as List<dynamic>? ?? const [])
          .whereType<Map<String, dynamic>>()
          .map(ManifestRunSummary.fromJson)
          .toList(growable: false),
      isSelected: json['isSelected'] as bool? ?? false,
    );
  }
}

Map<String, RoleRuntimeDefaults> _roleRuntimeDefaultsFromJson(Object? value) {
  if (value is! Map<String, dynamic>) {
    return const <String, RoleRuntimeDefaults>{};
  }
  return value.map((key, raw) {
    final defaults = raw is Map<String, dynamic>
        ? RoleRuntimeDefaults.fromJson(raw)
        : const RoleRuntimeDefaults();
    return MapEntry(key, defaults);
  });
}

class RoleRuntimeDefaults {
  const RoleRuntimeDefaults({
    this.approvalPolicy,
    this.sandboxMode,
    this.networkAccess,
  });

  final String? approvalPolicy;
  final String? sandboxMode;
  final bool? networkAccess;

  factory RoleRuntimeDefaults.fromJson(Map<String, dynamic> json) {
    return RoleRuntimeDefaults(
      approvalPolicy: json['approvalPolicy'] as String?,
      sandboxMode: json['sandboxMode'] as String?,
      networkAccess: json['networkAccess'] as bool?,
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
        if (approvalPolicy != null) 'approvalPolicy': approvalPolicy,
        if (sandboxMode != null) 'sandboxMode': sandboxMode,
        if (networkAccess != null) 'networkAccess': networkAccess,
      };
}

class ManifestRunSummary {
  const ManifestRunSummary({
    required this.runId,
    required this.planId,
    required this.title,
    required this.status,
    required this.currentPhaseId,
    required this.sourceHash,
    required this.phases,
  });

  final String runId;
  final String planId;
  final String title;
  final String status;
  final String? currentPhaseId;
  final String sourceHash;
  final List<ManifestPhaseSummary> phases;

  factory ManifestRunSummary.fromJson(Map<String, dynamic> json) {
    return ManifestRunSummary(
      runId: json['runId'] as String? ?? '',
      planId: json['planId'] as String? ?? '',
      title: json['title'] as String? ?? '',
      status: json['status'] as String? ?? '',
      currentPhaseId: json['currentPhaseId'] as String?,
      sourceHash: json['sourceHash'] as String? ?? '',
      phases: (json['phases'] as List<dynamic>? ?? const [])
          .whereType<Map<String, dynamic>>()
          .map(ManifestPhaseSummary.fromJson)
          .toList(growable: false),
    );
  }
}

class ManifestPhaseSummary {
  const ManifestPhaseSummary({
    required this.phaseId,
    required this.title,
    required this.status,
    required this.workerThreadId,
    required this.archiveCleanupState,
    required this.archiveSafe,
    required this.hasHandoff,
    required this.hasBlocker,
    required this.hasWaiver,
    required this.hasResumeDecision,
  });

  final String phaseId;
  final String title;
  final String status;
  final String? workerThreadId;
  final String archiveCleanupState;
  final bool archiveSafe;
  final bool hasHandoff;
  final bool hasBlocker;
  final bool hasWaiver;
  final bool hasResumeDecision;

  factory ManifestPhaseSummary.fromJson(Map<String, dynamic> json) {
    return ManifestPhaseSummary(
      phaseId: json['phaseId'] as String? ?? '',
      title: json['title'] as String? ?? '',
      status: json['status'] as String? ?? '',
      workerThreadId: json['workerThreadId'] as String?,
      archiveCleanupState: json['archiveCleanupState'] as String? ?? '',
      archiveSafe: json['archiveSafe'] as bool? ?? false,
      hasHandoff: json['hasHandoff'] as bool? ?? false,
      hasBlocker: json['hasBlocker'] as bool? ?? false,
      hasWaiver: json['hasWaiver'] as bool? ?? false,
      hasResumeDecision: json['hasResumeDecision'] as bool? ?? false,
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
    required this.requirementReview,
    this.projectId = '',
    this.projectRootPath = '',
    this.projectOrchestratorThreadId,
    this.projectOrchestratorName,
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
  });

  final String id;
  final String title;
  final String role;
  final String projectId;
  final String projectRootPath;
  final String? projectOrchestratorThreadId;
  final String? projectOrchestratorName;
  final String projectName;
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
  final String preview;
  final bool isRunning;
  final int unreadCount;
  final RequirementReviewSummary? requirementReview;

  factory ThreadItem.fromJson(Map<String, dynamic> json) {
    return ThreadItem(
      id: json['id'] as String? ?? '',
      title: json['title'] as String? ?? '',
      role: json['role'] as String? ?? 'worker',
      projectId: json['projectId'] as String? ?? '',
      projectRootPath: json['projectRootPath'] as String? ?? '',
      projectOrchestratorThreadId: json['projectOrchestratorThreadId'] as String?,
      projectOrchestratorName: json['projectOrchestratorName'] as String?,
      projectName: json['projectName'] as String? ?? '',
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
      preview: json['preview'] as String? ?? '',
      isRunning: json['isRunning'] as bool? ?? false,
      unreadCount: json['unreadCount'] as int? ?? 0,
      requirementReview: (json['requirementReview'] as Map<String, dynamic>?) == null
          ? null
          : RequirementReviewSummary.fromJson(
              json['requirementReview'] as Map<String, dynamic>,
            ),
    );
  }
}

class RequirementReviewSummary {
  const RequirementReviewSummary({
    required this.activeRequirementCount,
    required this.storedRequirementCount,
    required this.requirementSetActive,
    required this.status,
    required this.reviewerThreadId,
    required this.parentThreadId,
    required this.requirementSetId,
    required this.latestClaimPacket,
    required this.latestVerdictPacket,
    required this.passedCount,
    required this.failedCount,
    required this.blockedCount,
    required this.waiverRequiredCount,
    required this.unknownCount,
    required this.updatedAt,
    required this.requirements,
    required this.verdicts,
  });

  final int activeRequirementCount;
  final int storedRequirementCount;
  final bool requirementSetActive;
  final String? status;
  final String? reviewerThreadId;
  final String? parentThreadId;
  final String? requirementSetId;
  final Map<String, dynamic>? latestClaimPacket;
  final Map<String, dynamic>? latestVerdictPacket;
  final int passedCount;
  final int failedCount;
  final int blockedCount;
  final int waiverRequiredCount;
  final int unknownCount;
  final int? updatedAt;
  final List<RequirementReviewRequirement> requirements;
  final List<RequirementVerdictSummary> verdicts;

  bool get hasActionableReview =>
      failedCount > 0 || blockedCount > 0 || waiverRequiredCount > 0;

  String get displayStatus {
    return switch (status) {
      'inReview' => 'In review',
      'passed' => 'Passed',
      'failed' => 'Failed',
      'blocked' => 'Blocked',
      'waiverRequired' => 'Human waiver required',
      _ => activeRequirementCount > 0 ? 'Requirements active' : 'No review',
    };
  }

  factory RequirementReviewSummary.fromJson(Map<String, dynamic> json) {
    return RequirementReviewSummary(
      activeRequirementCount: json['activeRequirementCount'] as int? ?? 0,
      storedRequirementCount: (json['storedRequirementCount'] as int?) ??
          (json['activeRequirementCount'] as int?) ??
          0,
      requirementSetActive: (json['requirementSetActive'] as bool?) ??
          ((json['activeRequirementCount'] as int? ?? 0) > 0),
      status: json['status'] as String?,
      reviewerThreadId: json['reviewerThreadId'] as String?,
      parentThreadId: json['parentThreadId'] as String?,
      requirementSetId: json['requirementSetId'] as String?,
      latestClaimPacket: json['latestClaimPacket'] as Map<String, dynamic>?,
      latestVerdictPacket: json['latestVerdictPacket'] as Map<String, dynamic>?,
      passedCount: json['passedCount'] as int? ?? 0,
      failedCount: json['failedCount'] as int? ?? 0,
      blockedCount: json['blockedCount'] as int? ?? 0,
      waiverRequiredCount: json['waiverRequiredCount'] as int? ?? 0,
      unknownCount: json['unknownCount'] as int? ?? 0,
      updatedAt: json['updatedAt'] as int?,
      requirements: (json['requirements'] as List<dynamic>? ?? const [])
          .whereType<Map<String, dynamic>>()
          .map(RequirementReviewRequirement.fromJson)
          .toList(growable: false),
      verdicts: (json['verdicts'] as List<dynamic>? ?? const [])
          .whereType<Map<String, dynamic>>()
          .map(RequirementVerdictSummary.fromJson)
          .toList(growable: false),
    );
  }
}

class RequirementReviewRequirement {
  const RequirementReviewRequirement({
    required this.key,
    required this.statement,
    required this.severity,
    required this.verificationMethod,
  });

  final String key;
  final String statement;
  final String severity;
  final String verificationMethod;

  factory RequirementReviewRequirement.fromJson(Map<String, dynamic> json) {
    return RequirementReviewRequirement(
      key: json['key'] as String? ?? '',
      statement: json['statement'] as String? ?? '',
      severity: json['severity'] as String? ?? 'medium',
      verificationMethod: json['verificationMethod'] as String? ?? 'manualEvidence',
    );
  }
}

class RequirementVerdictSummary {
  const RequirementVerdictSummary({
    required this.key,
    required this.verdict,
    required this.reason,
    required this.evidenceAssessment,
    required this.requiredCorrection,
  });

  final String key;
  final String? verdict;
  final String? reason;
  final String? evidenceAssessment;
  final String? requiredCorrection;

  String get displayVerdict {
    return switch (verdict) {
      'pass' => 'Pass',
      'fail' => 'Fail',
      'acceptedBlocked' => 'Blocked',
      'rejectedBlocked' => 'Rejected blocker',
      'waiverRequired' => 'Waiver required',
      _ => 'Pending',
    };
  }

  factory RequirementVerdictSummary.fromJson(Map<String, dynamic> json) {
    return RequirementVerdictSummary(
      key: json['key'] as String? ?? '',
      verdict: json['verdict'] as String?,
      reason: json['reason'] as String?,
      evidenceAssessment: json['evidenceAssessment'] as String?,
      requiredCorrection: json['requiredCorrection'] as String?,
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


class FullSizeImageData {
  const FullSizeImageData({
    required this.path,
    required this.bytesBase64,
    required this.contentType,
  });

  final String path;
  final String bytesBase64;
  final String contentType;
}

typedef FullSizeImageLoader = Future<FullSizeImageData> Function(String path);

class ChatEntry {
  const ChatEntry({
    required this.id,
    required this.author,
    required this.displayLabel,
    required this.timestamp,
    required this.body,
    this.subtitle,
    this.kind,
    this.status,
    this.processId,
    this.command,
    this.output,
    this.imagePreviewBase64,
    this.imagePreviewContentType,
    this.imagePreviewError,
    this.deliveryState,
    this.semanticCard,
    this.planItems = const <PlanChecklistItem>[],
    this.isStreaming = false,
    this.isTool = false,
  });

  final String id;
  final String author;
  final String displayLabel;
  final int? timestamp;
  final String body;
  final String? subtitle;
  final String? kind;
  final String? status;
  final String? processId;
  final String? command;
  final String? output;
  final String? imagePreviewBase64;
  final String? imagePreviewContentType;
  final String? imagePreviewError;
  final String? deliveryState;
  final ChatSemanticCard? semanticCard;
  final List<PlanChecklistItem> planItems;
  final bool isStreaming;
  final bool isTool;

  bool get hasPlanItems => planItems.isNotEmpty;

  factory ChatEntry.fromJson(Map<String, dynamic> json) {
    final timestampValue = json['timestamp'];
    final timestamp = switch (timestampValue) {
      int value => value,
      double value => value.floor(),
      String value => int.tryParse(value),
      _ => null,
    };
    final author = json['author'] as String? ?? 'Unknown';
    final isStreaming = json['isStreaming'] as bool? ?? false;
    final body = json['body'] as String? ?? '';
    return ChatEntry(
      id: json['id'] as String? ?? '',
      author: author,
      displayLabel: json['displayLabel'] as String? ?? json['author'] as String? ?? 'Unknown',
      timestamp: timestamp,
      body: body,
      subtitle: json['subtitle'] as String?,
      kind: json['kind'] as String?,
      status: json['status'] as String?,
      processId: json['processId'] as String?,
      command: json['command'] as String?,
      output: json['output'] as String?,
      imagePreviewBase64: json['imagePreviewBase64'] as String?,
      imagePreviewContentType: json['imagePreviewContentType'] as String?,
      imagePreviewError: json['imagePreviewError'] as String?,
      deliveryState: json['deliveryState'] as String?,
      semanticCard: json['semanticCard'] is Map<String, dynamic>
          ? ChatSemanticCard.fromJson(json['semanticCard'] as Map<String, dynamic>)
          : _semanticCardFromMessageBody(
              author: author,
              body: body,
              isStreaming: isStreaming,
            ),
      planItems: (json['planItems'] as List<dynamic>? ?? const [])
          .whereType<Map<String, dynamic>>()
          .map(PlanChecklistItem.fromJson)
          .toList(growable: false),
      isStreaming: isStreaming,
      isTool: json['isTool'] as bool? ?? false,
    );
  }
}

ChatSemanticCard? _semanticCardFromMessageBody({
  required String author,
  required String body,
  required bool isStreaming,
}) {
  if (isStreaming || author.toLowerCase() != 'assistant') {
    return null;
  }

  final text = body.trim();
  if (!text.startsWith('{') || !text.endsWith('}')) {
    return null;
  }

  final decoded = () {
    try {
      final value = jsonDecode(text);
      return value is Map<String, dynamic> ? value : null;
    } catch (_) {
      return null;
    }
  }();
  if (decoded == null) {
    return null;
  }

  final verdictPayload = _asObject(decoded['requirements']) ?? decoded;
  if (verdictPayload['overallVerdict'] is String &&
      _asObject(verdictPayload['route']) != null) {
    return _requirementsVerdictCard(verdictPayload);
  }

  if (decoded.containsKey('requirements')) {
    final requirements = decoded['requirements'];
    if (decoded['summary'] is String && (requirements == null || requirements is Map<String, dynamic>)) {
      return _requirementsClaimCard(decoded);
    }
  }

  if (decoded['finalDisposition'] is String && decoded['summary'] is String) {
    return _requirementsClaimCard(decoded);
  }

  if (decoded['response'] is String && decoded.containsKey('currentPlan')) {
    return _plannerResponseCard(decoded);
  }

  return null;
}

Map<String, dynamic>? _asObject(Object? value) {
  return value is Map<String, dynamic> ? value : null;
}

ChatSemanticCard _requirementsClaimCard(Map<String, dynamic> payload) {
  final summary = payload['summary'] as String? ?? '';
  final disposition = payload['finalDisposition'] as String? ?? 'unknown';
  final nestedRequirements = _asObject(payload['requirements']);
  final sourceEntries =
      nestedRequirements != null
          ? nestedRequirements.entries.toList()
          : payload.entries
              .where(
                (entry) =>
                    entry.key != 'summary' &&
                    entry.key != 'finalDisposition' &&
                    entry.key != 'requirements' &&
                    _asObject(entry.value) != null,
              )
              .toList();
  final rows =
      sourceEntries.whereType<MapEntry<String, dynamic>>().where((entry) {
        return _asObject(entry.value) != null;
      }).map((entry) {
        final value = _asObject(entry.value)!;
        final claim = _requirementClaimKind(value);

        return ChatSemanticRow(
          key: entry.key,
          title: entry.key,
          summary: _requirementClaimSummary(value),
          detail: _requirementClaimDetail(value),
          trailingLabel: _requirementClaimTrailingLabel(claim, value),
          tone: _claimTone(claim),
          icon: _claimIcon(claim),
          bullets: _requirementClaimBullets(value),
        );
      }).toList(growable: false);
  final isCommentary =
      payload.containsKey('requirements') && payload['requirements'] == null;
  final titleToneIcon =
      isCommentary
          ? const <String>['Requirements Commentary', 'secondary', 'notes']
          : switch (disposition) {
            'readyForRequirementsReview' => const <String>['Requirements Claim', 'success', 'factCheck'],
            'blockedNeedsOwnerAction' => const <String>['Requirements Blocked', 'warning', 'warning'],
            'continueWorkNeeded' => const <String>['Requirements Need Work', 'danger', 'build'],
            _ when rows.isEmpty => const <String>['Requirements Output', 'secondary', 'rule'],
            _ => const <String>['Requirements Claim', 'success', 'factCheck'],
          };
  return ChatSemanticCard(
    kind: 'requirementsClaim',
    title: titleToneIcon[0],
    summary: summary,
    statusLabel:
        isCommentary
            ? 'commentary'
            : rows.isEmpty
            ? null
            : '${rows.length} ${rows.length == 1 ? 'claim' : 'claims'}',
    tone: titleToneIcon[1],
    icon: titleToneIcon[2],
    rows: rows,
    plannerOptions: const <PlannerOption>[],
  );
}

ChatSemanticCard _requirementsVerdictCard(Map<String, dynamic> payload) {
  final overall = payload['overallVerdict'] as String? ?? 'unknown';
  final routeMessage = _asObject(payload['route'])?['message'] as String? ?? '';
  final rows =
      payload.entries
          .where((entry) => entry.key != 'overallVerdict' && entry.key != 'route')
          .where((entry) => _asObject(entry.value) != null)
          .map((entry) {
            final key = entry.key;
            final value = _asObject(entry.value)!;
            final verdict = value['verdict'] as String? ?? 'unknown';
            final reason = value['reason'] as String? ?? '';
            final evidence = value['evidenceAssessment'] as String? ?? '';
            final correction = value['requiredCorrection'] as String? ?? '';
            final bullets = <String>[
              if (evidence.trim().isNotEmpty) 'Evidence: ${evidence.trim()}',
              if (correction.trim().isNotEmpty) 'Correction: ${correction.trim()}',
            ];

            return ChatSemanticRow(
              key: key,
              title: key,
              summary: reason,
              detail: null,
              trailingLabel: _titleCaseVerdict(verdict),
              tone: _verdictTone(verdict),
              icon: _verdictIcon(verdict),
              bullets: bullets,
            );
          })
          .where((row) => row.key.isNotEmpty)
          .toList(growable: false);

  return ChatSemanticCard(
    kind: 'requirementsVerdict',
    title: switch (overall) {
      'pass' => 'Requirements Review Passed',
      'fail' => 'Requirements Review Failed',
      'acceptedBlocked' => 'Requirements Review Accepted Blocker',
      'rejectedBlocked' => 'Requirements Review Rejected Blocker',
      'needsHumanWaiver' => 'Requirements Review Needs Waiver',
      _ => 'Requirements Review',
    },
    summary: routeMessage,
    statusLabel: null,
    tone: _verdictTone(overall),
    icon: _verdictIcon(overall),
    rows: rows,
    plannerOptions: const <PlannerOption>[],
  );
}

ChatSemanticCard _plannerResponseCard(Map<String, dynamic> payload) {
  final clarification = _asObject(payload['clarification']);
  final options =
      ((clarification?['options'] as List<dynamic>?) ?? const <dynamic>[]).whereType<Map<String, dynamic>>().map((
        option,
      ) {
        return PlannerOption(
          label: option['label'] as String? ?? '',
          description: option['description'] as String? ?? '',
        );
      }).toList(growable: false);
  final question = clarification?['question'] as String? ?? '';
  final currentPlan = payload['currentPlan'];
  final title =
      currentPlan is String && currentPlan.trim().isNotEmpty
          ? currentPlan.trim()
          : 'Planner';
  return ChatSemanticCard(
    kind: 'plannerResponse',
    title: title,
    summary: payload['response'] as String? ?? '',
    statusLabel: null,
    tone: 'primary',
    icon: 'planner',
    rows:
        question.trim().isEmpty
            ? const <ChatSemanticRow>[]
            : [
              ChatSemanticRow(
                key: 'clarification',
                title: question.trim(),
                summary: '',
                tone: 'primary',
                icon: 'question',
                detail: null,
                trailingLabel: null,
                bullets: const <String>[],
              ),
            ],
    plannerOptions: options,
  );
}

String _verdictTone(String value) {
  return switch (value) {
    'pass' => 'success',
    'fail' || 'rejectedBlocked' => 'danger',
    'acceptedBlocked' || 'needsHumanWaiver' => 'warning',
    _ => 'secondary',
  };
}

String _claimTone(String value) {
  return switch (value) {
    'satisfied' => 'success',
    'notSatisfied' => 'danger',
    'blocked' => 'warning',
    'notApplicable' => 'muted',
    _ => 'secondary',
  };
}

String _verdictIcon(String value) {
  return switch (value) {
    'pass' => 'verified',
    'fail' => 'cancel',
    'acceptedBlocked' => 'warning',
    'rejectedBlocked' => 'problem',
    'needsHumanWaiver' => 'gavel',
    _ => 'review',
  };
}

String _claimIcon(String value) {
  return switch (value) {
    'satisfied' => 'check',
    'notSatisfied' => 'cancel',
    'blocked' => 'warning',
    'notApplicable' => 'remove',
    'notFinished' => 'dot',
    _ => 'dot',
  };
}

String _titleCaseVerdict(String value) {
  return switch (value) {
    'acceptedBlocked' => 'Accepted blocker',
    'rejectedBlocked' => 'Rejected blocker',
    'needsHumanWaiver' => 'Needs waiver',
    'pass' => 'Pass',
    'fail' => 'Fail',
    _ => 'Unknown',
  };
}

String _titleCaseClaim(String value) {
  return switch (value) {
    'satisfied' => 'Satisfied',
    'notSatisfied' => 'Not satisfied',
    'blocked' => 'Blocked',
    'notApplicable' => 'Not applicable',
    'notFinished' => 'Not finished',
    _ => 'Unknown',
  };
}

String _requirementClaimKind(Map<String, dynamic> value) {
  final kind = value['kind'] as String?;
  if (kind != null && kind.trim().isNotEmpty) {
    return kind.trim();
  }
  final claim = value['claim'] as String?;
  if (claim != null && claim.trim().isNotEmpty) {
    return claim.trim();
  }
  if (value['notFinished'] == true) {
    return 'notFinished';
  }
  return 'unknown';
}

String _requirementClaimSummary(Map<String, dynamic> value) {
  for (final key in ['summary', 'justification', 'blocker', 'reason']) {
    final text = value[key] as String?;
    if (text != null && text.trim().isNotEmpty) {
      return text.trim();
    }
  }
  return '';
}

String? _requirementClaimDetail(Map<String, dynamic> value) {
  final details = <String>[];
  final ownerDecision = value['ownerDecisionNeeded'] as String?;
  if (ownerDecision != null && ownerDecision.trim().isNotEmpty) {
    details.add('Owner decision: ${ownerDecision.trim()}');
  }
  final blocker = value['blocker'] as String?;
  final summary = value['summary'] as String?;
  if (blocker != null &&
      blocker.trim().isNotEmpty &&
      blocker.trim() != summary?.trim()) {
    details.add('Blocker: ${blocker.trim()}');
  }
  return details.isEmpty ? null : details.join('\n');
}

String _requirementClaimTrailingLabel(
  String claim,
  Map<String, dynamic> value,
) {
  final risk = value['risk'] as String?;
  final label = _titleCaseClaim(claim);
  if (risk != null && risk.trim().isNotEmpty) {
    return '$label · risk ${risk.trim()}';
  }
  return label;
}

List<String> _requirementClaimBullets(Map<String, dynamic> value) {
  final evidence = value['evidence'];
  if (evidence is! List<dynamic>) {
    return const <String>[];
  }
  return evidence
      .map((item) {
        if (item is String) {
          return item.trim();
        }
        if (item is Map<String, dynamic>) {
          final type = item['type'] as String?;
          final evidenceValue = item['value'] as String?;
          final pieces = <String>[
            if (type != null && type.trim().isNotEmpty) type.trim(),
            if (evidenceValue != null && evidenceValue.trim().isNotEmpty)
              evidenceValue.trim(),
          ];
          return pieces.join(': ');
        }
        return '';
      })
      .where((item) => item.isNotEmpty)
      .take(4)
      .toList(growable: false);
}

class ChatSemanticCard {
  const ChatSemanticCard({
    required this.kind,
    required this.title,
    required this.summary,
    required this.tone,
    required this.icon,
    this.statusLabel,
    this.rows = const <ChatSemanticRow>[],
    this.plannerOptions = const <PlannerOption>[],
  });

  final String kind;
  final String title;
  final String summary;
  final String tone;
  final String icon;
  final String? statusLabel;
  final List<ChatSemanticRow> rows;
  final List<PlannerOption> plannerOptions;

  factory ChatSemanticCard.fromJson(Map<String, dynamic> json) {
    return ChatSemanticCard(
      kind: json['kind'] as String? ?? '',
      title: json['title'] as String? ?? '',
      summary: json['summary'] as String? ?? '',
      statusLabel: json['statusLabel'] as String?,
      tone: json['tone'] as String? ?? 'secondary',
      icon: json['icon'] as String? ?? 'review',
      rows: (json['rows'] as List<dynamic>? ?? const [])
          .whereType<Map<String, dynamic>>()
          .map(ChatSemanticRow.fromJson)
          .toList(growable: false),
      plannerOptions: (json['plannerOptions'] as List<dynamic>? ?? const [])
          .whereType<Map<String, dynamic>>()
          .map(PlannerOption.fromJson)
          .toList(growable: false),
    );
  }
}

class ChatSemanticRow {
  const ChatSemanticRow({
    required this.key,
    required this.title,
    required this.summary,
    required this.tone,
    required this.icon,
    this.detail,
    this.trailingLabel,
    this.bullets = const <String>[],
  });

  final String key;
  final String title;
  final String summary;
  final String tone;
  final String icon;
  final String? detail;
  final String? trailingLabel;
  final List<String> bullets;

  factory ChatSemanticRow.fromJson(Map<String, dynamic> json) {
    return ChatSemanticRow(
      key: json['key'] as String? ?? '',
      title: json['title'] as String? ?? '',
      summary: json['summary'] as String? ?? '',
      detail: json['detail'] as String?,
      trailingLabel: json['trailingLabel'] as String?,
      tone: json['tone'] as String? ?? 'secondary',
      icon: json['icon'] as String? ?? 'review',
      bullets: (json['bullets'] as List<dynamic>? ?? const [])
          .whereType<String>()
          .toList(growable: false),
    );
  }
}

class PlannerOption {
  const PlannerOption({required this.label, required this.description});

  final String label;
  final String description;

  factory PlannerOption.fromJson(Map<String, dynamic> json) {
    return PlannerOption(
      label: json['label'] as String? ?? '',
      description: json['description'] as String? ?? '',
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
