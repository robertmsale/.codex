import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:robdex_app/src/agent_runtime/agent_runtime_control_tower_controller.dart';

void main() {
  test('role activate operation maps role and version ids for Rust transport', () {
    final operation = agentRuntimeRoleActivateOperationForTest('runtime-allow', 'role-version-0');

    expect(operation['operation'], 'activateRoleVersion');
    expect(operation['request'], {
      'roleId': 'runtime-allow',
      'versionId': 'role-version-0',
    });
  });

  test('workflow memory feedback operations map to typed Rust envelopes', () {
    final attempted = agentRuntimeWorkflowMemoryFeedbackOperationForTest(
      memoryId: 'memory-1',
      sessionId: 'session-1',
      feedback: 'attempted',
      payload: const {'source': 'gui.controlTower', 'variant': true},
    );
    final helpful = agentRuntimeWorkflowMemoryFeedbackOperationForTest(
      memoryId: 'memory-1',
      sessionId: 'session-1',
      feedback: 'helpful',
      payload: const {'source': 'gui.controlTower'},
    );
    final notHelpful = agentRuntimeWorkflowMemoryFeedbackOperationForTest(
      memoryId: 'memory-1',
      sessionId: 'session-1',
      feedback: 'notHelpful',
      payload: const {'source': 'gui.controlTower', 'reason': 'marked from Control Tower'},
    );

    for (final operation in [attempted, helpful, notHelpful]) {
      expect(operation['operation'], 'workflowMemoryFeedback');
      expect((operation['request'] as Map<String, dynamic>)['memoryId'], 'memory-1');
      expect((operation['request'] as Map<String, dynamic>)['sessionId'], 'session-1');
    }
    expect((attempted['request'] as Map<String, dynamic>)['payload'], {'source': 'gui.controlTower', 'variant': true});
    expect((helpful['request'] as Map<String, dynamic>)['feedback'], 'helpful');
    expect((notHelpful['request'] as Map<String, dynamic>)['feedback'], 'notHelpful');
  });

  test('workflow memory selection operation maps memory id for Rust transport', () {
    final operation = agentRuntimeWorkflowMemorySelectOperationForTest('memory-2');

    expect(operation['operation'], 'selectWorkflowMemory');
    expect(operation['request'], {'memoryId': 'memory-2'});
  });

  test('iCloud remote discovery transport packet shapes are stable JSON intents', () {
    expect(agentRuntimeIcloudRefreshIntentForTest(), {'type': 'refreshIcloudRemoteDiscovery'});
    expect(agentRuntimeIcloudConnectIntentForTest(), {
      'type': 'connectIcloudRemoteRuntime',
      'payload': {'selectedSessionId': null},
    });
  });

  test('imported remote profile transport packet shapes are stable JSON intents', () {
    expect(agentRuntimeImportProfileIntentForTest(profilePath: '/tmp/profile.json'), {
      'type': 'importRemoteProfileDocument',
      'payload': {'profilePath': '/tmp/profile.json'},
    });
    expect(agentRuntimeImportProfileIntentForTest(), {
      'type': 'importRemoteProfileDocument',
      'payload': {'profilePath': null},
    });
    expect(agentRuntimeRefreshImportedProfileIntentForTest(), {'type': 'refreshImportedRemoteProfile'});
    expect(agentRuntimeConnectImportedProfileIntentForTest(), {
      'type': 'connectImportedRemoteRuntime',
      'payload': {'selectedSessionId': null},
    });
  });

  test('import profile action passes selected document path to Rust without parsing JSON', () async {
    final sentPackets = <Map<String, dynamic>>[];
    final controller = AgentRuntimeControlTowerController(
      remoteProfilePicker: () async => '/tmp/imported-agent-runtime-profile.json',
      requestSink: (requestId, packetJson) {
        sentPackets.add(jsonDecode(packetJson) as Map<String, dynamic>);
      },
    );
    addTearDown(controller.dispose);

    controller.importRemoteProfileDocument();
    await pumpEventQueue();

    expect(sentPackets, hasLength(1));
    expect(sentPackets.single['intent'], {
      'type': 'importRemoteProfileDocument',
      'payload': {'profilePath': '/tmp/imported-agent-runtime-profile.json'},
    });
  });

  test('import profile picker failures stay on typed unsupported Rust error path', () async {
    final sentPackets = <Map<String, dynamic>>[];
    final controller = AgentRuntimeControlTowerController(
      remoteProfilePicker: () async => throw UnsupportedError('picker unavailable'),
      requestSink: (requestId, packetJson) {
        sentPackets.add(jsonDecode(packetJson) as Map<String, dynamic>);
      },
    );
    addTearDown(controller.dispose);

    controller.importRemoteProfileDocument();
    await pumpEventQueue();

    expect(sentPackets, hasLength(1));
    expect(sentPackets.single['intent'], {
      'type': 'importRemoteProfileDocument',
      'payload': {'profilePath': null},
    });
  });
}
