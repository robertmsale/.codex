import 'package:flutter_test/flutter_test.dart';
import 'package:robdex_app/src/agent_runtime/agent_runtime_control_tower_controller.dart';
import 'package:robdex_app/src/bindings/bindings.dart' as bindings;
import 'package:robdex_design_system/robdex_design_system.dart';

void main() {
  test('role activate operation maps role and version ids for typed transport', () {
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
        reason: 'marked from Agent Runtime',
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
    expect((notHelpful as bindings.AgentRuntimeGuiOperationWorkflowMemoryFeedback).payload.reason, 'marked from Agent Runtime');
  });

  test('workflow memory selection operation maps memory id for typed transport', () {
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

  test('disconnect sends generated typed signal intent', () {
    final sentRequests = <bindings.AgentRuntimeRequest>[];
    final controller = AgentRuntimeControlTowerController(
      requestSink: (requestId, request) {
        sentRequests.add(request);
      },
    );
    addTearDown(controller.dispose);

    controller.disconnect();

    expect(sentRequests.single, isA<bindings.AgentRuntimeRequestDisconnect>());
  });

  test('conversation shell session actions send generated typed operation intents', () {
    final sentRequests = <bindings.AgentRuntimeRequest>[];
    final controller = AgentRuntimeControlTowerController(
      requestSink: (requestId, request) {
        sentRequests.add(request);
      },
    );
    addTearDown(controller.dispose);

    controller.createSession();
    controller.selectSession('session-2');
    controller.sendMessage('session-2', '  hello runtime  ');

    expect(sentRequests, hasLength(3));
    final create = sentRequests[0] as bindings.AgentRuntimeRequestDispatchOperation;
    expect(create.operation, isA<bindings.AgentRuntimeGuiOperationCreateSession>());
    final select = sentRequests[1] as bindings.AgentRuntimeRequestDispatchOperation;
    expect(select.operation, isA<bindings.AgentRuntimeGuiOperationSelectSession>());
    expect((select.operation as bindings.AgentRuntimeGuiOperationSelectSession).sessionId, 'session-2');
    final send = sentRequests[2] as bindings.AgentRuntimeRequestDispatchOperation;
    expect(send.operation, isA<bindings.AgentRuntimeGuiOperationSendMessage>());
    final sendMessage = send.operation as bindings.AgentRuntimeGuiOperationSendMessage;
    expect(sendMessage.sessionId, 'session-2');
    expect(sendMessage.message, 'hello runtime');
  });

  test('session close archive and fork use generated typed operation intents', () {
    final sentRequests = <bindings.AgentRuntimeRequest>[];
    final controller = AgentRuntimeControlTowerController(
      requestSink: (requestId, request) {
        sentRequests.add(request);
      },
    );
    addTearDown(controller.dispose);

    controller.closeSession('session-2');
    controller.archiveSession('session-2');
    controller.forkSession('session-2');

    expect(sentRequests, hasLength(3));
    expect((sentRequests[0] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationCloseSession>());
    expect((sentRequests[1] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationArchiveSession>());
    expect((sentRequests[2] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationForkSession>());
  });

  test('managed process controls use generated typed operation intents', () {
    final terminate = agentRuntimeTerminateProcessOperationForTest('session-2', 'proc_1');
    final input = agentRuntimeInputProcessOperationForTest('session-2', 'proc_1', 'hello');
    final flush = agentRuntimeFlushProcessOperationForTest('session-2', 'proc_1');

    expect(terminate, isA<bindings.AgentRuntimeGuiOperationTerminateProcess>());
    expect((terminate as bindings.AgentRuntimeGuiOperationTerminateProcess).sessionId, 'session-2');
    expect(terminate.handle, 'proc_1');
    expect(input, isA<bindings.AgentRuntimeGuiOperationInputProcess>());
    expect((input as bindings.AgentRuntimeGuiOperationInputProcess).text, 'hello');
    expect(flush, isA<bindings.AgentRuntimeGuiOperationFlushProcess>());
    expect((flush as bindings.AgentRuntimeGuiOperationFlushProcess).handle, 'proc_1');
  });

  test('project and settings shell entry points rehydrate through typed runtime state requests', () {
    final sentRequests = <bindings.AgentRuntimeRequest>[];
    final controller = AgentRuntimeControlTowerController(
      requestSink: (requestId, request) {
        sentRequests.add(request);
      },
    );
    addTearDown(controller.dispose);

    controller.selectProject('runtime');
    controller.openSettings();

    expect(sentRequests, hasLength(2));
    expect(sentRequests[0], isA<bindings.AgentRuntimeRequestSelectProject>());
    expect((sentRequests[0] as bindings.AgentRuntimeRequestSelectProject).projectId, 'runtime');
    expect(sentRequests[1], isA<bindings.AgentRuntimeRequestHydrate>());
  });

  test('role admin shell actions send generated typed operations with payloads', () {
    final sentRequests = <bindings.AgentRuntimeRequest>[];
    final controller = AgentRuntimeControlTowerController(
      requestSink: (requestId, request) {
        sentRequests.add(request);
      },
    );
    addTearDown(controller.dispose);
    final draft = mockAgentRuntimeRoleAdminSelected.editorDraft!;

    controller.validateRoleDraft(draft);
    controller.createRoleFromDraft(draft);
    controller.updateRoleFromDraft(draft);
    controller.exportRole('runtime-allow');
    controller.archiveRole('runtime-allow');
    controller.unarchiveRole('runtime-allow');
    controller.activateRoleVersion('runtime-allow', 'role-version-1');

    expect(sentRequests, hasLength(7));
    expect((sentRequests[0] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationValidateRoleDraft>());
    expect((sentRequests[1] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationCreateRoleFromDraft>());
    final update = (sentRequests[2] as bindings.AgentRuntimeRequestDispatchOperation).operation as bindings.AgentRuntimeGuiOperationUpdateRoleFromDraft;
    expect(update.roleId, 'runtime-allow');
    expect((sentRequests[3] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationExportRole>());
    expect((sentRequests[4] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationArchiveRole>());
    expect((sentRequests[5] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationUnarchiveRole>());
    final activate = (sentRequests[6] as bindings.AgentRuntimeRequestDispatchOperation).operation as bindings.AgentRuntimeGuiOperationActivateRoleVersion;
    expect(activate.roleId, 'runtime-allow');
    expect(activate.versionId, 'role-version-1');
  });

  test('approval and command-registry request actions use generated typed operation intents', () {
    final sentRequests = <bindings.AgentRuntimeRequest>[];
    final controller = AgentRuntimeControlTowerController(
      requestSink: (requestId, request) {
        sentRequests.add(request);
      },
    );
    addTearDown(controller.dispose);
    const approval = AgentRuntimeActionItem(
      id: 'approval-1',
      title: 'Approval requested',
      subtitle: '',
      kind: 'approval',
      stateText: 'Needs decision',
      tone: 'warning',
    );
    const request = AgentRuntimeActionItem(
      id: 'registry-request-1',
      title: 'Registry request',
      subtitle: '',
      kind: 'commandRegistryRequest',
      stateText: 'Needs decision',
      tone: 'warning',
    );

    controller.approveAction(approval);
    controller.resumeApproval(approval);
    controller.approveCommandRegistryRequest(request, 'session-2');
    controller.applyCommandRegistryRequest(request, 'session-2');

    expect(sentRequests, hasLength(4));
    expect((sentRequests[0] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationDecideApproval>());
    expect((sentRequests[1] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationResumeApproval>());
    expect((sentRequests[2] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationDecideCommandRegistryRequest>());
    expect((sentRequests[3] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationApplyCommandRegistryRequest>());
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
