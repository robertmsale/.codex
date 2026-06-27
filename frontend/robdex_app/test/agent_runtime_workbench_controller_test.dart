import 'dart:ui' show SemanticsFlag;

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

  test('requirements review operations map to typed Rust envelopes', () {
    final requirement = bindings.AgentRuntimeRequirementInput(
      key: 'prove_contract',
      statement: 'Prove the contract.',
      severity: 'must',
      verificationMethod: '{"method":"test"}',
    );
    final set = agentRuntimeSetRequirementsOperationForTest('session-1', [requirement], title: 'Contract');
    final clear = agentRuntimeClearRequirementsOperationForTest('session-1');
    final status = agentRuntimeRequirementsStatusOperationForTest('session-1');
    final packets = agentRuntimeRequirementsPacketsOperationForTest('session-1');

    expect(set, isA<bindings.AgentRuntimeGuiOperationSetRequirements>());
    final typedSet = set as bindings.AgentRuntimeGuiOperationSetRequirements;
    expect(typedSet.sessionId, 'session-1');
    expect(typedSet.title, 'Contract');
    expect(typedSet.requirements.single.key, 'prove_contract');
    expect(clear, isA<bindings.AgentRuntimeGuiOperationClearRequirements>());
    expect(status, isA<bindings.AgentRuntimeGuiOperationShowRequirementsStatus>());
    expect(packets, isA<bindings.AgentRuntimeGuiOperationListRequirementsPackets>());
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
    expect(controller.data.connectionState, 'connecting');
    expect(controller.data.statusLabel, 'Connecting to runtime');
    expect(controller.data.errorMessage, isNull);
  });

  test('manual connect failure displays actionable inline error', () {
    late String requestId;
    final controller = AgentRuntimeWorkbenchController(
      requestSink: (id, request) {
        requestId = id;
      },
    );
    addTearDown(controller.dispose);

    controller.connect('http://127.0.0.1:8765');
    controller.applyOutputForRequestForTest(
      requestId,
      const bindings.AgentRuntimeOutputError(
        error: bindings.AgentRuntimeApiError(
          code: 'unavailable',
          message: 'Runtime did not respond. Check the service, then refresh discovery.',
          details: [],
        ),
      ),
    );

    expect(controller.data.errorMessage, contains('Runtime did not respond'));
    expect(controller.data.pendingRequestCount, 0);
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
      result: bindings.AgentRuntimeOperationResult(operation: 'CreateSession', outcome: 'accepted', message: 'created', valueJson: '', hasValueJson: false),
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
        valueJson: '',
        hasValueJson: false,
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
        ConversationProject(
          id: 'project-a',
          title: 'Project A',
          defaultWorkdir: '/work/project-a',
          defaultWorktreeRoot: '/work/project-a/root',
          defaultRoleId: 'runtime-safe-builder',
          defaultModel: 'codex-live-model',
        ),
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
            statusLabel: 'Stopped',
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
            toolCallId: 'tool-call-terminal',
            hasToolCallId: true,
            scriptRunId: 'script-run-terminal',
            hasScriptRunId: true,
            stdoutArtifactId: 'stdout-terminal',
            hasStdoutArtifactId: true,
            stderrArtifactId: 'stderr-terminal',
            hasStderrArtifactId: true,
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
    final toolEntry = controller.data.selectedConversation.firstWhere((entry) => entry.isTool);
    expect(toolEntry.toolCallId, 'tool-call-terminal');
    expect(toolEntry.scriptRunId, 'script-run-terminal');
    expect(toolEntry.stdoutArtifactId, 'stdout-terminal');
    expect(toolEntry.stderrArtifactId, 'stderr-terminal');
    expect(controller.data.selectedConversation.every((entry) => entry.status == 'completed'), isTrue);
    expect(controller.data.selectedConversation.any((entry) => entry.isStreaming), isFalse);
    expect(controller.data.selectedConversation.length, 3);
    expect(sentRequests, hasLength(1));
  });

  test('project CRUD actions send generated typed operation intents without project visibility payload', () {
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
    );
    controller.updateProject(
      projectKey: 'project-a',
      displayName: 'Project Alpha',
      defaultWorkdir: '/work/alpha',
      defaultWorktreeRoot: '/work/alpha',
      defaultRoleId: 'runtime-allow',
      defaultModel: 'gpt-5.5',
    );
    controller.archiveProject('project-a');

    expect(sentRequests, hasLength(3));
    expect((sentRequests[0] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationCreateProject>());
    final update = (sentRequests[1] as bindings.AgentRuntimeRequestDispatchOperation).operation as bindings.AgentRuntimeGuiOperationUpdateProject;
    expect(update.projectKey, 'project-a');
    expect(update.defaultModel, 'gpt-5.5');
    expect(update.toString(), isNot(contains('tracked')));
    expect(update.toString(), isNot(contains('listed')));
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
          result: bindings.AgentRuntimeOperationResult(operation: 'approval', outcome: 'accepted', message: 'updated', valueJson: '', hasValueJson: false),
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
      imagePreviewBase64: '',
      hasImagePreviewBase64: false,
      imagePreviewContentType: '',
      hasImagePreviewContentType: false,
      imagePreviewError: '',
      hasImagePreviewError: false,
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
      imagePreviewBase64: '',
      hasImagePreviewBase64: false,
      imagePreviewContentType: '',
      hasImagePreviewContentType: false,
      imagePreviewError: '',
      hasImagePreviewError: false,
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

  test('agent runtime RFC3339 timestamp maps to epoch seconds for local wall-clock labels', () {
    final localWallClock = DateTime(2026, 6, 25, 12, 34);
    final timestamp = localWallClock.toUtc().toIso8601String();
    final entry = agentRuntimeChatEntryToChatEntryForTest(bindings.AgentRuntimeChatEntry(
      id: 'turn:1:assistant',
      author: 'Assistant',
      displayLabel: 'Assistant',
      timestamp: timestamp,
      hasTimestamp: true,
      body: 'timestamp check',
      subtitle: 'completed',
      kind: 'message',
      status: 'completed',
      processId: '',
      hasProcessId: false,
      command: '',
      output: '',
      imagePreviewBase64: '',
      hasImagePreviewBase64: false,
      imagePreviewContentType: '',
      hasImagePreviewContentType: false,
      imagePreviewError: '',
      hasImagePreviewError: false,
      deliveryState: 'delivered',
      isStreaming: false,
      isTool: false,
    ));

    final expectedEpochSeconds = localWallClock.millisecondsSinceEpoch ~/ 1000;
    expect(entry.timestamp, expectedEpochSeconds);
    expect(entry.timestamp, isNot(localWallClock.millisecondsSinceEpoch));
    expect(formatLocalTimeLabel(entry.timestamp), '12:34 PM');
  });

  test('agent runtime image artifacts map to shared chat image fields and full-size typed loading', () async {
    final sentRequests = <String, bindings.AgentRuntimeRequest>{};
    final controller = AgentRuntimeWorkbenchController(
      requestSink: (requestId, request) {
        sentRequests[requestId] = request;
      },
    );
    addTearDown(controller.dispose);

    final entry = agentRuntimeChatEntryToChatEntryForTest(const bindings.AgentRuntimeChatEntry(
      id: 'imageArtifact:image-1',
      author: 'Runtime',
      displayLabel: 'Image',
      timestamp: '2026-06-25T12:00:00Z',
      hasTimestamp: true,
      body: 'Screenshot evidence',
      subtitle: 'image/png · 1 × 1 · 68 bytes',
      kind: 'imageView',
      status: 'stored',
      processId: '',
      hasProcessId: false,
      command: '',
      output: 'agent-runtime-image://session-1/image-1',
      imagePreviewBase64: 'iVBORw0KGgo=',
      hasImagePreviewBase64: true,
      imagePreviewContentType: 'image/png',
      hasImagePreviewContentType: true,
      imagePreviewError: '',
      hasImagePreviewError: false,
      deliveryState: 'stored',
      isStreaming: false,
      isTool: false,
    ));
    expect(entry.kind, 'imageView');
    expect(entry.output, 'agent-runtime-image://session-1/image-1');
    expect(entry.imagePreviewBase64, 'iVBORw0KGgo=');
    expect(entry.imagePreviewContentType, 'image/png');

    final load = controller.loadFullSizeImage('agent-runtime-image://session-1/image-1');
    expect(sentRequests, hasLength(1));
    final requestId = sentRequests.keys.single;
    final request = sentRequests.values.single;
    expect(request, isA<bindings.AgentRuntimeRequestDispatchOperation>());
    final operation = (request as bindings.AgentRuntimeRequestDispatchOperation).operation;
    expect(operation, isA<bindings.AgentRuntimeGuiOperationLoadFullSizeImage>());
    final typed = operation as bindings.AgentRuntimeGuiOperationLoadFullSizeImage;
    expect(typed.sessionId, 'session-1');
    expect(typed.imageArtifactId, 'image-1');

    controller.applyOutputForRequestForTest(
      requestId,
      const bindings.AgentRuntimeOutputOperationResult(
        result: bindings.AgentRuntimeOperationResult(
          operation: 'LoadFullSizeImage',
          outcome: 'directValue',
          message: 'loaded',
          valueJson: '{"path":"agent-runtime-image://session-1/image-1","bytesBase64":"iVBORw0KGgo=","contentType":"image/png"}',
          hasValueJson: true,
        ),
      ),
    );

    final full = await load;
    expect(full.path, 'agent-runtime-image://session-1/image-1');
    expect(full.bytesBase64, 'iVBORw0KGgo=');
    expect(full.contentType, 'image/png');
  });

  testWidgets('agent runtime conversation renders image preview and opens shared full-size viewer', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1366, 1024));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    var loadedPath = '';
    await tester.pumpWidget(
      MaterialApp(
        home: ConversationShellScreen(
          data: agentRuntimeConversationShellData(mockAgentRuntimeConnected),
          onSessionSelected: (_) {},
          onCreateSession: () {},
          onSendMessage: (_) {},
          onInterrupt: () {},
          loadFullSizeImage: (path) async {
            loadedPath = path;
            return FullSizeImageData(
              path: path,
              bytesBase64:
                  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/p9sAAAAASUVORK5CYII=',
              contentType: 'image/png',
            );
          },
        ),
      ),
    );

    expect(find.byType(Image), findsWidgets);
    await tester.tap(find.byTooltip('Open full size image'));
    await tester.pumpAndSettle();

    expect(loadedPath, 'agent-runtime-image://session-a/image-artifact-1');
    expect(find.byTooltip('Close image'), findsOneWidget);
    expect(find.byType(InteractiveViewer), findsOneWidget);
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
            key: const ValueKey('agentRuntime.toolbar.operations'),
            tooltip: 'Runtime operations',
            onPressed: () {
              showModalBottomSheet<void>(
                context: tester.element(find.byKey(const ValueKey('agentRuntime.toolbar.operations'))),
                builder: (_) => const AgentRuntimeOperationsDetail(data: mockAgentRuntimeConnected, focusSurfaceId: 'compaction'),
              );
            },
            icon: const Icon(Icons.manage_history_rounded),
          ),
        ),
      ),
    );

    expect(find.byKey(const ValueKey('conversationShell.center')), findsOneWidget);
    expect(find.text('Compaction'), findsNothing);
    expect(find.text('More'), findsNothing);
    expect(find.text('New'), findsNothing);
    expect(find.text('Details'), findsNothing);
    expect(find.text('Compact selected session'), findsNothing);
    await tester.tap(find.byTooltip('New session'));
    await tester.pump();
    expect(globalCreates, 1);

    await tester.tap(find.byKey(const ValueKey('agentRuntime.toolbar.operations')));
    await tester.pumpAndSettle();
    expect(find.text('Compaction'), findsWidgets);
    expect(find.text('Compact selected session'), findsWidgets);
  });

  testWidgets('runtime operations modal closes through visible control and restores shell interactivity', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1400, 1600));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final controller = AgentRuntimeWorkbenchController(requestSink: (_, _) {});
    addTearDown(controller.dispose);
    controller.setViewDataForTest(mockAgentRuntimeConnected, shell: agentRuntimeConversationShellData(mockAgentRuntimeConnected));

    await tester.pumpWidget(MaterialApp(home: AgentRuntimeWorkbenchHost(controller: controller)));
    await tester.pump();

    await tester.tap(find.byKey(const ValueKey('agentRuntime.toolbar.runtimeOperations')));
    await tester.pumpAndSettle();

    expect(find.text('Runtime detail'), findsOneWidget);
    expect(find.text('Process Manager'), findsWidgets);
    expect(find.byKey(const ValueKey('agentRuntime.operationsDetail.close')), findsOneWidget);
    expect(find.byTooltip('Close runtime operations'), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('agentRuntime.operationsDetail.close')));
    await tester.pumpAndSettle();

    expect(find.text('Runtime detail'), findsNothing);
    await tester.tap(find.byTooltip('New session'));
    await tester.pump();
    expect(find.byType(AgentRuntimeCreateSessionDialog), findsOneWidget);
  });

  testWidgets('Agent Runtime controls keep usable semantics after reset-state render and modal dismissal', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1400, 1600));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(MaterialApp(
      home: Scaffold(body: AgentRuntimeWorkbench(
        data: mockAgentRuntimeEmpty.copyWith(
          connectionState: 'disconnected',
          connectionTone: 'muted',
          statusLabel: 'Not connected',
          discovery: const AgentRuntimeDiscoveryInfo(
            state: 'notLoaded',
            tone: 'muted',
            title: 'Discovery not loaded',
            message: 'Refresh discovery to check the local Agent Runtime service.',
            discoveryPath: '',
            connectable: false,
          ),
        ),
        baseUrlController: TextEditingController(text: 'http://127.0.0.1:42080'),
        onConnect: () {},
        onRefreshDiscovery: () {},
        onConnectDiscovered: () {},
        onRefreshIcloudRemoteDiscovery: () {},
        onConnectIcloudRemote: () {},
        onImportRemoteProfile: () {},
        onRefreshImportedRemoteProfile: () {},
        onConnectImportedRemoteProfile: () {},
        onDisconnect: () {},
      )),
    ));
    await tester.pump();

    expect(find.bySemanticsLabel('Agent Runtime workbench'), findsOneWidget);
    expect(find.bySemanticsLabel('Runtime URL'), findsWidgets);
    expect(find.bySemanticsLabel('Connect to URL'), findsOneWidget);
    expect(find.bySemanticsLabel('Refresh Local discovery'), findsOneWidget);
    expect(find.bySemanticsLabel('Refresh iCloud discovery'), findsOneWidget);
    expect(find.bySemanticsLabel('Refresh Imported discovery'), findsOneWidget);

    final controller = AgentRuntimeWorkbenchController(requestSink: (_, _) {});
    addTearDown(controller.dispose);
    controller.setViewDataForTest(mockAgentRuntimeConnected, shell: agentRuntimeConversationShellData(mockAgentRuntimeConnected));

    await tester.pumpWidget(MaterialApp(home: AgentRuntimeWorkbenchHost(controller: controller)));
    await tester.pump();

    expect(find.bySemanticsLabel('Runtime operations'), findsWidgets);
    expect(find.bySemanticsLabel('Session settings'), findsWidgets);

    await tester.tap(find.byKey(const ValueKey('agentRuntime.toolbar.runtimeOperations')));
    await tester.pumpAndSettle();
    expect(find.bySemanticsLabel('Runtime operations detail'), findsOneWidget);
    expect(find.bySemanticsLabel('Close runtime operations detail'), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('agentRuntime.operationsDetail.close')));
    await tester.pumpAndSettle();
    expect(find.bySemanticsLabel('Runtime operations'), findsWidgets);
    expect(find.bySemanticsLabel('Close runtime operations detail'), findsNothing);
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
        ConversationProject(
          id: 'project-a',
          title: 'Project A',
          defaultWorkdir: '/work/project-a',
          defaultWorktreeRoot: '/work/project-a/root',
          defaultRoleId: 'runtime-safe-builder',
          defaultModel: 'codex-live-model',
        ),
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
            statusLabel: 'stopped',
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
            statusLabel: 'stopped',
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
            statusLabel: 'stopped',
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
            statusLabel: 'stopped',
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
                  Text('Connection controls'),
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
    expect(find.text('Connection controls'), findsOneWidget);
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

  testWidgets('Connect to URL dispatches the visible Runtime URL field value', (tester) async {
    final urlController = TextEditingController(text: 'http://127.0.0.1:8765');
    addTearDown(urlController.dispose);
    var connectedUrl = '';
    await tester.pumpWidget(MaterialApp(
      home: Scaffold(body: AgentRuntimeWorkbench(
        data: mockAgentRuntimeEmpty.copyWith(
          connectionState: 'disconnected',
          connectionTone: 'muted',
          statusLabel: 'Not connected',
          discovery: const AgentRuntimeDiscoveryInfo(
            state: 'missing',
            tone: 'muted',
            title: 'No local runtime',
            message: 'Start the local runtime or connect manually.',
            discoveryPath: '',
            connectable: false,
          ),
          remoteDiscovery: mockAgentRuntimeIcloudRemoteStale,
          importedRemoteDiscovery: mockAgentRuntimeImportedRemoteStale,
        ),
        baseUrlController: urlController,
        onConnect: () => connectedUrl = urlController.text,
        onRefreshDiscovery: () {},
        onConnectDiscovered: () {},
        onRefreshIcloudRemoteDiscovery: () {},
        onConnectIcloudRemote: () {},
        onImportRemoteProfile: () {},
        onRefreshImportedRemoteProfile: () {},
        onConnectImportedRemoteProfile: () {},
        onDisconnect: () {},
      )),
    ));
    await tester.pump();

    await tester.enterText(find.bySemanticsLabel('Runtime URL').last, 'http://127.0.0.1:8765');
    await tester.tap(find.bySemanticsLabel('Connect to URL'));
    await tester.pump();

    expect(connectedUrl, 'http://127.0.0.1:8765');
  });

  testWidgets('Global Settings modal renders concrete controls, inline errors, and dispatches every action', (tester) async {
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

  testWidgets('Process Manager shows truthful zero state and reachable typed controls for process rows', (tester) async {
    final events = <String>[];
    const emptyProcessSurface = AgentRuntimeOperationSurface(
      surfaceId: 'processManager',
      title: 'Process Manager',
      subtitle: 'Managed process handles',
      rows: [],
      actions: [],
    );
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: AgentRuntimeOperationsDetail(
            data: mockAgentRuntimeConnected.copyWith(
              operationSurfaces: [
                for (final surface in mockAgentRuntimeConnected.operationSurfaces)
                  if (surface.surfaceId == 'processManager') emptyProcessSurface else surface,
              ],
            ),
            focusSurfaceId: 'processManager',
          ),
        ),
      ),
    );
    expect(find.text('No managed processes are running for this session. Start work or refresh runtime state.'), findsOneWidget);
    expect(find.text('Terminate process'), findsNothing);

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: AgentRuntimeOperationsDetail(
            data: mockAgentRuntimeConnected,
            focusSurfaceId: 'processManager',
            onProcessInput: (handle, text) => events.add('input:$handle:$text'),
            onProcessFlush: (handle) => events.add('flush:$handle'),
            onProcessTerminate: (handle) => events.add('terminate:$handle'),
          ),
        ),
      ),
    );
    await tester.ensureVisible(find.byKey(const ValueKey('agentRuntime.process.input.dev-server')));
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.process.input.dev-server')), 'status');
    await tester.tap(find.widgetWithText(OutlinedButton, 'Send input'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Flush output'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Terminate process'));
    await tester.pump();

    expect(events, containsAll(<String>['input:dev-server:status', 'flush:dev-server', 'terminate:dev-server']));
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
    expect(find.text('Stored in runtime audit history'), findsOneWidget);
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

  testWidgets('Workflow Memory shows truthful empty state and reachable typed feedback controls', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1100, 1000));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final feedback = <String>[];
    final baseMemory = mockAgentRuntimeConnected.workflowMemory;
    final emptyData = mockAgentRuntimeConnected.copyWith(
      workflowMemory: AgentRuntimeWorkflowMemoryData(
        title: baseMemory.title,
        subtitle: baseMemory.subtitle,
        emptyTitle: baseMemory.emptyTitle,
        emptyText: baseMemory.emptyText,
        rows: const [],
        recentEvents: const [],
        feedbackActions: const [],
      ),
      operationSurfaces: [
        for (final surface in mockAgentRuntimeConnected.operationSurfaces)
          if (surface.surfaceId == 'workflowMemory')
            const AgentRuntimeOperationSurface(
              surfaceId: 'workflowMemory',
              title: 'Workflow Memory',
              subtitle: 'Reusable workflow context',
              rows: [],
              actions: [],
            )
          else
            surface,
      ],
    );
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: AgentRuntimeOperationsDetail(
            data: emptyData,
            focusSurfaceId: 'workflowMemory',
          ),
        ),
      ),
    );

    expect(find.textContaining('No workflow memories'), findsOneWidget);
    expect(find.widgetWithText(OutlinedButton, 'Helpful'), findsNothing);

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: AgentRuntimeOperationsDetail(
            data: mockAgentRuntimeConnected,
            focusSurfaceId: 'workflowMemory',
            onWorkflowMemoryAttempted: (detail) => feedback.add('${detail.id}:attempted'),
            onWorkflowMemoryHelpful: (detail) => feedback.add('${detail.id}:helpful'),
            onWorkflowMemoryNotHelpful: (detail) => feedback.add('${detail.id}:notHelpful'),
          ),
        ),
      ),
    );
    await tester.ensureVisible(find.widgetWithText(OutlinedButton, 'Attempted'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Attempted'));
    await tester.ensureVisible(find.widgetWithText(OutlinedButton, 'Helpful'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Helpful'));
    await tester.ensureVisible(find.widgetWithText(OutlinedButton, 'Not helpful'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Not helpful'));
    await tester.pump();

    expect(feedback, containsAll(<String>['memory-1:attempted', 'memory-1:helpful', 'memory-1:notHelpful']));
  });

  testWidgets('Runtime Operations omits removed operation sheets and keeps active sections', (tester) async {
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
      'Compaction',
      'Process Manager',
      'Approvals',
      'Command Registry',
      'Role Admin',
      'Workflow Memory',
    ]) {
      expect(find.text(section), findsWidgets);
    }
    for (final removed in ['History', 'Diagnostics', 'Statistics', 'Image artifacts', 'Settings']) {
      expect(find.text(removed), findsNothing);
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
    await tester.tap(find.widgetWithText(OutlinedButton, 'Archive session').first);
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

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: AgentRuntimeRoleManagerPage(
            data: mockAgentRuntimeConnected.roleAdmin,
            onValidate: (_) => events.add('role.validate'),
            onCreate: (_) => events.add('role.create'),
            onUpdate: (_) => events.add('role.update'),
            onExport: (_) => events.add('role.export'),
            onArchive: (_) => events.add('role.archive'),
            onUnarchive: (_) => events.add('role.unarchive'),
            onActivate: (_, _) => events.add('role.activate'),
            onShowDetail: (_) => events.add('role.detail'),
            onShowVersions: (_) => events.add('role.versions'),
            onShowVersionData: (_) => events.add('role.versionData'),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(OutlinedButton, 'Validate Draft'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Save Version'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Export'));
    await tester.ensureVisible(find.widgetWithText(OutlinedButton, 'Show detail'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Show detail'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Show versions'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Show data').first);
    await tester.tap(find.widgetWithText(OutlinedButton, 'Activate').last);
    await tester.ensureVisible(find.widgetWithText(OutlinedButton, 'Archive'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Archive'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Unarchive'));

    await pumpOperations(focusSurfaceId: 'workflowMemory');
    await tester.tap(find.text('Use saved output excerpts'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Attempted'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Helpful'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Not helpful'));

    expect(
      events,
      containsAll([
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

  testWidgets('Create Project modal validates key and submits a fully populated typed draft without visibility controls', (tester) async {
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
            }) {
              submitted = {
                'projectKey': projectKey,
                'displayName': displayName,
                'defaultWorkdir': defaultWorkdir,
                'defaultWorktreeRoot': defaultWorktreeRoot,
                'defaultRoleId': defaultRoleId,
                'defaultModel': defaultModel,
              };
            },
          ),
        ),
      ),
    );

    await tester.enterText(find.byKey(const ValueKey('agentRuntime.createProject.key')), 'qa-runtime-20260627');
    tester.testTextInput.hide();
    await tester.pump();
    expect(find.byType(AgentRuntimeCreateProjectDialog), findsOneWidget);
    expect(find.text('qa-runtime-20260627'), findsOneWidget);

    await tester.enterText(find.byKey(const ValueKey('agentRuntime.createProject.key')), 'bad key with spaces');
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.createProject.displayName')), 'Bad Project');
    expect(find.text('Tracked'), findsNothing);
    expect(find.text('Listed'), findsNothing);
    expect(find.text('Visibility'), findsNothing);
    expect(find.text('Hidden'), findsNothing);
    expect(find.text('Invisible'), findsNothing);
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
        ConversationProject(
          id: 'project-a',
          title: 'Project A',
          defaultWorkdir: '/work/project-a',
          defaultWorktreeRoot: '/work/project-a/root',
          defaultRoleId: 'runtime-safe-builder',
          defaultModel: 'codex-live-model',
        ),
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
    ]) {
      expect(find.text(label), findsOneWidget);
    }
    for (final label in [
      'Title',
      'Generated session name',
      'Workdir',
      'Worktree root',
    ]) {
      expect(find.text(label), findsWidgets);
    }
    expect(find.byKey(const ValueKey('agentRuntime.createSession.model')), findsOneWidget);
    expect(find.text('Codex live model'), findsOneWidget);
    expect(find.text('Runtime Safe Builder'), findsOneWidget);
    expect(find.text('/work/project-a'), findsOneWidget);
    expect(find.text('/work/project-a/root'), findsOneWidget);

    await tester.enterText(find.byKey(const ValueKey('agentRuntime.createSession.title')), 'QA runtime smoke 20260627');
    tester.testTextInput.hide();
    await tester.pump();
    expect(find.byType(AgentRuntimeCreateSessionDialog), findsOneWidget);
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.createSession.workdir')), '/work/live');
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.createSession.worktreeRoot')), '/work/live/root');
    expect(find.text('qa-runtime-smoke-20260627'), findsOneWidget);
    await tester.tap(find.widgetWithText(FilledButton, 'Create'));
    await tester.pumpAndSettle();

    expect(submitted, isNotNull);
    expect(submitted!['project'], 'project-a');
    expect(submitted!['model'], mockAgentRuntimeConnected.modelOptions.single.id);
    expect(submitted!['title'], 'QA runtime smoke 20260627');
    expect(submitted!['name'], 'qa-runtime-smoke-20260627');
    expect(submitted!['workdir'], '/work/live');
    expect(submitted!['worktreeRoot'], '/work/live/root');
  });

  testWidgets('Create Session modal falls back to persisted project model when runtime model cache is empty', (tester) async {
    Map<String, String>? submitted;
    const shell = ConversationShellData(
      appTitle: 'Agent Runtime',
      connectionLabel: 'Runtime healthy',
      projects: [
        ConversationProject(id: '__unassigned__', title: 'Unassigned'),
        ConversationProject(
          id: 'project-a',
          title: 'Project A',
          defaultWorkdir: '/work/project-a',
          defaultWorktreeRoot: '/work/project-a/root',
          defaultRoleId: 'runtime-safe-builder',
          defaultModel: 'codex-cached-project-model',
        ),
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
              submitted = {'project': project, 'model': model};
            },
          ),
        ),
      ),
    );

    expect(find.textContaining('Model options are unavailable'), findsNothing);
    expect(find.textContaining('Codex auth'), findsNothing);
    expect(find.byKey(const ValueKey('agentRuntime.createSession.noModel')), findsNothing);
    expect(find.byKey(const ValueKey('agentRuntime.createSession.model')), findsOneWidget);
    expect(find.text('Codex Cached Project Model'), findsOneWidget);

    await tester.enterText(find.byKey(const ValueKey('agentRuntime.createSession.title')), 'Cached Model Session');
    await tester.tap(find.widgetWithText(FilledButton, 'Create'));
    await tester.pumpAndSettle();

    expect(submitted, isNotNull);
    expect(submitted!['project'], 'project-a');
    expect(submitted!['model'], 'codex-cached-project-model');
  });

  testWidgets('Project Settings modal saves every field and exposes only archive and unarchive lifecycle actions', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1400, 1600));
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
            }) {
              saved = {
                'projectKey': projectKey,
                'displayName': displayName,
                'defaultWorkdir': defaultWorkdir,
                'defaultWorktreeRoot': defaultWorktreeRoot,
                'defaultRoleId': defaultRoleId,
                'defaultModel': defaultModel,
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
    expect(find.text('Tracked'), findsNothing);
    expect(find.text('Listed'), findsNothing);
    expect(find.text('Visibility'), findsNothing);
    expect(find.text('Hidden'), findsNothing);
    expect(find.text('Invisible'), findsNothing);
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
            }) {
              saved = {
                'projectKey': projectKey,
                'displayName': displayName,
                'defaultWorkdir': defaultWorkdir,
                'defaultWorktreeRoot': defaultWorktreeRoot,
                'defaultRoleId': defaultRoleId,
                'defaultModel': defaultModel,
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
            }) {
              saved = {
                'projectKey': projectKey,
                'displayName': displayName,
                'defaultWorkdir': defaultWorkdir,
                'defaultWorktreeRoot': defaultWorktreeRoot,
                'defaultRoleId': defaultRoleId,
                'defaultModel': defaultModel,
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

  testWidgets('Session control plane saves projected fields and dispatches lifecycle actions', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1400, 1600));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final actions = <String>[];
    Map<String, Object?>? saved;

    Future<void> pumpDialog({AgentRuntimeWorkbenchData data = mockAgentRuntimeConnected}) async {
      await tester.pumpWidget(
        MaterialApp(
          home: AgentRuntimeSessionControlPlane(
            data: data,
            onClose: () {},
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
            onArchiveSession: (id) => actions.add('archive:$id'),
            onForkSession: (id) => actions.add('fork:$id'),
            onCompact: (id) => actions.add('compact:$id'),
            onGrantGodMode: (id) => actions.add('grantGodMode:$id'),
            onRevokeGodMode: (id) => actions.add('revokeGodMode:$id'),
            onTerminateProcess: (handle) => actions.add('terminate:$handle'),
            onFlushProcess: (handle) => actions.add('flush:$handle'),
            onInputProcess: (handle, text) => actions.add('input:$handle:$text'),
            onApprove: (id, reason) => actions.add('approve:$id:$reason'),
            onDeny: (id, reason) => actions.add('deny:$id:$reason'),
            onResumeApproval: (id) => actions.add('resume:$id'),
            onPreviewCommandRequest: (id) => actions.add('preview:$id'),
            onApproveCommandRequest: (id) => actions.add('cmdApprove:$id'),
            onDenyCommandRequest: (id) => actions.add('cmdDeny:$id'),
            onApplyCommandRequest: (id) => actions.add('cmdApply:$id'),
            onShowCommand: (id) => actions.add('showCommand:$id'),
            onShowCommandRequest: (id) => actions.add('showRequest:$id'),
            onSetRequirements: (id, {required title, required key, required statement}) => actions.add('requirements:$id:$title:$key:$statement'),
          ),
        ),
      );
    }
    Future<void> tapVisible(Finder finder) async {
      await tester.ensureVisible(finder);
      await tester.pumpAndSettle();
      await tester.tap(finder);
      await tester.pumpAndSettle();
    }

    await pumpDialog();
    expect(find.text('Session Settings'), findsOneWidget);
    expect(find.text('Processes (2)'), findsOneWidget);
    expect(find.text('Approve command execution'), findsOneWidget);
    expect(find.text('Started 09:14'), findsOneWidget);
    expect(find.textContaining('Requested 4m ago'), findsOneWidget);
    expect(find.text('Duplicate Settings Unavailable'), findsOneWidget);
    expect(find.text('No typed duplicate operation exists'), findsOneWidget);
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.sessionControl.title')), 'Updated Session');
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.sessionControl.workdir')), '/work/session');
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.sessionControl.worktreeroot')), '/work/session/root');
    await tester.tap(find.text('Save changes'));
    await tester.pumpAndSettle();
    expect(saved!['sessionId'], 'session-a');
    expect(saved!['title'], 'Updated Session');
    expect(saved!['name'], 'runtime-validation');
    expect(saved!['model'], 'codex-live-model');
    expect(saved!['workdir'], '/work/session');
    expect(saved!['worktreeRoot'], '/work/session/root');
    expect(saved!['project'], 'project-a');
    expect(saved!['role'], 'runtime-allow');
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.sessionControl.processInput')), 'ping');
    await tapVisible(find.text('Send input'));
    await tapVisible(find.text('Flush output'));
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.sessionControl.approvalReason')), 'Approved for test');
    await tapVisible(find.widgetWithText(OutlinedButton, 'Approve').first);
    await tester.enterText(find.byKey(const ValueKey('agentRuntime.sessionControl.approvalReason')), 'Denied for test');
    await tapVisible(find.widgetWithText(OutlinedButton, 'Deny').first);
    await tapVisible(find.widgetWithText(OutlinedButton, 'Resume').last);
    await tapVisible(find.widgetWithText(OutlinedButton, 'Preview'));
    await tapVisible(find.widgetWithText(OutlinedButton, 'Approve').last);
    await tapVisible(find.widgetWithText(OutlinedButton, 'Deny').last);
    await tapVisible(find.widgetWithText(OutlinedButton, 'Apply'));
    await tapVisible(find.widgetWithText(OutlinedButton, 'Show Command'));
    await tapVisible(find.widgetWithText(OutlinedButton, 'View Details').last);
    await tapVisible(find.text('Compact…'));
    await tapVisible(find.text('Grant God Mode…'));
    expect(actions, containsAll(['input:dev-server:ping', 'flush:dev-server', 'approve:approval-1:Approved for test', 'deny:approval-1:Denied for test', 'resume:approval-2', 'preview:registry-request-1', 'cmdApprove:registry-request-1', 'cmdDeny:registry-request-1', 'cmdApply:registry-request-1', 'showCommand:cmd.registry.audit', 'showRequest:registry-request-1', 'compact:session-a', 'grantGodMode:session-a']));

    final activeGodMode = mockAgentRuntimeConnected.copyWith(
      selectedSessionControlPlane: mockAgentRuntimeConnected.selectedSessionControlPlane!.copyWith(
        godMode: const AgentRuntimeGodModeState(
          active: true,
          reason: 'Owner enabled break-glass shell for this session',
          grantedBy: 'Owner',
          grantedAt: '09:30',
        ),
      ),
    );
    await pumpDialog(data: activeGodMode);
    await tapVisible(find.text('Revoke God Mode…'));
    expect(actions, contains('revokeGodMode:session-a'));

    for (final key in ['archivesession', 'forksession']) {
      await pumpDialog();
      final finder = find.byKey(ValueKey('agentRuntime.sessionControl.$key'));
      await tester.ensureVisible(finder);
      await tester.tap(finder);
      await tester.pumpAndSettle();
    }
    expect(actions, containsAll(['archive:session-a', 'fork:session-a']));
    await pumpDialog();
    await tester.tap(find.text('Set Requirements…'));
    await tester.pumpAndSettle();
    expect(find.text('Robdex frontend redesign'), findsNothing);
    expect(find.text('The UI must match the reference image on large screens.'), findsNothing);
    expect(tester.widget<TextField>(find.widgetWithText(TextField, 'Title')).controller!.text, isEmpty);
    expect(tester.widget<TextField>(find.widgetWithText(TextField, 'Requirement statement')).controller!.text, isEmpty);
    await tester.tap(find.widgetWithText(FilledButton, 'Set Requirements'));
    await tester.pump();
    expect(find.text('Every requirement needs a statement.'), findsOneWidget);
    await tester.enterText(find.widgetWithText(TextField, 'Requirement statement'), 'Prove the selected-session control plane.');
    await tester.pump();
    expect(find.text('Every requirement needs a statement.'), findsNothing);
    await tester.tap(find.widgetWithText(FilledButton, 'Set Requirements'));
    await tester.pumpAndSettle();
    expect(actions, contains('requirements:session-a:::Prove the selected-session control plane.'));
  });

  testWidgets('production host opens full-screen session control plane from toolbar', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1400, 1600));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final controller = AgentRuntimeWorkbenchController(requestSink: (_, _) {});
    addTearDown(controller.dispose);
    controller.setViewDataForTest(mockAgentRuntimeConnected, shell: agentRuntimeConversationShellData(mockAgentRuntimeConnected));

    await tester.pumpWidget(MaterialApp(home: AgentRuntimeWorkbenchHost(controller: controller)));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('agentRuntime.toolbar.sessionSettings')));
    await tester.pumpAndSettle();

    expect(find.byType(AgentRuntimeSessionControlPlane), findsOneWidget);
    expect(find.byType(AlertDialog), findsNothing);
    expect(find.text('Session Settings'), findsOneWidget);
    expect(find.text('Processes (2)'), findsOneWidget);
  });

  testWidgets('session settings normalizes duplicate model dropdown values before render', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1400, 1600));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final duplicateModelData = mockAgentRuntimeConnected.copyWith(
      selectedSessionControlPlane: mockAgentRuntimeConnected.selectedSessionControlPlane!.copyWith(
        activeModel: 'codex-live-model',
        modelOptions: const [
          AgentRuntimeModelOption(id: 'codex-live-model', displayLabel: 'Codex live model', source: 'runtime', isDefault: true),
          AgentRuntimeModelOption(id: 'codex-live-model', displayLabel: 'Codex live model duplicate', source: 'runtime', isDefault: false),
          AgentRuntimeModelOption(id: 'gpt-5.4-mini', displayLabel: 'GPT-5.4 Mini', source: 'runtime', isDefault: false),
        ],
      ),
    );
    final controller = AgentRuntimeWorkbenchController(requestSink: (_, _) {});
    addTearDown(controller.dispose);
    controller.setViewDataForTest(duplicateModelData, shell: agentRuntimeConversationShellData(duplicateModelData));

    await tester.pumpWidget(MaterialApp(home: AgentRuntimeWorkbenchHost(controller: controller)));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('agentRuntime.toolbar.sessionSettings')));
    await tester.pumpAndSettle();

    expect(find.byType(AgentRuntimeSessionControlPlane), findsOneWidget);
    expect(find.byType(DropdownButtonFormField<String>), findsNWidgets(3));
    expect(tester.takeException(), isNull);
  });

  testWidgets('production host closes session control plane through visible toolbar close and restores shell interactivity', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1400, 1600));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final controller = AgentRuntimeWorkbenchController(requestSink: (_, _) {});
    addTearDown(controller.dispose);
    controller.setViewDataForTest(mockAgentRuntimeConnected, shell: agentRuntimeConversationShellData(mockAgentRuntimeConnected));

    await tester.pumpWidget(MaterialApp(home: AgentRuntimeWorkbenchHost(controller: controller)));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('agentRuntime.toolbar.sessionSettings')));
    await tester.pumpAndSettle();

    expect(find.byType(AgentRuntimeSessionControlPlane), findsOneWidget);
    expect(find.byKey(const ValueKey('agentRuntime.sessionControl.close')), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('agentRuntime.sessionControl.close')));
    await tester.pumpAndSettle();

    expect(find.byType(AgentRuntimeSessionControlPlane), findsNothing);
    await tester.tap(find.byTooltip('New session'));
    await tester.pump();
    expect(find.byType(AgentRuntimeCreateSessionDialog), findsOneWidget);
  });

  testWidgets('toolbar sections menu reaches Command Registry surface', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1400, 1600));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final sentRequests = <bindings.AgentRuntimeRequest>[];
    final controller = AgentRuntimeWorkbenchController(requestSink: (_, request) => sentRequests.add(request));
    addTearDown(controller.dispose);
    controller.setViewDataForTest(mockAgentRuntimeConnected, shell: agentRuntimeConversationShellData(mockAgentRuntimeConnected));

    await tester.pumpWidget(MaterialApp(home: AgentRuntimeWorkbenchHost(controller: controller)));
    await tester.pump();

    await tester.tap(find.byKey(const ValueKey('agentRuntime.toolbar.sections')));
    await tester.pumpAndSettle();
    expect(find.text('Command Registry'), findsOneWidget);

    await tester.tap(find.text('Command Registry'));
    await tester.pumpAndSettle();
    expect(find.bySemanticsLabel('Runtime operations detail'), findsOneWidget);
    expect(find.widgetWithText(OutlinedButton, 'Refresh installed commands'), findsOneWidget);
    expect(find.widgetWithText(OutlinedButton, 'Refresh pending requests'), findsOneWidget);

    await tester.tap(find.widgetWithText(OutlinedButton, 'Refresh installed commands'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Refresh pending requests'));
    await tester.pump();
    expect(sentRequests.whereType<bindings.AgentRuntimeRequestDispatchOperation>().length, 2);
  });

  testWidgets('session control plane can close without a selected session and treats stopped as open idle', (tester) async {
    final actions = <String>[];
    var closed = 0;
    Future<void> pumpPlane(AgentRuntimeWorkbenchData data) async {
      await tester.pumpWidget(MaterialApp(
        home: AgentRuntimeSessionControlPlane(
          data: data,
          onClose: () => closed += 1,
          onSave: ({required sessionId, required project, required role, required model, required workdir, required worktreeRoot, required title, required name, required tracked}) {},
          onArchiveSession: (id) => actions.add('archive:$id'),
          onForkSession: (id) => actions.add('fork:$id'),
          onCompact: (id) => actions.add('compact:$id'),
          onGrantGodMode: (id) => actions.add('grant:$id'),
          onRevokeGodMode: (id) => actions.add('revoke:$id'),
          onTerminateProcess: (handle) {},
          onFlushProcess: (handle) {},
          onInputProcess: (handle, text) {},
          onApprove: (id, reason) {},
          onDeny: (id, reason) {},
          onResumeApproval: (id) {},
          onPreviewCommandRequest: (id) {},
          onApproveCommandRequest: (id) {},
          onDenyCommandRequest: (id) {},
          onApplyCommandRequest: (id) {},
          onShowCommand: (id) {},
          onShowCommandRequest: (id) {},
          onSetRequirements: (id, {required title, required key, required statement}) {},
        ),
      ));
      await tester.pump();
    }

    await pumpPlane(mockAgentRuntimeEmpty);
    expect(find.text('Select a session to open settings.'), findsOneWidget);
    expect(find.bySemanticsLabel('Close session settings'), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('agentRuntime.sessionControl.close')));
    await tester.pump();
    expect(closed, 1);

    await pumpPlane(mockAgentRuntimeConnected.copyWith(
      selectedSessionControlPlane: mockAgentRuntimeConnected.selectedSessionControlPlane!.copyWith(status: 'stopped'),
    ));
    expect(find.text('Idle'), findsOneWidget);
    expect(find.text('Compact…'), findsOneWidget);
    expect(find.text('Grant God Mode…'), findsOneWidget);
    await tester.tap(find.text('Compact…'));
    await tester.tap(find.text('Grant God Mode…'));
    await tester.pump();
    expect(actions, containsAll(['compact:session-a', 'grant:session-a']));

    await pumpPlane(mockAgentRuntimeConnected.copyWith(
      selectedSessionControlPlane: mockAgentRuntimeConnected.selectedSessionControlPlane!.copyWith(projectKey: ''),
    ));
    expect(find.text('Project'), findsWidgets);
    expect(find.text('Unassigned'), findsOneWidget);
  });

  testWidgets('production host Danger Zone Archive session and Archive session dispatch typed operations and return to session settings', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1400, 1600));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final sentRequests = <bindings.AgentRuntimeRequest>[];
    final controller = AgentRuntimeWorkbenchController(requestSink: (_, request) => sentRequests.add(request));
    addTearDown(controller.dispose);
    controller.setViewDataForTest(mockAgentRuntimeConnected, shell: agentRuntimeConversationShellData(mockAgentRuntimeConnected));

    Future<void> openDangerZone() async {
      await tester.ensureVisible(find.widgetWithText(OutlinedButton, 'Danger Zone'));
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(OutlinedButton, 'Danger Zone'));
      await tester.pumpAndSettle();
      expect(find.text('Danger Zone'), findsNWidgets(2));
    }

    await tester.pumpWidget(MaterialApp(home: AgentRuntimeWorkbenchHost(controller: controller)));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('agentRuntime.toolbar.sessionSettings')));
    await tester.pumpAndSettle();
    expect(find.byType(AgentRuntimeSessionControlPlane), findsOneWidget);

    await openDangerZone();
    expect(find.byKey(const ValueKey('agentRuntime.sessionControl.danger.cancel')), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('agentRuntime.sessionControl.danger.cancel')));
    await tester.pumpAndSettle();
    expect(sentRequests, isEmpty);
    expect(find.text('Danger Zone'), findsOneWidget);
    expect(find.byType(AgentRuntimeSessionControlPlane), findsOneWidget);
    expect(find.byKey(const ValueKey('agentRuntime.sessionControl.danger.archiveSession')), findsNothing);

    await openDangerZone();
    await tester.tap(find.byKey(const ValueKey('agentRuntime.sessionControl.danger.archiveSession')));
    await tester.pumpAndSettle();
    expect((sentRequests.single as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationArchiveSession>());
    expect(find.text('Danger Zone'), findsOneWidget);
    expect(find.byType(AgentRuntimeSessionControlPlane), findsOneWidget);
    expect(find.byKey(const ValueKey('agentRuntime.sessionControl.danger.archiveSession')), findsNothing);

    await openDangerZone();
    await tester.tap(find.byKey(const ValueKey('agentRuntime.sessionControl.danger.archiveSession')));
    await tester.pumpAndSettle();
    expect(sentRequests, hasLength(2));
    expect((sentRequests.last as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationArchiveSession>());
    expect(find.text('Danger Zone'), findsOneWidget);
    expect(find.byType(AgentRuntimeSessionControlPlane), findsOneWidget);
    expect(find.byKey(const ValueKey('agentRuntime.sessionControl.danger.archiveSession')), findsNothing);
  });

  testWidgets('session control plane remains dismissable without a selected session', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1400, 1600));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    var closed = false;

    await tester.pumpWidget(MaterialApp(
      home: AgentRuntimeSessionControlPlane(
        data: mockAgentRuntimeEmpty,
        onClose: () => closed = true,
        onSave: ({required sessionId, required project, required role, required model, required workdir, required worktreeRoot, required title, required name, required tracked}) {},
        onArchiveSession: (_) {},
        onForkSession: (_) {},
        onCompact: (_) {},
        onGrantGodMode: (_) {},
        onRevokeGodMode: (_) {},
        onTerminateProcess: (_) {},
        onFlushProcess: (_) {},
        onInputProcess: (_, _) {},
        onApprove: (_, _) {},
        onDeny: (_, _) {},
        onResumeApproval: (_) {},
        onPreviewCommandRequest: (_) {},
        onApproveCommandRequest: (_) {},
        onDenyCommandRequest: (_) {},
        onApplyCommandRequest: (_) {},
        onShowCommand: (_) {},
        onShowCommandRequest: (_) {},
        onSetRequirements: (_, {required title, required key, required statement}) {},
      ),
    ));
    await tester.pumpAndSettle();

    expect(find.byType(AgentRuntimeSessionControlPlane), findsOneWidget);
    expect(find.text('Session Settings'), findsOneWidget);
    expect(find.text('Select a session to open settings.'), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('agentRuntime.sessionControl.close')));
    await tester.pumpAndSettle();

    expect(closed, isTrue);
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
        command: 'print("ok")',
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
            statusLabel: 'stopped',
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
          command: 'print("ok")',
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

  test('session archive and fork use generated typed operation intents', () {
    final sentRequests = <bindings.AgentRuntimeRequest>[];
    final controller = AgentRuntimeWorkbenchController(
      requestSink: (requestId, request) {
        sentRequests.add(request);
      },
    );
    addTearDown(controller.dispose);

    controller.archiveSession('session-2');
    controller.forkSession('session-2');

    expect(sentRequests, hasLength(2));
    expect((sentRequests[0] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationArchiveSession>());
    expect((sentRequests[1] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationForkSession>());
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

  test('role draft controller operations preserve edited structured fields', () {
    final sentRequests = <bindings.AgentRuntimeRequest>[];
    final controller = AgentRuntimeWorkbenchController(
      requestSink: (requestId, request) {
        sentRequests.add(request);
      },
    );
    addTearDown(controller.dispose);

    const edited = AgentRuntimeRoleEditorDraft(
      roleId: 'runtime-allow',
      version: '4.0.0',
      displayName: 'Runtime Allow Edited',
      model: 'gpt-5.4-mini',
      reasoningEffort: 'high',
      instructionText: 'Edited instructions',
      capabilities: ['tool.execute_code', 'fs.write'],
      policy: [
        AgentRuntimeRolePolicyRow(action: 'tool.execute_code', decision: 'allow'),
        AgentRuntimeRolePolicyRow(action: 'fs.write', decision: 'deny'),
      ],
      routingMode: 'direct',
      routingReservedActions: ['message.send'],
      defaultRecipient: 'owner',
      allowedRecipients: ['owner'],
      listed: false,
      ownerVisible: true,
      canSpawnAgents: true,
      canArchiveAgents: false,
      lifecycleReservedActions: ['agent.archive'],
    );

    controller.validateRoleDraft(edited);
    controller.updateRoleFromDraft(edited);

    final validate = (sentRequests[0] as bindings.AgentRuntimeRequestDispatchOperation).operation as bindings.AgentRuntimeGuiOperationValidateRoleDraft;
    final update = (sentRequests[1] as bindings.AgentRuntimeRequestDispatchOperation).operation as bindings.AgentRuntimeGuiOperationUpdateRoleFromDraft;
    for (final draft in [validate.draft, update.draft]) {
      expect(draft.id, 'runtime-allow');
      expect(draft.version, '4.0.0');
      expect(draft.modelDefaults.model, 'gpt-5.4-mini');
      expect(draft.modelDefaults.reasoningEffort, 'high');
      expect(draft.instructionText, 'Edited instructions');
      expect(draft.capabilities, ['tool.execute_code', 'fs.write']);
      expect(draft.policyEntries.map((entry) => '${entry.key}=${entry.value}'), ['tool.execute_code=allow', 'fs.write=deny']);
      expect(draft.routing.defaultRecipient, 'owner');
      expect(draft.routing.allowedRecipients, ['owner']);
      expect(draft.routing.reservedActions, ['message.send']);
      expect(draft.visibility.listed, false);
      expect(draft.visibility.ownerVisible, true);
      expect(draft.lifecycleAuthority.canSpawnAgents, true);
      expect(draft.lifecycleAuthority.reservedActions, ['agent.archive']);
    }
    expect(update.roleId, 'runtime-allow');
  });

  test('role draft controller serializes capabilities from canonical policy rows only', () {
    final sentRequests = <bindings.AgentRuntimeRequest>[];
    final controller = AgentRuntimeWorkbenchController(
      requestSink: (requestId, request) {
        sentRequests.add(request);
      },
    );
    addTearDown(controller.dispose);

    const edited = AgentRuntimeRoleEditorDraft(
      roleId: 'runtime-allow',
      version: '4.0.1',
      displayName: 'Runtime Allow Edited',
      model: 'gpt-5.4-mini',
      reasoningEffort: 'high',
      instructionText: 'Edited instructions',
      capabilities: ['tool.execute_code', 'fs.write', 'git.status'],
      policy: [
        AgentRuntimeRolePolicyRow(action: 'tool.execute_code', decision: 'allow'),
        AgentRuntimeRolePolicyRow(action: 'git.status', decision: 'allow'),
      ],
      routingMode: 'direct',
      routingReservedActions: [],
      defaultRecipient: 'owner',
      allowedRecipients: ['owner'],
      listed: true,
      ownerVisible: true,
      canSpawnAgents: false,
      canArchiveAgents: false,
      lifecycleReservedActions: [],
    );

    controller.updateRoleFromDraft(edited);

    final update = (sentRequests.single as bindings.AgentRuntimeRequestDispatchOperation).operation as bindings.AgentRuntimeGuiOperationUpdateRoleFromDraft;
    expect(update.draft.capabilities, ['tool.execute_code', 'git.status']);
    expect(update.draft.policyEntries.map((entry) => entry.key), ['tool.execute_code', 'git.status']);
    expect(update.draft.capabilities, isNot(contains('command_registry.decide')));
  });

  testWidgets('role manager edits structured draft and dispatches visible values', (tester) async {
    AgentRuntimeRoleEditorDraft? validated;
    AgentRuntimeRoleEditorDraft? saved;
    tester.view.physicalSize = const Size(1600, 1600);
    tester.view.devicePixelRatio = 1;
    addTearDown(() {
      tester.view.resetPhysicalSize();
      tester.view.resetDevicePixelRatio();
    });

    await tester.pumpWidget(
      MaterialApp(
        home: SizedBox(
          width: 1600,
          height: 1600,
          child: AgentRuntimeRoleManagerPage(
            data: mockAgentRuntimeRoleAdminSelected,
            onValidate: (draft) => validated = draft,
            onUpdate: (draft) => saved = draft,
          ),
        ),
      ),
    );
    await tester.pump();

    expect(find.text('Role Snapshot'), findsNothing);
    expect(find.textContaining('Route evaluation'), findsNothing);
    expect(find.text('Audit Events'), findsNothing);
    expect(find.text('Role authority', skipOffstage: false), findsOneWidget);
    expect(find.text('Capabilities', skipOffstage: false), findsNothing);
    expect(find.text('Policy decisions', skipOffstage: false), findsNothing);
    expect(find.text('Allowed recipients', skipOffstage: false), findsOneWidget);
    expect(find.byKey(const ValueKey('roleEditor.policySelect.command.registry'), skipOffstage: false), findsNothing);
    expect(find.byKey(const ValueKey('roleEditor.policySelect.workflow.memory'), skipOffstage: false), findsNothing);

    await tester.tap(find.byKey(const ValueKey('roleEditor.model')));
    await tester.pump();
    await tester.tap(find.text('gpt-5.4-mini').last);
    await tester.pump();

    final instructionsField = find.byKey(const ValueKey('roleEditor.instructions'));
    (tester.widget(instructionsField) as dynamic).controller.text = 'Updated runtime instructions.';
    await tester.pump();

    await tester.scrollUntilVisible(find.byKey(const ValueKey('roleEditor.policyDecision.command_registry.apply')), 40, scrollable: find.byType(Scrollable).last);
    await tester.tap(find.byKey(const ValueKey('roleEditor.policyDecision.command_registry.apply')));
    await tester.pump();
    expect(find.text('Off / absent'), findsWidgets);
    expect(find.text('Allow'), findsWidgets);
    expect(find.text('Deny'), findsWidgets);
    expect(find.text('Owner approval'), findsWidgets);
    expect(find.text('Orchestrator approval'), findsWidgets);
    await tester.tapAt(const Offset(20, 20));
    await tester.pump();

    await tester.ensureVisible(find.byKey(const ValueKey('roleEditor.recipient.runtime-safe-builder')));
    await tester.tap(find.byKey(const ValueKey('roleEditor.recipient.runtime-safe-builder')));
    await tester.pump();

    await tester.ensureVisible(find.byKey(const ValueKey('roleEditor.routingReserved.command_registry.apply')));
    await tester.tap(find.byKey(const ValueKey('roleEditor.routingReserved.command_registry.apply')));
    await tester.pump();

    await tester.ensureVisible(find.byKey(const ValueKey('roleEditor.lifecycleReserved.workflow_memory.feedback')));
    await tester.tap(find.byKey(const ValueKey('roleEditor.lifecycleReserved.workflow_memory.feedback')));
    await tester.pump();

    await tester.ensureVisible(find.byKey(const ValueKey('roleEditor.canSpawnAgents')));
    await tester.tap(find.descendant(of: find.byKey(const ValueKey('roleEditor.canSpawnAgents')), matching: find.byType(Switch)));
    await tester.pump();
    await tester.ensureVisible(find.byKey(const ValueKey('roleEditor.ownerVisible')));
    await tester.tap(find.descendant(of: find.byKey(const ValueKey('roleEditor.ownerVisible')), matching: find.byType(Switch)));
    await tester.pump();

    await tester.ensureVisible(find.widgetWithText(OutlinedButton, 'Validate Draft'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Validate Draft'));
    await tester.pump();
    await tester.ensureVisible(find.widgetWithText(OutlinedButton, 'Save Version'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Save Version'));
    await tester.pump();

    for (final draft in [validated, saved]) {
      expect(draft, isNotNull);
      expect(draft!.model, 'gpt-5.4-mini');
      expect(draft.instructionText, 'Updated runtime instructions.');
      expect(draft.capabilities, contains('command_registry.apply'));
      expect(draft.policy.any((row) => row.action == 'command_registry.apply' && row.decision == 'ownerApproval'), isTrue);
      expect(draft.policy.any((row) => row.action == 'fs.write' && row.decision == 'ownerApproval'), isTrue);
      expect(draft.allowedRecipients, contains('owner'));
      expect(draft.allowedRecipients, isNot(contains('runtime-safe-builder')));
      expect(draft.routingReservedActions, contains('command_registry.apply'));
      expect(draft.lifecycleReservedActions, contains('workflow_memory.feedback'));
      expect(draft.canSpawnAgents, isTrue);
      expect(draft.ownerVisible, isFalse);
    }
  });

  testWidgets('role authority policy editor separates selection from decisions and serializes decisions', (tester) async {
    AgentRuntimeRoleEditorDraft? saved;
    final roleAdmin = AgentRuntimeRoleAdminData(
      title: mockAgentRuntimeRoleAdminSelected.title,
      subtitle: mockAgentRuntimeRoleAdminSelected.subtitle,
      emptyTitle: mockAgentRuntimeRoleAdminSelected.emptyTitle,
      emptyText: mockAgentRuntimeRoleAdminSelected.emptyText,
      rows: mockAgentRuntimeRoleAdminSelected.rows,
      selectedDetail: mockAgentRuntimeRoleAdminSelected.selectedDetail,
      versionRows: mockAgentRuntimeRoleAdminSelected.versionRows,
      editorDraft: mockAgentRuntimeRoleAdminSelected.editorDraft,
      validationErrors: const [],
      actionStates: mockAgentRuntimeRoleAdminSelected.actionStates,
      editorOptions: AgentRuntimeRoleEditorOptions(
        models: mockAgentRuntimeRoleEditorOptions.models,
        reasoningEfforts: mockAgentRuntimeRoleEditorOptions.reasoningEfforts,
        capabilities: mockAgentRuntimeRoleEditorOptions.capabilities,
        policyActions: mockAgentRuntimeRoleEditorOptions.policyActions,
        policyDecisions: mockAgentRuntimeRoleEditorOptions.policyDecisions,
        routingModes: mockAgentRuntimeRoleEditorOptions.routingModes,
        recipients: mockAgentRuntimeRoleEditorOptions.recipients,
        reservedActions: mockAgentRuntimeRoleEditorOptions.reservedActions,
      ),
    );
    await tester.binding.setSurfaceSize(const Size(1400, 1600));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(MaterialApp(
      home: AgentRuntimeRoleManagerPage(
        data: roleAdmin,
        onUpdate: (draft) => saved = draft,
      ),
    ));
    await tester.pump();

    await tester.scrollUntilVisible(find.byKey(const ValueKey('roleEditor.policyDecision.command_registry.apply')), 40, scrollable: find.byType(Scrollable).last);
    await tester.tap(find.byKey(const ValueKey('roleEditor.policyDecision.command_registry.apply')));
    await tester.pump();
    expect(find.text('Off / absent'), findsWidgets);
    expect(find.text('Allow'), findsWidgets);
    expect(find.text('Deny'), findsWidgets);
    expect(find.text('Owner approval'), findsWidgets);
    expect(find.text('Orchestrator approval'), findsWidgets);
    await tester.tap(find.text('Deny').last, warnIfMissed: false);
    await tester.pump();

    await tester.scrollUntilVisible(find.byKey(const ValueKey('roleEditor.policySelect.command_registry.apply')), 40, scrollable: find.byType(Scrollable).last);
    await tester.tap(find.byKey(const ValueKey('roleEditor.policySelect.command_registry.apply')));
    await tester.pump();
    await tester.scrollUntilVisible(find.byKey(const ValueKey('roleEditor.policySelect.command_registry.decide')), 40, scrollable: find.byType(Scrollable).last);
    await tester.tap(find.byKey(const ValueKey('roleEditor.policySelect.command_registry.decide')));
    await tester.pump();

    await tester.tap(find.byKey(const ValueKey('roleEditor.policyDecision.command_registry.decide')));
    await tester.pump();
    await tester.tap(find.text('Owner approval').last, warnIfMissed: false);
    await tester.pump();

    await tester.tap(find.byKey(const ValueKey('roleEditor.policyClearSelection')));
    await tester.pump();

    await tester.tap(find.byKey(const ValueKey('roleEditor.policyDecision.command_registry.decide')));
    await tester.pump();
    await tester.tap(find.text('Off / absent').last, warnIfMissed: false);
    await tester.pump();

    await tester.tap(find.byKey(const ValueKey('roleEditor.policySelectAll')));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('roleEditor.policyClearSelection')));
    await tester.pump();

    await tester.ensureVisible(find.widgetWithText(OutlinedButton, 'Save Version'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Save Version'));
    await tester.pump();
    expect(saved, isNotNull);
    expect(saved!.policy.any((row) => row.action == 'command_registry.apply' && row.decision == 'ownerApproval'), isTrue);
    expect(saved!.policy.any((row) => row.action == 'command_registry.decide'), isFalse);
    expect(saved!.capabilities, contains('command_registry.apply'));
    expect(saved!.capabilities, isNot(contains('command_registry.decide')));

  });

  testWidgets('role manager reloads same-count authority edits from refreshed projection', (tester) async {
    AgentRuntimeRoleEditorDraft? saved;
    tester.view.physicalSize = const Size(1600, 1600);
    tester.view.devicePixelRatio = 1;
    addTearDown(() {
      tester.view.resetPhysicalSize();
      tester.view.resetDevicePixelRatio();
    });
    final base = mockAgentRuntimeRoleAdminSelected;
    final baseDraft = base.editorDraft!;
    final refreshedDraft = AgentRuntimeRoleEditorDraft(
      roleId: baseDraft.roleId,
      version: '3.2.1',
      displayName: baseDraft.displayName,
      model: baseDraft.model,
      reasoningEffort: baseDraft.reasoningEffort,
      instructionText: baseDraft.instructionText,
      capabilities: baseDraft.capabilities,
      policy: [
        for (final row in baseDraft.policy)
          AgentRuntimeRolePolicyRow(action: row.action, decision: row.action == 'fs.write' ? 'deny' : row.decision),
      ],
      routingMode: baseDraft.routingMode,
      routingReservedActions: baseDraft.routingReservedActions,
      defaultRecipient: baseDraft.defaultRecipient,
      allowedRecipients: baseDraft.allowedRecipients,
      listed: baseDraft.listed,
      ownerVisible: baseDraft.ownerVisible,
      canSpawnAgents: baseDraft.canSpawnAgents,
      canArchiveAgents: baseDraft.canArchiveAgents,
      lifecycleReservedActions: baseDraft.lifecycleReservedActions,
    );
    final refreshed = AgentRuntimeRoleAdminData(
      title: base.title,
      subtitle: base.subtitle,
      emptyTitle: base.emptyTitle,
      emptyText: base.emptyText,
      rows: base.rows,
      selectedDetail: base.selectedDetail,
      versionRows: base.versionRows,
      editorDraft: refreshedDraft,
      validationErrors: base.validationErrors,
      actionStates: base.actionStates,
      editorOptions: base.editorOptions,
    );

    await tester.pumpWidget(MaterialApp(home: SizedBox(width: 1600, height: 1600, child: AgentRuntimeRoleManagerPage(data: base, onUpdate: (draft) => saved = draft))));
    await tester.pump();
    await tester.pumpWidget(MaterialApp(home: SizedBox(width: 1600, height: 1600, child: AgentRuntimeRoleManagerPage(data: refreshed, onUpdate: (draft) => saved = draft))));
    await tester.pump();
    await tester.ensureVisible(find.widgetWithText(OutlinedButton, 'Save Version'));
    await tester.tap(find.widgetWithText(OutlinedButton, 'Save Version'));
    await tester.pump();

    expect(saved, isNotNull);
    expect(saved!.version, '3.2.1');
    expect(saved!.policy.length, baseDraft.policy.length);
    expect(saved!.policy.any((row) => row.action == 'fs.write' && row.decision == 'deny'), isTrue);
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
    controller.denyApprovalById('approval-2', 'No');
    controller.previewCommandRegistryRequestById('registry-request-2', 'session-2');
    controller.denyCommandRegistryRequestById('registry-request-3', 'session-2');
    controller.compactSessionById('session-2');
    controller.grantGodModeById('session-2');
    controller.revokeGodModeById('session-2');
    controller.setRequirementsForSession('session-2');

    expect(sentRequests, hasLength(11));
    expect((sentRequests[0] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationDecideApproval>());
    expect((sentRequests[1] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationResumeApproval>());
    expect((sentRequests[2] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationDecideCommandRegistryRequest>());
    expect((sentRequests[3] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationApplyCommandRegistryRequest>());
    expect((sentRequests[4] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationDecideApproval>());
    expect((sentRequests[5] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationPreviewCommandRegistryRequest>());
    expect((sentRequests[6] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationDecideCommandRegistryRequest>());
    expect((sentRequests[7] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationCompactSession>());
    expect((sentRequests[8] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationGrantGodMode>());
    expect((sentRequests[9] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationRevokeGodMode>());
    expect((sentRequests[10] as bindings.AgentRuntimeRequestDispatchOperation).operation, isA<bindings.AgentRuntimeGuiOperationSetRequirements>());
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
