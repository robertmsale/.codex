import 'dart:async';
import 'package:file_selector/file_selector.dart';
import 'package:flutter/foundation.dart';
import 'package:rinf/rinf.dart';
import 'package:robdex_design_system/robdex_design_system.dart';

import '../bindings/bindings.dart' as bindings;

typedef AgentRuntimeRemoteProfilePicker = Future<String?> Function();
typedef AgentRuntimeRequestSink = void Function(String requestId, bindings.AgentRuntimeRequest request);

Future<String?> pickAgentRuntimeRemoteProfileDocumentPath() async {
  final file = await openFile(
    acceptedTypeGroups: const <XTypeGroup>[
      XTypeGroup(
        label: 'Agent Runtime remote profile JSON',
        extensions: <String>['json'],
        mimeTypes: <String>['application/json'],
        uniformTypeIdentifiers: <String>['public.json'],
      ),
    ],
  );
  return file?.path;
}

@visibleForTesting
bindings.AgentRuntimeGuiOperation agentRuntimeRoleActivateOperationForTest(String roleId, String versionId) {
  return bindings.AgentRuntimeGuiOperationActivateRoleVersion(roleId: roleId, versionId: versionId);
}

@visibleForTesting
bindings.AgentRuntimeGuiOperation agentRuntimeWorkflowMemoryFeedbackOperationForTest({
  required String memoryId,
  required String sessionId,
  required String feedback,
  bindings.AgentRuntimeWorkflowMemoryFeedbackPayload payload = const bindings.AgentRuntimeWorkflowMemoryFeedbackPayload(
    source: '',
    reason: '',
    variant: false,
    hasVariant: false,
  ),
}) {
  return bindings.AgentRuntimeGuiOperationWorkflowMemoryFeedback(
    memoryId: memoryId,
    sessionId: sessionId,
    feedback: feedback,
    payload: payload,
  );
}

@visibleForTesting
bindings.AgentRuntimeGuiOperation agentRuntimeWorkflowMemorySelectOperationForTest(String memoryId) {
  return bindings.AgentRuntimeGuiOperationSelectWorkflowMemory(memoryId: memoryId);
}

@visibleForTesting
bindings.AgentRuntimeGuiOperation agentRuntimeSetRequirementsOperationForTest(
  String sessionId,
  List<bindings.AgentRuntimeRequirementInput> requirements, {
  String title = '',
}) {
  return bindings.AgentRuntimeGuiOperationSetRequirements(
    sessionId: sessionId,
    title: title,
    requirements: requirements,
  );
}

@visibleForTesting
bindings.AgentRuntimeGuiOperation agentRuntimeClearRequirementsOperationForTest(String sessionId) {
  return bindings.AgentRuntimeGuiOperationClearRequirements(sessionId: sessionId);
}

@visibleForTesting
bindings.AgentRuntimeGuiOperation agentRuntimeRequirementsStatusOperationForTest(String sessionId) {
  return bindings.AgentRuntimeGuiOperationShowRequirementsStatus(sessionId: sessionId);
}

@visibleForTesting
bindings.AgentRuntimeGuiOperation agentRuntimeRequirementsPacketsOperationForTest(String sessionId) {
  return bindings.AgentRuntimeGuiOperationListRequirementsPackets(sessionId: sessionId);
}

@visibleForTesting
bindings.AgentRuntimeGuiOperation agentRuntimeCloseSessionOperationForTest(String sessionId) {
  return bindings.AgentRuntimeGuiOperationCloseSession(sessionId: sessionId, reason: 'Closed from Agent Runtime shell');
}

@visibleForTesting
bindings.AgentRuntimeGuiOperation agentRuntimeArchiveSessionOperationForTest(String sessionId) {
  return bindings.AgentRuntimeGuiOperationArchiveSession(sessionId: sessionId);
}

@visibleForTesting
bindings.AgentRuntimeGuiOperation agentRuntimeForkSessionOperationForTest(String sessionId) {
  return bindings.AgentRuntimeGuiOperationForkSession(sessionId: sessionId, atTurn: '');
}

@visibleForTesting
bindings.AgentRuntimeGuiOperation agentRuntimeTerminateProcessOperationForTest(String sessionId, String handle) {
  return bindings.AgentRuntimeGuiOperationTerminateProcess(sessionId: sessionId, handle: handle);
}

@visibleForTesting
bindings.AgentRuntimeGuiOperation agentRuntimeInputProcessOperationForTest(String sessionId, String handle, String text) {
  return bindings.AgentRuntimeGuiOperationInputProcess(sessionId: sessionId, handle: handle, text: text);
}

@visibleForTesting
bindings.AgentRuntimeGuiOperation agentRuntimeFlushProcessOperationForTest(String sessionId, String handle) {
  return bindings.AgentRuntimeGuiOperationFlushProcess(sessionId: sessionId, handle: handle);
}

@visibleForTesting
bindings.AgentRuntimeGuiOperation agentRuntimeApprovalDecisionOperationForTest(String approvalId, String decision, {String reason = 'Agent Runtime shell action'}) {
  return bindings.AgentRuntimeGuiOperationDecideApproval(approvalId: approvalId, decision: decision, reason: reason);
}

@visibleForTesting
bindings.AgentRuntimeGuiOperation agentRuntimeApprovalResumeOperationForTest(String approvalId) {
  return bindings.AgentRuntimeGuiOperationResumeApproval(approvalId: approvalId);
}

ChatEntry agentRuntimeChatEntryToChatEntryForTest(bindings.AgentRuntimeChatEntry entry) => _chatEntry(entry);

@visibleForTesting
bindings.AgentRuntimeGuiOperation agentRuntimeCommandRegistryDecisionOperationForTest(
  String requestId,
  String sessionId,
  AgentRuntimeCommandRegistryDecisionDraft decision,
) {
  return bindings.AgentRuntimeGuiOperationDecideCommandRegistryRequest(
    requestId: requestId,
    decision: _typedRegistryDecision(sessionId, decision),
  );
}

@visibleForTesting
bindings.AgentRuntimeGuiOperation agentRuntimeCommandRegistryDenyOperationForTest(
  String requestId,
  String sessionId,
  AgentRuntimeCommandRegistryDecisionDraft decision,
) {
  return bindings.AgentRuntimeGuiOperationDecideCommandRegistryRequest(
    requestId: requestId,
    decision: _typedRegistryDecision(sessionId, decision),
  );
}

@visibleForTesting
bindings.AgentRuntimeGuiOperation agentRuntimeCommandRegistryPreviewOperationForTest(
  String requestId,
  String sessionId,
  AgentRuntimeCommandRegistryDecisionDraft decision,
) {
  return bindings.AgentRuntimeGuiOperationPreviewCommandRegistryRequest(
    requestId: requestId,
    decision: _typedRegistryDecision(sessionId, decision),
  );
}

@visibleForTesting
bindings.AgentRuntimeGuiOperation agentRuntimeCommandRegistryApplyOperationForTest(String requestId, String sessionId) {
  return bindings.AgentRuntimeGuiOperationApplyCommandRegistryRequest(requestId: requestId, sessionId: sessionId);
}

@visibleForTesting
bindings.AgentRuntimeRequest agentRuntimeIcloudRefreshIntentForTest() {
  return const bindings.AgentRuntimeRequestRefreshIcloudRemoteDiscovery(profilePath: '');
}

@visibleForTesting
bindings.AgentRuntimeRequest agentRuntimeIcloudConnectIntentForTest() {
  return const bindings.AgentRuntimeRequestConnectIcloudRemoteRuntime(profilePath: '', selectedSessionId: '');
}

@visibleForTesting
bindings.AgentRuntimeRequest agentRuntimeImportProfileIntentForTest({String? profilePath}) {
  return bindings.AgentRuntimeRequestImportRemoteProfileDocument(profilePath: profilePath ?? '');
}

@visibleForTesting
bindings.AgentRuntimeRequest agentRuntimeRefreshImportedProfileIntentForTest() {
  return const bindings.AgentRuntimeRequestRefreshImportedRemoteProfile();
}

@visibleForTesting
bindings.AgentRuntimeRequest agentRuntimeConnectImportedProfileIntentForTest() {
  return const bindings.AgentRuntimeRequestConnectImportedRemoteRuntime(selectedSessionId: '');
}

class AgentRuntimeWorkbenchController extends ChangeNotifier {
  StreamSubscription<RustSignalPack<bindings.AgentRuntimeOutputSignal>>? _subscription;
  final AgentRuntimeRemoteProfilePicker _remoteProfilePicker;
  final AgentRuntimeRequestSink _requestSink;
  final Set<String> _pendingRequestIds = <String>{};
  final Map<String, String> _approvalListUpdates = <String, String>{};
  int _serial = 0;
  String _baseUrl = 'http://127.0.0.1:8765';
  AgentRuntimeWorkbenchData? _viewModel;
  ConversationShellData? _shellViewModel;
  String? _bridgeErrorMessage;

  AgentRuntimeWorkbenchController({
    AgentRuntimeRemoteProfilePicker remoteProfilePicker = pickAgentRuntimeRemoteProfileDocumentPath,
    AgentRuntimeRequestSink? requestSink,
  })  : _remoteProfilePicker = remoteProfilePicker,
        _requestSink = requestSink ?? _sendRequestSignalToRust {
    try {
      _subscription = bindings.AgentRuntimeOutputSignal.rustSignalStream.listen(
        _handleOutput,
        onError: (Object _) {
          _bridgeErrorMessage = 'Agent Runtime bridge is not ready. Restart the app, then refresh discovery.';
          notifyListeners();
        },
      );
    } catch (_) {
      _bridgeErrorMessage = 'Agent Runtime bridge is not ready. Restart the app, then refresh discovery.';
    }
  }

  AgentRuntimeWorkbenchData get data {
    return (_viewModel ?? _disconnectedViewModel).copyWith(
      pendingRequestCount: _pendingRequestIds.length,
      errorMessage: _bridgeErrorMessage,
    );
  }

  ConversationShellData? get shellData => _shellViewModel;

  void connect(String baseUrl) {
    _baseUrl = baseUrl.trim().isEmpty ? _baseUrl : baseUrl.trim();
    _send('connect', bindings.AgentRuntimeRequestConnect(baseUrl: _baseUrl, selectedSessionId: ''));
  }

  void refreshDiscovery() {
    _send('discover', const bindings.AgentRuntimeRequestRefreshDiscovery(discoveryPath: ''));
  }

  void selectProject(String projectId) {
    _send('project-select', bindings.AgentRuntimeRequestSelectProject(projectId: projectId));
  }

  void openSettings() {
    _bridgeErrorMessage = 'Open Global settings from the toolbar.';
    notifyListeners();
  }

  void updateRuntimeSettings({
    required String baseUrl,
    required String selectedProjectId,
  }) {
    _dispatchOperation(
      'runtime-settings-update',
      bindings.AgentRuntimeGuiOperationUpdateRuntimeSettings(
        baseUrl: baseUrl.trim(),
        selectedProjectId: selectedProjectId.trim(),
      ),
    );
  }

  void connectDiscoveredRuntime() {
    _send('connect-discovered', const bindings.AgentRuntimeRequestConnectDiscoveredRuntime(discoveryPath: '', selectedSessionId: ''));
  }

  void refreshIcloudRemoteDiscovery() {
    _send('icloud-discover', agentRuntimeIcloudRefreshIntentForTest());
  }

  void connectIcloudRemoteRuntime() {
    _send('icloud-connect', agentRuntimeIcloudConnectIntentForTest());
  }

  void importRemoteProfileDocument() {
    unawaited(_importRemoteProfileDocument());
  }

  Future<void> _importRemoteProfileDocument() async {
    String? profilePath;
    try {
      profilePath = await _remoteProfilePicker();
    } catch (_) {
      _send('import-profile', agentRuntimeImportProfileIntentForTest());
      return;
    }
    if (profilePath == null) {
      return;
    }
    final trimmedPath = profilePath.trim();
    _send(
      'import-profile',
      agentRuntimeImportProfileIntentForTest(
        profilePath: trimmedPath.isEmpty ? null : trimmedPath,
      ),
    );
  }

  void refreshImportedRemoteProfile() {
    _send('imported-refresh', agentRuntimeRefreshImportedProfileIntentForTest());
  }

  void connectImportedRemoteRuntime() {
    _send('imported-connect', agentRuntimeConnectImportedProfileIntentForTest());
  }

  void disconnect() {
    _send('disconnect', const bindings.AgentRuntimeRequestDisconnect());
  }

  void createSession() {
    _bridgeErrorMessage = 'Use the New session dialog to choose project, role, model, title, name, workdir, and worktree root.';
    notifyListeners();
  }

  void createSessionFromDraft({
    required String role,
    required String project,
    required String model,
    required String workdir,
    required String worktreeRoot,
    required String title,
    required String name,
  }) {
    final missing = <String>[
      if (role.trim().isEmpty) 'role',
      if (project.trim().isEmpty) 'project',
      if (model.trim().isEmpty) 'model',
      if (workdir.trim().isEmpty) 'workdir',
      if (worktreeRoot.trim().isEmpty) 'worktree root',
      if (title.trim().isEmpty) 'title',
      if (name.trim().isEmpty) 'name',
    ];
    if (missing.isNotEmpty) {
      _bridgeErrorMessage = 'Create session requires ${missing.join(', ')}.';
      notifyListeners();
      return;
    }
    _dispatchOperation(
      'session-create',
      bindings.AgentRuntimeGuiOperationCreateSession(
        role: role.trim(),
        project: project.trim(),
        model: model.trim(),
        workdir: workdir.trim(),
        worktreeRoot: worktreeRoot.trim(),
        title: title.trim(),
        name: name.trim(),
      ),
    );
  }

  void createProject({
    required String projectKey,
    required String displayName,
    required String defaultWorkdir,
    required String defaultWorktreeRoot,
    required String defaultRoleId,
    required String defaultModel,
  }) {
    _dispatchOperation(
      'project-create',
      bindings.AgentRuntimeGuiOperationCreateProject(
        projectKey: projectKey.trim(),
        displayName: displayName.trim(),
        defaultWorkdir: defaultWorkdir.trim(),
        defaultWorktreeRoot: defaultWorktreeRoot.trim(),
        defaultRoleId: defaultRoleId.trim(),
        defaultModel: defaultModel.trim(),
      ),
    );
  }

  void updateProject({
    required String projectKey,
    required String displayName,
    required String defaultWorkdir,
    required String defaultWorktreeRoot,
    required String defaultRoleId,
    required String defaultModel,
  }) {
    _dispatchOperation(
      'project-update',
      bindings.AgentRuntimeGuiOperationUpdateProject(
        projectKey: projectKey.trim(),
        displayName: displayName.trim(),
        defaultWorkdir: defaultWorkdir.trim(),
        defaultWorktreeRoot: defaultWorktreeRoot.trim(),
        defaultRoleId: defaultRoleId.trim(),
        defaultModel: defaultModel.trim(),
      ),
    );
  }

  void archiveProject(String projectKey) {
    _dispatchOperation('project-archive', bindings.AgentRuntimeGuiOperationArchiveProject(projectKey: projectKey.trim()));
  }

  void unarchiveProject(String projectKey) {
    _dispatchOperation('project-unarchive', bindings.AgentRuntimeGuiOperationUnarchiveProject(projectKey: projectKey.trim()));
  }

  void updateSessionSettings({
    required String sessionId,
    required String project,
    required String role,
    required String model,
    required String workdir,
    required String worktreeRoot,
    required String title,
    required String name,
    required bool tracked,
  }) {
    _dispatchOperation(
      'session-settings-update',
      bindings.AgentRuntimeGuiOperationUpdateSessionSettings(
        sessionId: sessionId.trim(),
        project: project.trim(),
        role: role.trim(),
        model: model.trim(),
        workdir: workdir.trim(),
        worktreeRoot: worktreeRoot.trim(),
        title: title.trim(),
        name: name.trim(),
        tracked: tracked,
      ),
    );
  }

  void selectSession(String sessionId) {
    _dispatchOperation('session-select', bindings.AgentRuntimeGuiOperationSelectSession(sessionId: sessionId));
  }

  void sendMessage(String sessionId, String message) {
    if (sessionId.isEmpty || message.trim().isEmpty) {
      return;
    }
    _dispatchOperation(
      'session-send',
      bindings.AgentRuntimeGuiOperationSendMessage(sessionId: sessionId, message: message.trim()),
    );
  }

  void terminateProcess(String handle) {
    final sessionId = shellData?.selectedSessionId;
    if (sessionId == null || sessionId.isEmpty || handle.isEmpty) {
      return;
    }
    _dispatchOperation('process-terminate', agentRuntimeTerminateProcessOperationForTest(sessionId, handle));
  }

  void inputProcess(String handle, String text) {
    final sessionId = shellData?.selectedSessionId;
    if (sessionId == null || sessionId.isEmpty || handle.isEmpty || text.isEmpty) {
      return;
    }
    _dispatchOperation('process-input', agentRuntimeInputProcessOperationForTest(sessionId, handle, text));
  }

  void flushProcess(String handle) {
    final sessionId = shellData?.selectedSessionId;
    if (sessionId == null || sessionId.isEmpty || handle.isEmpty) {
      return;
    }
    _dispatchOperation('process-flush', agentRuntimeFlushProcessOperationForTest(sessionId, handle));
  }

  void closeSession(String sessionId) {
    if (sessionId.isEmpty) {
      return;
    }
    _dispatchOperation('session-close', agentRuntimeCloseSessionOperationForTest(sessionId));
  }

  void archiveSession(String sessionId) {
    if (sessionId.isEmpty) {
      return;
    }
    _dispatchOperation('session-archive', agentRuntimeArchiveSessionOperationForTest(sessionId));
  }

  void forkSession(String sessionId) {
    if (sessionId.isEmpty) {
      return;
    }
    _dispatchOperation('session-fork', agentRuntimeForkSessionOperationForTest(sessionId));
  }

  void approveAction(AgentRuntimeActionItem action, String reason) {
    if (reason.trim().isEmpty) {
      _bridgeErrorMessage = 'Approval reason is required.';
      notifyListeners();
      return;
    }
    final requestId = _dispatchOperation('approval-decide', agentRuntimeApprovalDecisionOperationForTest(action.id, 'approved', reason: reason.trim()));
    _approvalListUpdates[requestId] = action.id;
  }

  void denyAction(AgentRuntimeActionItem action, String reason) {
    if (reason.trim().isEmpty) {
      _bridgeErrorMessage = 'Approval reason is required.';
      notifyListeners();
      return;
    }
    final requestId = _dispatchOperation('approval-deny', agentRuntimeApprovalDecisionOperationForTest(action.id, 'denied', reason: reason.trim()));
    _approvalListUpdates[requestId] = action.id;
  }

  void resumeApproval(AgentRuntimeActionItem action) {
    final requestId = _dispatchOperation('approval-resume', agentRuntimeApprovalResumeOperationForTest(action.id));
    _approvalListUpdates[requestId] = action.id;
  }


  void listCommandRegistry(String sessionId, String projectKey) {
    _dispatchOperation('registry-list', bindings.AgentRuntimeGuiOperationListCommandRegistry(sessionId: sessionId, projectKey: projectKey));
  }

  void listCommandRegistryRequests() {
    _dispatchOperation('registry-requests', bindings.AgentRuntimeGuiOperationListCommandRegistryRequests());
  }

  void showCommand(AgentRuntimeActionItem action, String sessionId, String projectKey) {
    _dispatchOperation('registry-show-command', bindings.AgentRuntimeGuiOperationShowCommand(actionId: action.id, sessionId: sessionId, projectKey: projectKey));
  }

  void showCommandRegistryRequest(AgentRuntimeActionItem action) {
    _dispatchOperation('registry-review', bindings.AgentRuntimeGuiOperationShowCommandRegistryRequest(requestId: action.id));
  }

  void compactSession(AgentRuntimeActionItem action) {
    _dispatchOperation('session-compact', bindings.AgentRuntimeGuiOperationCompactSession(sessionId: action.id, throughTurn: ''));
  }

  void grantGodMode(AgentRuntimeActionItem action) {
    _dispatchOperation('god-mode-grant', bindings.AgentRuntimeGuiOperationGrantGodMode(sessionId: action.id, reason: 'Owner enabled break-glass shell for this session'));
  }

  void revokeGodMode(AgentRuntimeActionItem action) {
    _dispatchOperation('god-mode-revoke', bindings.AgentRuntimeGuiOperationRevokeGodMode(sessionId: action.id, reason: 'Owner revoked break-glass shell for this session'));
  }

  void approveCommandRegistryRequest(AgentRuntimeActionItem action, String sessionId, AgentRuntimeCommandRegistryDecisionDraft decision) {
    _dispatchOperation('registry-decide', agentRuntimeCommandRegistryDecisionOperationForTest(action.id, sessionId, decision.copyWith(status: 'approved')));
  }

  void denyCommandRegistryRequest(AgentRuntimeActionItem action, String sessionId, AgentRuntimeCommandRegistryDecisionDraft decision) {
    _dispatchOperation('registry-deny', agentRuntimeCommandRegistryDenyOperationForTest(action.id, sessionId, decision.copyWith(status: 'denied')));
  }

  void previewCommandRegistryRequest(AgentRuntimeActionItem action, String sessionId, AgentRuntimeCommandRegistryDecisionDraft decision) {
    _dispatchOperation('registry-preview', agentRuntimeCommandRegistryPreviewOperationForTest(action.id, sessionId, decision));
  }

  void applyCommandRegistryRequest(AgentRuntimeActionItem action, String sessionId) {
    _dispatchOperation('registry-apply', agentRuntimeCommandRegistryApplyOperationForTest(action.id, sessionId));
  }

  void validateRoleDraft(AgentRuntimeRoleEditorDraft draft) {
    _dispatchOperation('role-validate', bindings.AgentRuntimeGuiOperationValidateRoleDraft(draft: _typedRoleDraft(draft)));
  }

  void createRoleFromDraft(AgentRuntimeRoleEditorDraft draft) {
    _dispatchOperation('role-create', bindings.AgentRuntimeGuiOperationCreateRoleFromDraft(draft: _typedRoleDraft(draft)));
  }

  void updateRoleFromDraft(AgentRuntimeRoleEditorDraft draft) {
    _dispatchOperation('role-update', bindings.AgentRuntimeGuiOperationUpdateRoleFromDraft(roleId: draft.roleId, draft: _typedRoleDraft(draft)));
  }

  void showRoleDetail(String roleId) {
    _dispatchOperation('role-detail', bindings.AgentRuntimeGuiOperationShowRoleDetail(roleId: roleId));
  }

  void listRoleVersions(String roleId) {
    _dispatchOperation('role-versions', bindings.AgentRuntimeGuiOperationListRoleVersions(roleId: roleId));
  }

  void showRoleVersion(String versionId) {
    _dispatchOperation('role-version-data', bindings.AgentRuntimeGuiOperationShowRoleVersion(versionId: versionId));
  }

  void exportRole(String roleId) {
    _dispatchOperation('role-export', bindings.AgentRuntimeGuiOperationExportRole(roleId: roleId));
  }

  void archiveRole(String roleId) {
    _dispatchOperation('role-archive', bindings.AgentRuntimeGuiOperationArchiveRole(roleId: roleId));
  }

  void unarchiveRole(String roleId) {
    _dispatchOperation('role-unarchive', bindings.AgentRuntimeGuiOperationUnarchiveRole(roleId: roleId));
  }

  void activateRoleVersion(String roleId, String versionId) {
    _dispatchOperation('role-activate', agentRuntimeRoleActivateOperationForTest(roleId, versionId));
  }

  void selectWorkflowMemory(AgentRuntimeWorkflowMemoryRow row) {
    _dispatchOperation('workflow-memory-select', agentRuntimeWorkflowMemorySelectOperationForTest(row.id));
  }

  void markWorkflowMemoryAttempted(AgentRuntimeWorkflowMemoryDetail detail) {
    final sessionId = detail.feedbackSessionId;
    if (sessionId == null || !detail.feedbackEnabled) {
      return;
    }
    _dispatchOperation(
      'workflow-memory-attempted',
      agentRuntimeWorkflowMemoryFeedbackOperationForTest(
        memoryId: detail.id,
        sessionId: sessionId,
        feedback: 'attempted',
        payload: const bindings.AgentRuntimeWorkflowMemoryFeedbackPayload(
          source: 'gui.workbench',
          reason: '',
          variant: true,
          hasVariant: true,
        ),
      ),
    );
  }

  void markWorkflowMemoryHelpful(AgentRuntimeWorkflowMemoryDetail detail) {
    final sessionId = detail.feedbackSessionId;
    if (sessionId == null || !detail.feedbackEnabled) {
      return;
    }
    _dispatchOperation(
      'workflow-memory-helpful',
      agentRuntimeWorkflowMemoryFeedbackOperationForTest(
        memoryId: detail.id,
        sessionId: sessionId,
        feedback: 'helpful',
        payload: const bindings.AgentRuntimeWorkflowMemoryFeedbackPayload(
          source: 'gui.workbench',
          reason: '',
          variant: false,
          hasVariant: false,
        ),
      ),
    );
  }

  void markWorkflowMemoryNotHelpful(AgentRuntimeWorkflowMemoryDetail detail) {
    final sessionId = detail.feedbackSessionId;
    if (sessionId == null || !detail.feedbackEnabled) {
      return;
    }
    _dispatchOperation(
      'workflow-memory-not-helpful',
      agentRuntimeWorkflowMemoryFeedbackOperationForTest(
        memoryId: detail.id,
        sessionId: sessionId,
        feedback: 'notHelpful',
        payload: const bindings.AgentRuntimeWorkflowMemoryFeedbackPayload(
          source: 'gui.workbench',
          reason: 'marked from Agent Runtime',
          variant: false,
          hasVariant: false,
        ),
      ),
    );
  }

  String _dispatchOperation(String prefix, bindings.AgentRuntimeGuiOperation operation) {
    return _send(prefix, bindings.AgentRuntimeRequestDispatchOperation(operation: operation));
  }

  String _send(String prefix, bindings.AgentRuntimeRequest request) {
    return _sendWithTracking(prefix, request, trackPending: true);
  }

  String _sendWithTracking(String prefix, bindings.AgentRuntimeRequest request, {required bool trackPending}) {
    _serial += 1;
    final requestId = 'agent-runtime-$prefix-$_serial';
    if (trackPending) {
      _pendingRequestIds.add(requestId);
    }
    try {
      _requestSink(requestId, request);
    } catch (_) {
      _pendingRequestIds.remove(requestId);
      _bridgeErrorMessage = 'Agent Runtime bridge is not ready. Restart the app, then refresh discovery.';
    }
    notifyListeners();
    return requestId;
  }

  static void _sendRequestSignalToRust(String requestId, bindings.AgentRuntimeRequest request) {
    bindings.AgentRuntimeRequestSignal(requestId: requestId, request: request).sendSignalToRust();
  }

  void _handleOutput(RustSignalPack<bindings.AgentRuntimeOutputSignal> pack) {
    final signal = pack.message;
    _pendingRequestIds.remove(signal.requestId);
    _applyOutput(signal.output);
    _applyApprovalListUpdate(signal.requestId, signal.output);
  }

  @visibleForTesting
  void applyOutputForTest(bindings.AgentRuntimeOutput output) {
    _applyOutput(output);
  }

  @visibleForTesting
  void setViewDataForTest(AgentRuntimeWorkbenchData data, {ConversationShellData? shell}) {
    _viewModel = data;
    _shellViewModel = shell;
    notifyListeners();
  }

  @visibleForTesting
  void applyOutputForRequestForTest(String requestId, bindings.AgentRuntimeOutput output) {
    _pendingRequestIds.remove(requestId);
    _applyOutput(output);
    _applyApprovalListUpdate(requestId, output);
  }

  void _applyOutput(bindings.AgentRuntimeOutput output) {
    if (output is bindings.AgentRuntimeOutputWorkbenchView) {
      _viewModel = _workbenchData(output.viewModel);
      _shellViewModel = _shellData(output.viewModel.shell);
      _bridgeErrorMessage = null;
    } else if (output is bindings.AgentRuntimeOutputOperationResult) {
      final current = _viewModel ?? _disconnectedViewModel;
      final isError = output.result.outcome == 'error';
      final message = output.result.message.isEmpty ? output.result.outcome : output.result.message;
      _viewModel = current.copyWith(
        outputLog: <String>[
          ...current.outputLog.take(49),
          '${output.result.operation}: $message',
        ],
        pendingRequestCount: _pendingRequestIds.length,
        errorMessage: isError ? message : null,
      );
      _shellViewModel = _copyShellWithError(_shellViewModel, isError ? message : null);
      _bridgeErrorMessage = isError ? message : null;
    } else if (output is bindings.AgentRuntimeOutputProjectionSnapshot) {
      final current = _viewModel ?? _disconnectedViewModel;
      _viewModel = current.copyWith(
        watermarkLabel: output.projection.watermark.toString(),
        statusBadges: [
          ...current.statusBadges.where((badge) => badge.label != 'Sessions' && badge.label != 'History events'),
          AgentRuntimeStatusBadge(label: 'Sessions', value: output.projection.sessionCount.toString(), tone: output.projection.sessionCount == 0 ? 'muted' : 'info'),
          AgentRuntimeStatusBadge(label: 'History events', value: output.projection.timelineCount.toString(), tone: output.projection.timelineCount == 0 ? 'muted' : 'info'),
        ],
        pendingRequestCount: _pendingRequestIds.length,
      );
    } else if (output is bindings.AgentRuntimeOutputControllerState) {
      final current = _viewModel ?? _disconnectedViewModel;
      _viewModel = current.copyWith(
        connectionState: output.controllerState.connectionState,
        baseUrl: output.controllerState.baseUrl.isEmpty ? current.baseUrl : output.controllerState.baseUrl,
        selectedSessionLabel: output.controllerState.hasSelectedSessionId ? output.controllerState.selectedSessionId : current.selectedSessionLabel,
        errorMessage: output.controllerState.hasLastError ? output.controllerState.lastError : null,
        pendingRequestCount: _pendingRequestIds.length,
      );
      _bridgeErrorMessage = output.controllerState.hasLastError ? output.controllerState.lastError : null;
    } else if (output is bindings.AgentRuntimeOutputStreamOutcome) {
      final current = _viewModel ?? _disconnectedViewModel;
      final outcome = output.outcome;
      final selectedChatEntries = output.hasProjection && output.projection.selectedChatEntries.isNotEmpty
          ? output.projection.selectedChatEntries.map(_chatEntry).toList(growable: false)
          : null;
      final streamLabel = switch (outcome) {
        bindings.AgentRuntimeStreamOutcomeHello(:final watermark) => 'stream hello · $watermark',
        bindings.AgentRuntimeStreamOutcomeDeltaApplied(:final applyOutcome) => 'stream delta · $applyOutcome',
        bindings.AgentRuntimeStreamOutcomeResyncRequired(:final reason, :final hasReason) => 'stream resync · ${hasReason ? reason : 'required'}',
        bindings.AgentRuntimeStreamOutcomeServerShutdown() => 'stream shutdown',
        bindings.AgentRuntimeStreamOutcomeStreamClosed() => 'stream closed',
        _ => 'stream update',
      };
      _viewModel = current.copyWith(
        connectionState: output.controllerState.connectionState,
        watermarkLabel: output.hasProjection ? output.projection.watermark.toString() : current.watermarkLabel,
        outputLog: <String>[...current.outputLog.take(49), streamLabel],
        selectedConversation: selectedChatEntries,
        pendingRequestCount: _pendingRequestIds.length,
        errorMessage: output.controllerState.hasLastError ? output.controllerState.lastError : null,
      );
      if (selectedChatEntries != null) {
        _shellViewModel = _copyShellWithEntries(
          _shellViewModel,
          selectedChatEntries,
          output.controllerState.hasSelectedSessionId ? output.controllerState.selectedSessionId : null,
        );
      }
      _bridgeErrorMessage = output.controllerState.hasLastError ? output.controllerState.lastError : null;
    } else if (output is bindings.AgentRuntimeOutputError) {
      final current = _viewModel ?? _disconnectedViewModel;
      final message = output.error.message.isNotEmpty ? output.error.message : output.error.code;
      _viewModel = current.copyWith(
        errorMessage: message,
        outputLog: <String>[...current.outputLog.take(49), 'Error: $message'],
        pendingRequestCount: _pendingRequestIds.length,
      );
      _shellViewModel = _copyShellWithError(_shellViewModel, message);
      _bridgeErrorMessage = message;
    }
    notifyListeners();
  }

  void _applyApprovalListUpdate(String requestId, bindings.AgentRuntimeOutput output) {
    final approvalId = _approvalListUpdates.remove(requestId);
    if (approvalId == null || output is! bindings.AgentRuntimeOutputOperationResult || output.result.outcome == 'error') {
      return;
    }
    bool matchesApproval(AgentRuntimeActionItem action) {
      return action.id == approvalId && (action.kind == 'approval' || action.kind == 'approvalDeny' || action.kind == 'approvalResume');
    }

    AgentRuntimeOperationSurface refreshSurface(AgentRuntimeOperationSurface surface) {
      if (surface.surfaceId != 'approvals') {
        return surface;
      }
      return AgentRuntimeOperationSurface(
        surfaceId: surface.surfaceId,
        title: surface.title,
        subtitle: surface.subtitle,
        rows: surface.rows,
        actions: surface.actions.where((action) => !matchesApproval(action)).toList(growable: false),
      );
    }

    final current = _viewModel;
    if (current == null) {
      return;
    }
    _viewModel = current.copyWith(
      actions: current.actions.where((action) => !matchesApproval(action)).toList(growable: false),
      operationSurfaces: current.operationSurfaces.map(refreshSurface).toList(growable: false),
    );
    notifyListeners();
  }

  AgentRuntimeWorkbenchData get _disconnectedViewModel => AgentRuntimeWorkbenchData(
        connectionState: 'disconnected',
        discovery: const AgentRuntimeDiscoveryInfo(
          state: 'notLoaded',
          tone: 'muted',
          title: 'Discovery not loaded',
          message: 'Refresh discovery to check the local Agent Runtime service.',
          discoveryPath: '',
          connectable: false,
        ),
        remoteDiscovery: const AgentRuntimeDiscoveryInfo(
          sourceType: 'iCloudRemoteProfile',
          state: 'notLoaded',
          tone: 'muted',
          title: 'iCloud remote profile not loaded',
          message: 'Refresh iCloud profile discovery to inspect the synced remote profile. /health determines connectability.',
          discoveryPath: '',
          connectable: false,
        ),
        importedRemoteDiscovery: const AgentRuntimeDiscoveryInfo(
          sourceType: 'importedRemoteProfile',
          state: 'notLoaded',
          tone: 'muted',
          title: 'Imported profile not loaded',
          message: 'Import a remote profile document. The app saves a copy and checks health before connecting.',
          discoveryPath: '',
          connectable: false,
        ),
        connectionTone: 'muted',
        baseUrl: _baseUrl,
        statusLabel: 'Not connected',
        watermarkLabel: '—',
        statusBadges: const [
          AgentRuntimeStatusBadge(label: 'Connection', value: 'disconnected', tone: 'muted'),
        ],
        selectedSessionLabel: 'none selected',
        sessionsTitle: 'Sessions',
        sessionsSubtitle: '',
        timelineTitle: 'Selected session',
        timelineSubtitle: '',
        actionsTitle: 'Attention',
        actionsSubtitle: '',
        detailTitle: 'Runtime detail',
        detailSubtitle: '',
        sessionsEmptyTitle: 'No sessions',
        sessionsEmptyText: 'Connect to hydrate runtime sessions.',
        timelineEmptyTitle: 'No timeline',
        timelineEmptyText: 'Create or select a session to see activity.',
        actionsEmptyTitle: 'No action required',
        actionsEmptyText: 'No items need attention.',
        sessions: const [],
        timeline: const [],
        actions: const [],
        roleAdmin: mockAgentRuntimeRoleAdminEmpty,
        workflowMemory: mockAgentRuntimeWorkflowMemoryEmpty,
        controllerFacts: const [],
        operationSurfaces: const [],
        outputLog: const [],
        pendingRequestCount: _pendingRequestIds.length,
      );

  @override
  void dispose() {
    _subscription?.cancel();
    super.dispose();
  }
}

ConversationShellData? _copyShellWithEntries(ConversationShellData? shell, List<ChatEntry> entries, String? selectedSessionId) {
  if (shell == null) {
    return null;
  }
  return ConversationShellData(
    appTitle: shell.appTitle,
    connectionLabel: shell.connectionLabel,
    projects: shell.projects,
    sessions: shell.sessions,
    selectedSessionId: selectedSessionId ?? shell.selectedSessionId,
    timelineTitle: shell.timelineTitle,
    entries: entries,
    composerEnabled: shell.composerEnabled,
    isRunning: entries.any((entry) => entry.isStreaming || (entry.status ?? '').toLowerCase().contains('running')),
    detailTitle: shell.detailTitle,
    detailSections: shell.detailSections,
    emptyTitle: shell.emptyTitle,
    emptyText: shell.emptyText,
    projectLabel: shell.projectLabel,
    sessionLabel: shell.sessionLabel,
    composerPlaceholder: shell.composerPlaceholder,
    composerDisabledHint: shell.composerDisabledHint,
    inlineErrorMessage: shell.inlineErrorMessage,
  );
}

ConversationShellData? _copyShellWithError(ConversationShellData? shell, String? message) {
  if (shell == null) {
    return null;
  }
  return ConversationShellData(
    appTitle: shell.appTitle,
    connectionLabel: shell.connectionLabel,
    projects: shell.projects,
    sessions: shell.sessions,
    selectedSessionId: shell.selectedSessionId,
    timelineTitle: shell.timelineTitle,
    entries: shell.entries,
    composerEnabled: shell.composerEnabled,
    isRunning: shell.isRunning,
    detailTitle: shell.detailTitle,
    detailSections: shell.detailSections,
    emptyTitle: shell.emptyTitle,
    emptyText: shell.emptyText,
    projectLabel: shell.projectLabel,
    sessionLabel: shell.sessionLabel,
    composerPlaceholder: shell.composerPlaceholder,
    composerDisabledHint: shell.composerDisabledHint,
    inlineErrorMessage: message,
  );
}

bindings.AgentRuntimeRoleEditorDraft _typedRoleDraft(AgentRuntimeRoleEditorDraft draft) {
  return bindings.AgentRuntimeRoleEditorDraft(
    id: draft.roleId,
    version: draft.version,
    displayName: draft.displayName,
    modelDefaults: bindings.AgentRuntimeRoleEditorModelDefaults(
      model: draft.model,
      reasoningEffort: draft.reasoningEffort,
    ),
    instructionText: draft.instructionText,
    capabilities: draft.capabilities,
    policyEntries: draft.policy
        .map((row) => bindings.AgentRuntimeRolePolicyEntry(key: row.action, value: row.decision))
        .toList(growable: false),
    routing: bindings.AgentRuntimeRoleEditorRoutingMetadata(
      mode: draft.routingMode,
      defaultRecipient: draft.defaultRecipient ?? '',
      hasDefaultRecipient: draft.defaultRecipient != null && draft.defaultRecipient!.isNotEmpty,
      allowedRecipients: draft.allowedRecipients,
      reservedActions: draft.routingReservedActions,
    ),
    visibility: bindings.AgentRuntimeRoleEditorVisibilityMetadata(
      listed: draft.listed,
      ownerVisible: draft.ownerVisible,
    ),
    lifecycleAuthority: bindings.AgentRuntimeRoleEditorLifecycleAuthorityMetadata(
      canSpawnAgents: draft.canSpawnAgents,
      canArchiveAgents: draft.canArchiveAgents,
      reservedActions: draft.lifecycleReservedActions,
    ),
  );
}

AgentRuntimeWorkbenchData _workbenchData(bindings.AgentRuntimeWorkbenchViewModel view) {
  return AgentRuntimeWorkbenchData(
    discovery: _discovery(view.discovery),
    remoteDiscovery: _discovery(view.remoteDiscovery),
    importedRemoteDiscovery: _discovery(view.importedRemoteDiscovery),
    connectionState: view.connectionState,
    connectionTone: view.connectionTone,
    baseUrl: view.baseUrl,
    statusLabel: view.statusLabel,
    watermarkLabel: view.watermarkLabel,
    statusBadges: view.statusBadges.map(_badge).toList(growable: false),
    modelOptions: view.modelOptions.map(_modelOption).toList(growable: false),
    selectedSessionLabel: view.selectedSessionLabel,
    sessionsTitle: view.sessionsTitle,
    sessionsSubtitle: view.sessionsSubtitle,
    timelineTitle: view.timelineTitle,
    timelineSubtitle: view.timelineSubtitle,
    actionsTitle: view.actionsTitle,
    actionsSubtitle: view.actionsSubtitle,
    detailTitle: view.detailTitle,
    detailSubtitle: view.detailSubtitle,
    sessionsEmptyTitle: view.sessionsEmptyTitle,
    sessionsEmptyText: view.sessionsEmptyText,
    timelineEmptyTitle: view.timelineEmptyTitle,
    timelineEmptyText: view.timelineEmptyText,
    actionsEmptyTitle: view.actionsEmptyTitle,
    actionsEmptyText: view.actionsEmptyText,
    sessions: view.sessions.map(_session).toList(growable: false),
    timeline: view.timeline.map(_timelineRow).toList(growable: false),
    selectedConversation: view.shell.selectedConversation.map(_chatEntry).toList(growable: false),
    actions: view.actions.map(_action).toList(growable: false),
    roleAdmin: _roleAdmin(view.roleAdmin),
    workflowMemory: _workflowMemory(view.workflowMemory),
    controllerFacts: view.controllerFacts.map(_fact).toList(growable: false),
    operationSurfaces: view.shell.operationSurfaces.map(_operationSurface).toList(growable: false),
    outputLog: view.outputLog,
    pendingRequestCount: view.pendingRequestCount,
    errorMessage: view.hasErrorMessage ? view.errorMessage : null,
  );
}

AgentRuntimeModelOption _modelOption(bindings.AgentRuntimeModelOption option) => AgentRuntimeModelOption(
      id: option.id,
      displayLabel: option.displayLabel,
      source: option.source,
      isDefault: option.isDefault,
    );

ConversationShellData _shellData(bindings.AgentRuntimeConversationShellViewModel view) {
  final roles = {for (final role in view.dynamicRoles) role.roleId: role};
  final selectedSessionId = view.hasSelectedSessionId ? view.selectedSessionId : null;
  return ConversationShellData(
    appTitle: 'Agent Runtime',
    connectionLabel: 'Runtime connected',
    projects: view.projects
        .map((project) => ConversationProject(
              id: project.id,
              title: project.title,
              subtitle: project.subtitle,
              canEdit: project.id != '__all__' && project.id != '__unassigned__',
              canArchive: project.id != '__all__' && project.id != '__unassigned__',
              canCreateSession: true,
              defaultWorkdir: project.defaultWorkdir,
              defaultWorktreeRoot: project.defaultWorktreeRoot,
              defaultRoleId: project.defaultRoleId,
              defaultModel: project.defaultModel,
              archived: project.archived,
            ))
        .toList(growable: false),
    sessions: view.sessions
        .map((session) {
          final role = roles[session.groupLabel] ??
              bindings.AgentRuntimeShellRolePresentation(
                roleId: session.groupLabel,
                displayLabel: session.groupLabel,
                shortLabel: session.groupLabel.isEmpty ? 'AR' : session.groupLabel.substring(0, 1).toUpperCase(),
                tone: session.tone,
                description: session.subtitle,
              );
          return ConversationSession(
            id: session.id,
            title: _runtimeDisplayCopy(session.title),
            subtitle: _runtimeDisplayCopy(session.subtitle),
            role: _runtimeDisplayCopy(session.groupLabel),
            selected: session.id == selectedSessionId,
            rolePresentation: ConversationRolePresentation(
              roleId: _runtimeDisplayCopy(role.roleId),
              displayLabel: _runtimeDisplayCopy(role.displayLabel),
              shortLabel: role.shortLabel,
              iconKey: role.roleId,
              tone: role.tone,
              statusLabel: _runtimeDisplayCopy(session.status),
              description: _runtimeDisplayCopy(role.description),
            ),
          );
        })
        .toList(growable: false),
    selectedSessionId: selectedSessionId,
    timelineTitle: selectedSessionId == null ? 'Select a session' : 'Selected session',
    entries: view.selectedConversation
        .map(_chatEntry)
        .toList(growable: false),
    composerEnabled: selectedSessionId != null,
    isRunning: view.selectedConversation.any((entry) => entry.isStreaming || entry.status.toLowerCase().contains('running')),
    detailTitle: 'Operations',
    detailSections: view.operationSurfaces
        .map((surface) => ConversationDetailSection(
              title: surface.title,
              rows: [
                ...surface.rows.map((fact) => ConversationDetailRow(label: _runtimeDisplayCopy(fact.label), value: _runtimeDisplayCopy(fact.value))),
                ...surface.actions.map((action) => ConversationDetailRow(label: _runtimeDisplayCopy(action.title), value: _runtimeDisplayCopy(action.stateText))),
              ],
            ))
        .toList(growable: false),
    emptyTitle: 'No sessions yet',
    emptyText: 'Create a session to start working.',
  );
}

ChatEntry _chatEntry(bindings.AgentRuntimeChatEntry entry) {
  return ChatEntry(
    id: entry.id,
    author: entry.author,
    displayLabel: entry.displayLabel,
    timestamp: entry.hasTimestamp ? DateTime.tryParse(entry.timestamp)?.millisecondsSinceEpoch : null,
    body: entry.body,
    subtitle: entry.subtitle,
    kind: entry.kind,
    status: entry.status,
    processId: entry.hasProcessId ? entry.processId : null,
    command: entry.command,
    output: entry.output,
    deliveryState: entry.deliveryState,
    isStreaming: entry.isStreaming,
    isTool: entry.isTool,
  );
}

String _runtimeDisplayCopy(String value) {
  return value
      .replaceAll(RegExp(r'/Users/[^ ]+'), 'Project workspace')
      .replaceAll('tool.call execute_code', 'Execute code')
      .replaceAll('tool.completed', 'Execute code')
      .replaceAll('execute_code', 'Code run')
      .replaceAll('approval.requested', 'Approval requested')
      .replaceAll('cmd.rg.audit', 'Command review')
      .replaceAll('rg · audit', 'Search audit')
      .replaceAll('runtime-allow', 'Runtime allow')
      .replaceAll('projection', 'runtime')
      .replaceAll('controller', 'connection')
      .trim();
}

AgentRuntimeDiscoveryInfo _discovery(bindings.AgentRuntimeDiscoveryView view) => AgentRuntimeDiscoveryInfo(
      state: view.state,
      tone: view.tone,
      title: view.title,
      message: view.message,
      sourceType: view.sourceType,
      sourcePath: view.sourcePath,
      lastImportedAt: view.hasLastImportedAt ? view.lastImportedAt : null,
      discoveryPath: view.discoveryPath,
      connectable: view.connectable,
      baseUrl: view.hasBaseUrl ? view.baseUrl : null,
      healthUrl: view.hasHealthUrl ? view.healthUrl : null,
      webSocketUrl: view.hasWebSocketUrl ? view.webSocketUrl : null,
      runtimeIdentity: view.hasRuntimeIdentity ? view.runtimeIdentity : null,
      serviceState: view.hasServiceState ? view.serviceState : null,
      diagnostics: view.diagnostics,
    );

AgentRuntimeFact _fact(bindings.AgentRuntimeFact fact) => AgentRuntimeFact(label: fact.label, value: _humanRuntimeLabel(fact.value));

AgentRuntimeOperationSurface _operationSurface(bindings.AgentRuntimeOperationSurface surface) => AgentRuntimeOperationSurface(
      surfaceId: surface.surfaceId,
      title: surface.title,
      subtitle: surface.subtitle,
      rows: surface.rows.map(_fact).toList(growable: false),
      actions: surface.actions.map(_action).toList(growable: false),
    );

AgentRuntimeStatusBadge _badge(bindings.AgentRuntimeBadge badge) => AgentRuntimeStatusBadge(label: badge.label, value: badge.value, tone: badge.tone);

AgentRuntimeActionItem _action(bindings.AgentRuntimeActionRow row) => AgentRuntimeActionItem(
      id: row.id,
      title: _humanRuntimeLabel(row.title),
      subtitle: _humanRuntimeLabel(row.subtitle),
      kind: row.kind,
      stateText: _humanRuntimeLabel(row.stateText),
      tone: row.tone,
    );

AgentRuntimeSessionItem _session(bindings.AgentRuntimeSessionRow row) => AgentRuntimeSessionItem(
      id: row.id,
      title: row.title,
      status: row.status,
      subtitle: _runtimeDisplayCopy(row.subtitle),
      groupLabel: _runtimeDisplayCopy(row.groupLabel),
      tone: row.tone,
    );

AgentRuntimeTimelineItem _timelineRow(bindings.AgentRuntimeTimelineRow row) => AgentRuntimeTimelineItem(
      id: row.id,
      title: _humanRuntimeLabel(row.title),
      subtitle: _humanRuntimeLabel(row.subtitle),
      status: _humanRuntimeLabel(row.status),
      tone: row.tone,
    );

AgentRuntimeRoleAdminData _roleAdmin(bindings.AgentRuntimeRoleAdminView view) => AgentRuntimeRoleAdminData(
      title: view.title,
      subtitle: view.subtitle,
      emptyTitle: view.emptyTitle,
      emptyText: view.emptyText,
      rows: view.rows.map(_roleRow).toList(growable: false),
      selectedDetail: view.hasSelectedDetail ? _roleDetail(view.selectedDetail) : null,
      versionRows: view.versionRows.map((row) => AgentRuntimeRoleVersionRow(
            versionId: row.versionId,
            version: row.version,
            status: row.status,
            createdAt: row.createdAt.isEmpty ? null : row.createdAt,
          )).toList(growable: false),
      editorDraft: view.hasEditorDraft ? _roleEditorDraft(view.editorDraft) : null,
      validationErrors: view.validationErrors,
      actionStates: view.actionStates.map(_action).toList(growable: false),
      editorOptions: AgentRuntimeRoleEditorOptions(
        models: view.editorOptions.models,
        reasoningEfforts: view.editorOptions.reasoningEfforts,
        capabilities: view.editorOptions.capabilities,
        policyActions: view.editorOptions.policyActions,
        policyDecisions: view.editorOptions.policyDecisions,
        routingModes: view.editorOptions.routingModes,
        recipients: view.editorOptions.recipients,
        reservedActions: view.editorOptions.reservedActions,
      ),
    );

AgentRuntimeRoleRow _roleRow(bindings.AgentRuntimeRoleRow row) => AgentRuntimeRoleRow(
      id: row.id,
      title: row.title,
      subtitle: row.subtitle,
      status: row.status,
      tone: row.tone,
      currentVersionId: row.currentVersion,
    );

AgentRuntimeRoleDetail _roleDetail(bindings.AgentRuntimeRoleDetail detail) => AgentRuntimeRoleDetail(
      id: detail.id,
      displayName: detail.displayName,
      version: detail.version,
      model: detail.modelLabel,
      status: detail.status,
      instructionText: detail.instructionsPreview,
      capabilities: const <String>[],
      policy: detail.policyRows.map((row) => AgentRuntimeRolePolicyRow(action: row.label, decision: row.value)).toList(growable: false),
      routing: [AgentRuntimeFact(label: 'Routing', value: detail.routingLabel)],
      visibility: [AgentRuntimeFact(label: 'Visibility', value: detail.visibilityLabel)],
      lifecycleAuthority: [AgentRuntimeFact(label: 'Lifecycle', value: detail.lifecycleLabel)],
    );

AgentRuntimeRoleEditorDraft _roleEditorDraft(bindings.AgentRuntimeRoleEditorDraftView draft) => AgentRuntimeRoleEditorDraft(
      roleId: draft.roleId,
      version: draft.version,
      displayName: draft.displayName,
      model: draft.model,
      reasoningEffort: draft.reasoningEffort,
      instructionText: draft.instructionText,
      capabilities: draft.capabilities,
      policy: draft.policyRows.map((row) => AgentRuntimeRolePolicyRow(action: row.label, decision: row.value)).toList(growable: false),
      routingMode: draft.routingMode,
      routingReservedActions: const <String>[],
      defaultRecipient: draft.defaultRecipient.isEmpty ? null : draft.defaultRecipient,
      allowedRecipients: draft.allowedRecipients,
      listed: draft.listed,
      ownerVisible: draft.ownerVisible,
      canSpawnAgents: draft.canSpawnAgents,
      canArchiveAgents: draft.canArchiveAgents,
      lifecycleReservedActions: const <String>[],
    );

AgentRuntimeWorkflowMemoryData _workflowMemory(bindings.AgentRuntimeWorkflowMemoryView view) => AgentRuntimeWorkflowMemoryData(
      title: view.title,
      subtitle: view.subtitle,
      emptyTitle: view.emptyTitle,
      emptyText: view.emptyText,
      selectedMemoryId: view.hasSelectedDetail ? view.selectedDetail.id : null,
      rows: view.rows.map(_workflowMemoryRow).toList(growable: false),
      selectedDetail: view.hasSelectedDetail ? _workflowMemoryDetail(view.selectedDetail) : null,
      recentEvents: view.hasSelectedDetail ? view.selectedDetail.events.map(_workflowMemoryEvent).toList(growable: false) : const <AgentRuntimeWorkflowMemoryEventRow>[],
      feedbackActions: view.actionStates.map(_action).toList(growable: false),
    );

AgentRuntimeWorkflowMemoryRow _workflowMemoryRow(bindings.AgentRuntimeWorkflowMemoryRow row) => AgentRuntimeWorkflowMemoryRow(
      id: row.id,
      title: row.title,
      subtitle: row.reason,
      scopeType: row.scopeLabel,
      projectKey: row.hasProjectKey ? row.projectKey : null,
      helpfulScore: double.tryParse(row.helpfulScore) ?? 0,
      promotedAt: row.hasPromotedAt ? row.promotedAt : null,
      sourceSessionId: row.sourceSessionId,
      tone: row.tone,
      selected: row.isSelected,
    );

AgentRuntimeWorkflowMemoryDetail _workflowMemoryDetail(bindings.AgentRuntimeWorkflowMemoryDetail detail) => AgentRuntimeWorkflowMemoryDetail(
      id: detail.id,
      title: detail.title,
      reason: detail.reason,
      summary: detail.summary,
      sourceSessionId: detail.sourceSessionId,
      sourceScriptRunId: detail.hasSourceScriptRunId ? detail.sourceScriptRunId : null,
      sourceStarlark: detail.sourcePreview,
      sourcePreview: detail.sourcePreview,
      provider: detail.provider.isEmpty ? null : detail.provider,
      model: detail.model.isEmpty ? null : detail.model,
      dimensions: int.tryParse(detail.dimensions),
      storageType: detail.storageLabel.isEmpty ? null : detail.storageLabel,
      sourceHash: detail.sourceHash.isEmpty ? null : detail.sourceHash,
      commandFingerprint: detail.commandFingerprint.isEmpty ? null : detail.commandFingerprint,
      helpfulScore: double.tryParse(detail.score) ?? 0,
      scopeLabel: detail.scopeLabel,
      feedbackSessionId: detail.hasFeedbackSessionId ? detail.feedbackSessionId : null,
      feedbackEnabled: detail.feedbackEnabled,
    );

AgentRuntimeWorkflowMemoryEventRow _workflowMemoryEvent(bindings.AgentRuntimeWorkflowMemoryEvent event) => AgentRuntimeWorkflowMemoryEventRow(
      id: event.id,
      title: _humanRuntimeLabel(event.title),
      subtitle: _humanRuntimeLabel(event.subtitle),
      createdAt: event.createdAt.isEmpty ? null : event.createdAt,
      tone: event.tone,
    );

String _humanRuntimeLabel(String value) {
  return value
      .replaceAll('role.imported', 'Role imported')
      .replaceAll('session.created', 'Session created')
      .replaceAll('turn.started', 'Turn started')
      .replaceAll('turn.completed', 'Turn completed')
      .replaceAll('route.decision', 'Route selected')
      .replaceAll('policy.decision', 'Policy decision')
      .replaceAll('model.tool_call', 'Tool requested')
      .replaceAll('model.final_response', 'Assistant response')
      .replaceAll('tool.started', 'Tool started')
      .replaceAll('tool.completed', 'Tool completed')
      .replaceAll('script.started', 'Script started')
      .replaceAll('script.completed', 'Script completed')
      .replaceAll('host_api.completed', 'Host action completed')
      .replaceAll('workflow_memory.', 'Workflow memory ')
      .replaceAll('command_registry.', 'Command registry ')
      .replaceAll('approval.', 'Approval ')
      .replaceAll('compaction.', 'Compaction ')
      .trim();
}

bindings.AgentRuntimeCommandRegistryDecisionInput _typedRegistryDecision(String sessionId, AgentRuntimeCommandRegistryDecisionDraft decision) {
  return bindings.AgentRuntimeCommandRegistryDecisionInput(
    sessionId: sessionId,
    status: decision.status,
    finalScope: bindings.AgentRuntimeRegistryScope(scopeType: decision.scopeType, projectKey: decision.projectKey),
    hasFinalScope: decision.scopeType.isNotEmpty,
    finalExecutionPolicy: bindings.AgentRuntimeFinalExecutionPolicy(decision: decision.policyDecision, reason: decision.policyReason),
    hasFinalExecutionPolicy: decision.policyDecision.isNotEmpty,
    finalCommand: bindings.AgentRuntimeCommandSeed(
      actionId: decision.actionId,
      binaryName: decision.binaryName,
      candidatePaths: const <String>[],
      starlarkObject: decision.actionId,
      starlarkMethod: 'run',
      argvPrefix: decision.argvTemplate,
      defaultCwd: decision.defaultCwd,
      cwdPolicy: decision.cwdPolicy,
      envPolicy: decision.envPolicy,
      syncAllowed: decision.syncAllowed,
      asyncAllowed: decision.asyncAllowed,
      maxRuntimeMs: decision.maxRuntimeMs ?? 0,
      hasMaxRuntimeMs: decision.maxRuntimeMs != null,
      endOfTurnBehavior: decision.endOfTurnBehavior,
      endOfSessionBehavior: decision.endOfSessionBehavior,
      stdinPolicy: decision.stdinPolicy,
      minAwaitMs: 0,
      maxAwaitMs: 0,
      outputBufferBytes: 65536,
      terminateGraceMs: 1000,
      outputLimitBytes: 65536,
      mutationClass: decision.mutationClass,
      modelDescription: decision.modelDescription,
      allowCwdArg: decision.allowCwdArg,
      allowArgsArg: decision.allowArgsArg,
      forbiddenArgs: decision.forbiddenArgs,
      executionPolicy: decision.executionPolicy,
    ),
    hasFinalCommand: decision.actionId.isNotEmpty && decision.binaryName.isNotEmpty,
  );
}
