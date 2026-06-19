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

  test('disconnect sends only disconnect intent while Rust owns stream shutdown', () {
    final sentRequests = <bindings.AgentRuntimeRequest>[];
    final controller = AgentRuntimeWorkbenchController(
      requestSink: (requestId, request) {
        sentRequests.add(request);
      },
    );
    addTearDown(controller.dispose);

    controller.sendMessage('session-live', 'hello');
    expect(sentRequests.single, isA<bindings.AgentRuntimeRequestDispatchOperation>());

    controller.disconnect();
    expect(sentRequests.last, isA<bindings.AgentRuntimeRequestDisconnect>());
    expect(sentRequests, hasLength(2));

    controller.applyOutputForTest(const bindings.AgentRuntimeOutputControllerState(
      controllerState: bindings.AgentRuntimeControllerState(
        connectionState: 'streaming',
        selectedSessionId: 'session-live',
        hasSelectedSessionId: true,
        baseUrl: 'http://127.0.0.1:8765',
        lastError: '',
        hasLastError: false,
      ),
    ));
    expect(sentRequests, hasLength(2));

    controller.applyOutputForTest(const bindings.AgentRuntimeOutputControllerState(
      controllerState: bindings.AgentRuntimeControllerState(
        connectionState: 'disconnected',
        selectedSessionId: '',
        hasSelectedSessionId: false,
        baseUrl: 'http://127.0.0.1:8765',
        lastError: '',
        hasLastError: false,
      ),
    ));
    expect(controller.data.connectionState, 'disconnected');
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

  test('composer send emits only send intent and Rust stream signals apply selected chat deltas', () {
    final sentRequests = <bindings.AgentRuntimeRequest>[];
    final controller = AgentRuntimeWorkbenchController(
      requestSink: (requestId, request) {
        sentRequests.add(request);
      },
    );
    addTearDown(controller.dispose);

    controller.sendMessage('session-live', 'show files');
    expect(sentRequests, hasLength(1));
    expect(sentRequests.single, isA<bindings.AgentRuntimeRequestDispatchOperation>());

    controller.applyOutputForTest(const bindings.AgentRuntimeOutputStreamOutcome(
      outcome: bindings.AgentRuntimeStreamOutcomeDeltaApplied(applyOutcome: 'selectedChatAppend'),
      projection: bindings.AgentRuntimeProjectionSnapshot(
        watermark: 7,
        sessionCount: 1,
        timelineCount: 2,
        actionCount: 0,
        roleCount: 1,
        workflowMemoryCount: 0,
        selectedChatEntries: [
          bindings.AgentRuntimeChatEntry(
            id: 'turn-user-1',
            author: 'User',
            displayLabel: 'User',
            timestamp: '',
            hasTimestamp: false,
            body: 'show files',
            subtitle: 'sent',
            kind: 'message',
            status: 'completed',
            processId: '',
            hasProcessId: false,
            command: '',
            output: '',
            deliveryState: 'delivered',
            isStreaming: false,
            isTool: false,
          ),
          bindings.AgentRuntimeChatEntry(
            id: 'tool-1',
            author: 'Tool',
            displayLabel: 'List files',
            timestamp: '',
            hasTimestamp: false,
            body: 'Read directory',
            subtitle: 'running',
            kind: 'tool',
            status: 'running',
            processId: 'proc-1',
            hasProcessId: true,
            command: 'ls',
            output: 'README.md',
            deliveryState: 'streaming',
            isStreaming: true,
            isTool: true,
          ),
        ],
      ),
      hasProjection: true,
      controllerState: bindings.AgentRuntimeControllerState(
        connectionState: 'streaming',
        selectedSessionId: 'session-live',
        hasSelectedSessionId: true,
        baseUrl: 'http://127.0.0.1:8765',
        lastError: '',
        hasLastError: false,
      ),
    ));

    expect(controller.data.selectedConversation.map((entry) => entry.body), contains('show files'));
    expect(controller.data.selectedConversation.map((entry) => entry.body), contains('Read directory'));
    expect(sentRequests, hasLength(1));
  });

  test('controller state output after send does not trigger Dart stream polling', () {
    final sentRequests = <String, bindings.AgentRuntimeRequest>{};
    final controller = AgentRuntimeWorkbenchController(
      requestSink: (requestId, request) {
        sentRequests[requestId] = request;
      },
    );
    addTearDown(controller.dispose);

    controller.sendMessage('session-live', 'show files');
    expect(sentRequests.values, hasLength(1));
    final sendRequestId = sentRequests.keys.single;

    controller.applyOutputForRequestForTest(sendRequestId, const bindings.AgentRuntimeOutputControllerState(
      controllerState: bindings.AgentRuntimeControllerState(
        connectionState: 'streaming',
        selectedSessionId: 'session-live',
        hasSelectedSessionId: true,
        baseUrl: 'http://127.0.0.1:8765',
        lastError: '',
        hasLastError: false,
      ),
    ));

    expect(sentRequests.values, hasLength(1));
  });

  test('Rust terminal stream output marks user tool and assistant entries completed without reconnect', () {
    final sentRequests = <bindings.AgentRuntimeRequest>[];
    final controller = AgentRuntimeWorkbenchController(
      requestSink: (requestId, request) {
        sentRequests.add(request);
      },
    );
    addTearDown(controller.dispose);

    controller.sendMessage('session-live', 'terminal state check');
    expect(sentRequests, hasLength(1));

    controller.applyOutputForTest(const bindings.AgentRuntimeOutputStreamOutcome(
      outcome: bindings.AgentRuntimeStreamOutcomeDeltaApplied(applyOutcome: 'selectedChatFinalize'),
      projection: bindings.AgentRuntimeProjectionSnapshot(
        watermark: 9,
        sessionCount: 1,
        timelineCount: 3,
        actionCount: 0,
        roleCount: 1,
        workflowMemoryCount: 0,
        selectedChatEntries: [
          bindings.AgentRuntimeChatEntry(
            id: 'turn-user-terminal',
            author: 'User',
            displayLabel: 'User',
            timestamp: '',
            hasTimestamp: false,
            body: 'terminal state check',
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
          ),
          bindings.AgentRuntimeChatEntry(
            id: 'tool-terminal',
            author: 'Tool',
            displayLabel: 'Tool',
            timestamp: '',
            hasTimestamp: false,
            body: 'tool result',
            subtitle: 'completed',
            kind: 'tool',
            status: 'completed',
            processId: 'proc-terminal',
            hasProcessId: true,
            command: 'execute_code',
            output: 'ok',
            deliveryState: 'delivered',
            isStreaming: false,
            isTool: true,
          ),
          bindings.AgentRuntimeChatEntry(
            id: 'assistant-terminal',
            author: 'Assistant',
            displayLabel: 'Assistant',
            timestamp: '',
            hasTimestamp: false,
            body: 'done',
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
          ),
        ],
      ),
      hasProjection: true,
      controllerState: bindings.AgentRuntimeControllerState(
        connectionState: 'streaming',
        selectedSessionId: 'session-live',
        hasSelectedSessionId: true,
        baseUrl: 'http://127.0.0.1:8765',
        lastError: '',
        hasLastError: false,
      ),
    ));

    expect(controller.data.selectedConversation.map((entry) => entry.author), containsAll(<String>['User', 'Tool', 'Assistant']));
    expect(controller.data.selectedConversation.every((entry) => entry.status == 'completed'), isTrue);
    expect(controller.data.selectedConversation.any((entry) => entry.isStreaming), isFalse);
    expect(controller.data.selectedConversation.length, 3);
    expect(sentRequests, hasLength(1));
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
    const registryDecision = AgentRuntimeCommandRegistryDecisionDraft(
      status: 'approved',
      scopeType: 'project',
      projectKey: 'project-a',
      policyDecision: 'allow',
      policyReason: 'Owner reviewed command registry request.',
      actionId: 'cmd.registry.audit',
      displayName: 'Registry audit',
      binaryName: 'rg',
      argvTemplate: ['--files'],
      defaultCwd: '.',
      cwdPolicy: 'project',
      envPolicy: 'inherit',
      stdinPolicy: 'deny',
      syncAllowed: true,
      asyncAllowed: false,
      maxRuntimeMs: 30000,
      endOfTurnBehavior: 'terminate',
      endOfSessionBehavior: 'terminate',
      mutationClass: 'readOnly',
      modelDescription: 'Search project files.',
      allowCwdArg: false,
      allowArgsArg: true,
      forbiddenArgs: ['--pcre2'],
      executionPolicy: 'allow',
    );

    controller.approveAction(approval, 'Looks safe');
    controller.denyAction(approval, 'Not safe');
    controller.resumeApproval(approval);
    controller.previewCommandRegistryRequest(registry, 'session-1', registryDecision);
    controller.approveCommandRegistryRequest(registry, 'session-1', registryDecision);
    controller.denyCommandRegistryRequest(registry, 'session-1', registryDecision);
    controller.applyCommandRegistryRequest(registry, 'session-1');

    final operations = sentRequests.cast<bindings.AgentRuntimeRequestDispatchOperation>().map((request) => request.operation).toList(growable: false);
    expect(operations[0], isA<bindings.AgentRuntimeGuiOperationDecideApproval>());
    expect((operations[0] as bindings.AgentRuntimeGuiOperationDecideApproval).decision, 'approved');
    expect((operations[0] as bindings.AgentRuntimeGuiOperationDecideApproval).reason, 'Looks safe');
    expect((operations[1] as bindings.AgentRuntimeGuiOperationDecideApproval).decision, 'denied');
    expect((operations[1] as bindings.AgentRuntimeGuiOperationDecideApproval).reason, 'Not safe');
    expect(operations[2], isA<bindings.AgentRuntimeGuiOperationResumeApproval>());
    expect(operations[3], isA<bindings.AgentRuntimeGuiOperationPreviewCommandRegistryRequest>());
    final approvedRegistry = (operations[4] as bindings.AgentRuntimeGuiOperationDecideCommandRegistryRequest).decision;
    expect(approvedRegistry.status, 'approved');
    expect(approvedRegistry.hasFinalScope, isTrue);
    expect(approvedRegistry.finalScope.projectKey, 'project-a');
    expect(approvedRegistry.finalExecutionPolicy.decision, 'allow');
    expect(approvedRegistry.hasFinalExecutionPolicy, isTrue);
    expect(approvedRegistry.hasFinalCommand, isTrue);
    expect(approvedRegistry.finalCommand.actionId, 'cmd.registry.audit');
    expect(approvedRegistry.finalCommand.binaryName, 'rg');
    expect(approvedRegistry.finalCommand.argvPrefix, ['--files']);
    expect(approvedRegistry.finalCommand.endOfTurnBehavior, 'terminate');
    expect(approvedRegistry.finalCommand.endOfSessionBehavior, 'terminate');
    expect((operations[5] as bindings.AgentRuntimeGuiOperationDecideCommandRegistryRequest).decision.status, 'denied');
    expect(operations[6], isA<bindings.AgentRuntimeGuiOperationApplyCommandRegistryRequest>());
  });

  test('approval decisions and resume remove approval actions after typed completion', () {
    final sent = <String, bindings.AgentRuntimeRequest>{};
    final controller = AgentRuntimeWorkbenchController(
      requestSink: (requestId, request) => sent[requestId] = request,
    );
    addTearDown(controller.dispose);
    const approve = AgentRuntimeActionItem(id: 'approval-1', title: 'Approve', subtitle: '', kind: 'approval', stateText: 'ready', tone: 'warning');
    const deny = AgentRuntimeActionItem(id: 'approval-1', title: 'Deny', subtitle: '', kind: 'approvalDeny', stateText: 'ready', tone: 'warning');
    const resume = AgentRuntimeActionItem(id: 'approval-1', title: 'Resume', subtitle: '', kind: 'approvalResume', stateText: 'ready', tone: 'info');
    final data = mockAgentRuntimeConnected.copyWith(
      actions: const [approve, deny, resume],
      operationSurfaces: const [
        AgentRuntimeOperationSurface(
          surfaceId: 'approvals',
          title: 'Approvals',
          subtitle: 'Pending approvals',
          rows: [],
          actions: [approve, deny, resume],
        ),
      ],
    );

    for (final run in [
      () => controller.approveAction(approve, 'owner approved'),
      () => controller.denyAction(deny, 'owner denied'),
      () => controller.resumeApproval(resume),
    ]) {
      sent.clear();
      controller.setViewDataForTest(data);
      run();
      final requestId = sent.keys.single;
      controller.applyOutputForRequestForTest(
        requestId,
        const bindings.AgentRuntimeOutputOperationResult(
          result: bindings.AgentRuntimeOperationResult(operation: 'approval', outcome: 'accepted', message: 'updated'),
        ),
      );
      expect(controller.data.actions.where((action) => action.id == 'approval-1'), isEmpty);
      expect(controller.data.operationSurfaces.single.actions.where((action) => action.id == 'approval-1'), isEmpty);
    }
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
    var globalCreates = 0;
    await tester.pumpWidget(
      MaterialApp(
        home: ConversationShellScreen(
          data: agentRuntimeConversationShellData(mockAgentRuntimeConnected),
          onSessionSelected: (_) {},
          onCreateSession: () => globalCreates += 1,
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
    await tester.tap(find.byTooltip('New session'));
    await tester.pump();
    expect(globalCreates, 1);

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
    await tester.tap(find.text('New session in project'));
    await tester.pumpAndSettle();
    expect(selectedActions, contains('new:project-a'));
  });

  testWidgets('all production new-session affordances route to Create Session intent paths', (tester) async {
    final routeIntents = <String>[];
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
          onCreateSession: () => routeIntents.add('global-create-session-modal'),
          onSendMessage: (_) {},
          onInterrupt: () {},
          onNewSessionInProject: (projectId) => routeIntents.add('project-create-session-modal:$projectId'),
          showPermanentDetail: false,
        ),
      ),
    );

    await tester.tap(find.byTooltip('New session'));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('conversationProject.menu.__all__')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('New session'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('conversationProject.menu.__unassigned__')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('New unassigned session'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('conversationProject.menu.project-a')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('New session in project'));
    await tester.pumpAndSettle();

    expect(routeIntents, [
      'global-create-session-modal',
      'project-create-session-modal:__all__',
      'project-create-session-modal:__unassigned__',
      'project-create-session-modal:project-a',
    ]);

    final sentRequests = <bindings.AgentRuntimeRequest>[];
    final controller = AgentRuntimeWorkbenchController(requestSink: (_, request) => sentRequests.add(request));
    addTearDown(controller.dispose);
    controller.createSessionFromDraft(
      role: 'runtime-allow',
      project: 'project-a',
      model: mockAgentRuntimeConnected.modelOptions.single.id,
      workdir: '/work/project-a',
      worktreeRoot: '/work/project-a',
      title: 'Project session',
      name: 'project-session',
    );
    final operation = (sentRequests.single as bindings.AgentRuntimeRequestDispatchOperation).operation;
    expect(operation, isA<bindings.AgentRuntimeGuiOperationCreateSession>());
    final create = operation as bindings.AgentRuntimeGuiOperationCreateSession;
    expect(create.role, 'runtime-allow');
    expect(create.project, 'project-a');
    expect(create.model, mockAgentRuntimeConnected.modelOptions.single.id);
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

  testWidgets('approval decisions require a visible reason and process input uses typed user text', (tester) async {
    final approvalReasons = <String>[];
    final processInputs = <String>[];
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: AgentRuntimeOperationsDetail(
            data: mockAgentRuntimeConnected,
            focusSurfaceId: 'approvals',
            onApprovalApprove: (action, reason) => approvalReasons.add('${action.id}:$reason'),
            onProcessInput: (handle, text) => processInputs.add('$handle:$text'),
          ),
        ),
      ),
    );

    await tester.ensureVisible(find.byKey(const ValueKey('agentRuntime.approval.reason.approval-1')));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Approve').first);
    await tester.pump();
    expect(find.text('Reason is required.'), findsOneWidget);
    expect(approvalReasons, isEmpty);

    await tester.enterText(find.byKey(const ValueKey('agentRuntime.approval.reason.approval-1')), 'Owner reviewed the request.');
    await tester.tap(find.widgetWithText(OutlinedButton, 'Approve').first);
    await tester.pump();
    expect(approvalReasons, contains('approval-1:Owner reviewed the request.'));

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: AgentRuntimeOperationsDetail(
            data: mockAgentRuntimeConnected,
            focusSurfaceId: 'processManager',
            onProcessInput: (handle, text) => processInputs.add('$handle:$text'),
          ),
        ),
      ),
    );
    await tester.ensureVisible(find.byKey(const ValueKey('agentRuntime.process.input.dev-server')));
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.process.input.dev-server')), 'status\\n');
    await tester.tap(find.widgetWithText(OutlinedButton, 'Send input'));
    await tester.pump();
    expect(processInputs, contains('dev-server:status\\n'));
  });

  testWidgets('process input is disabled with policy context when stdin is rejected', (tester) async {
    var dispatched = false;
    final processSurface = AgentRuntimeOperationSurface(
      surfaceId: 'processManager',
      title: 'Process Manager',
      subtitle: 'Managed process handles',
      rows: const [
        AgentRuntimeFact(label: 'batch-job', value: 'running · stdin rejected by policy'),
      ],
      actions: const [
        AgentRuntimeActionItem(
          id: 'batch-job',
          title: 'Send input',
          subtitle: 'Stdin is rejected for this process',
          kind: 'processInput',
          stateText: 'disabled: stdin rejected by policy',
          tone: 'warning',
        ),
      ],
    );
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: AgentRuntimeOperationsDetail(
            data: mockAgentRuntimeConnected.copyWith(
              operationSurfaces: [
                for (final surface in mockAgentRuntimeConnected.operationSurfaces)
                  if (surface.surfaceId == 'processManager') processSurface else surface,
              ],
            ),
            focusSurfaceId: 'processManager',
            onProcessInput: (handle, text) => dispatched = true,
          ),
        ),
      ),
    );

    expect(find.text('Stdin is disabled by process policy.'), findsOneWidget);
    final sendButton = tester.widget<OutlinedButton>(find.widgetWithText(OutlinedButton, 'Send input'));
    expect(sendButton.onPressed, isNull);
    expect(dispatched, false);
  });

  testWidgets('Workflow Memory section renders detail metadata and typed feedback actions', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1100, 1000));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final selections = <String>[];
    final feedback = <String>[];
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: AgentRuntimeOperationsDetail(
            data: mockAgentRuntimeConnected,
            focusSurfaceId: 'workflowMemory',
            onWorkflowMemorySelect: (row) => selections.add(row.id),
            onWorkflowMemoryAttempted: (detail) => feedback.add('${detail.id}:attempted'),
            onWorkflowMemoryHelpful: (detail) => feedback.add('${detail.id}:helpful'),
            onWorkflowMemoryNotHelpful: (detail) => feedback.add('${detail.id}:notHelpful'),
          ),
        ),
      ),
    );

    expect(find.text('Selected memory'), findsOneWidget);
    expect(find.text('Scope'), findsOneWidget);
    expect(find.text('Origin'), findsOneWidget);
    expect(find.text('Saved from a session'), findsOneWidget);
    expect(find.text('Source details'), findsOneWidget);
    expect(find.text('Available in Diagnostics'), findsOneWidget);
    expect(find.text('Source script'), findsNothing);
    expect(find.text('Source hash'), findsNothing);
    expect(find.text('Command fingerprint'), findsNothing);
    expect(find.text('Starlark source'), findsNothing);
    expect(find.text('session-1'), findsNothing);
    expect(find.text('script-run-1'), findsNothing);
    expect(find.text('Saved source'), findsNothing);
    expect(find.text('Command review'), findsNothing);
    expect(find.textContaining('saved_workflow.help'), findsNothing);
    expect(find.text('Recent events'), findsOneWidget);

    await tester.tap(find.text('Use saved output excerpts'));
    await tester.pump();
    expect(selections, contains('memory-2'));

    await tester.ensureVisible(find.widgetWithText(OutlinedButton, 'Attempted'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Attempted'));
    await tester.ensureVisible(find.widgetWithText(OutlinedButton, 'Helpful'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Helpful'));
    await tester.ensureVisible(find.widgetWithText(OutlinedButton, 'Not helpful'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Not helpful'));
    await tester.pump();
    expect(feedback, containsAll(['memory-1:attempted', 'memory-1:helpful', 'memory-1:notHelpful']));
  });

  testWidgets('Runtime Operations stays one detail surface with all required sections', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1400, 4000));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(
          body: AgentRuntimeOperationsDetail(data: mockAgentRuntimeConnected),
        ),
      ),
    );

    for (final section in [
      'History',
      'Diagnostics',
      'Statistics',
      'Compaction',
      'Process Manager',
      'Approvals',
      'Command Registry',
      'Role Admin',
      'Workflow Memory',
    ]) {
      expect(find.text(section), findsWidgets);
    }
    expect(find.byType(AgentRuntimeOperationsDetail), findsOneWidget);
  });

  testWidgets('Runtime Operations section controls dispatch typed callbacks from the detail surface', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1400, 4000));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final events = <String>[];

    Future<void> pumpOperations({String? focusSurfaceId, AgentRuntimeWorkbenchData? data}) async {
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: AgentRuntimeOperationsDetail(
              data: data ?? mockAgentRuntimeConnected,
              focusSurfaceId: focusSurfaceId,
              onRoleValidate: (_) => events.add('role.validate'),
              onRoleCreate: (_) => events.add('role.create'),
              onRoleUpdate: (_) => events.add('role.update'),
              onRoleExport: (_) => events.add('role.export'),
              onRoleArchive: (_) => events.add('role.archive'),
              onRoleUnarchive: (_) => events.add('role.unarchive'),
              onRoleActivate: (_, _) => events.add('role.activate'),
              onRoleShowDetail: (_) => events.add('role.detail'),
              onRoleShowVersions: (_) => events.add('role.versions'),
              onRoleShowVersionData: (_) => events.add('role.versionData'),
              onWorkflowMemorySelect: (_) => events.add('memory.select'),
              onWorkflowMemoryAttempted: (_) => events.add('memory.attempted'),
              onWorkflowMemoryHelpful: (_) => events.add('memory.helpful'),
              onWorkflowMemoryNotHelpful: (_) => events.add('memory.notHelpful'),
              onSessionClose: (_) => events.add('session.close'),
              onSessionArchive: (_) => events.add('session.archive'),
              onSessionFork: (_) => events.add('session.fork'),
              onProcessTerminate: (_) => events.add('process.terminate'),
              onProcessInput: (_, _) => events.add('process.input'),
              onProcessFlush: (_) => events.add('process.flush'),
              onCompactSession: (_) => events.add('compaction.compact'),
              onApprovalApprove: (_, _) => events.add('approval.approve'),
              onApprovalDeny: (_, _) => events.add('approval.deny'),
              onApprovalResume: (_) => events.add('approval.resume'),
              onCommandRegistryApprove: (_, _) => events.add('command.approve'),
              onCommandRegistryDeny: (_, _) => events.add('command.deny'),
              onCommandRegistryPreview: (_, _) => events.add('command.preview'),
              onCommandRegistryApply: (_) => events.add('command.apply'),
              onCommandRegistryReview: (_) => events.add('command.review'),
              onCommandRegistryShowCommand: (_, _, _) => events.add('command.show'),
              onCommandRegistryListInstalled: (_, _) => events.add('command.listInstalled'),
              onCommandRegistryListRequests: () => events.add('command.listRequests'),
            ),
          ),
        ),
      );
    }

    await pumpOperations(focusSurfaceId: 'session');
    await tester.tap(find.widgetWithText(OutlinedButton, 'Close session'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Archive session'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Fork session'));

    await pumpOperations(focusSurfaceId: 'processManager');
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.process.input.dev-server')), 'status');
    await tester.tap(find.widgetWithText(OutlinedButton, 'Send input'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Flush output'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Terminate process'));

    const compact = AgentRuntimeActionItem(id: 'session-1', title: 'Compact selected session', subtitle: '', kind: 'compactionManual', stateText: 'ready', tone: 'warning');
    await pumpOperations(
      focusSurfaceId: 'compaction',
      data: mockAgentRuntimeConnected.copyWith(
        operationSurfaces: const [
          AgentRuntimeOperationSurface(
            surfaceId: 'compaction',
            title: 'Compaction',
            subtitle: 'Checkpoint and context budget',
            rows: [],
            actions: [compact],
          ),
        ],
      ),
    );
    await tester.tap(find.widgetWithText(OutlinedButton, 'Compact selected session'));

    const approve = AgentRuntimeActionItem(id: 'approval-1', title: 'Approve', subtitle: '', kind: 'approval', stateText: 'ready', tone: 'info');
    const deny = AgentRuntimeActionItem(id: 'approval-1', title: 'Deny', subtitle: '', kind: 'approvalDeny', stateText: 'ready', tone: 'warning');
    const resume = AgentRuntimeActionItem(id: 'approval-1', title: 'Resume', subtitle: '', kind: 'approvalResume', stateText: 'ready', tone: 'info');
    await pumpOperations(
      focusSurfaceId: 'approvals',
      data: mockAgentRuntimeConnected.copyWith(
        operationSurfaces: const [
          AgentRuntimeOperationSurface(
            surfaceId: 'approvals',
            title: 'Approvals',
            subtitle: 'Pending approvals',
            rows: [],
            actions: [approve, deny, resume],
          ),
        ],
      ),
    );
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.approval.reason.approval-1')).first, 'owner approved');
    await tester.tap(find.widgetWithText(OutlinedButton, 'Approve'));
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.approval.reason.approval-1')).last, 'owner denied');
    await tester.tap(find.widgetWithText(OutlinedButton, 'Deny'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Resume'));

    const review = AgentRuntimeActionItem(id: 'registry-request-1', title: 'Review', subtitle: '', kind: 'commandRegistryReview', stateText: 'ready', tone: 'info');
    const show = AgentRuntimeActionItem(id: 'cmd.registry.audit', title: 'Show installed command', subtitle: '', kind: 'commandRegistryShow', stateText: 'enabled', tone: 'info');
    const preview = AgentRuntimeActionItem(id: 'registry-request-1', title: 'Preview Decision', subtitle: '', kind: 'commandRegistryPreview', stateText: 'ready', tone: 'info');
    const request = AgentRuntimeActionItem(id: 'registry-request-1', title: 'Approve command', subtitle: '', kind: 'commandRegistryRequest', stateText: 'ready', tone: 'warning');
    const commandDeny = AgentRuntimeActionItem(id: 'registry-request-1', title: 'Deny command', subtitle: '', kind: 'commandRegistryDeny', stateText: 'ready', tone: 'warning');
    const apply = AgentRuntimeActionItem(id: 'registry-request-1', title: 'Apply', subtitle: '', kind: 'commandRegistryApply', stateText: 'ready', tone: 'info');
    await pumpOperations(
      focusSurfaceId: 'commandRegistry',
      data: mockAgentRuntimeConnected.copyWith(
        operationSurfaces: const [
          AgentRuntimeOperationSurface(
            surfaceId: 'commandRegistry',
            title: 'Command Registry',
            subtitle: 'Pending command requests',
            rows: [],
            actions: [show, review, preview, request, commandDeny, apply],
          ),
        ],
      ),
    );
    await tester.tap(find.widgetWithText(OutlinedButton, 'Refresh installed commands'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Refresh pending requests'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Show installed command'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Review'));
    expect(find.text('Final template editor'), findsWidgets);
    expect(find.text('Decision'), findsWidgets);
    expect(find.text('Scope'), findsWidgets);
    expect(find.text('Execution policy'), findsWidgets);
    expect(find.text('Action id'), findsWidgets);
    expect(find.text('Binary'), findsWidgets);
    expect(find.text('Cwd policy'), findsWidgets);
    expect(find.text('Env policy'), findsWidgets);
    expect(find.text('Stdin policy'), findsWidgets);
    expect(find.text('End of turn'), findsWidgets);
    expect(find.text('End of session'), findsWidgets);
    expect(find.text('Mutation class'), findsWidgets);
    await tester.tap(find.widgetWithText(OutlinedButton, 'Preview Decision').first);
    await tester.tap(find.widgetWithText(OutlinedButton, 'Approve').first);
    await tester.tap(find.widgetWithText(OutlinedButton, 'Deny').first);
    await tester.tap(find.widgetWithText(OutlinedButton, 'Apply'));

    await pumpOperations(focusSurfaceId: 'roleAdmin');
    await tester.tap(find.widgetWithText(OutlinedButton, 'Show role detail'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Show versions'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Show current version data'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Show data').first);
    await tester.tap(find.widgetWithText(OutlinedButton, 'Activate').last);

    await pumpOperations(focusSurfaceId: 'workflowMemory');
    await tester.tap(find.text('Use saved output excerpts'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Attempted'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Helpful'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Not helpful'));

    expect(
      events,
      containsAll([
        'session.close',
        'session.archive',
        'session.fork',
        'process.input',
        'process.flush',
        'process.terminate',
        'compaction.compact',
        'approval.approve',
        'approval.deny',
        'approval.resume',
        'command.listInstalled',
        'command.listRequests',
        'command.show',
        'command.review',
        'command.preview',
        'command.approve',
        'command.deny',
        'command.apply',
        'role.detail',
        'role.versions',
        'role.versionData',
        'role.activate',
        'memory.select',
        'memory.attempted',
        'memory.helpful',
        'memory.notHelpful',
      ]),
    );
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

    await tester.enterText(find.byKey(const ValueKey('agentRuntime.createProject.key')), 'bad key with spaces');
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.createProject.displayName')), 'Bad Project');
    await tester.tap(find.text('Create'));
    await tester.pump();
    expect(submitted, isNull);
    expect(find.text('Project key must use letters, numbers, dot, dash, or underscore.'), findsOneWidget);

    await tester.enterText(find.byKey(const ValueKey('agentRuntime.createProject.key')), 'existing-project');
    await tester.tap(find.text('Create'));
    await tester.pump();
    expect(submitted, isNull);
    expect(find.text('Project key already exists.'), findsOneWidget);

    await tester.enterText(find.byKey(const ValueKey('agentRuntime.createProject.key')), 'project.validation');
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.createProject.displayName')), 'Validation Project');
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.createProject.workdir')), '/work/validation');
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.createProject.worktreeRoot')), '/work/validation/root');
    await tester.tap(find.text('Create'));
    await tester.pump();

    expect(submitted, isNotNull);
    expect(submitted!['projectKey'], 'project.validation');
    expect(submitted!['displayName'], 'Validation Project');
    expect(submitted!['defaultWorkdir'], '/work/validation');
    expect(submitted!['defaultWorktreeRoot'], '/work/validation/root');
    expect(submitted!['defaultRoleId'], isNot(''));
    expect(submitted!['defaultModel'], mockAgentRuntimeConnected.modelOptions.single.id);
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
    const canonicalProject = ConversationProject(
      id: 'project-a',
      title: 'Canonical Project',
      defaultWorkdir: '/canonical/workdir',
      defaultWorktreeRoot: '/canonical/worktree',
      defaultRoleId: 'runtime-allow',
      defaultModel: 'codex-live-model',
      tracked: false,
      listed: true,
    );
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: AgentRuntimeProjectSettingsDialog(
            data: mockAgentRuntimeConnected,
            projectId: 'project-a',
            project: canonicalProject,
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
    expect(find.widgetWithText(TextField, 'Project key'), findsNothing);
    expect(find.byKey(const ValueKey('agentRuntime.projectSettings.model')), findsOneWidget);
    expect(find.widgetWithText(TextField, 'Canonical Project'), findsOneWidget);
    expect(find.widgetWithText(TextField, '/canonical/workdir'), findsOneWidget);
    expect(find.widgetWithText(TextField, '/canonical/worktree'), findsOneWidget);
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.projectSettings.displayName')), 'Project Alpha');
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.projectSettings.workdir')), '/work/alpha');
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.projectSettings.worktreeRoot')), '/work/alpha/root');
    tester.widget<TextButton>(find.widgetWithText(TextButton, 'Archive')).onPressed!();
    await tester.pumpAndSettle();
    expect(actions, contains('archive:project-a'));

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: AgentRuntimeProjectSettingsDialog(
            data: mockAgentRuntimeConnected,
            projectId: 'project-a',
            project: canonicalProject,
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
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.projectSettings.displayName')), 'Project Alpha');
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.projectSettings.workdir')), '/work/alpha');
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.projectSettings.worktreeRoot')), '/work/alpha/root');
    tester.widget<TextButton>(find.widgetWithText(TextButton, 'Unarchive')).onPressed!();
    await tester.pumpAndSettle();
    expect(actions, contains('unarchive:project-a'));

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: AgentRuntimeProjectSettingsDialog(
            data: mockAgentRuntimeConnected,
            projectId: 'project-a',
            project: canonicalProject,
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
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.projectSettings.displayName')), 'Project Alpha');
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.projectSettings.workdir')), '/work/alpha');
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.projectSettings.worktreeRoot')), '/work/alpha/root');
    await tester.ensureVisible(find.widgetWithText(FilledButton, 'Save'));
    await tester.tap(find.widgetWithText(FilledButton, 'Save'));
    await tester.pumpAndSettle();

    expect(saved!['projectKey'], 'project-a');
    expect(saved!['displayName'], 'Project Alpha');
    expect(saved!['defaultWorkdir'], '/work/alpha');
    expect(saved!['defaultWorktreeRoot'], '/work/alpha/root');
    expect(saved!['defaultRoleId'], isNot(''));
    expect(saved!['defaultModel'], mockAgentRuntimeConnected.modelOptions.single.id);
  });

  testWidgets('Project Settings requires canonical project data instead of synthesized defaults', (tester) async {
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
              };
            },
            onArchive: (_) {},
            onUnarchive: (_) {},
          ),
        ),
      ),
    );

    expect(find.textContaining('Project settings are unavailable'), findsOneWidget);
    expect(find.widgetWithText(TextField, 'project-a'), findsNothing);
    expect(find.widgetWithText(TextField, '.'), findsNothing);
    expect(tester.widget<FilledButton>(find.widgetWithText(FilledButton, 'Save')).onPressed, isNull);
    expect(tester.widget<TextButton>(find.widgetWithText(TextButton, 'Archive')).onPressed, isNull);
    expect(tester.widget<TextButton>(find.widgetWithText(TextButton, 'Unarchive')).onPressed, isNull);
    expect(saved, isNull);
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
    expect(find.widgetWithText(TextField, 'Session'), findsNothing);
    expect(find.widgetWithText(TextField, 'Name'), findsNothing);
    expect(find.byKey(const ValueKey('agentRuntime.sessionSettings.model')), findsOneWidget);
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.sessionSettings.title')), 'Updated Session');
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.sessionSettings.workdir')), '/work/session');
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.sessionSettings.worktreeRoot')), '/work/session/root');
    await tester.tap(find.text('Save'));
    await tester.pumpAndSettle();
    expect(saved!['sessionId'], 'session-a');
    expect(saved!['title'], 'Updated Session');
    expect(saved!['name'], 'session-alpha');
    expect(saved!['model'], mockAgentRuntimeConnected.modelOptions.single.id);
    expect(saved!['workdir'], '/work/session');
    expect(saved!['worktreeRoot'], '/work/session/root');
    expect(saved!['project'], 'project-a');
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

    controller.listCommandRegistry('session-1', 'project-a');
    controller.showCommand(const AgentRuntimeActionItem(id: 'cmd.registry.audit', title: 'Show command', subtitle: '', kind: 'commandRegistryShow', stateText: 'ready', tone: 'info'), 'session-1', 'project-a');
    controller.listCommandRegistryRequests();
    controller.showCommandRegistryRequest(const AgentRuntimeActionItem(id: 'registry-request-1', title: 'Review', subtitle: '', kind: 'commandRegistryReview', stateText: 'ready', tone: 'info'));
    controller.compactSession(const AgentRuntimeActionItem(id: 'session-1', title: 'Compact selected session', subtitle: '', kind: 'compactionManual', stateText: 'ready', tone: 'warning'));

    controller.validateRoleDraft(draft);
    controller.createRoleFromDraft(draft);
    controller.updateRoleFromDraft(draft);
    controller.showRoleDetail('runtime-allow');
    controller.listRoleVersions('runtime-allow');
    controller.showRoleVersion('role-version-1');
    controller.exportRole('runtime-allow');
    controller.archiveRole('runtime-allow');
    controller.unarchiveRole('runtime-allow');
    controller.activateRoleVersion('runtime-allow', 'role-version-1');

    expect(sentRequests, hasLength(15));
    expect((sentRequests[0] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationListCommandRegistry>());
    expect((sentRequests[1] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationShowCommand>());
    expect((sentRequests[2] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationListCommandRegistryRequests>());
    expect((sentRequests[3] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationShowCommandRegistryRequest>());
    expect((sentRequests[4] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationCompactSession>());
    expect((sentRequests[5] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationValidateRoleDraft>());
    expect((sentRequests[6] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationCreateRoleFromDraft>());
    final update = (sentRequests[7] as bindings.AgentRuntimeRequestDispatchOperation).operation as bindings.AgentRuntimeGuiOperationUpdateRoleFromDraft;
    expect(update.roleId, 'runtime-allow');
    expect((sentRequests[8] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationShowRoleDetail>());
    expect((sentRequests[9] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationListRoleVersions>());
    expect((sentRequests[10] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationShowRoleVersion>());
    expect((sentRequests[11] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationExportRole>());
    expect((sentRequests[12] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationArchiveRole>());
    expect((sentRequests[13] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationUnarchiveRole>());
    final activate = (sentRequests[14] as bindings.AgentRuntimeRequestDispatchOperation).operation as bindings.AgentRuntimeGuiOperationActivateRoleVersion;
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
    const registryDecision = AgentRuntimeCommandRegistryDecisionDraft(
      status: 'approved',
      scopeType: 'project',
      projectKey: 'project-a',
      policyDecision: 'allow',
      policyReason: 'Owner reviewed command registry request.',
      actionId: 'cmd.registry.audit',
      displayName: 'Registry audit',
      binaryName: 'rg',
      argvTemplate: ['--files'],
      defaultCwd: '.',
      cwdPolicy: 'project',
      envPolicy: 'inherit',
      stdinPolicy: 'deny',
      syncAllowed: true,
      asyncAllowed: false,
      maxRuntimeMs: 30000,
      endOfTurnBehavior: 'terminate',
      endOfSessionBehavior: 'terminate',
      mutationClass: 'readOnly',
      modelDescription: 'Search project files.',
      allowCwdArg: false,
      allowArgsArg: true,
      forbiddenArgs: ['--pcre2'],
      executionPolicy: 'allow',
    );

    controller.approveAction(approval, 'Proceed');
    controller.resumeApproval(approval);
    controller.approveCommandRegistryRequest(request, 'session-2', registryDecision);
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
