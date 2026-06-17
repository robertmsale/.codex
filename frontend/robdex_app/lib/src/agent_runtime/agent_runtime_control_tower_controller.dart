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

class AgentRuntimeControlTowerController extends ChangeNotifier {
  StreamSubscription<RustSignalPack<bindings.AgentRuntimeOutputSignal>>? _subscription;
  final AgentRuntimeRemoteProfilePicker _remoteProfilePicker;
  final AgentRuntimeRequestSink _requestSink;
  final Set<String> _pendingRequestIds = <String>{};
  int _serial = 0;
  String _baseUrl = 'http://127.0.0.1:8765';
  AgentRuntimeControlTowerData? _viewModel;
  String? _bridgeErrorMessage;

  AgentRuntimeControlTowerController({
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

  AgentRuntimeControlTowerData get data {
    return (_viewModel ?? _disconnectedViewModel).copyWith(
      pendingRequestCount: _pendingRequestIds.length,
      errorMessage: _bridgeErrorMessage,
    );
  }

  void connect(String baseUrl) {
    _baseUrl = baseUrl.trim().isEmpty ? _baseUrl : baseUrl.trim();
    _send('connect', bindings.AgentRuntimeRequestConnect(baseUrl: _baseUrl, selectedSessionId: ''));
  }

  void refreshDiscovery() {
    _send('discover', const bindings.AgentRuntimeRequestRefreshDiscovery(discoveryPath: ''));
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

  void pollStreamOnce() {
    _send('poll', const bindings.AgentRuntimeRequestPollStreamOnce());
  }

  void disconnect() {
    _send('disconnect', const bindings.AgentRuntimeRequestDisconnect());
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
          source: 'gui.controlTower',
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
          source: 'gui.controlTower',
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
          source: 'gui.controlTower',
          reason: 'marked from Control Tower',
          variant: false,
          hasVariant: false,
        ),
      ),
    );
  }

  void _dispatchOperation(String prefix, bindings.AgentRuntimeGuiOperation operation) {
    _send(prefix, bindings.AgentRuntimeRequestDispatchOperation(operation: operation));
  }

  void _send(String prefix, bindings.AgentRuntimeRequest request) {
    _serial += 1;
    final requestId = 'agent-runtime-$prefix-$_serial';
    _pendingRequestIds.add(requestId);
    try {
      _requestSink(requestId, request);
    } catch (_) {
      _pendingRequestIds.remove(requestId);
      _bridgeErrorMessage = 'Agent Runtime bridge is not ready. Restart the app, then refresh discovery.';
    }
    notifyListeners();
  }

  static void _sendRequestSignalToRust(String requestId, bindings.AgentRuntimeRequest request) {
    bindings.AgentRuntimeRequestSignal(requestId: requestId, request: request).sendSignalToRust();
  }

  void _handleOutput(RustSignalPack<bindings.AgentRuntimeOutputSignal> pack) {
    final signal = pack.message;
    _pendingRequestIds.remove(signal.requestId);
    final output = signal.output;
    if (output is bindings.AgentRuntimeOutputControlTowerView) {
      _viewModel = AgentRuntimeControlTowerData.fromJson(_viewModelJson(output.viewModel));
      _bridgeErrorMessage = null;
    } else if (output is bindings.AgentRuntimeOutputError) {
      _bridgeErrorMessage = output.error.message.isNotEmpty ? output.error.message : output.error.code;
    }
    notifyListeners();
  }

  AgentRuntimeControlTowerData get _disconnectedViewModel => AgentRuntimeControlTowerData(
        connectionState: 'disconnected',
        discovery: const AgentRuntimeDiscoveryInfo(
          state: 'notLoaded',
          tone: 'muted',
          title: 'Discovery not loaded',
          message: 'Refresh discovery to inspect the local Agent Runtime service packet.',
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
          message: 'Import a remote profile JSON document; Rust stores an app-local copy and probes /health.',
          discoveryPath: '',
          connectable: false,
        ),
        connectionTone: 'muted',
        baseUrl: _baseUrl,
        statusLabel: 'No projection packet',
        watermarkLabel: '—',
        statusBadges: const [
          AgentRuntimeStatusBadge(label: 'Connection', value: 'disconnected', tone: 'muted'),
        ],
        selectedSessionLabel: 'none selected',
        sessionsTitle: 'Sessions',
        sessionsSubtitle: '',
        timelineTitle: 'Selected session stream',
        timelineSubtitle: '',
        actionsTitle: 'Action queue',
        actionsSubtitle: '',
        detailTitle: 'Controller detail',
        detailSubtitle: '',
        sessionsEmptyTitle: 'No sessions',
        sessionsEmptyText: 'Connect to hydrate runtime sessions.',
        timelineEmptyTitle: 'No timeline',
        timelineEmptyText: 'Select a session to inspect its event stream.',
        actionsEmptyTitle: 'No action queue',
        actionsEmptyText: 'No runtime action queue is loaded.',
        sessions: const [],
        timeline: const [],
        actions: const [],
        roleAdmin: mockAgentRuntimeRoleAdminEmpty,
        workflowMemory: mockAgentRuntimeWorkflowMemoryEmpty,
        controllerFacts: const [],
        outputLog: const [],
        pendingRequestCount: _pendingRequestIds.length,
      );

  @override
  void dispose() {
    _subscription?.cancel();
    super.dispose();
  }
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

Map<String, dynamic> _viewModelJson(bindings.AgentRuntimeControlTowerViewModel view) {
  return {
    'discovery': _discoveryJson(view.discovery),
    'remoteDiscovery': _discoveryJson(view.remoteDiscovery),
    'importedRemoteDiscovery': _discoveryJson(view.importedRemoteDiscovery),
    'connectionState': view.connectionState,
    'connectionTone': view.connectionTone,
    'baseUrl': view.baseUrl,
    'statusLabel': view.statusLabel,
    'watermarkLabel': view.watermarkLabel,
    'statusBadges': view.statusBadges.map(_badgeJson).toList(growable: false),
    'selectedSessionLabel': view.selectedSessionLabel,
    'sessionsTitle': view.sessionsTitle,
    'sessionsSubtitle': view.sessionsSubtitle,
    'timelineTitle': view.timelineTitle,
    'timelineSubtitle': view.timelineSubtitle,
    'actionsTitle': view.actionsTitle,
    'actionsSubtitle': view.actionsSubtitle,
    'detailTitle': view.detailTitle,
    'detailSubtitle': view.detailSubtitle,
    'sessionsEmptyTitle': view.sessionsEmptyTitle,
    'sessionsEmptyText': view.sessionsEmptyText,
    'timelineEmptyTitle': view.timelineEmptyTitle,
    'timelineEmptyText': view.timelineEmptyText,
    'actionsEmptyTitle': view.actionsEmptyTitle,
    'actionsEmptyText': view.actionsEmptyText,
    'sessions': view.sessions.map((row) => {
          'id': row.id,
          'title': row.title,
          'status': row.status,
          'subtitle': row.subtitle,
          'groupLabel': row.groupLabel,
          'tone': row.tone,
        }).toList(growable: false),
    'timeline': view.timeline.map((row) => {
          'id': row.id,
          'title': row.title,
          'subtitle': row.subtitle,
          'status': row.status,
          'tone': row.tone,
        }).toList(growable: false),
    'actions': view.actions.map(_actionJson).toList(growable: false),
    'roleAdmin': _roleAdminJson(view.roleAdmin),
    'workflowMemory': _workflowMemoryJson(view.workflowMemory),
    'controllerFacts': view.controllerFacts.map(_factJson).toList(growable: false),
    'outputLog': view.outputLog,
    'pendingRequestCount': view.pendingRequestCount,
    'errorMessage': view.hasErrorMessage ? view.errorMessage : null,
  };
}

Map<String, dynamic> _discoveryJson(bindings.AgentRuntimeDiscoveryView view) => {
      'sourceType': view.sourceType,
      'sourcePath': view.sourcePath,
      'state': view.state,
      'tone': view.tone,
      'title': view.title,
      'message': view.message,
      'baseUrl': view.hasBaseUrl ? view.baseUrl : null,
      'healthUrl': view.hasHealthUrl ? view.healthUrl : null,
      'webSocketUrl': view.hasWebSocketUrl ? view.webSocketUrl : null,
      'runtimeIdentity': view.hasRuntimeIdentity ? view.runtimeIdentity : null,
      'discoveryPath': view.discoveryPath,
      'lastImportedAt': view.hasLastImportedAt ? view.lastImportedAt : null,
      'serviceState': view.hasServiceState ? view.serviceState : null,
      'connectable': view.connectable,
      'diagnostics': view.diagnostics,
    };

Map<String, dynamic> _factJson(bindings.AgentRuntimeFact fact) => {'label': fact.label, 'value': fact.value};
Map<String, dynamic> _badgeJson(bindings.AgentRuntimeBadge badge) => {'label': badge.label, 'value': badge.value, 'tone': badge.tone};
Map<String, dynamic> _actionJson(bindings.AgentRuntimeActionRow row) => {
      'id': row.id,
      'title': row.title,
      'subtitle': row.subtitle,
      'kind': row.kind,
      'stateText': row.stateText,
      'tone': row.tone,
    };

Map<String, dynamic> _roleAdminJson(bindings.AgentRuntimeRoleAdminView view) => {
      'title': view.title,
      'subtitle': view.subtitle,
      'emptyTitle': view.emptyTitle,
      'emptyText': view.emptyText,
      'rows': view.rows.map((row) => {
            'id': row.id,
            'title': row.title,
            'subtitle': row.subtitle,
            'status': row.status,
            'tone': row.tone,
            'currentVersionId': row.currentVersion,
          }).toList(growable: false),
      'selectedDetail': view.hasSelectedDetail ? {
        'id': view.selectedDetail.id,
        'displayName': view.selectedDetail.displayName,
        'version': view.selectedDetail.version,
        'model': view.selectedDetail.modelLabel,
        'status': view.selectedDetail.status,
        'instructionText': view.selectedDetail.instructionsPreview,
        'capabilities': const <String>[],
        'policy': view.selectedDetail.policyRows.map((row) => {'action': row.label, 'decision': row.value}).toList(growable: false),
        'routing': <Map<String, String>>[{'label': 'Routing', 'value': view.selectedDetail.routingLabel}],
        'visibility': <Map<String, String>>[{'label': 'Visibility', 'value': view.selectedDetail.visibilityLabel}],
        'lifecycleAuthority': <Map<String, String>>[{'label': 'Lifecycle', 'value': view.selectedDetail.lifecycleLabel}],
      } : null,
      'versionRows': view.versionRows.map((row) => {
            'versionId': row.versionId,
            'version': row.version,
            'status': row.status,
            'createdAt': row.createdAt.isEmpty ? null : row.createdAt,
          }).toList(growable: false),
      'editorDraft': view.hasEditorDraft ? {
        'roleId': view.editorDraft.roleId,
        'version': view.editorDraft.version,
        'displayName': view.editorDraft.displayName,
        'model': view.editorDraft.model,
        'reasoningEffort': view.editorDraft.reasoningEffort,
        'instructionText': view.editorDraft.instructionText,
        'capabilities': view.editorDraft.capabilities,
        'policy': view.editorDraft.policyRows.map((row) => {'action': row.label, 'decision': row.value}).toList(growable: false),
        'routingMode': view.editorDraft.routingMode,
        'routingReservedActions': const <String>[],
        'defaultRecipient': view.editorDraft.defaultRecipient.isEmpty ? null : view.editorDraft.defaultRecipient,
        'allowedRecipients': view.editorDraft.allowedRecipients,
        'listed': view.editorDraft.listed,
        'ownerVisible': view.editorDraft.ownerVisible,
        'canSpawnAgents': view.editorDraft.canSpawnAgents,
        'canArchiveAgents': view.editorDraft.canArchiveAgents,
        'lifecycleReservedActions': const <String>[],
      } : null,
      'validationErrors': view.validationErrors,
      'actionStates': view.actionStates.map(_actionJson).toList(growable: false),
    };

Map<String, dynamic> _workflowMemoryJson(bindings.AgentRuntimeWorkflowMemoryView view) => {
      'title': view.title,
      'subtitle': view.subtitle,
      'emptyTitle': view.emptyTitle,
      'emptyText': view.emptyText,
      'selectedMemoryId': view.hasSelectedDetail ? view.selectedDetail.id : null,
      'rows': view.rows.map((row) => {
            'id': row.id,
            'title': row.title,
            'subtitle': row.reason,
            'scopeType': row.scopeLabel,
            'projectKey': row.hasProjectKey ? row.projectKey : null,
            'helpfulScore': double.tryParse(row.helpfulScore) ?? 0,
            'promotedAt': row.hasPromotedAt ? row.promotedAt : null,
            'sourceSessionId': row.sourceSessionId,
            'tone': row.tone,
            'selected': row.isSelected,
          }).toList(growable: false),
      'selectedDetail': view.hasSelectedDetail ? {
        'id': view.selectedDetail.id,
        'title': view.selectedDetail.title,
        'reason': view.selectedDetail.reason,
        'summary': view.selectedDetail.summary,
        'sourceSessionId': view.selectedDetail.sourceSessionId,
        'sourceScriptRunId': view.selectedDetail.hasSourceScriptRunId ? view.selectedDetail.sourceScriptRunId : null,
        'sourceStarlark': view.selectedDetail.sourcePreview,
        'sourcePreview': view.selectedDetail.sourcePreview,
        'provider': view.selectedDetail.provider.isEmpty ? null : view.selectedDetail.provider,
        'model': view.selectedDetail.model.isEmpty ? null : view.selectedDetail.model,
        'dimensions': int.tryParse(view.selectedDetail.dimensions),
        'storageType': view.selectedDetail.storageLabel.isEmpty ? null : view.selectedDetail.storageLabel,
        'sourceHash': view.selectedDetail.sourceHash.isEmpty ? null : view.selectedDetail.sourceHash,
        'commandFingerprint': view.selectedDetail.commandFingerprint.isEmpty ? null : view.selectedDetail.commandFingerprint,
        'helpfulScore': double.tryParse(view.selectedDetail.score) ?? 0,
        'scopeLabel': view.selectedDetail.scopeLabel,
        'feedbackSessionId': view.selectedDetail.hasFeedbackSessionId ? view.selectedDetail.feedbackSessionId : null,
        'feedbackEnabled': view.selectedDetail.feedbackEnabled,
      } : null,
      'recentEvents': view.hasSelectedDetail ? view.selectedDetail.events.map((event) => {
        'id': event.id,
        'title': event.title,
        'subtitle': event.subtitle,
        'createdAt': event.createdAt.isEmpty ? null : event.createdAt,
        'tone': event.tone,
      }).toList(growable: false) : const <Map<String, Object?>>[],
      'feedbackActions': view.actionStates.map(_actionJson).toList(growable: false),
    };
