import 'package:flutter_test/flutter_test.dart';
import 'package:flutter/material.dart';
import 'package:robdex_app/src/agent_runtime/agent_runtime_workbench_controller.dart';
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
        source: 'gui.workbench',
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
        source: 'gui.workbench',
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
        source: 'gui.workbench',
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
    final controller = AgentRuntimeWorkbenchController(
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
    final controller = AgentRuntimeWorkbenchController(
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
    final controller = AgentRuntimeWorkbenchController(
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
    final controller = AgentRuntimeWorkbenchController(
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

  test('composer send blocks empty selected session id and empty message text', () {
    final sentRequests = <bindings.AgentRuntimeRequest>[];
    final controller = AgentRuntimeWorkbenchController(
      requestSink: (requestId, request) {
        sentRequests.add(request);
      },
    );
    addTearDown(controller.dispose);

    controller.sendMessage('', 'hello');
    controller.sendMessage('session-2', '   ');

    expect(sentRequests, isEmpty);
  });

  test('composer send dispatch failure surfaces typed visible GUI error', () {
    final controller = AgentRuntimeWorkbenchController(
      requestSink: (requestId, request) {
        throw StateError('bridge down');
      },
    );
    addTearDown(controller.dispose);

    controller.sendMessage('session-2', 'hello runtime');

    expect(controller.data.errorMessage, contains('Agent Runtime bridge is not ready'));
  });

  test('typed selected conversation maps to canonical ChatEntry authors without lowercase runtime labels', () {
    final user = agentRuntimeChatEntryToChatEntryForTest(const bindings.AgentRuntimeChatEntry(
      id: 'turn:1:user',
      author: 'User',
      displayLabel: 'User',
      timestamp: '',
      hasTimestamp: false,
      body: 'exact submitted text',
      subtitle: 'completed',
      kind: 'message',
      status: 'completed',
      processId: '',
      hasProcessId: false,
      command: '',
      output: '',
      deliveryState: 'delivered',
      isStreaming: false,
      isTool: false,
    ));
    final assistant = agentRuntimeChatEntryToChatEntryForTest(const bindings.AgentRuntimeChatEntry(
      id: 'model:1:assistant',
      author: 'Assistant',
      displayLabel: 'Assistant',
      timestamp: '',
      hasTimestamp: false,
      body: '**actual final response**',
      subtitle: 'completed',
      kind: 'message',
      status: 'completed',
      processId: '',
      hasProcessId: false,
      command: '',
      output: '',
      deliveryState: 'delivered',
      isStreaming: false,
      isTool: false,
    ));

    expect(user.author, 'User');
    expect(assistant.author, 'Assistant');
    expect([user.author, assistant.author], isNot(contains('owner')));
    expect([user.author, assistant.author], isNot(contains('assistant')));
    expect(user.body, 'exact submitted text');
    expect(assistant.body, '**actual final response**');
  });

  testWidgets('connected Agent Runtime shell renders left rail plus center chat with modal operation toolbar', (tester) async {
    await tester.pumpWidget(
      MaterialApp(
        home: ConversationShellScreen(
          data: agentRuntimeConversationShellData(mockAgentRuntimeConnected),
          onSessionSelected: (_) {},
          onCreateSession: () {},
          onSendMessage: (_) {},
          onInterrupt: () {},
          showPermanentDetail: false,
          headerControls: TextButton(
            key: const ValueKey('agentRuntime.toolbar.history'),
            onPressed: () {
              showModalBottomSheet<void>(
                context: tester.element(find.byKey(const ValueKey('agentRuntime.toolbar.history'))),
                builder: (_) => const AgentRuntimeOperationsDetail(data: mockAgentRuntimeConnected, focusSurfaceId: 'history'),
              );
            },
            child: const Text('History'),
          ),
        ),
      ),
    );

    expect(find.byKey(const ValueKey('conversationShell.center')), findsOneWidget);
    expect(find.text('History'), findsOneWidget);
    expect(find.text('Details'), findsNothing);
    expect(find.text('Role imported'), findsNothing);

    await tester.tap(find.byKey(const ValueKey('agentRuntime.toolbar.history')));
    await tester.pumpAndSettle();
    expect(find.text('History'), findsWidgets);
    expect(find.text('Role imported'), findsWidgets);
  });

  testWidgets('typed selected conversation renders chat bubbles and excludes raw runtime event names', (tester) async {
    final entries = [
      agentRuntimeChatEntryToChatEntryForTest(const bindings.AgentRuntimeChatEntry(
        id: 'turn:1:user',
        author: 'User',
        displayLabel: 'User',
        timestamp: '',
        hasTimestamp: false,
        body: 'render this user prompt',
        subtitle: 'completed',
        kind: 'message',
        status: 'completed',
        processId: '',
        hasProcessId: false,
        command: '',
        output: '',
        deliveryState: 'delivered',
        isStreaming: false,
        isTool: false,
      )),
      agentRuntimeChatEntryToChatEntryForTest(const bindings.AgentRuntimeChatEntry(
        id: 'tool:1',
        author: 'Tool',
        displayLabel: 'Tool',
        timestamp: '',
        hasTimestamp: false,
        body: '',
        subtitle: 'execute_code',
        kind: 'execute_code',
        status: 'completed',
        processId: 'process-1',
        hasProcessId: true,
        command: 'output("ok")',
        output: 'ok',
        deliveryState: 'delivered',
        isStreaming: false,
        isTool: true,
      )),
      agentRuntimeChatEntryToChatEntryForTest(const bindings.AgentRuntimeChatEntry(
        id: 'model:1:assistant',
        author: 'Assistant',
        displayLabel: 'Assistant',
        timestamp: '',
        hasTimestamp: false,
        body: 'render this assistant response',
        subtitle: 'completed',
        kind: 'message',
        status: 'completed',
        processId: '',
        hasProcessId: false,
        command: '',
        output: '',
        deliveryState: 'delivered',
        isStreaming: false,
        isTool: false,
      )),
    ];

    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: ChatTimeline(
          threadId: 'session-1',
          entries: entries,
          title: 'Selected session',
          contextWindowRemainingPercent: null,
          onSend: (_) {},
          onInterrupt: () {},
          composerEnabled: true,
          isRunning: false,
        ),
      ),
    ));

    expect(find.text('render this user prompt'), findsOneWidget);
    expect(find.text('render this assistant response'), findsOneWidget);
    for (final raw in ['role.imported', 'turn.started', 'model.final_response', 'tool.completed', 'read History', 'Output details are available in History.']) {
      expect(find.textContaining(raw), findsNothing);
    }
  });

  testWidgets('Agent Runtime selected ChatTimeline excludes raw event names even when History contains them', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1440, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    const historyRows = [
      ConversationDetailRow(label: 'Role imported', value: '#1 · completed · role'),
      ConversationDetailRow(label: 'Tool completed', value: '#2 · completed · tool'),
    ];
    final data = ConversationShellData(
      appTitle: 'Agent Runtime',
      connectionLabel: 'Runtime connected',
      projects: const [ConversationProject(id: 'runtime', title: 'Runtime')],
      sessions: const [
        ConversationSession(
          id: 'session-1',
          title: 'Runtime validation',
          subtitle: 'Selected',
          role: 'Runtime',
          selected: true,
          rolePresentation: ConversationRolePresentation(
            roleId: 'runtime',
            displayLabel: 'Runtime',
            shortLabel: 'RT',
            iconKey: 'runtime',
            tone: 'success',
            statusLabel: 'open',
            description: 'Selected',
          ),
        ),
      ],
      selectedSessionId: 'session-1',
      timelineTitle: 'Selected session',
      entries: [
        agentRuntimeChatEntryToChatEntryForTest(const bindings.AgentRuntimeChatEntry(
          id: 'turn:1:user',
          author: 'User',
          displayLabel: 'User',
          timestamp: '',
          hasTimestamp: false,
          body: 'selected user text',
          subtitle: 'completed',
          kind: 'message',
          status: 'completed',
          processId: '',
          hasProcessId: false,
          command: '',
          output: '',
          deliveryState: 'delivered',
          isStreaming: false,
          isTool: false,
        )),
        agentRuntimeChatEntryToChatEntryForTest(const bindings.AgentRuntimeChatEntry(
          id: 'tool:1',
          author: 'Tool',
          displayLabel: 'Tool',
          timestamp: '',
          hasTimestamp: false,
          body: '',
          subtitle: 'execute_code',
          kind: 'execute_code',
          status: 'completed',
          processId: 'process-1',
          hasProcessId: true,
          command: 'output("ok")',
          output: 'ok',
          deliveryState: 'delivered',
          isStreaming: false,
          isTool: true,
        )),
        agentRuntimeChatEntryToChatEntryForTest(const bindings.AgentRuntimeChatEntry(
          id: 'model:1:assistant',
          author: 'Assistant',
          displayLabel: 'Assistant',
          timestamp: '',
          hasTimestamp: false,
          body: 'selected assistant final',
          subtitle: 'completed',
          kind: 'message',
          status: 'completed',
          processId: '',
          hasProcessId: false,
          command: '',
          output: '',
          deliveryState: 'delivered',
          isStreaming: false,
          isTool: false,
        )),
      ],
      composerEnabled: true,
      isRunning: false,
      detailTitle: 'Runtime detail',
      detailSections: const [ConversationDetailSection(title: 'History', rows: historyRows)],
    );

    await tester.pumpWidget(MaterialApp(
      home: ConversationShellScreen(
        data: data,
        onSessionSelected: (_) {},
        onCreateSession: () {},
        onSendMessage: (_) {},
        onInterrupt: () {},
        showPermanentDetail: false,
        headerControls: const Text('History'),
      ),
    ));

    expect(find.text('Role imported'), findsNothing);
    expect(find.text('Tool completed'), findsNothing);
    final center = find.byKey(const ValueKey('conversationShell.center'));
    expect(find.descendant(of: center, matching: find.text('selected user text')), findsOneWidget);
    expect(find.descendant(of: center, matching: find.text('selected assistant final')), findsOneWidget);
    for (final raw in ['role.imported', 'turn.started', 'model.final_response', 'tool.completed', 'read History', 'Output details are available in History.']) {
      expect(find.descendant(of: center, matching: find.textContaining(raw)), findsNothing);
    }
  });

  test('session close archive and fork use generated typed operation intents', () {
    final sentRequests = <bindings.AgentRuntimeRequest>[];
    final controller = AgentRuntimeWorkbenchController(
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
    final controller = AgentRuntimeWorkbenchController(
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

  test('runtime and session settings submit through typed operation variants', () {
    final sentRequests = <bindings.AgentRuntimeRequest>[];
    final controller = AgentRuntimeWorkbenchController(
      requestSink: (requestId, request) {
        sentRequests.add(request);
      },
    );
    addTearDown(controller.dispose);

    controller.updateRuntimeSettings(baseUrl: ' http://127.0.0.1:8765 ', selectedProjectId: ' project-a ');
    controller.updateSessionSettings(
      sessionId: ' session-1 ',
      project: ' project-a ',
      role: ' runtime-allow ',
      model: ' gpt-5.5 ',
      workdir: ' /tmp/project-a ',
      worktreeRoot: ' /tmp/project-a ',
      title: ' Updated title ',
      name: ' updated-name ',
      tracked: true,
    );

    final runtime = sentRequests[0] as bindings.AgentRuntimeRequestDispatchOperation;
    expect(runtime.operation, isA<bindings.AgentRuntimeGuiOperationUpdateRuntimeSettings>());
    final runtimeSettings = runtime.operation as bindings.AgentRuntimeGuiOperationUpdateRuntimeSettings;
    expect(runtimeSettings.baseUrl, 'http://127.0.0.1:8765');
    expect(runtimeSettings.selectedProjectId, 'project-a');

    final session = sentRequests[1] as bindings.AgentRuntimeRequestDispatchOperation;
    expect(session.operation, isA<bindings.AgentRuntimeGuiOperationUpdateSessionSettings>());
    final sessionSettings = session.operation as bindings.AgentRuntimeGuiOperationUpdateSessionSettings;
    expect(sessionSettings.sessionId, 'session-1');
    expect(sessionSettings.project, 'project-a');
    expect(sessionSettings.role, 'runtime-allow');
    expect(sessionSettings.model, 'gpt-5.5');
    expect(sessionSettings.workdir, '/tmp/project-a');
    expect(sessionSettings.worktreeRoot, '/tmp/project-a');
    expect(sessionSettings.title, 'Updated title');
    expect(sessionSettings.name, 'updated-name');
    expect(sessionSettings.tracked, true);
  });

  test('role admin shell actions send generated typed operations with payloads', () {
    final sentRequests = <bindings.AgentRuntimeRequest>[];
    final controller = AgentRuntimeWorkbenchController(
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
    final controller = AgentRuntimeWorkbenchController(
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
    final controller = AgentRuntimeWorkbenchController(
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
    final controller = AgentRuntimeWorkbenchController(
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
