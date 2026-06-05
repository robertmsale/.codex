import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:rinf/rinf.dart';
import 'package:robdex_design_system/robdex_design_system.dart';

import '../../bindings/bindings.dart';

class WorkbenchController extends ChangeNotifier {
  WorkbenchController();

  WorkbenchViewData? _view;
  bool _isLoading = true;
  Object? _error;
  StreamSubscription<RustSignalPack<WorkbenchStateSignal>>? _subscription;
  StreamSubscription<RustSignalPack<ThreadHistoryStateSignal>>? _historySubscription;
  List<ChatEntry> _threadHistoryEntries = const [];
  bool _isThreadHistoryLoading = false;
  Object? _threadHistoryError;

  WorkbenchViewData? get view => _view;
  bool get isLoading => _isLoading;
  Object? get error => _error;
  List<ChatEntry> get threadHistoryEntries => _threadHistoryEntries;
  bool get isThreadHistoryLoading => _isThreadHistoryLoading;
  Object? get threadHistoryError => _threadHistoryError;

  Future<void> start({
    required String host,
    required int port,
  }) async {
    _subscription?.cancel();
    _historySubscription?.cancel();
    _subscription = WorkbenchStateSignal.rustSignalStream.listen(
      (pack) {
        final signal = pack.message;
        _isLoading = signal.isLoading;
        _error = signal.errorMessage.isEmpty ? null : signal.errorMessage;
        if (signal.viewJson.isNotEmpty) {
          final decoded = jsonDecode(signal.viewJson) as Map<String, dynamic>;
          _view = WorkbenchViewData.fromJson(decoded);
        }
        notifyListeners();
      },
      onError: (Object error) {
        _error = error;
        _isLoading = false;
        notifyListeners();
      },
    );
    _historySubscription = ThreadHistoryStateSignal.rustSignalStream.listen(
      (pack) {
        final signal = pack.message;
        _isThreadHistoryLoading = signal.isLoading;
        _threadHistoryError = signal.errorMessage.isEmpty ? null : signal.errorMessage;
        if (signal.entriesJson.isNotEmpty) {
          final decoded = jsonDecode(signal.entriesJson) as List<dynamic>;
          _threadHistoryEntries = decoded
              .whereType<Map<String, dynamic>>()
              .map(ChatEntry.fromJson)
              .toList(growable: false);
        }
        notifyListeners();
      },
      onError: (Object error) {
        _threadHistoryError = error;
        _isThreadHistoryLoading = false;
        notifyListeners();
      },
    );
    InitializeWorkbenchSignal(
      host: host,
      port: port,
    ).sendSignalToRust();
  }

  void reload() {
    const ReloadWorkbenchSignal().sendSignalToRust();
  }

  void selectThread(String threadId) {
    SelectThreadSignal(threadId: threadId).sendSignalToRust();
  }

  void fetchThreadHistory() {
    const FetchThreadHistorySignal().sendSignalToRust();
  }

  void compactThread() {
    const ThreadCompactSignal().sendSignalToRust();
  }

  void terminateCommandExecution(String processId) {
    if (processId.trim().isEmpty) {
      return;
    }
    TerminateCommandExecutionSignal(processId: processId).sendSignalToRust();
  }

  void createProject({
    required String name,
    required String rootPath,
    required String defaultCwd,
  }) {
    CreateProjectSignal(
      name: name,
      rootPath: rootPath,
      defaultCwd: defaultCwd,
    ).sendSignalToRust();
  }

  void createThread({
    required String projectId,
    required String title,
    required String initialPrompt,
    required String role,
    required String approvalPolicy,
    required String sandboxMode,
    required String networkAccessMode,
    required String modelId,
    required String reasoningEffort,
    String? requirementSetJson,
  }) {
    CreateThreadSignal(
      projectId: projectId,
      title: title,
      initialPrompt: initialPrompt,
      role: role,
      approvalPolicy: approvalPolicy,
      sandboxMode: sandboxMode,
      networkAccessMode: networkAccessMode,
      modelId: modelId,
      reasoningEffort: reasoningEffort,
      requirementSetJson: requirementSetJson ?? '',
    ).sendSignalToRust();
  }

  void spawnAgent({
    required String name,
    required String role,
    required String prompt,
    String? requirementSetJson,
  }) {
    SpawnAgentSignal(
      name: name,
      role: role,
      prompt: prompt,
      requirementSetJson: requirementSetJson ?? '',
    ).sendSignalToRust();
  }

  void setProjectOrchestrator({
    required String projectId,
    required String projectPath,
    required String threadId,
  }) {
    SetProjectOrchestratorSignal(
      projectId: projectId,
      projectPath: projectPath,
      threadId: threadId,
    ).sendSignalToRust();
  }

  void createThreadGroup(String title) {
    if (title.trim().isEmpty) {
      return;
    }
    CreateThreadGroupSignal(title: title.trim()).sendSignalToRust();
  }

  void renameThreadGroup({
    required String groupId,
    required String title,
  }) {
    if (title.trim().isEmpty) {
      return;
    }
    RenameThreadGroupSignal(
      groupId: groupId,
      title: title.trim(),
    ).sendSignalToRust();
  }

  void deleteThreadGroup(String groupId) {
    DeleteThreadGroupSignal(groupId: groupId).sendSignalToRust();
  }

  void archiveThreadGroup(String groupId) {
    ArchiveThreadGroupSignal(groupId: groupId).sendSignalToRust();
  }

  void moveSelectedThreadToGroup(String? groupId) {
    MoveSelectedThreadToGroupSignal(groupId: groupId ?? '').sendSignalToRust();
  }

  void updateWorkerMetadata({
    required String issueNumber,
    required String pullRequestNumber,
    required String blockedReason,
    required String unblockWhen,
    required bool clearBlocked,
  }) {
    UpdateWorkerMetadataSignal(
      issueNumber: issueNumber,
      pullRequestNumber: pullRequestNumber,
      blockedReason: blockedReason,
      unblockWhen: unblockWhen,
      clearBlocked: clearBlocked,
    ).sendSignalToRust();
  }

  void selectProject(String? projectId) {
    SelectProjectSignal(projectId: projectId ?? '').sendSignalToRust();
  }

  void deleteProject(String projectId) {
    DeleteProjectSignal(projectId: projectId).sendSignalToRust();
  }

  void updateProject({
    required String projectId,
    required String name,
    required String defaultCwd,
    required bool autoRouteReplies,
    required bool routeApprovalRequests,
    required String preferredModelProvider,
    required String defaultModelId,
    required String defaultReasoningEffort,
    required String defaultSandboxMode,
    required String defaultApprovalPolicy,
    required String defaultNetworkAccessMode,
    required String orchestratorModelId,
    required String orchestratorReasoningEffort,
    required String workerModelId,
    required String workerReasoningEffort,
    required String qaModelId,
    required String qaReasoningEffort,
    required String designerModelId,
    required String designerReasoningEffort,
    required String plannerModelId,
    required String plannerReasoningEffort,
    required String requirementsReviewerModelId,
    required String requirementsReviewerReasoningEffort,
    required String orchestratorDeveloperInstructions,
    required String workerDeveloperInstructions,
    required String qaDeveloperInstructions,
    required String designerDeveloperInstructions,
    required String operatorDeveloperInstructions,
    required String hiddenDeveloperInstructions,
    required List<String> permanentRequirementComposables,
  }) {
    UpdateProjectSignal(
      projectId: projectId,
      name: name,
      defaultCwd: defaultCwd,
      autoRouteReplies: autoRouteReplies,
      routeApprovalRequests: routeApprovalRequests,
      preferredModelProvider: preferredModelProvider,
      defaultModelId: defaultModelId,
      defaultReasoningEffort: defaultReasoningEffort,
      defaultSandboxMode: defaultSandboxMode,
      defaultApprovalPolicy: defaultApprovalPolicy,
      defaultNetworkAccessMode: defaultNetworkAccessMode,
      orchestratorModelId: orchestratorModelId,
      orchestratorReasoningEffort: orchestratorReasoningEffort,
      workerModelId: workerModelId,
      workerReasoningEffort: workerReasoningEffort,
      qaModelId: qaModelId,
      qaReasoningEffort: qaReasoningEffort,
      designerModelId: designerModelId,
      designerReasoningEffort: designerReasoningEffort,
      plannerModelId: plannerModelId,
      plannerReasoningEffort: plannerReasoningEffort,
      requirementsReviewerModelId: requirementsReviewerModelId,
      requirementsReviewerReasoningEffort: requirementsReviewerReasoningEffort,
      orchestratorDeveloperInstructions: orchestratorDeveloperInstructions,
      workerDeveloperInstructions: workerDeveloperInstructions,
      qaDeveloperInstructions: qaDeveloperInstructions,
      designerDeveloperInstructions: designerDeveloperInstructions,
      operatorDeveloperInstructions: operatorDeveloperInstructions,
      hiddenDeveloperInstructions: hiddenDeveloperInstructions,
      permanentRequirementComposables: permanentRequirementComposables,
    ).sendSignalToRust();
  }

  void sendMessage(
    String text, {
    List<String> localImagePaths = const [],
    String? requirementSetJson,
  }) {
    final trimmed = text.trim();
    if (trimmed.isEmpty && localImagePaths.isEmpty) {
      return;
    }
    SendThreadMessageSignal(
      text: trimmed,
      localImagePaths: localImagePaths,
      requirementSetJson: requirementSetJson ?? '',
    ).sendSignalToRust();
  }

  void interruptThread() {
    const InterruptThreadSignal().sendSignalToRust();
  }

  void decideApproval({
    required String approvalId,
    required String decision,
    String? message,
  }) {
    DecideApprovalSignal(
      approvalId: approvalId,
      decision: decision,
      message: message ?? '',
    ).sendSignalToRust();
  }

  void updateThreadSettings({
    required String role,
    required String approvalPolicy,
    required String sandboxMode,
    required String networkAccessMode,
    required String modelId,
    required String reasoningEffort,
    required String serviceTier,
  }) {
    UpdateThreadSettingsSignal(
      role: role,
      approvalPolicy: approvalPolicy,
      sandboxMode: sandboxMode,
      networkAccessMode: networkAccessMode,
      modelId: modelId,
      reasoningEffort: reasoningEffort,
      serviceTier: serviceTier,
    ).sendSignalToRust();
  }

  void setThreadRunningState(bool running) {
    SetThreadRunningStateSignal(running: running).sendSignalToRust();
  }

  void renameThread(String name) {
    if (name.trim().isEmpty) {
      return;
    }
    RenameThreadSignal(name: name.trim()).sendSignalToRust();
  }

  void archiveThread() {
    const ArchiveThreadSignal().sendSignalToRust();
  }

  void warmHandoff(String prompt) {
    if (prompt.trim().isEmpty) {
      return;
    }
    WarmHandoffSignal(prompt: prompt.trim()).sendSignalToRust();
  }

  void disconnect() {
    _subscription?.cancel();
    _subscription = null;
    _historySubscription?.cancel();
    _historySubscription = null;
    _view = null;
    _isLoading = false;
    _error = null;
    _threadHistoryEntries = const [];
    _isThreadHistoryLoading = false;
    _threadHistoryError = null;
    notifyListeners();
  }

  @override
  void dispose() {
    _subscription?.cancel();
    _historySubscription?.cancel();
    super.dispose();
  }
}
