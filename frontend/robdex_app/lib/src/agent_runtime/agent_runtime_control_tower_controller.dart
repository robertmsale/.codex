import 'dart:async';
import 'dart:convert';

import 'package:file_selector/file_selector.dart';
import 'package:flutter/foundation.dart';
import 'package:rinf/rinf.dart';
import 'package:robdex_design_system/robdex_design_system.dart';

import '../bindings/bindings.dart';

typedef AgentRuntimeRemoteProfilePicker = Future<String?> Function();
typedef AgentRuntimeRequestSink = void Function(String requestId, String packetJson);

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
Map<String, dynamic> agentRuntimeRoleActivateOperationForTest(String roleId, String versionId) {
  return {
    'operation': 'activateRoleVersion',
    'request': {'roleId': roleId, 'versionId': versionId},
  };
}

@visibleForTesting
Map<String, dynamic> agentRuntimeWorkflowMemoryFeedbackOperationForTest({
  required String memoryId,
  required String sessionId,
  required String feedback,
  Map<String, dynamic> payload = const {},
}) {
  return {
    'operation': 'workflowMemoryFeedback',
    'request': {
      'memoryId': memoryId,
      'sessionId': sessionId,
      'feedback': feedback,
      'payload': payload,
    },
  };
}

@visibleForTesting
Map<String, dynamic> agentRuntimeWorkflowMemorySelectOperationForTest(String memoryId) {
  return {
    'operation': 'selectWorkflowMemory',
    'request': {'memoryId': memoryId},
  };
}

@visibleForTesting
Map<String, dynamic> agentRuntimeIcloudRefreshIntentForTest() {
  return {'type': 'refreshIcloudRemoteDiscovery'};
}

@visibleForTesting
Map<String, dynamic> agentRuntimeIcloudConnectIntentForTest() {
  return {
    'type': 'connectIcloudRemoteRuntime',
    'payload': {'selectedSessionId': null},
  };
}

@visibleForTesting
Map<String, dynamic> agentRuntimeImportProfileIntentForTest({String? profilePath}) {
  return {
    'type': 'importRemoteProfileDocument',
    'payload': {'profilePath': profilePath},
  };
}

@visibleForTesting
Map<String, dynamic> agentRuntimeRefreshImportedProfileIntentForTest() {
  return {'type': 'refreshImportedRemoteProfile'};
}

@visibleForTesting
Map<String, dynamic> agentRuntimeConnectImportedProfileIntentForTest() {
  return {
    'type': 'connectImportedRemoteRuntime',
    'payload': {'selectedSessionId': null},
  };
}

class AgentRuntimeControlTowerController extends ChangeNotifier {
  StreamSubscription<RustSignalPack<AgentRuntimeOutputSignal>>? _subscription;
  final AgentRuntimeRemoteProfilePicker _remoteProfilePicker;
  final AgentRuntimeRequestSink _requestSink;
  final Set<String> _pendingRequestIds = <String>{};
  int _serial = 0;
  String _baseUrl = 'http://127.0.0.1:8765';
  AgentRuntimeControlTowerData? _viewModel;

  AgentRuntimeControlTowerController({
    AgentRuntimeRemoteProfilePicker remoteProfilePicker = pickAgentRuntimeRemoteProfileDocumentPath,
    AgentRuntimeRequestSink? requestSink,
  })  : _remoteProfilePicker = remoteProfilePicker,
        _requestSink = requestSink ?? _sendRequestSignalToRust {
    _subscription = AgentRuntimeOutputSignal.rustSignalStream.listen(
      _handleOutput,
      onError: (Object _) {
        notifyListeners();
      },
    );
  }

  AgentRuntimeControlTowerData get data {
    return (_viewModel ?? _disconnectedViewModel).copyWith(
      pendingRequestCount: _pendingRequestIds.length,
    );
  }

  void connect(String baseUrl) {
    _baseUrl = baseUrl.trim().isEmpty ? _baseUrl : baseUrl.trim();
    _send('connect', {
      'type': 'connect',
      'payload': {
        'baseUrl': _baseUrl,
        'selectedSessionId': null,
      },
    });
  }

  void refreshDiscovery() {
    _send('discover', {'type': 'refreshDiscovery'});
  }

  void connectDiscoveredRuntime() {
    _send('connect-discovered', {
      'type': 'connectDiscoveredRuntime',
      'payload': {
        'selectedSessionId': null,
      },
    });
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
    _send('poll', {'type': 'pollStreamOnce'});
  }

  void disconnect() {
    _send('disconnect', {'type': 'disconnect'});
  }

  void validateRoleDraft(AgentRuntimeRoleEditorDraft draft) {
    _dispatchOperation('role-validate', {
      'operation': 'validateRoleDraft',
      'request': {'draft': draft.toDraftJson()},
    });
  }

  void createRoleFromDraft(AgentRuntimeRoleEditorDraft draft) {
    _dispatchOperation('role-create', {
      'operation': 'createRoleFromDraft',
      'request': {'draft': draft.toDraftJson()},
    });
  }

  void updateRoleFromDraft(AgentRuntimeRoleEditorDraft draft) {
    _dispatchOperation('role-update', {
      'operation': 'updateRoleFromDraft',
      'request': {'roleId': draft.roleId, 'draft': draft.toDraftJson()},
    });
  }

  void exportRole(String roleId) {
    _dispatchOperation('role-export', {
      'operation': 'exportRole',
      'request': {'roleId': roleId},
    });
  }

  void archiveRole(String roleId) {
    _dispatchOperation('role-archive', {
      'operation': 'archiveRole',
      'request': {'roleId': roleId},
    });
  }

  void unarchiveRole(String roleId) {
    _dispatchOperation('role-unarchive', {
      'operation': 'unarchiveRole',
      'request': {'roleId': roleId},
    });
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
        payload: const {'source': 'gui.controlTower', 'variant': true},
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
        payload: const {'source': 'gui.controlTower'},
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
        payload: const {'source': 'gui.controlTower', 'reason': 'marked from Control Tower'},
      ),
    );
  }

  void _dispatchOperation(String prefix, Map<String, dynamic> operation) {
    _send(prefix, {
      'type': 'dispatchOperation',
      'payload': {'operation': operation},
    });
  }

  void _send(String prefix, Map<String, dynamic> intent) {
    _serial += 1;
    final requestId = 'agent-runtime-$prefix-$_serial';
    _pendingRequestIds.add(requestId);
    _requestSink(
      requestId,
      jsonEncode({
        'packetId': requestId,
        'intent': intent,
      }),
    );
    notifyListeners();
  }

  static void _sendRequestSignalToRust(String requestId, String packetJson) {
    AgentRuntimeRequestSignal(requestId: requestId, packetJson: packetJson).sendSignalToRust();
  }

  void _handleOutput(RustSignalPack<AgentRuntimeOutputSignal> pack) {
    final signal = pack.message;
    _pendingRequestIds.remove(signal.requestId);
    final decoded = jsonDecode(signal.outputJson) as Map<String, dynamic>;
    final output = decoded['output'] as Map<String, dynamic>? ?? const {};
    final type = output['type'] as String? ?? 'unknown';
    final payload = output['payload'];
    if (type == 'controlTowerView' && payload is Map<String, dynamic>) {
      final viewModel = payload['viewModel'];
      if (viewModel is Map) {
        _viewModel = AgentRuntimeControlTowerData.fromJson(
          Map<String, dynamic>.from(viewModel),
        );
      }
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
