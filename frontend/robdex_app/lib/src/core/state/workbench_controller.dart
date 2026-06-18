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
  StreamSubscription<RustSignalPack<BridgeTaskResultSignal>>? _bridgeTaskSubscription;
  StreamSubscription<RustSignalPack<WorkbenchSelectedChatDeltaSignal>>? _selectedChatDeltaSubscription;
  StreamSubscription<RustSignalPack<WorkbenchDiagnosticsSignal>>? _diagnosticsSubscription;
  final Map<String, Completer<dynamic>> _bridgeTaskCompleters = <String, Completer<dynamic>>{};
  int _bridgeTaskSerial = 0;
  List<ChatEntry> _threadHistoryEntries = const [];
  bool _isThreadHistoryLoading = false;
  Object? _threadHistoryError;
  WorkbenchDiagnosticsSignal? _diagnostics;
  int _selectedChatDeltaApplyCount = 0;
  int _lastFullSnapshotDecodeMicros = 0;

  WorkbenchViewData? get view => _view;
  bool get isLoading => _isLoading;
  Object? get error => _error;
  List<ChatEntry> get threadHistoryEntries => _threadHistoryEntries;
  bool get isThreadHistoryLoading => _isThreadHistoryLoading;
  Object? get threadHistoryError => _threadHistoryError;
  WorkbenchDiagnosticsSignal? get diagnostics => _diagnostics;
  int get selectedChatDeltaApplyCount => _selectedChatDeltaApplyCount;
  int get lastFullSnapshotDecodeMicros => _lastFullSnapshotDecodeMicros;

  Future<void> start({
    required String host,
    required int port,
  }) async {
    _subscription?.cancel();
    _historySubscription?.cancel();
    _bridgeTaskSubscription?.cancel();
    _selectedChatDeltaSubscription?.cancel();
    _diagnosticsSubscription?.cancel();
    _subscription = WorkbenchStateSignal.rustSignalStream.listen(
      (pack) {
        final signal = pack.message;
        _isLoading = signal.isLoading;
        _error = signal.errorMessage.isEmpty ? null : signal.errorMessage;
        if (signal.viewJson.isNotEmpty) {
          final watch = Stopwatch()..start();
          final decoded = jsonDecode(signal.viewJson) as Map<String, dynamic>;
          _view = WorkbenchViewData.fromJson(decoded);
          watch.stop();
          _lastFullSnapshotDecodeMicros = watch.elapsedMicroseconds;
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
    _bridgeTaskSubscription = BridgeTaskResultSignal.rustSignalStream.listen(
      (pack) {
        final signal = pack.message;
        final completer = _bridgeTaskCompleters.remove(signal.requestId);
        if (completer == null || completer.isCompleted) {
          return;
        }
        if (signal.errorMessage.isNotEmpty) {
          completer.completeError(StateError(signal.errorMessage));
          return;
        }
        if (signal.payloadJson.isEmpty) {
          completer.complete(null);
          return;
        }
        completer.complete(jsonDecode(signal.payloadJson));
      },
      onError: (Object error) {
        final pending = _bridgeTaskCompleters.values.toList(growable: false);
        _bridgeTaskCompleters.clear();
        for (final completer in pending) {
          if (!completer.isCompleted) {
            completer.completeError(error);
          }
        }
      },
    );

    _selectedChatDeltaSubscription = WorkbenchSelectedChatDeltaSignal.rustSignalStream.listen(
      (pack) {
        final signal = pack.message;
        _applySelectedChatDelta(signal);
        notifyListeners();
      },
      onError: (Object error) {
        _error = error;
        notifyListeners();
      },
    );
    _diagnosticsSubscription = WorkbenchDiagnosticsSignal.rustSignalStream.listen(
      (pack) {
        _diagnostics = pack.message;
        notifyListeners();
      },
      onError: (Object error) {
        _error = error;
        notifyListeners();
      },
    );
    InitializeWorkbenchSignal(
      host: host,
      port: port,
    ).sendSignalToRust();
  }


  void _applySelectedChatDelta(WorkbenchSelectedChatDeltaSignal signal) {
    final view = _view;
    if (view == null || view.selection.threadId != signal.threadId) {
      return;
    }
    final entries = List<ChatEntry>.from(view.chatEntries);
    final index = entries.indexWhere((entry) => entry.id == signal.messageId);
    if (index < 0) {
      entries.add(ChatEntry(
        id: signal.messageId,
        author: 'Assistant',
        displayLabel: 'Assistant',
        timestamp: null,
        body: signal.replacementText.isNotEmpty ? signal.replacementText : signal.appendedText,
        status: signal.deliveryState,
        deliveryState: signal.deliveryState,
        isStreaming: !signal.isFinal,
      ));
    } else {
      final current = entries[index];
      final body = signal.replacementText.isNotEmpty ? signal.replacementText : current.body + signal.appendedText;
      entries[index] = ChatEntry(
        id: current.id,
        author: current.author,
        displayLabel: current.displayLabel,
        timestamp: current.timestamp,
        body: body,
        subtitle: current.subtitle,
        kind: current.kind,
        status: signal.deliveryState,
        processId: current.processId,
        command: current.command,
        output: current.output,
        imagePreviewBase64: current.imagePreviewBase64,
        imagePreviewContentType: current.imagePreviewContentType,
        imagePreviewError: current.imagePreviewError,
        deliveryState: signal.deliveryState,
        semanticCard: current.semanticCard,
        planItems: current.planItems,
        isStreaming: !signal.isFinal,
        isTool: current.isTool,
      );
    }
    final capped = entries.length <= 50 ? entries : entries.sublist(entries.length - 50);
    _view = view.copyWith(chatEntries: capped);
    _selectedChatDeltaApplyCount += 1;
  }

  @visibleForTesting
  void applySelectedChatDeltaForTest(WorkbenchViewData view, WorkbenchSelectedChatDeltaSignal signal) {
    _view = view;
    _applySelectedChatDelta(signal);
  }

  String _nextBridgeTaskRequestId(String task) {
    _bridgeTaskSerial += 1;
    return '$task-$_bridgeTaskSerial';
  }

  Future<dynamic> _awaitBridgeTask(String requestId) {
    final completer = Completer<dynamic>();
    _bridgeTaskCompleters[requestId] = completer;
    return completer.future.timeout(
      const Duration(seconds: 45),
      onTimeout: () {
        _bridgeTaskCompleters.remove(requestId);
        throw StateError('Bridge task timed out.');
      },
    );
  }

  Future<ThreadStatsData> loadThreadStats(String threadId) async {
    final requestId = _nextBridgeTaskRequestId('threadStats');
    LoadThreadStatsSignal(requestId: requestId, threadId: threadId).sendSignalToRust();
    final payload = await _awaitBridgeTask(requestId);
    return ThreadStatsData.fromJson(payload as Map<String, dynamic>);
  }

  Future<PeriodStatsData> loadPeriodStats(PeriodStatsRequest request) async {
    final requestId = _nextBridgeTaskRequestId('periodStats');
    LoadPeriodStatsSignal(
      requestId: requestId,
      startMs: Uint64(BigInt.from(request.startMs)),
      endMs: Uint64(BigInt.from(request.endMs)),
      label: request.label,
      quotaResetAtMs: Uint64(BigInt.from(request.quotaResetAtMs ?? 0)),
      quotaRemainingPercent: request.quotaRemainingPercent ?? 0,
      hasQuota: request.quotaResetAtMs != null && request.quotaRemainingPercent != null,
    ).sendSignalToRust();
    final payload = await _awaitBridgeTask(requestId);
    return PeriodStatsData.fromJson(payload as Map<String, dynamic>);
  }

  Future<List<Map<String, dynamic>>> loadProjectHookLogs(String projectId) async {
    final requestId = _nextBridgeTaskRequestId('projectHookLogs');
    LoadProjectHookLogsSignal(requestId: requestId, projectId: projectId).sendSignalToRust();
    final payload = await _awaitBridgeTask(requestId);
    final logs = payload is Map<String, dynamic> ? payload['logs'] : null;
    return (logs as List<dynamic>? ?? const <dynamic>[])
        .whereType<Map<String, dynamic>>()
        .toList(growable: false);
  }

  Future<void> clearProjectHookLogs(String projectId) async {
    final requestId = _nextBridgeTaskRequestId('clearProjectHookLogs');
    ClearProjectHookLogsSignal(requestId: requestId, projectId: projectId).sendSignalToRust();
    await _awaitBridgeTask(requestId);
  }

  Future<List<Map<String, dynamic>>> loadRequirementComposables({
    String? senderThreadId,
    String? recipientThreadId,
    String? projectPath,
  }) async {
    final requestId = _nextBridgeTaskRequestId('requirementComposables');
    LoadRequirementComposablesSignal(
      requestId: requestId,
      senderThreadId: senderThreadId ?? '',
      recipientThreadId: recipientThreadId ?? '',
      projectPath: projectPath ?? '',
    ).sendSignalToRust();
    final payload = await _awaitBridgeTask(requestId);
    final items = payload is Map<String, dynamic> ? payload['items'] : null;
    return (items as List<dynamic>? ?? const <dynamic>[])
        .whereType<Map<String, dynamic>>()
        .toList(growable: false);
  }

  Future<void> setThreadRequirements({
    String? senderThreadId,
    required String recipientThreadId,
    String? projectPath,
    String? requirementSetJson,
  }) async {
    final requestId = _nextBridgeTaskRequestId('setThreadRequirements');
    SetThreadRequirementsSignal(
      requestId: requestId,
      senderThreadId: senderThreadId ?? '',
      recipientThreadId: recipientThreadId,
      projectPath: projectPath ?? '',
      requirementSetJson: requirementSetJson ?? '',
    ).sendSignalToRust();
    await _awaitBridgeTask(requestId);
  }

  Future<String> uploadImageBytes({
    required String filename,
    required String contentType,
    required Uint8List bytes,
  }) async {
    final requestId = _nextBridgeTaskRequestId('uploadImageBytes');
    UploadImageBytesSignal(
      requestId: requestId,
      filename: filename,
      contentType: contentType,
      bytes: bytes,
    ).sendSignalToRust();
    final payload = await _awaitBridgeTask(requestId);
    if (payload case {'path': final String path} when path.trim().isNotEmpty) {
      return path;
    }
    throw StateError('Bridge upload response missing path.');
  }


  Future<FullSizeImageData> loadFullSizeImage(String path) async {
    final requestId = _nextBridgeTaskRequestId('loadImageBytes');
    LoadImageBytesSignal(
      requestId: requestId,
      path: path,
    ).sendSignalToRust();
    final payload = await _awaitBridgeTask(requestId);
    if (payload case {
      'path': final String imagePath,
      'bytesBase64': final String bytesBase64,
      'contentType': final String contentType,
    } when bytesBase64.isNotEmpty) {
      return FullSizeImageData(
        path: imagePath,
        bytesBase64: bytesBase64,
        contentType: contentType,
      );
    }
    throw StateError('Bridge image response missing image bytes.');
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

  void updateGlobalSettings({
    required String approvalPolicy,
    required String sandboxMode,
    required String networkAccessMode,
  }) {
    UpdateGlobalSettingsSignal(
      approvalPolicy: approvalPolicy,
      sandboxMode: sandboxMode,
      networkAccessMode: networkAccessMode,
    ).sendSignalToRust();
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
    required String roleRuntimeDefaultsJson,
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
      roleRuntimeDefaultsJson: roleRuntimeDefaultsJson,
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
    _bridgeTaskSubscription?.cancel();
    _selectedChatDeltaSubscription?.cancel();
    _diagnosticsSubscription?.cancel();
    _bridgeTaskSubscription = null;
    final pending = _bridgeTaskCompleters.values.toList(growable: false);
    _bridgeTaskCompleters.clear();
    for (final completer in pending) {
      if (!completer.isCompleted) {
        completer.completeError(StateError('Disconnected from bridge.'));
      }
    }
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
    _bridgeTaskSubscription?.cancel();
    _selectedChatDeltaSubscription?.cancel();
    _diagnosticsSubscription?.cancel();
    super.dispose();
  }
}
