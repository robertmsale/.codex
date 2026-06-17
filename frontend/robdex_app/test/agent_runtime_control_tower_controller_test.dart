import 'package:flutter_test/flutter_test.dart';
import 'package:robdex_app/src/agent_runtime/agent_runtime_control_tower_controller.dart';
import 'package:robdex_app/src/bindings/bindings.dart' as bindings;

void main() {
  test('role activate operation maps role and version ids for Rust transport', () {
    final operation = agentRuntimeRoleActivateOperationForTest('runtime-allow', 'role-version-0');

    expect(operation, isA<bindings.AgentRuntimeGuiOperationActivateRoleVersion>());
    final activate = operation as bindings.AgentRuntimeGuiOperationActivateRoleVersion;
    expect(activate.roleId, 'runtime-allow');
    expect(activate.versionId, 'role-version-0');
  });

  test('workflow memory feedback operations map to typed Rust envelopes', () {
    final attempted = agentRuntimeWorkflowMemoryFeedbackOperationForTest(
      memoryId: 'memory-1',
      sessionId: 'session-1',
      feedback: 'attempted',
      payload: const bindings.AgentRuntimeWorkflowMemoryFeedbackPayload(
        source: 'gui.controlTower',
        reason: '',
        variant: true,
        hasVariant: true,
      ),
    );
    final helpful = agentRuntimeWorkflowMemoryFeedbackOperationForTest(
      memoryId: 'memory-1',
      sessionId: 'session-1',
      feedback: 'helpful',
      payload: const bindings.AgentRuntimeWorkflowMemoryFeedbackPayload(
        source: 'gui.controlTower',
        reason: '',
        variant: false,
        hasVariant: false,
      ),
    );
    final notHelpful = agentRuntimeWorkflowMemoryFeedbackOperationForTest(
      memoryId: 'memory-1',
      sessionId: 'session-1',
      feedback: 'notHelpful',
      payload: const bindings.AgentRuntimeWorkflowMemoryFeedbackPayload(
        source: 'gui.controlTower',
        reason: 'marked from Control Tower',
        variant: false,
        hasVariant: false,
      ),
    );

    for (final operation in [attempted, helpful, notHelpful]) {
      expect(operation, isA<bindings.AgentRuntimeGuiOperationWorkflowMemoryFeedback>());
      final feedback = operation as bindings.AgentRuntimeGuiOperationWorkflowMemoryFeedback;
      expect(feedback.memoryId, 'memory-1');
      expect(feedback.sessionId, 'session-1');
    }
    expect((attempted as bindings.AgentRuntimeGuiOperationWorkflowMemoryFeedback).payload.hasVariant, true);
    expect((helpful as bindings.AgentRuntimeGuiOperationWorkflowMemoryFeedback).feedback, 'helpful');
    expect((notHelpful as bindings.AgentRuntimeGuiOperationWorkflowMemoryFeedback).payload.reason, 'marked from Control Tower');
  });

  test('workflow memory selection operation maps memory id for Rust transport', () {
    final operation = agentRuntimeWorkflowMemorySelectOperationForTest('memory-2');

    expect(operation, isA<bindings.AgentRuntimeGuiOperationSelectWorkflowMemory>());
    expect((operation as bindings.AgentRuntimeGuiOperationSelectWorkflowMemory).memoryId, 'memory-2');
  });

  test('iCloud remote discovery typed signals are stable generated intents', () {
    expect(agentRuntimeIcloudRefreshIntentForTest(), isA<bindings.AgentRuntimeRequestRefreshIcloudRemoteDiscovery>());
    expect(agentRuntimeIcloudConnectIntentForTest(), isA<bindings.AgentRuntimeRequestConnectIcloudRemoteRuntime>());
  });

  test('local discovery refresh sends generated typed signal intent', () {
    final sentRequests = <bindings.AgentRuntimeRequest>[];
    final controller = AgentRuntimeControlTowerController(
      requestSink: (requestId, request) {
        sentRequests.add(request);
      },
    );
    addTearDown(controller.dispose);

    controller.refreshDiscovery();

    expect(sentRequests, hasLength(1));
    expect(sentRequests.single, isA<bindings.AgentRuntimeRequestRefreshDiscovery>());
    expect((sentRequests.single as bindings.AgentRuntimeRequestRefreshDiscovery).discoveryPath, '');
  });

  test('manual connect sends generated typed signal intent', () {
    final sentRequests = <bindings.AgentRuntimeRequest>[];
    final controller = AgentRuntimeControlTowerController(
      requestSink: (requestId, request) {
        sentRequests.add(request);
      },
    );
    addTearDown(controller.dispose);

    controller.connect('http://127.0.0.1:8765');

    expect(sentRequests, hasLength(1));
    expect(sentRequests.single, isA<bindings.AgentRuntimeRequestConnect>());
    final connect = sentRequests.single as bindings.AgentRuntimeRequestConnect;
    expect(connect.baseUrl, 'http://127.0.0.1:8765');
    expect(connect.selectedSessionId, '');
  });

  test('imported remote profile typed signals are stable generated intents', () {
    final import = agentRuntimeImportProfileIntentForTest(profilePath: '/tmp/profile.json');
    expect(import, isA<bindings.AgentRuntimeRequestImportRemoteProfileDocument>());
    expect((import as bindings.AgentRuntimeRequestImportRemoteProfileDocument).profilePath, '/tmp/profile.json');
    expect(agentRuntimeRefreshImportedProfileIntentForTest(), isA<bindings.AgentRuntimeRequestRefreshImportedRemoteProfile>());
    expect(agentRuntimeConnectImportedProfileIntentForTest(), isA<bindings.AgentRuntimeRequestConnectImportedRemoteRuntime>());
  });

  test('import profile action passes selected document path to Rust without parsing JSON', () async {
    final sentRequests = <bindings.AgentRuntimeRequest>[];
    final controller = AgentRuntimeControlTowerController(
      remoteProfilePicker: () async => '/tmp/imported-agent-runtime-profile.json',
      requestSink: (requestId, request) {
        sentRequests.add(request);
      },
    );
    addTearDown(controller.dispose);

    controller.importRemoteProfileDocument();
    await pumpEventQueue();

    expect(sentRequests, hasLength(1));
    final request = sentRequests.single as bindings.AgentRuntimeRequestImportRemoteProfileDocument;
    expect(request.profilePath, '/tmp/imported-agent-runtime-profile.json');
  });

  test('import profile picker failures stay on typed unsupported Rust error path', () async {
    final sentRequests = <bindings.AgentRuntimeRequest>[];
    final controller = AgentRuntimeControlTowerController(
      remoteProfilePicker: () async => throw UnsupportedError('picker unavailable'),
      requestSink: (requestId, request) {
        sentRequests.add(request);
      },
    );
    addTearDown(controller.dispose);

    controller.importRemoteProfileDocument();
    await pumpEventQueue();

    expect(sentRequests, hasLength(1));
    final request = sentRequests.single as bindings.AgentRuntimeRequestImportRemoteProfileDocument;
    expect(request.profilePath, '');
  });
}
