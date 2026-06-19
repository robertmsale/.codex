import 'package:flutter_test/flutter_test.dart';
import 'package:flutter/material.dart';
import 'package:robdex_app/src/agent_runtime/agent_runtime_workbench_controller.dart';
import 'package:robdex_app/src/agent_runtime/agent_runtime_workbench_host.dart';
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

  test('controller consumes typed operation, snapshot, controller, and stream outputs continuously', () {
    final controller = AgentRuntimeWorkbenchController(requestSink: (_, _) {});
    addTearDown(controller.dispose);

    controller.applyOutputForTest(const bindings.AgentRuntimeOutputOperationResult(
      result: bindings.AgentRuntimeOperationResult(operation: 'CreateSession', outcome: 'accepted', message: 'created'),
    ));
    expect(controller.data.outputLog.last, contains('CreateSession'));

    controller.applyOutputForTest(const bindings.AgentRuntimeOutputProjectionSnapshot(
      projection: bindings.AgentRuntimeProjectionSnapshot(
        watermark: 42,
        sessionCount: 3,
        timelineCount: 8,
        actionCount: 1,
        roleCount: 2,
        workflowMemoryCount: 1,
        selectedChatEntries: [],
      ),
    ));
    expect(controller.data.watermarkLabel, '42');
    expect(controller.data.statusBadges.any((badge) => badge.label == 'Sessions' && badge.value == '3'), isTrue);

    controller.applyOutputForTest(const bindings.AgentRuntimeOutputControllerState(
      controllerState: bindings.AgentRuntimeControllerState(
        connectionState: 'streaming',
        selectedSessionId: 'session-typed',
        hasSelectedSessionId: true,
        baseUrl: 'http://127.0.0.1:8765',
        lastError: '',
        hasLastError: false,
      ),
    ));
    expect(controller.data.connectionState, 'streaming');
    expect(controller.data.selectedSessionLabel, 'session-typed');

    controller.applyOutputForTest(const bindings.AgentRuntimeOutputStreamOutcome(
      outcome: bindings.AgentRuntimeStreamOutcomeDeltaApplied(applyOutcome: 'selectedChatDelta'),
      projection: bindings.AgentRuntimeProjectionSnapshot(
        watermark: 43,
        sessionCount: 3,
        timelineCount: 9,
        actionCount: 1,
        roleCount: 2,
        workflowMemoryCount: 1,
        selectedChatEntries: [
          bindings.AgentRuntimeChatEntry(
            id: 'turn-1-user',
            author: 'User',
            displayLabel: 'User',
            timestamp: '',
            hasTimestamp: false,
            body: 'exact composer text before final',
            subtitle: 'running',
            kind: 'message',
            status: 'running',
            processId: '',
            hasProcessId: false,
            command: '',
            output: '',
            deliveryState: 'sending',
            isStreaming: true,
            isTool: false,
          ),
          bindings.AgentRuntimeChatEntry(
            id: 'assistant-1',
            author: 'Assistant',
            displayLabel: 'Assistant',
            timestamp: '',
            hasTimestamp: false,
            body: 'partial assistant delta',
            subtitle: 'running',
            kind: 'message',
            status: 'running',
            processId: '',
            hasProcessId: false,
            command: '',
            output: '',
            deliveryState: 'streaming',
            isStreaming: true,
            isTool: false,
          ),
        ],
      ),
      hasProjection: true,
      controllerState: bindings.AgentRuntimeControllerState(
        connectionState: 'streaming',
        selectedSessionId: 'session-typed',
        hasSelectedSessionId: true,
        baseUrl: 'http://127.0.0.1:8765',
        lastError: '',
        hasLastError: false,
      ),
    ));
    expect(controller.data.watermarkLabel, '43');
    expect(controller.data.outputLog.last, contains('stream delta'));
    expect(controller.data.selectedConversation.map((entry) => entry.body), contains('exact composer text before final'));
    expect(controller.data.selectedConversation.map((entry) => entry.body), contains('partial assistant delta'));
    expect(controller.data.selectedConversation.any((entry) => entry.isStreaming), isTrue);
  });

  test('typed operation errors render inline in controller data', () {
    final controller = AgentRuntimeWorkbenchController(requestSink: (_, _) {});
    addTearDown(controller.dispose);

    controller.applyOutputForTest(const bindings.AgentRuntimeOutputOperationResult(
      result: bindings.AgentRuntimeOperationResult(
        operation: 'CreateSession',
        outcome: 'error',
        message: 'Create session requires role.',
      ),
    ));

    expect(controller.data.errorMessage, contains('requires role'));
    expect(controller.data.outputLog.last, contains('CreateSession'));

    controller.applyOutputForTest(const bindings.AgentRuntimeOutputError(
      error: bindings.AgentRuntimeApiError(
        code: 'unavailable',
        message: 'Connect failed: runtime did not respond.',
        details: [],
      ),
    ));

    expect(controller.data.errorMessage, contains('runtime did not respond'));
    expect(controller.data.outputLog.last, contains('Connect failed'));
  });

  testWidgets('connected shell renders typed errors inline near the composer surface', (tester) async {
    tester.view.physicalSize = const Size(1200, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final shell = ConversationShellData(
      appTitle: 'Agent Runtime',
      connectionLabel: 'Connected',
      projects: const [
        ConversationProject(id: 'project-a', title: 'Project A'),
      ],
      sessions: const [
        ConversationSession(
          id: 'session-a',
          title: 'Session A',
          subtitle: 'Project A · Runtime allow',
          role: 'Runtime allow',
          selected: true,
          rolePresentation: ConversationRolePresentation(
            roleId: 'runtime-allow',
            displayLabel: 'Runtime allow',
            shortLabel: 'RA',
            iconKey: 'runtime-allow',
            tone: 'info',
            statusLabel: 'Open',
            description: 'Runtime role',
          ),
        ),
      ],
      selectedSessionId: 'session-a',
      timelineTitle: 'Selected session',
      entries: const [],
      composerEnabled: true,
      isRunning: false,
      detailTitle: 'Operations',
      detailSections: const [],
      inlineErrorMessage: 'Create session requires a role. Choose a role and try again.',
    );

    await tester.pumpWidget(MaterialApp(
      home: ConversationShellScreen(
        data: shell,
        onSessionSelected: (_) {},
        onCreateSession: () {},
        onSendMessage: (_) {},
        onInterrupt: () {},
      ),
    ));

    expect(find.text('Create session requires a role. Choose a role and try again.'), findsOneWidget);
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

    expect(sentRequests, hasLength(2));
    expect(controller.data.errorMessage, contains('Use the New session dialog'));
    final select = sentRequests[0] as bindings.AgentRuntimeRequestDispatchOperation;
    expect(select.operation, isA<bindings.AgentRuntimeGuiOperationSelectSession>());
    expect((select.operation as bindings.AgentRuntimeGuiOperationSelectSession).sessionId, 'session-2');
    final send = sentRequests[1] as bindings.AgentRuntimeRequestDispatchOperation;
    expect(send.operation, isA<bindings.AgentRuntimeGuiOperationSendMessage>());
    final sendMessage = send.operation as bindings.AgentRuntimeGuiOperationSendMessage;
    expect(sendMessage.sessionId, 'session-2');
    expect(sendMessage.message, 'hello runtime');
  });

  test('project CRUD actions send generated typed operation intents', () {
    final sentRequests = <bindings.AgentRuntimeRequest>[];
    final controller = AgentRuntimeWorkbenchController(
      requestSink: (requestId, request) {
        sentRequests.add(request);
      },
    );
    addTearDown(controller.dispose);

    controller.createProject(
      projectKey: 'project-a',
      displayName: 'Project A',
      defaultWorkdir: '/work/a',
      defaultWorktreeRoot: '/work/a',
      defaultRoleId: 'runtime-no-rg',
      defaultModel: 'gpt-5.4-mini',
      tracked: true,
      listed: true,
    );
    controller.updateProject(
      projectKey: 'project-a',
      displayName: 'Project Alpha',
      defaultWorkdir: '/work/alpha',
      defaultWorktreeRoot: '/work/alpha',
      defaultRoleId: 'runtime-allow',
      defaultModel: 'gpt-5.5',
      tracked: false,
      listed: true,
    );
    controller.archiveProject('project-a');

    expect(sentRequests, hasLength(3));
    expect((sentRequests[0] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationCreateProject>());
    final update = (sentRequests[1] as bindings.AgentRuntimeRequestDispatchOperation).operation as bindings.AgentRuntimeGuiOperationUpdateProject;
    expect(update.projectKey, 'project-a');
    expect(update.defaultModel, 'gpt-5.5');
    expect(update.tracked, false);
    final archive = (sentRequests[2] as bindings.AgentRuntimeRequestDispatchOperation).operation as bindings.AgentRuntimeGuiOperationArchiveProject;
    expect(archive.projectKey, 'project-a');
  });

  test('approval and command registry operation controls dispatch typed approve deny preview resume and apply intents', () {
    final sentRequests = <bindings.AgentRuntimeRequest>[];
    final controller = AgentRuntimeWorkbenchController(
      requestSink: (requestId, request) {
        sentRequests.add(request);
      },
    );
    addTearDown(controller.dispose);
    const approval = AgentRuntimeActionItem(id: 'approval-1', title: 'Approval', subtitle: '', kind: 'approval', stateText: 'ready', tone: 'warning');
    const registry = AgentRuntimeActionItem(id: 'registry-1', title: 'Registry', subtitle: '', kind: 'commandRegistryRequest', stateText: 'ready', tone: 'warning');

    controller.approveAction(approval);
    controller.denyAction(approval);
    controller.resumeApproval(approval);
    controller.previewCommandRegistryRequest(registry, 'session-1');
    controller.approveCommandRegistryRequest(registry, 'session-1');
    controller.denyCommandRegistryRequest(registry, 'session-1');
    controller.applyCommandRegistryRequest(registry, 'session-1');

    final operations = sentRequests.cast<bindings.AgentRuntimeRequestDispatchOperation>().map((request) => request.operation).toList(growable: false);
    expect(operations[0], isA<bindings.AgentRuntimeGuiOperationDecideApproval>());
    expect((operations[0] as bindings.AgentRuntimeGuiOperationDecideApproval).decision, 'approved');
    expect((operations[1] as bindings.AgentRuntimeGuiOperationDecideApproval).decision, 'denied');
    expect(operations[2], isA<bindings.AgentRuntimeGuiOperationResumeApproval>());
    expect(operations[3], isA<bindings.AgentRuntimeGuiOperationPreviewCommandRegistryRequest>());
    expect((operations[4] as bindings.AgentRuntimeGuiOperationDecideCommandRegistryRequest).decision.status, 'approved');
    expect((operations[5] as bindings.AgentRuntimeGuiOperationDecideCommandRegistryRequest).decision.status, 'denied');
    expect(operations[6], isA<bindings.AgentRuntimeGuiOperationApplyCommandRegistryRequest>());
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
          headerControls: IconButton(
            key: const ValueKey('agentRuntime.toolbar.history'),
            tooltip: 'Runtime operations',
            onPressed: () {
              showModalBottomSheet<void>(
                context: tester.element(find.byKey(const ValueKey('agentRuntime.toolbar.history'))),
                builder: (_) => const AgentRuntimeOperationsDetail(data: mockAgentRuntimeConnected, focusSurfaceId: 'history'),
              );
            },
            icon: const Icon(Icons.manage_history_rounded),
          ),
        ),
      ),
    );

    expect(find.byKey(const ValueKey('conversationShell.center')), findsOneWidget);
    expect(find.text('History'), findsNothing);
    expect(find.text('More'), findsNothing);
    expect(find.text('New'), findsNothing);
    expect(find.text('Details'), findsNothing);
    expect(find.text('Role imported'), findsNothing);

    await tester.tap(find.byKey(const ValueKey('agentRuntime.toolbar.history')));
    await tester.pumpAndSettle();
    expect(find.text('History'), findsWidgets);
    expect(find.text('Role imported'), findsWidgets);
  });

  testWidgets('project rows expose scoped management menus without forbidden actions on All or Unassigned', (tester) async {
    final selectedActions = <String>[];
    await tester.binding.setSurfaceSize(const Size(700, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(
      MaterialApp(
        home: ConversationShellScreen(
          data: const ConversationShellData(
            appTitle: 'Agent Runtime',
            connectionLabel: 'Connected',
            projects: [
              ConversationProject(id: '__all__', title: 'All', canCreateSession: true),
              ConversationProject(id: '__unassigned__', title: 'Unassigned', canCreateSession: true),
              ConversationProject(id: 'project-a', title: 'Project A', canEdit: true, canArchive: true, canCreateSession: true),
            ],
            sessions: [],
            selectedSessionId: null,
            timelineTitle: 'Selected session',
            entries: [],
            composerEnabled: false,
            isRunning: false,
            detailTitle: 'Details',
            detailSections: [],
          ),
          onSessionSelected: (_) {},
          onCreateSession: () {},
          onSendMessage: (_) {},
          onInterrupt: () {},
          onEditProject: (id) => selectedActions.add('edit:$id'),
          onNewSessionInProject: (id) => selectedActions.add('new:$id'),
          onArchiveProject: (id) => selectedActions.add('archive:$id'),
          showPermanentDetail: false,
        ),
      ),
    );

    await tester.tap(find.byKey(const ValueKey('conversationProject.menu.__all__')));
    await tester.pumpAndSettle();
    expect(find.text('New session'), findsOneWidget);
    expect(find.text('Edit project'), findsNothing);
    expect(find.text('Archive project'), findsNothing);
    await tester.tap(find.text('New session'));
    await tester.pumpAndSettle();
    expect(selectedActions, contains('new:__all__'));

    await tester.tap(find.byKey(const ValueKey('conversationProject.menu.__unassigned__')));
    await tester.pumpAndSettle();
    expect(find.text('New unassigned session'), findsOneWidget);
    expect(find.text('Edit project'), findsNothing);
    expect(find.text('Archive project'), findsNothing);
    await tester.tap(find.text('New unassigned session'));
    await tester.pumpAndSettle();
    expect(selectedActions, contains('new:__unassigned__'));

    await tester.tap(find.byKey(const ValueKey('conversationProject.menu.project-a')));
    await tester.pumpAndSettle();
    expect(find.text('Edit project'), findsOneWidget);
    expect(find.text('New session in project'), findsOneWidget);
    expect(find.text('Archive project'), findsOneWidget);
  });

  testWidgets('desktop conversation left rail resize handle changes bounded persisted width and hides on phone', (tester) async {
    double persistedWidth = 288;
    final data = ConversationShellData(
      appTitle: 'Agent Runtime',
      connectionLabel: 'Runtime healthy',
      projects: const [
        ConversationProject(id: '__all__', title: 'All'),
        ConversationProject(id: '__unassigned__', title: 'Unassigned'),
        ConversationProject(id: 'project-a', title: 'Project A'),
      ],
      sessions: const [
        ConversationSession(
          id: 'session-1',
          title: 'Resize proof',
          subtitle: 'Project A',
          role: 'Runtime',
          selected: true,
          rolePresentation: ConversationRolePresentation(
            roleId: 'runtime',
            displayLabel: 'Runtime',
            shortLabel: 'RT',
            iconKey: 'runtime',
            tone: 'success',
            statusLabel: 'open',
            description: 'Runtime role',
          ),
        ),
      ],
      selectedSessionId: 'session-1',
      timelineTitle: 'Selected session',
      entries: const [],
      composerEnabled: true,
      isRunning: false,
      detailTitle: 'Operations',
      detailSections: const [],
    );

    Future<void> pumpShell(Size size) async {
      await tester.binding.setSurfaceSize(size);
      await tester.pumpWidget(
        MaterialApp(
          home: StatefulBuilder(
            builder: (context, setState) => ConversationShellScreen(
              data: data,
              onSessionSelected: (_) {},
              onCreateSession: () {},
              onSendMessage: (_) {},
              onInterrupt: () {},
              showPermanentDetail: false,
              leftRailWidth: persistedWidth,
              onLeftRailWidthChanged: (value) {
                setState(() {
                  persistedWidth = value;
                });
              },
            ),
          ),
        ),
      );
      await tester.pump();
    }

    addTearDown(() => tester.binding.setSurfaceSize(null));
    await pumpShell(const Size(1200, 800));
    final handle = find.byKey(const ValueKey('conversationShell.leftRailResizeHandle'));
    expect(handle, findsOneWidget);
    await tester.drag(handle, const Offset(180, 0));
    await tester.pump();
    expect(persistedWidth, ConversationShellScreen.maxLeftRailWidth);

    await tester.drag(handle, const Offset(-400, 0));
    await tester.pump();
    expect(persistedWidth, ConversationShellScreen.minLeftRailWidth);

    persistedWidth = 360;
    await pumpShell(const Size(1200, 800));
    expect(persistedWidth, 360);

    await pumpShell(const Size(390, 844));
    expect(handle, findsNothing);
  });

  testWidgets('project and session headers stay outside scroll areas and split is resizable', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1200, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final data = ConversationShellData(
      appTitle: 'Agent Runtime',
      connectionLabel: 'Runtime healthy',
      projects: const [
        ConversationProject(id: '__all__', title: 'All'),
        ConversationProject(id: '__unassigned__', title: 'Unassigned'),
        ConversationProject(id: 'project-a', title: 'Project A'),
        ConversationProject(id: 'project-b', title: 'Project B'),
        ConversationProject(id: 'project-c', title: 'Project C'),
      ],
      sessions: const [
        ConversationSession(
          id: 'session-1',
          title: 'Session one',
          subtitle: 'Project A',
          role: 'Runtime',
          rolePresentation: ConversationRolePresentation(
            roleId: 'runtime',
            displayLabel: 'Runtime',
            shortLabel: 'RT',
            iconKey: 'runtime',
            tone: 'success',
            statusLabel: 'open',
            description: 'Runtime role',
          ),
        ),
        ConversationSession(
          id: 'session-2',
          title: 'Session two',
          subtitle: 'Project B',
          role: 'Runtime',
          rolePresentation: ConversationRolePresentation(
            roleId: 'runtime',
            displayLabel: 'Runtime',
            shortLabel: 'RT',
            iconKey: 'runtime',
            tone: 'success',
            statusLabel: 'open',
            description: 'Runtime role',
          ),
        ),
      ],
      selectedSessionId: null,
      timelineTitle: 'Selected session',
      entries: const [],
      composerEnabled: false,
      isRunning: false,
      detailTitle: 'Operations',
      detailSections: const [],
    );

    await tester.pumpWidget(
      MaterialApp(
        home: ConversationShellScreen(
          data: data,
          onSessionSelected: (_) {},
          onCreateSession: () {},
          onSendMessage: (_) {},
          onInterrupt: () {},
          showPermanentDetail: false,
        ),
      ),
    );

    final projectsScroll = find.byKey(const ValueKey('conversationShell.projectsScrollView'));
    final sessionsScroll = find.byKey(const ValueKey('conversationShell.sessionsScrollView'));
    expect(projectsScroll, findsOneWidget);
    expect(sessionsScroll, findsOneWidget);
    expect(find.descendant(of: projectsScroll, matching: find.text('Projects')), findsNothing);
    expect(find.descendant(of: sessionsScroll, matching: find.text('Sessions')), findsNothing);

    final initialProjectsHeight = tester.getSize(projectsScroll).height;
    final initialSessionsHeight = tester.getSize(sessionsScroll).height;
    await tester.drag(find.byKey(const ValueKey('conversationShell.projectSessionResizeHandle')), const Offset(0, 72));
    await tester.pump();
    expect(tester.getSize(projectsScroll).height, greaterThan(initialProjectsHeight));
    expect(tester.getSize(sessionsScroll).height, lessThan(initialSessionsHeight));
  });

  testWidgets('top settings affordance opens structured modal path without changing project filter or selected session', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1200, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    var selectedProjectId = 'project-b';
    var selectedSessionId = 'session-b';
    var settingsOpened = false;
    var projectSelectionCalls = 0;
    var sessionSelectionCalls = 0;
    var hydrateOrReconnectCalls = 0;
    final data = ConversationShellData(
      appTitle: 'Agent Runtime',
      connectionLabel: 'Runtime healthy',
      projects: const [
        ConversationProject(id: '__all__', title: 'All'),
        ConversationProject(id: '__unassigned__', title: 'Unassigned'),
        ConversationProject(id: 'project-a', title: 'Project A'),
        ConversationProject(id: 'project-b', title: 'Project B'),
      ],
      sessions: const [
        ConversationSession(
          id: 'session-b',
          title: 'Selected project session',
          subtitle: 'Project B',
          role: 'Runtime',
          selected: true,
          rolePresentation: ConversationRolePresentation(
            roleId: 'runtime',
            displayLabel: 'Runtime',
            shortLabel: 'RT',
            iconKey: 'runtime',
            tone: 'success',
            statusLabel: 'open',
            description: 'Runtime role',
          ),
        ),
      ],
      selectedSessionId: selectedSessionId,
      timelineTitle: 'Selected session',
      entries: const [],
      composerEnabled: true,
      isRunning: false,
      detailTitle: 'Operations',
      detailSections: const [],
    );

    await tester.pumpWidget(MaterialApp(
      home: ConversationShellScreen(
        data: data,
        onSessionSelected: (sessionId) {
          sessionSelectionCalls += 1;
          selectedSessionId = sessionId;
        },
        onCreateSession: () {},
        onSendMessage: (_) {},
        onInterrupt: () {},
        onProjectSelected: (projectId) {
          projectSelectionCalls += 1;
          selectedProjectId = projectId;
        },
        onSettings: () {
          settingsOpened = true;
          showDialog<void>(
            context: tester.element(find.byKey(const ValueKey('conversationShell.globalSettings'))),
            builder: (context) => AlertDialog(
              title: const Text('Global settings'),
              content: Column(
                mainAxisSize: MainAxisSize.min,
                children: const [
                  TextField(decoration: InputDecoration(labelText: 'Base URL')),
                  Text('Runtime identity'),
                  Text('Current connection state'),
                ],
              ),
              actions: [
                TextButton(onPressed: () => Navigator.of(context).pop(), child: const Text('Close')),
              ],
            ),
          );
        },
        headerControls: IconButton(
          tooltip: 'Runtime operations',
          onPressed: () {
            hydrateOrReconnectCalls += 1;
          },
          icon: const Icon(Icons.manage_history_rounded),
        ),
      ),
    ));

    await tester.tap(find.byKey(const ValueKey('conversationShell.globalSettings')));
    await tester.pumpAndSettle();
    expect(settingsOpened, true);
    expect(find.text('Global settings'), findsOneWidget);
    expect(find.text('Base URL'), findsOneWidget);
    expect(find.text('Runtime identity'), findsOneWidget);
    expect(find.text('Current connection state'), findsOneWidget);
    expect(selectedProjectId, 'project-b');
    expect(selectedSessionId, 'session-b');
    expect(projectSelectionCalls, 0);
    expect(sessionSelectionCalls, 0);
    expect(hydrateOrReconnectCalls, 0);

    await tester.tap(find.text('Close'));
    await tester.pumpAndSettle();
    expect(find.text('Global settings'), findsNothing);
    expect(selectedProjectId, 'project-b');
    expect(selectedSessionId, 'session-b');
    expect(projectSelectionCalls, 0);
    expect(sessionSelectionCalls, 0);
    expect(hydrateOrReconnectCalls, 0);
  });

  testWidgets('Global Settings modal renders concrete controls, diagnostics, inline errors, and dispatches every action', (tester) async {
    final actions = <String>[];
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: AgentRuntimeGlobalSettingsDialog(
            data: mockAgentRuntimeConnected.copyWith(errorMessage: 'typed connect failure'),
            onConnectManual: (baseUrl) => actions.add('manual:$baseUrl'),
            onRefreshDiscovery: () => actions.add('refresh-local'),
            onConnectDiscovery: () => actions.add('connect-local'),
            onRefreshIcloud: () => actions.add('refresh-icloud'),
            onConnectIcloud: () => actions.add('connect-icloud'),
            onImportProfile: () => actions.add('import-profile'),
            onRefreshImportedProfile: () => actions.add('refresh-imported'),
            onConnectImportedProfile: () => actions.add('connect-imported'),
            onDisconnect: () => actions.add('disconnect'),
          ),
        ),
      ),
    );

    expect(find.text('Global settings'), findsOneWidget);
    expect(find.text('typed connect failure'), findsOneWidget);
    for (final label in [
      'Base URL',
      'Connect manual URL',
      'Refresh local discovery',
      'Connect local discovery',
      'Refresh iCloud profile',
      'Connect iCloud profile',
      'Import remote profile document',
      'Refresh imported profile',
      'Connect imported profile',
      'Disconnect',
      'Runtime identity',
      'Health URL',
      'WebSocket URL',
      'Discovery path',
      'iCloud profile path',
      'Imported profile path',
      'Connection state',
    ]) {
      expect(find.text(label), findsWidgets);
    }

    for (final label in [
      'Connect manual URL',
      'Refresh local discovery',
      'Connect local discovery',
      'Refresh iCloud profile',
      'Connect iCloud profile',
      'Import remote profile document',
      'Refresh imported profile',
      'Connect imported profile',
      'Disconnect',
    ]) {
      await tester.ensureVisible(find.text(label).last);
      await tester.tap(find.text(label).last);
      await tester.pump();
    }

    expect(actions, [
      startsWith('manual:'),
      'refresh-local',
      'connect-local',
      'refresh-icloud',
      'connect-icloud',
      'import-profile',
      'refresh-imported',
      'connect-imported',
      'disconnect',
    ]);
  });

  testWidgets('Create Project modal validates key and submits a fully populated typed draft', (tester) async {
    Map<String, Object?>? submitted;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: AgentRuntimeCreateProjectDialog(
            data: mockAgentRuntimeConnected,
            existingProjectKeys: const ['existing-project'],
            onCreate: ({
              required projectKey,
              required displayName,
              required defaultWorkdir,
              required defaultWorktreeRoot,
              required defaultRoleId,
              required defaultModel,
              required tracked,
              required listed,
            }) {
              submitted = {
                'projectKey': projectKey,
                'displayName': displayName,
                'defaultWorkdir': defaultWorkdir,
                'defaultWorktreeRoot': defaultWorktreeRoot,
                'defaultRoleId': defaultRoleId,
                'defaultModel': defaultModel,
                'tracked': tracked,
                'listed': listed,
              };
            },
          ),
        ),
      ),
    );

    await tester.enterText(find.widgetWithText(TextField, 'Project key'), 'bad key with spaces');
    await tester.enterText(find.widgetWithText(TextField, 'Display name'), 'Bad Project');
    await tester.tap(find.text('Create'));
    await tester.pump();
    expect(submitted, isNull);
    expect(find.text('Project key must use letters, numbers, dot, dash, or underscore.'), findsOneWidget);

    await tester.enterText(find.widgetWithText(TextField, 'Project key'), 'existing-project');
    await tester.tap(find.text('Create'));
    await tester.pump();
    expect(submitted, isNull);
    expect(find.text('Project key already exists.'), findsOneWidget);

    await tester.enterText(find.widgetWithText(TextField, 'Project key'), 'project.validation');
    await tester.enterText(find.widgetWithText(TextField, 'Display name'), 'Validation Project');
    await tester.enterText(find.widgetWithText(TextField, 'Default workdir'), '/work/validation');
    await tester.enterText(find.widgetWithText(TextField, 'Default worktree root'), '/work/validation/root');
    await tester.enterText(find.widgetWithText(TextField, 'Default model'), 'gpt-5.4-mini');
    await tester.tap(find.text('Create'));
    await tester.pump();

    expect(submitted, isNotNull);
    expect(submitted!['projectKey'], 'project.validation');
    expect(submitted!['displayName'], 'Validation Project');
    expect(submitted!['defaultWorkdir'], '/work/validation');
    expect(submitted!['defaultWorktreeRoot'], '/work/validation/root');
    expect(submitted!['defaultRoleId'], isNot(''));
    expect(submitted!['defaultModel'], 'gpt-5.4-mini');
    expect(submitted!['tracked'], true);
    expect(submitted!['listed'], true);
  });

  testWidgets('Create Session modal uses a structured non-overlapping form and runtime model options', (tester) async {
    await tester.binding.setSurfaceSize(const Size(900, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    Map<String, String>? submitted;
    const shell = ConversationShellData(
      appTitle: 'Agent Runtime',
      connectionLabel: 'Runtime healthy',
      projects: [
        ConversationProject(id: '__all__', title: 'All'),
        ConversationProject(id: '__unassigned__', title: 'Unassigned'),
        ConversationProject(id: 'project-a', title: 'Project A'),
      ],
      sessions: [],
      selectedSessionId: null,
      timelineTitle: 'Selected session',
      entries: [],
      composerEnabled: false,
      isRunning: false,
      detailTitle: 'Operations',
      detailSections: [],
    );

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: AgentRuntimeCreateSessionDialog(
            shell: shell,
            data: mockAgentRuntimeConnected,
            initialProjectId: 'project-a',
            onCreate: ({
              required role,
              required project,
              required model,
              required workdir,
              required worktreeRoot,
              required title,
              required name,
            }) {
              submitted = {
                'role': role,
                'project': project,
                'model': model,
                'workdir': workdir,
                'worktreeRoot': worktreeRoot,
                'title': title,
                'name': name,
              };
            },
          ),
        ),
      ),
    );

    for (final label in [
      'Session scope',
      'Session identity',
      'Workspace',
      'Project',
      'Role',
      'Model',
      'Title',
      'Generated session name',
      'Workdir',
      'Worktree root',
    ]) {
      expect(find.text(label), findsOneWidget);
    }
    expect(find.byKey(const ValueKey('agentRuntime.createSession.model')), findsOneWidget);
    expect(find.text('Codex live model'), findsOneWidget);

    await tester.enterText(find.byKey(const ValueKey('agentRuntime.createSession.title')), 'Live Test Session');
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.createSession.workdir')), '/work/live');
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.createSession.worktreeRoot')), '/work/live/root');
    expect(find.text('live-test-session'), findsOneWidget);
    await tester.tap(find.widgetWithText(FilledButton, 'Create'));
    await tester.pumpAndSettle();

    expect(submitted, isNotNull);
    expect(submitted!['project'], 'project-a');
    expect(submitted!['model'], mockAgentRuntimeConnected.modelOptions.single.id);
    expect(submitted!['title'], 'Live Test Session');
    expect(submitted!['name'], 'live-test-session');
    expect(submitted!['workdir'], '/work/live');
    expect(submitted!['worktreeRoot'], '/work/live/root');
  });

  testWidgets('Create Session modal blocks unavailable model options with inline recovery text', (tester) async {
    Map<String, String>? submitted;
    const shell = ConversationShellData(
      appTitle: 'Agent Runtime',
      connectionLabel: 'Runtime healthy',
      projects: [
        ConversationProject(id: '__unassigned__', title: 'Unassigned'),
      ],
      sessions: [],
      selectedSessionId: null,
      timelineTitle: 'Selected session',
      entries: [],
      composerEnabled: false,
      isRunning: false,
      detailTitle: 'Operations',
      detailSections: [],
    );

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: AgentRuntimeCreateSessionDialog(
            shell: shell,
            data: mockAgentRuntimeConnected.copyWith(modelOptions: const []),
            onCreate: ({
              required role,
              required project,
              required model,
              required workdir,
              required worktreeRoot,
              required title,
              required name,
            }) {
              submitted = {'model': model};
            },
          ),
        ),
      ),
    );

    expect(find.textContaining('Model options are unavailable'), findsOneWidget);
    expect(find.byKey(const ValueKey('agentRuntime.createSession.noModel')), findsOneWidget);
    expect(tester.widget<FilledButton>(find.widgetWithText(FilledButton, 'Create')).onPressed, isNull);
    expect(submitted, isNull);
  });

  testWidgets('Project Settings modal saves every field and exposes archive and unarchive typed actions', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1200, 1000));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final actions = <String>[];
    Map<String, Object?>? saved;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: AgentRuntimeProjectSettingsDialog(
            data: mockAgentRuntimeConnected,
            projectId: 'project-a',
            onSave: ({
              required projectKey,
              required displayName,
              required defaultWorkdir,
              required defaultWorktreeRoot,
              required defaultRoleId,
              required defaultModel,
              required tracked,
              required listed,
            }) {
              saved = {
                'projectKey': projectKey,
                'displayName': displayName,
                'defaultWorkdir': defaultWorkdir,
                'defaultWorktreeRoot': defaultWorktreeRoot,
                'defaultRoleId': defaultRoleId,
                'defaultModel': defaultModel,
                'tracked': tracked,
                'listed': listed,
              };
            },
            onArchive: (id) => actions.add('archive:$id'),
            onUnarchive: (id) => actions.add('unarchive:$id'),
          ),
        ),
      ),
    );

    expect(find.text('Project settings'), findsOneWidget);
    expect(find.text('Project key'), findsOneWidget);
    await tester.enterText(find.widgetWithText(TextField, 'Display name'), 'Project Alpha');
    await tester.enterText(find.widgetWithText(TextField, 'Default workdir'), '/work/alpha');
    await tester.enterText(find.widgetWithText(TextField, 'Default worktree root'), '/work/alpha/root');
    await tester.enterText(find.widgetWithText(TextField, 'Default model'), 'gpt-5.5');
    tester.widget<TextButton>(find.widgetWithText(TextButton, 'Archive')).onPressed!();
    await tester.pumpAndSettle();
    expect(actions, contains('archive:project-a'));

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: AgentRuntimeProjectSettingsDialog(
            data: mockAgentRuntimeConnected,
            projectId: 'project-a',
            onSave: ({
              required projectKey,
              required displayName,
              required defaultWorkdir,
              required defaultWorktreeRoot,
              required defaultRoleId,
              required defaultModel,
              required tracked,
              required listed,
            }) {
              saved = {
                'projectKey': projectKey,
                'displayName': displayName,
                'defaultWorkdir': defaultWorkdir,
                'defaultWorktreeRoot': defaultWorktreeRoot,
                'defaultRoleId': defaultRoleId,
                'defaultModel': defaultModel,
                'tracked': tracked,
                'listed': listed,
              };
            },
            onArchive: (id) => actions.add('archive:$id'),
            onUnarchive: (id) => actions.add('unarchive:$id'),
          ),
        ),
      ),
    );
    await tester.enterText(find.widgetWithText(TextField, 'Display name'), 'Project Alpha');
    await tester.enterText(find.widgetWithText(TextField, 'Default workdir'), '/work/alpha');
    await tester.enterText(find.widgetWithText(TextField, 'Default worktree root'), '/work/alpha/root');
    await tester.enterText(find.widgetWithText(TextField, 'Default model'), 'gpt-5.5');
    tester.widget<TextButton>(find.widgetWithText(TextButton, 'Unarchive')).onPressed!();
    await tester.pumpAndSettle();
    expect(actions, contains('unarchive:project-a'));

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: AgentRuntimeProjectSettingsDialog(
            data: mockAgentRuntimeConnected,
            projectId: 'project-a',
            onSave: ({
              required projectKey,
              required displayName,
              required defaultWorkdir,
              required defaultWorktreeRoot,
              required defaultRoleId,
              required defaultModel,
              required tracked,
              required listed,
            }) {
              saved = {
                'projectKey': projectKey,
                'displayName': displayName,
                'defaultWorkdir': defaultWorkdir,
                'defaultWorktreeRoot': defaultWorktreeRoot,
                'defaultRoleId': defaultRoleId,
                'defaultModel': defaultModel,
                'tracked': tracked,
                'listed': listed,
              };
            },
            onArchive: (id) => actions.add('archive:$id'),
            onUnarchive: (id) => actions.add('unarchive:$id'),
          ),
        ),
      ),
    );
    await tester.enterText(find.widgetWithText(TextField, 'Display name'), 'Project Alpha');
    await tester.enterText(find.widgetWithText(TextField, 'Default workdir'), '/work/alpha');
    await tester.enterText(find.widgetWithText(TextField, 'Default worktree root'), '/work/alpha/root');
    await tester.enterText(find.widgetWithText(TextField, 'Default model'), 'gpt-5.5');
    await tester.ensureVisible(find.widgetWithText(FilledButton, 'Save'));
    await tester.tap(find.widgetWithText(FilledButton, 'Save'));
    await tester.pumpAndSettle();

    expect(saved!['projectKey'], 'project-a');
    expect(saved!['displayName'], 'Project Alpha');
    expect(saved!['defaultWorkdir'], '/work/alpha');
    expect(saved!['defaultWorktreeRoot'], '/work/alpha/root');
    expect(saved!['defaultRoleId'], isNot(''));
    expect(saved!['defaultModel'], 'gpt-5.5');
  });

  testWidgets('Session Settings modal saves fields and dispatches lifecycle actions', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1200, 1000));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final actions = <String>[];
    Map<String, Object?>? saved;
    const shell = ConversationShellData(
      appTitle: 'Agent Runtime',
      connectionLabel: 'Runtime healthy',
      projects: [
        ConversationProject(id: '__unassigned__', title: 'Unassigned'),
        ConversationProject(id: 'project-a', title: 'Project A'),
      ],
      sessions: [
        ConversationSession(
          id: 'session-a',
          title: 'Session Alpha',
          subtitle: 'Project A',
          role: 'Runtime',
          selected: true,
          rolePresentation: ConversationRolePresentation(
            roleId: 'runtime-allow',
            displayLabel: 'Runtime Allow',
            shortLabel: 'RA',
            iconKey: 'runtime',
            tone: 'success',
            statusLabel: 'open',
            description: 'Runtime role',
          ),
        ),
      ],
      selectedSessionId: 'session-a',
      timelineTitle: 'Selected session',
      entries: [],
      composerEnabled: true,
      isRunning: false,
      detailTitle: 'Operations',
      detailSections: [],
    );

    Future<void> pumpDialog() async {
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: AgentRuntimeSessionSettingsDialog(
              shell: shell,
              data: mockAgentRuntimeConnected,
              onSave: ({
                required sessionId,
                required project,
                required role,
                required model,
                required workdir,
                required worktreeRoot,
                required title,
                required name,
                required tracked,
              }) {
                saved = {
                  'sessionId': sessionId,
                  'project': project,
                  'role': role,
                  'model': model,
                  'workdir': workdir,
                  'worktreeRoot': worktreeRoot,
                  'title': title,
                  'name': name,
                  'tracked': tracked,
                };
              },
              onClose: (id) => actions.add('close:$id'),
              onArchive: (id) => actions.add('archive:$id'),
              onFork: (id) => actions.add('fork:$id'),
            ),
          ),
        ),
      );
    }

    await pumpDialog();
    await tester.enterText(find.widgetWithText(TextField, 'Title'), 'Updated Session');
    await tester.enterText(find.widgetWithText(TextField, 'Name'), 'updated-session');
    await tester.enterText(find.widgetWithText(TextField, 'Model'), 'gpt-5.5');
    await tester.enterText(find.widgetWithText(TextField, 'Workdir'), '/work/session');
    await tester.enterText(find.widgetWithText(TextField, 'Worktree root'), '/work/session/root');
    await tester.tap(find.text('Save'));
    await tester.pumpAndSettle();
    expect(saved!['sessionId'], 'session-a');
    expect(saved!['title'], 'Updated Session');
    expect(saved!['name'], 'updated-session');
    expect(saved!['model'], 'gpt-5.5');
    expect(saved!['workdir'], '/work/session');
    expect(saved!['worktreeRoot'], '/work/session/root');
    expect(saved!['project'], '__unassigned__');
    expect(saved!['role'], 'runtime-allow');

    for (final label in ['Close session', 'Archive session', 'Fork session']) {
      await pumpDialog();
      tester.widget<TextButton>(find.widgetWithText(TextButton, label)).onPressed!();
      await tester.pumpAndSettle();
    }
    expect(actions, containsAll(['close:session-a', 'archive:session-a', 'fork:session-a']));
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

  test('project selection uses typed Rust state and legacy settings shortcut does not hydrate', () {
    final sentRequests = <bindings.AgentRuntimeRequest>[];
    final controller = AgentRuntimeWorkbenchController(
      requestSink: (requestId, request) {
        sentRequests.add(request);
      },
    );
    addTearDown(controller.dispose);

    controller.selectProject('runtime');
    controller.openSettings();

    expect(sentRequests, hasLength(1));
    expect(sentRequests[0], isA<bindings.AgentRuntimeRequestSelectProject>());
    expect((sentRequests[0] as bindings.AgentRuntimeRequestSelectProject).projectId, 'runtime');
    expect(controller.data.errorMessage, contains('Global settings'));
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
