import 'dart:convert';
import 'dart:async';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_markdown_plus/flutter_markdown_plus.dart';
import 'package:robdex_app/src/app/robdex_app.dart';
import 'package:robdex_app/src/bindings/signals/signals.dart';
import 'package:robdex_app/src/terminal/integrated_terminal.dart';
import 'package:robdex_design_system/robdex_design_system.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:xterm/xterm.dart';

void main() {
  testWidgets('bootstrap entry supports connect and macOS bootstrap flow', (
    WidgetTester tester,
  ) async {
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    var connectCount = 0;
    var bootstrapCount = 0;

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: BootstrapEntryPanel(
            host: '127.0.0.1',
            port: '42080',
            isBusy: false,
            onConnectExisting: () => connectCount += 1,
            onBootstrapLocal: () => bootstrapCount += 1,
          ),
        ),
      ),
    );

    expect(find.text('Bridge required'), findsOneWidget);
    expect(find.text('Bootstrap is available on macOS.'), findsOneWidget);
    await tester.tap(find.text('Connect existing'));
    await tester.tap(find.text('Bootstrap local'));
    expect(connectCount, 1);
    expect(bootstrapCount, 1);
    debugDefaultTargetPlatformOverride = null;
  });

  testWidgets('bootstrap entry labels Linux and Windows support correctly', (
    WidgetTester tester,
  ) async {
    Future<void> pumpFor(TargetPlatform platform) async {
      debugDefaultTargetPlatformOverride = platform;
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: BootstrapEntryPanel(
              host: 'bridge.local',
              port: '42080',
              isBusy: false,
              onConnectExisting: () {},
              onBootstrapLocal: () {},
            ),
          ),
        ),
      );
    }

    await pumpFor(TargetPlatform.linux);
    expect(find.text('Bootstrap is available on Linux.'), findsOneWidget);
    expect(
      tester
          .widget<OutlinedButton>(
            find.widgetWithText(OutlinedButton, 'Bootstrap local'),
          )
          .onPressed,
      isNotNull,
    );

    await pumpFor(TargetPlatform.windows);
    expect(find.text('Windows bootstrap is WSL/future support.'), findsOneWidget);
    expect(
      tester
          .widget<OutlinedButton>(
            find.widgetWithText(OutlinedButton, 'Bootstrap local'),
          )
          .onPressed,
      isNull,
    );
    debugDefaultTargetPlatformOverride = null;
  });

  testWidgets('bootstrap entry shows unhealthy bridge retry guidance', (
    WidgetTester tester,
  ) async {
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: BootstrapEntryPanel(
            host: '127.0.0.1',
            port: '42080',
            isBusy: false,
            errorText: 'Connection refused',
            onConnectExisting: () {},
            onBootstrapLocal: () {},
          ),
        ),
      ),
    );

    expect(find.text('Connect existing'), findsOneWidget);
    expect(find.text('Bootstrap local'), findsOneWidget);
    expect(find.textContaining('Bridge health unavailable'), findsOneWidget);
    debugDefaultTargetPlatformOverride = null;
  });

  testWidgets('bootstrap help dialog names public helper commands', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: BootstrapHelpDialog(),
      ),
    );

    expect(find.text('Bootstrap local Robdex'), findsOneWidget);
    expect(find.textContaining('robdex bootstrap doctor'), findsOneWidget);
    expect(
      find.textContaining('robdex bootstrap plan --profile minimal'),
      findsOneWidget,
    );
  });

  testWidgets('workbench shell renders primary regions', (WidgetTester tester) async {
    tester.view.physicalSize = const Size(1600, 1200);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(
      MaterialApp(
        home: RobdexShellScreen(
          workbench: mockWorkbenchData,
          onThreadSelected: (_) {},
          onProjectSelected: (_) {},
          onDisconnect: () {},
          onGlobalSettings: () {},
          onCreateProject: () {},
          onProjectSettings: (_) {},
          onCreateThread: (_) {},
          onSpawnAgent: () {},
          onSendMessage: (_) {},
          onOpenHistory: () {},
          onCompactThread: () {},
          onTerminateCommandExecution: (_) {},
          onInterruptThread: () {},
          onApprovalDecision: (_, _, _) async {},
          onSettingsChanged: (_) {},
          onRunningStateChanged: (_) {},
          onRenameThread: (_) {},
          onArchiveThread: () {},
          onWarmHandoff: (_) {},
          onSetProjectOrchestrator: () {},
          onCreateThreadGroup: (_) {},
          onRenameThreadGroup: (_) async {},
          onDeleteThreadGroup: (_) {},
          onArchiveThreadGroup: (_) {},
          onMoveSelectedThreadToGroup: (_) {},
          onUpdateWorkerMetadata: (_) {},
          loadThreadStats: (_) async => _widgetStats,
          enableGraphics: true,
        ),
      ),
    );
    await tester.pump(const Duration(milliseconds: 250));

    expect(find.text('Codex Control Plane'), findsAtLeastNWidgets(1));
    expect(find.text('Config Operator'), findsAtLeastNWidgets(1));
  });

  test('project item parses requirements reviewer defaults', () {
    final project = ProjectItem.fromJson(const {
      'id': 'project-codex',
      'name': 'Codex',
      'rootPath': '/Users/robertsale/.codex',
      'defaultCwd': '/Users/robertsale/.codex',
      'defaultModel': 'gpt-project',
      'defaultReasoningEffort': 'medium',
      'defaultSandboxMode': 'workspace-write',
      'defaultApprovalPolicy': 'on-request',
      'defaultNetworkAccess': true,
      'globalDefaultSandboxMode': 'workspace-write',
      'globalDefaultApprovalPolicy': 'on-request',
      'globalDefaultNetworkAccess': true,
      'roleRuntimeDefaults': {
        'worker': {
          'sandboxMode': 'workspace-write',
          'networkAccess': true,
          'approvalPolicy': 'on-failure',
        },
      },
      'plannerDefaultModel': 'gpt-planner',
      'plannerDefaultReasoningEffort': 'high',
      'requirementsReviewerDefaultModel': 'gpt-5.5',
      'requirementsReviewerDefaultReasoningEffort': 'high',
      'permanentRequirementComposables': ['no-legacy', 'non-negotiables'],
      'manifestRuns': [
        {
          'runId': 'run-1',
          'planId': 'serial-plan',
          'title': 'Serial Plan',
          'status': 'active',
          'currentPhaseId': 'phase-1',
          'sourceHash': 'sha256:abc',
          'phases': [
            {
              'phaseId': 'phase-1',
              'title': 'Phase 1',
              'status': 'running',
              'workerThreadId': 'worker-1',
              'archiveCleanupState': 'notReady',
              'archiveSafe': false,
              'hasHandoff': false,
              'hasBlocker': true,
              'hasWaiver': false,
              'hasResumeDecision': true,
            },
          ],
        },
      ],
      'autoRouteReplies': false,
      'routeApprovalRequests': true,
      'isSelected': true,
    });

    expect(project.defaultModel, 'gpt-project');
    expect(project.defaultReasoningEffort, 'medium');
    expect(project.defaultSandboxMode, 'workspace-write');
    expect(project.defaultApprovalPolicy, 'on-request');
    expect(project.defaultNetworkAccess, isTrue);
    expect(project.globalDefaultSandboxMode, 'workspace-write');
    expect(project.globalDefaultApprovalPolicy, 'on-request');
    expect(project.globalDefaultNetworkAccess, isTrue);
    expect(project.roleRuntimeDefaults['worker']?.sandboxMode, 'workspace-write');
    expect(project.roleRuntimeDefaults['worker']?.networkAccess, isTrue);
    expect(project.roleRuntimeDefaults['worker']?.approvalPolicy, 'on-failure');
    expect(project.plannerDefaultModel, 'gpt-planner');
    expect(project.plannerDefaultReasoningEffort, 'high');
    expect(project.requirementsReviewerDefaultModel, 'gpt-5.5');
    expect(project.requirementsReviewerDefaultReasoningEffort, 'high');
    expect(project.permanentRequirementComposables, [
      'no-legacy',
      'non-negotiables',
    ]);
    expect(project.manifestRuns.single.runId, 'run-1');
    expect(project.manifestRuns.single.phases.single.workerThreadId, 'worker-1');
    expect(project.manifestRuns.single.phases.single.hasBlocker, isTrue);
    expect(project.manifestRuns.single.phases.single.hasResumeDecision, isTrue);
  });

  testWidgets('project manifest runs panel renders phase timeline state', (
    WidgetTester tester,
  ) async {
    final project = ProjectItem.fromJson(const {
      'id': 'project-codex',
      'name': 'Codex',
      'rootPath': '/Users/robertsale/.codex',
      'defaultCwd': '/Users/robertsale/.codex',
      'permanentRequirementComposables': [],
      'manifestRuns': [
        {
          'runId': 'run-1',
          'planId': 'serial-plan',
          'title': 'Serial Plan',
          'status': 'active',
          'currentPhaseId': 'phase-1',
          'sourceHash': 'sha256:abc',
          'phases': [
            {
              'phaseId': 'phase-1',
              'title': 'Phase 1',
              'status': 'running',
              'workerThreadId': 'worker-1',
              'archiveCleanupState': 'notReady',
              'archiveSafe': false,
              'hasHandoff': false,
              'hasBlocker': true,
              'hasWaiver': false,
              'hasResumeDecision': true,
            },
          ],
        },
      ],
    });

    await tester.pumpWidget(
      MaterialApp(
        theme: buildRobdexTheme(),
        home: Scaffold(
          body: ProjectManifestRunsPane(project: project),
        ),
      ),
    );

    expect(find.byKey(const ValueKey('project.manifestRuns')), findsOneWidget);
    expect(find.text('Manifest runs'), findsOneWidget);
    expect(find.textContaining('robdex manifest activate/status/advance/cancel'), findsOneWidget);
    expect(find.textContaining('Serial Plan | active'), findsOneWidget);
    expect(find.textContaining('phase-1: running'), findsOneWidget);
    expect(find.textContaining('worker worker-1'), findsOneWidget);
    expect(find.textContaining('blocker yes'), findsOneWidget);
    expect(find.textContaining('resume yes'), findsOneWidget);
  });

  test('workbench view parses project permanent composables from native payload', () {
    final view = WorkbenchViewData.fromJson(const {
      'projects': [
        {
          'id': 'project-codex',
          'name': 'Codex',
          'rootPath': '/Users/robertsale/.codex',
          'defaultCwd': '/Users/robertsale/.codex',
          'requirementsReviewerDefaultModel': 'gpt-5.5',
          'requirementsReviewerDefaultReasoningEffort': 'high',
          'permanentRequirementComposables': ['no-legacy'],
          'autoRouteReplies': false,
          'routeApprovalRequests': true,
          'isSelected': true,
        },
      ],
      'selection': <String, dynamic>{},
      'threads': <dynamic>[],
      'availableModels': <dynamic>[],
      'threadGroups': <dynamic>[],
      'liveProcesses': <dynamic>[],
      'chatEntries': <dynamic>[],
      'workspaceFiles': <dynamic>[],
      'inspectorFacts': <dynamic>[],
      'pendingApprovals': <dynamic>[],
    });

    expect(view.projects, hasLength(1));
    expect(view.projects.single.permanentRequirementComposables, ['no-legacy']);
  });

  test('update project signal submits requirements reviewer defaults', () {
    const signal = UpdateProjectSignal(
      projectId: 'project-codex',
      name: 'Codex',
      defaultCwd: '/Users/robertsale/.codex',
      autoRouteReplies: false,
      routeApprovalRequests: true,
      preferredModelProvider: 'openai',
      defaultModelId: 'gpt-project',
      defaultReasoningEffort: 'medium',
      defaultSandboxMode: 'workspace-write',
      defaultApprovalPolicy: 'on-request',
      defaultNetworkAccessMode: 'enabled',
      roleRuntimeDefaultsJson: '{"worker":{"sandboxMode":"workspace-write","networkAccess":true}}',
      orchestratorModelId: 'gpt-5',
      orchestratorReasoningEffort: 'high',
      workerModelId: 'gpt-5.4-mini',
      workerReasoningEffort: 'medium',
      qaModelId: 'gpt-5.4-mini',
      qaReasoningEffort: 'medium',
      designerModelId: 'gpt-5.4-mini',
      designerReasoningEffort: 'high',
      plannerModelId: 'gpt-5.5',
      plannerReasoningEffort: 'high',
      requirementsReviewerModelId: 'gpt-5.5',
      requirementsReviewerReasoningEffort: 'high',
      orchestratorDeveloperInstructions: '',
      workerDeveloperInstructions: '',
      qaDeveloperInstructions: '',
      designerDeveloperInstructions: '',
      operatorDeveloperInstructions: '',
      hiddenDeveloperInstructions: '',
      permanentRequirementComposables: ['no-legacy'],
    );

    final decoded = UpdateProjectSignal.bincodeDeserialize(signal.bincodeSerialize());
    expect(decoded.defaultModelId, 'gpt-project');
    expect(decoded.defaultSandboxMode, 'workspace-write');
    expect(decoded.defaultApprovalPolicy, 'on-request');
    expect(decoded.defaultNetworkAccessMode, 'enabled');
    expect(decoded.roleRuntimeDefaultsJson, contains('"worker"'));
    expect(decoded.plannerModelId, 'gpt-5.5');
    expect(decoded.plannerReasoningEffort, 'high');
    expect(decoded.requirementsReviewerModelId, 'gpt-5.5');
    expect(decoded.requirementsReviewerReasoningEffort, 'high');
    expect(decoded.permanentRequirementComposables, ['no-legacy']);
  });

  test('delete project signal serializes project id', () {
    const signal = DeleteProjectSignal(projectId: 'project-123');

    final decoded = DeleteProjectSignal.bincodeDeserialize(signal.bincodeSerialize());

    expect(decoded.projectId, 'project-123');
  });

  test('spawn agent signal serializes planner role', () {
    const signal = SpawnAgentSignal(
      name: 'Research Planner',
      role: 'planner',
      prompt: 'Plan the migration.',
      requirementSetJson: '',
    );

    final decoded = SpawnAgentSignal.bincodeDeserialize(signal.bincodeSerialize());

    expect(decoded.name, 'Research Planner');
    expect(decoded.role, 'planner');
    expect(decoded.prompt, 'Plan the migration.');
  });

  testWidgets('project settings permanent composables render details and update selection only', (
    WidgetTester tester,
  ) async {
    var selectedIds = <String>['no-legacy'];
    var reviewerModel = 'gpt-5.5';

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: StatefulBuilder(
            builder: (context, setState) {
              return Column(
                children: [
                  ProjectPermanentComposablesPane(
                    composables: const [
                      ProjectRequirementComposable(
                        id: 'no-legacy',
                        title: 'No Legacy',
                        description: 'Do not leave legacy behavior behind.',
                        scope: 'global',
                        requirementCount: 1,
                        requirements: [
                          {
                            'key': 'noLegacyLeftBehind',
                            'statement': 'Remove obsolete behavior and docs.',
                            'severity': 'blocker',
                            'verificationMethod': 'diffReview',
                          },
                        ],
                      ),
                      ProjectRequirementComposable(
                        id: 'non-negotiables',
                        title: 'Non-negotiables',
                        description: 'Always-on engineering constraints.',
                        scope: 'global',
                        requirementCount: 3,
                        requirements: [],
                      ),
                    ],
                    selectedIds: selectedIds,
                    onChanged: (next) => setState(() => selectedIds = next),
                  ),
                  DropdownButton<String>(
                    key: const ValueKey('unrelated.reviewer.model'),
                    value: reviewerModel,
                    items: const [
                      DropdownMenuItem(value: 'gpt-5.5', child: Text('GPT-5.5')),
                      DropdownMenuItem(value: 'gpt-5.4-mini', child: Text('GPT-5.4 Mini')),
                    ],
                    onChanged: (value) => setState(() => reviewerModel = value ?? ''),
                  ),
                ],
              );
            },
          ),
        ),
      ),
    );

    expect(find.text('Permanent composables'), findsOneWidget);
    expect(find.textContaining('no-legacy | global | 1 requirements'), findsOneWidget);
    expect(find.textContaining('non-negotiables | global | 3 requirements'), findsOneWidget);

    await tester.tap(
      find.byKey(const ValueKey('project.permanentComposable.inspect.no-legacy')),
    );
    await tester.pumpAndSettle();
    expect(find.text('noLegacyLeftBehind'), findsOneWidget);
    expect(find.text('Remove obsolete behavior and docs.'), findsOneWidget);
    await tester.tap(find.text('Close'));
    await tester.pumpAndSettle();

    await tester.tap(
      find.byKey(const ValueKey('project.permanentComposable.non-negotiables')),
    );
    await tester.pumpAndSettle();

    expect(selectedIds, ['no-legacy', 'non-negotiables']);
    expect(reviewerModel, 'gpt-5.5');
  });

  testWidgets('requirements reviewer project settings controls render and update only reviewer fields', (
    WidgetTester tester,
  ) async {
    var reviewerModel = 'gpt-5.5';
    var reviewerReasoning = 'high';
    var workerModel = 'gpt-5.4-mini';

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: StatefulBuilder(
            builder: (context, setState) {
              return ProjectRoleModelSettingsPane(
                roleKey: 'requirements-reviewer',
                availableModels: const [
                  ModelItem(id: 'gpt-5.4-mini', name: 'GPT-5.4 Mini', hidden: false),
                  ModelItem(id: 'gpt-5.5', name: 'GPT-5.5', hidden: false),
                  ModelItem(id: 'hidden-model', name: 'Hidden', hidden: true),
                ],
                modelId: reviewerModel,
                reasoningEffort: reviewerReasoning,
                onModelChanged: (value) => setState(() => reviewerModel = value),
                onReasoningChanged: (value) =>
                    setState(() => reviewerReasoning = value),
              );
            },
          ),
        ),
      ),
    );

    expect(
      find.byKey(const ValueKey('project.settings.requirements-reviewer.model')),
      findsOneWidget,
    );
    expect(
      find.byKey(const ValueKey('project.settings.requirements-reviewer.reasoning')),
      findsOneWidget,
    );
    expect(find.text('GPT-5.5'), findsOneWidget);
    expect(find.text('High'), findsOneWidget);

    await tester.tap(
      find.byKey(const ValueKey('project.settings.requirements-reviewer.model')),
    );
    await tester.pump();
    await tester.tap(find.text('GPT-5.4 Mini').last);
    await tester.pump();

    await tester.tap(
      find.byKey(const ValueKey('project.settings.requirements-reviewer.reasoning')),
    );
    await tester.pump();
    await tester.tap(find.text('Medium').last);
    await tester.pump();

    expect(reviewerModel, 'gpt-5.4-mini');
    expect(reviewerReasoning, 'medium');
    expect(workerModel, 'gpt-5.4-mini');
  });

  testWidgets('planner project settings controls render model and reasoning fields only', (
    WidgetTester tester,
  ) async {
    var plannerModel = 'gpt-planner';
    var plannerReasoning = 'high';

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: StatefulBuilder(
            builder: (context, setState) {
              return ProjectRoleModelSettingsPane(
                roleKey: 'planner',
                availableModels: const [
                  ModelItem(id: 'gpt-planner', name: 'GPT Planner', hidden: false),
                  ModelItem(id: 'gpt-other', name: 'GPT Other', hidden: false),
                ],
                modelId: plannerModel,
                reasoningEffort: plannerReasoning,
                onModelChanged: (value) => setState(() => plannerModel = value),
                onReasoningChanged: (value) =>
                    setState(() => plannerReasoning = value),
              );
            },
          ),
        ),
      ),
    );

    expect(find.byKey(const ValueKey('project.settings.planner.model')), findsOneWidget);
    expect(find.byKey(const ValueKey('project.settings.planner.reasoning')), findsOneWidget);
    expect(find.text('GPT Planner'), findsOneWidget);
    expect(find.text('High'), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('project.settings.planner.model')));
    await tester.pump();
    await tester.tap(find.text('GPT Other').last);
    await tester.pump();

    expect(plannerModel, 'gpt-other');
    expect(plannerReasoning, 'high');
  });

  testWidgets('project settings project tab controls capture runtime defaults', (
    WidgetTester tester,
  ) async {
    var model = 'gpt-project';
    var reasoning = 'medium';
    var sandbox = 'workspace-write';
    var approval = 'on-request';
    var network = 'enabled';

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: StatefulBuilder(
            builder: (context, setState) {
              return Column(
                children: [
                  ProjectRoleModelSettingsPane(
                    roleKey: 'project',
                    availableModels: const [
                      ModelItem(id: 'gpt-project', name: 'GPT Project', hidden: false),
                      ModelItem(id: 'gpt-other', name: 'GPT Other', hidden: false),
                    ],
                    modelId: model,
                    reasoningEffort: reasoning,
                    onModelChanged: (value) => setState(() => model = value),
                    onReasoningChanged: (value) => setState(() => reasoning = value),
                  ),
                  ProjectDefaultRuntimeSettingsPane(
                    sandboxMode: sandbox,
                    approvalPolicy: approval,
                    networkAccessMode: network,
                    inheritedSandboxMode: 'workspace-write',
                    inheritedApprovalPolicy: 'on-request',
                    inheritedNetworkAccess: true,
                    onSandboxModeChanged: (value) => setState(() => sandbox = value),
                    onApprovalPolicyChanged: (value) => setState(() => approval = value),
                    onNetworkAccessModeChanged: (value) => setState(() => network = value),
                  ),
                ],
              );
            },
          ),
        ),
      ),
    );

    expect(find.byKey(const ValueKey('project.settings.project.model')), findsOneWidget);
    expect(find.byKey(const ValueKey('project.settings.project.reasoning')), findsOneWidget);
    expect(find.byKey(const ValueKey('project.settings.project.sandbox')), findsOneWidget);
    expect(find.byKey(const ValueKey('project.settings.project.approval')), findsOneWidget);
    expect(find.byKey(const ValueKey('project.settings.project.network')), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('project.settings.project.model')));
    await tester.pump();
    await tester.tap(find.text('GPT Other').last);
    await tester.pump();

    await tester.tap(find.byKey(const ValueKey('project.settings.project.reasoning')));
    await tester.pump();
    await tester.tap(find.text('High').last);
    await tester.pump();

    await tester.tap(find.byKey(const ValueKey('project.settings.project.sandbox')));
    await tester.pump();
    expect(find.text('Default (workspace-write)'), findsOneWidget);
    await tester.tap(find.text('Danger').last);
    await tester.pump();

    await tester.tap(find.byKey(const ValueKey('project.settings.project.approval')));
    await tester.pump();
    expect(find.text('Default (on-request)'), findsOneWidget);
    await tester.tap(find.text('never').last);
    await tester.pump();

    await tester.tap(find.byKey(const ValueKey('project.settings.project.network')));
    await tester.pump();
    expect(find.text('Default (enabled)'), findsOneWidget);
    await tester.tap(find.text('Disabled').last);
    await tester.pump();

    expect(model, 'gpt-other');
    expect(reasoning, 'high');
    expect(sandbox, 'danger-full-access');
    expect(approval, 'never');
    expect(network, 'disabled');
  });

  testWidgets('plan updates render as checklist rows', (WidgetTester tester) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: ChatTimeline(
            threadId: 'config-operator',
            entries: mockWorkbenchData.chatEntries,
            title: 'Config Operator',
            contextWindowRemainingPercent: 83,
            onSend: (_) {},
            onInterrupt: () {},
            composerEnabled: true,
            isRunning: true,
            showComposer: false,
          ),
        ),
      ),
    );
    await tester.pump();
    await tester.drag(find.byType(Scrollable).first, const Offset(0, 1000));
    await tester.pump();

    expect(find.text('Plan'), findsOneWidget);
    expect(find.text('Resume the three blocked workers with exact scope and proof constraints.'), findsOneWidget);
    expect(find.text('Keep the three QA agents held on warm simulator state pending their paired fixes.'), findsOneWidget);
    expect(find.text('Monitor for worker replies and approval requests and steer immediately per constraints.'), findsOneWidget);
    expect(find.text('Resuming interrupted QA-driven reliability sweep from existing agents without re-auditing from scratch.'), findsOneWidget);
  });

  testWidgets('requirements reviewer verdict renders as formatted card', (
    WidgetTester tester,
  ) async {
    tester.view.physicalSize = const Size(900, 620);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final boundaryKey = GlobalKey();
    const verdictJson = '''
{"overallVerdict":"pass","route":{"destination":"orchestrator","message":"Requirement passed after required short delay."},"workerDoesNotHaveToDoAnything":{"verdict":"pass","reason":"The worker slept for 20 seconds as instructed.","evidenceAssessment":"The command output shows the requested delay completed.","requiredCorrection":"None."}}
''';

    await tester.pumpWidget(
      MaterialApp(
        theme: buildRobdexTheme(),
        home: Scaffold(
          body: RepaintBoundary(
            key: boundaryKey,
            child: SizedBox(
              width: 900,
              height: 620,
              child: Padding(
                padding: const EdgeInsets.all(20),
                child: ChatTimeline(
                  threadId: 'requirements-reviewer',
                  entries: const [
                    ChatEntry(
                      id: 'verdict-1',
                      author: 'Assistant',
                      displayLabel: 'Assistant',
                      timestamp: null,
                      body: verdictJson,
                      semanticCard: ChatSemanticCard(
                        kind: 'requirementsVerdict',
                        title: 'Requirements Review Passed',
                        summary: 'Requirement passed after required short delay.',
                        tone: 'success',
                        icon: 'verified',
                        rows: [
                          ChatSemanticRow(
                            key: 'workerDoesNotHaveToDoAnything',
                            title: 'workerDoesNotHaveToDoAnything',
                            summary: 'The worker slept for 20 seconds as instructed.',
                            trailingLabel: 'Pass',
                            tone: 'success',
                            icon: 'verified',
                            bullets: [
                              'Evidence: The command output shows the requested delay completed.',
                              'Correction: None.',
                            ],
                          ),
                        ],
                      ),
                    ),
                  ],
                  title: 'Requirements Reviewer',
                  contextWindowRemainingPercent: 92,
                  onSend: (_) {},
                  onInterrupt: () {},
                  composerEnabled: false,
                  isRunning: false,
                  showComposer: false,
                ),
              ),
            ),
          ),
        ),
      ),
    );
    await tester.pump();

    expect(find.text('Requirements Review Passed'), findsOneWidget);
    expect(find.text('workerDoesNotHaveToDoAnything'), findsOneWidget);
    expect(find.textContaining('Requirement passed after required short delay.'), findsOneWidget);
    expect(find.textContaining('overallVerdict'), findsNothing);

    final boundary = boundaryKey.currentContext!.findRenderObject()! as RenderRepaintBoundary;
    final image = await boundary.toImage(pixelRatio: 1.0);
    final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
    await File('/tmp/robdex-requirements-review-verdict-card.png')
        .writeAsBytes(bytes!.buffer.asUint8List());
  }, skip: true);

  testWidgets('terminal composer button is icon-only in compact composer controls', (
    WidgetTester tester,
  ) async {
    var pressed = 0;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SizedBox(
            width: 980,
            height: 360,
            child: ChatTimeline(
              threadId: 'config-operator',
              entries: const [],
              title: 'Config Operator',
              contextWindowRemainingPercent: 92,
              onSend: (_) {},
              onInterrupt: () {},
              composerEnabled: true,
              isRunning: false,
              showComposer: true,
              selection: mockWorkbenchData.selection,
              availableModels: mockWorkbenchData.availableModels,
              onSettingsChanged: (_) {},
              terminalAvailable: true,
              onTerminalPressed: () => pressed += 1,
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Service tier'), findsNothing);
    expect(find.text('Sandbox'), findsNothing);
    expect(find.text('Network'), findsNothing);
    expect(find.text('Model'), findsNothing);
    expect(find.text('Reasoning'), findsNothing);
    expect(find.text('Role'), findsNothing);
    expect(find.text('Approval'), findsNothing);
    expect(find.byKey(const ValueKey('semantic.composer.addMenu')), findsOneWidget);
    final terminalFinder = find.byKey(const ValueKey('semantic.composer.terminal'));
    expect(terminalFinder, findsOneWidget);
    expect(find.widgetWithText(IconButton, 'Terminal'), findsNothing);

    await tester.tap(find.byTooltip('Open terminal'));
    await tester.pump();
    expect(pressed, 1);
  });

  testWidgets('planner structured output renders card and clarification buttons send pick', (
    WidgetTester tester,
  ) async {
    ComposerSubmission? submitted;
    const plannerJson = '''
{"response":"We should inspect the API boundary first.","clarification":{"question":"Which planning direction should I use?","options":[{"label":"Contract first","description":"Map DTOs before implementation."},{"label":"UI first","description":"Start from visible workflow."}]},"currentPlan":"Stripe planning"}
''';

    await tester.pumpWidget(
      MaterialApp(
        theme: buildRobdexTheme(),
        home: Scaffold(
          body: SizedBox(
            width: 900,
            height: 620,
            child: ChatTimeline(
              threadId: 'planner-1',
              entries: const [
                ChatEntry(
                  id: 'planner-response-1',
                  author: 'Assistant',
                  displayLabel: 'Assistant',
                  timestamp: null,
                  body: plannerJson,
                  semanticCard: ChatSemanticCard(
                    kind: 'plannerResponse',
                    title: 'Stripe planning',
                    summary: 'We should inspect the API boundary first.',
                    tone: 'primary',
                    icon: 'planner',
                    rows: [
                      ChatSemanticRow(
                        key: 'clarification',
                        title: 'Which planning direction should I use?',
                        summary: '',
                        tone: 'primary',
                        icon: 'question',
                      ),
                    ],
                    plannerOptions: [
                      PlannerOption(
                        label: 'Contract first',
                        description: 'Map DTOs before implementation.',
                      ),
                      PlannerOption(
                        label: 'UI first',
                        description: 'Start from visible workflow.',
                      ),
                    ],
                  ),
                ),
              ],
              title: 'Planner',
              contextWindowRemainingPercent: 90,
              onSend: (submission) => submitted = submission,
              onInterrupt: () {},
              composerEnabled: true,
              isRunning: false,
              showComposer: false,
            ),
          ),
        ),
      ),
    );

    expect(find.text('Stripe planning'), findsOneWidget);
    expect(find.text('We should inspect the API boundary first.'), findsOneWidget);
    final plannerResponseText = tester.widget<SelectableText>(
      find.widgetWithText(
        SelectableText,
        'We should inspect the API boundary first.',
      ),
    );
    expect(plannerResponseText.scrollPhysics, isA<NeverScrollableScrollPhysics>());
    expect(find.text('Which planning direction should I use?'), findsOneWidget);
    expect(find.text('Contract first'), findsOneWidget);
    expect(find.text('UI first'), findsOneWidget);

    await tester.tap(find.text('Contract first'));
    await tester.pump();

    expect(submitted?.text, 'I pick: Contract first');
    expect(submitted?.localImagePaths, isEmpty);
    expect(submitted?.requirementSetJson, isNull);
  });

  testWidgets('slash command autocomplete sets reasoning with compact feedback', (
    WidgetTester tester,
  ) async {
    ThreadSettingsDraft? draft;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SizedBox(
            width: 980,
            height: 420,
            child: ChatTimeline(
              threadId: 'config-operator',
              entries: const [],
              title: 'Config Operator',
              contextWindowRemainingPercent: 92,
              onSend: (_) {},
              onInterrupt: () {},
              composerEnabled: true,
              isRunning: false,
              showComposer: true,
              selection: mockWorkbenchData.selection,
              availableModels: mockWorkbenchData.availableModels,
              onSettingsChanged: (next) => draft = next,
            ),
          ),
        ),
      ),
    );

    await tester.enterText(find.byType(TextField).last, '/reasoning ');
    await tester.pump();
    expect(find.byKey(const ValueKey('slash.option.low')), findsOneWidget);
    expect(find.byKey(const ValueKey('slash.option.medium')), findsOneWidget);
    expect(find.text('CURRENT'), findsWidgets);

    await tester.tap(find.byKey(const ValueKey('slash.option.high')));
    await tester.pump();
    expect(draft?.reasoningEffort, 'high');
    expect(find.byKey(const ValueKey('slash.feedback')), findsOneWidget);
    expect(find.text('Reasoning set to high'), findsOneWidget);
    expect(tester.widget<TextField>(find.byType(TextField).last).controller?.text, '');
    await tester.pump(const Duration(milliseconds: 1600));
  });

  testWidgets('slash command keyboard completion and compact action use existing paths', (
    WidgetTester tester,
  ) async {
    var compactCount = 0;
    ThreadSettingsDraft? draft;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SizedBox(
            width: 980,
            height: 420,
            child: ChatTimeline(
              threadId: 'config-operator',
              entries: const [],
              title: 'Config Operator',
              contextWindowRemainingPercent: 92,
              onSend: (_) {},
              onInterrupt: () {},
              composerEnabled: true,
              isRunning: false,
              showComposer: true,
              selection: mockWorkbenchData.selection,
              availableModels: mockWorkbenchData.availableModels,
              onSettingsChanged: (next) => draft = next,
              onCompactThread: () => compactCount += 1,
            ),
          ),
        ),
      ),
    );

    await tester.enterText(find.byType(TextField).last, '/role ');
    await tester.tap(find.byType(TextField).last);
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump();
    expect(tester.widget<TextField>(find.byType(TextField).last).controller?.text, '/role orchestrator');
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(draft?.role, 'orchestrator');

    await tester.enterText(find.byType(TextField).last, '/compact');
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(compactCount, 1);
    expect(find.text('Compaction requested'), findsOneWidget);
    await tester.pump(const Duration(milliseconds: 1600));
  });

  testWidgets('handoff slash command inserts a warm handoff prompt', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SizedBox(
            width: 980,
            height: 420,
            child: ChatTimeline(
              threadId: 'config-operator',
              entries: const [],
              title: 'Config Operator',
              contextWindowRemainingPercent: 92,
              onSend: (_) {},
              onInterrupt: () {},
              composerEnabled: true,
              isRunning: false,
              showComposer: true,
              selection: mockWorkbenchData.selection,
              availableModels: mockWorkbenchData.availableModels,
            ),
          ),
        ),
      ),
    );

    await tester.enterText(find.byType(TextField).last, '/handoff');
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();

    final text = tester.widget<TextField>(find.byType(TextField).last).controller?.text ?? '';
    expect(text, contains('warm handoff'));
    expect(text, contains('new agent'));
    expect(text, contains('next best actions'));
    expect(find.text('Handoff prompt inserted'), findsOneWidget);
    await tester.pump(const Duration(milliseconds: 1600));
  });

  testWidgets('invalid slash-like text sends as normal message', (
    WidgetTester tester,
  ) async {
    ComposerSubmission? sent;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SizedBox(
            width: 980,
            height: 420,
            child: ChatTimeline(
              threadId: 'config-operator',
              entries: const [],
              title: 'Config Operator',
              contextWindowRemainingPercent: 92,
              onSend: (submission) => sent = submission,
              onInterrupt: () {},
              composerEnabled: true,
              isRunning: false,
              showComposer: true,
              selection: mockWorkbenchData.selection,
              availableModels: mockWorkbenchData.availableModels,
              onSettingsChanged: (_) {},
            ),
          ),
        ),
      ),
    );

    await tester.enterText(find.byType(TextField).last, 'Please switch to /reasoning high');
    await tester.pump();
    expect(find.byKey(const ValueKey('slash.option.reasoning')), findsNothing);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(sent?.text, 'Please switch to /reasoning high');

    await tester.enterText(find.byType(TextField).last, '/reasoning high please');
    await tester.pump();
    expect(find.byKey(const ValueKey('slash.option.high')), findsNothing);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(sent?.text, '/reasoning high please');
    await tester.pump(const Duration(milliseconds: 2500));
  });

  testWidgets('slash suggestions dismiss with escape and shift enter keeps draft', (
    WidgetTester tester,
  ) async {
    ComposerSubmission? sent;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SizedBox(
            width: 980,
            height: 420,
            child: ChatTimeline(
              threadId: 'config-operator',
              entries: const [],
              title: 'Config Operator',
              contextWindowRemainingPercent: 92,
              onSend: (submission) => sent = submission,
              onInterrupt: () {},
              composerEnabled: true,
              isRunning: false,
              showComposer: true,
              selection: mockWorkbenchData.selection,
              availableModels: mockWorkbenchData.availableModels,
              onSettingsChanged: (_) {},
            ),
          ),
        ),
      ),
    );

    final input = find.byType(TextField).last;
    await tester.enterText(input, '/');
    await tester.tap(input);
    await tester.pump();
    expect(find.byKey(const ValueKey('slash.option.reasoning')), findsOneWidget);

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(find.byKey(const ValueKey('slash.option.reasoning')), findsNothing);

    await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
    await tester.pump();
    expect(sent, isNull);

    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(sent?.text, '/');
    await tester.pump(const Duration(milliseconds: 2500));
  });

  testWidgets('terminal button opens drawer with ssh form without affecting thread list', (
    WidgetTester tester,
  ) async {
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    SharedPreferences.setMockInitialValues(<String, Object>{});
    final controller = IntegratedTerminalController();

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: Row(
            children: [
              const SizedBox(
                key: ValueKey('thread-list-pane'),
                width: 294,
                child: ColoredBox(color: Colors.black),
              ),
              Expanded(
                child: Column(
                  children: [
                    Expanded(
                      child: ChatTimeline(
                        threadId: 'config-operator',
                        entries: const [],
                        title: 'Config Operator',
                        contextWindowRemainingPercent: 92,
                        onSend: (_) {},
                        onInterrupt: () {},
                        composerEnabled: true,
                        isRunning: false,
                        showComposer: true,
                        selection: mockWorkbenchData.selection,
                        availableModels: mockWorkbenchData.availableModels,
                        onSettingsChanged: (_) {},
                        terminalAvailable: controller.isAvailable,
                        onTerminalPressed: controller.showDrawer,
                      ),
                    ),
                    IntegratedTerminalDrawer(
                      controller: controller,
                      host: 'bridge.internal',
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    final threadListSizeBefore = tester.getSize(find.byKey(const ValueKey('thread-list-pane')));
    expect(find.text('Bridge host'), findsNothing);
    expect(find.byKey(const ValueKey('semantic.composer.terminal')), findsOneWidget);

    await tester.ensureVisible(find.byTooltip('Open terminal'));
    await tester.tap(find.byTooltip('Open terminal'), warnIfMissed: false);
    await tester.pumpAndSettle();

    expect(find.text('Bridge host'), findsOneWidget);
    expect(find.text('bridge.internal'), findsOneWidget);
    expect(find.text('Username'), findsOneWidget);
    expect(find.text('Connect'), findsOneWidget);
    expect(find.byKey(const ValueKey('semantic.terminal.resizeHandle')), findsOneWidget);
    expect(controller.isDrawerVisible, true);
    expect(tester.getSize(find.byKey(const ValueKey('thread-list-pane'))), threadListSizeBefore);
    debugDefaultTargetPlatformOverride = null;
    controller.dispose();
  });

  testWidgets('terminal drawer height clamps and persists on drag end', (
    WidgetTester tester,
  ) async {
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    SharedPreferences.setMockInitialValues(<String, Object>{
      'terminal.drawerHeight': 340.0,
    });
    final controller = IntegratedTerminalController();

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: Column(
            children: [
              const Expanded(child: SizedBox()),
              IntegratedTerminalDrawer(
                controller: controller,
                host: 'bridge.internal',
              ),
            ],
          ),
        ),
      ),
    );
    controller.showDrawer();
    await tester.pumpAndSettle();
    expect(controller.drawerHeight, 340);

    await tester.drag(find.byKey(const ValueKey('semantic.terminal.resizeHandle')), const Offset(0, -80));
    await tester.pumpAndSettle();
    expect(controller.drawerHeight, 420);

    final prefs = await SharedPreferences.getInstance();
    expect(prefs.getDouble('terminal.drawerHeight'), 420);
    debugDefaultTargetPlatformOverride = null;
    controller.dispose();
  });

  testWidgets('terminal connection form hides after connected', (
    WidgetTester tester,
  ) async {
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    SharedPreferences.setMockInitialValues(<String, Object>{});
    final controller = IntegratedTerminalController();

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: Column(
            children: [
              const Expanded(child: SizedBox()),
              IntegratedTerminalDrawer(
                controller: controller,
                host: 'bridge.internal',
              ),
            ],
          ),
        ),
      ),
    );

    controller.showDrawer();
    await tester.pumpAndSettle();
    expect(find.text('Bridge host'), findsOneWidget);
    expect(find.text('Username'), findsOneWidget);
    expect(find.text('Connect'), findsOneWidget);

    controller.markConnectedForTest(
      sessionId: 'ssh-test',
      host: 'bridge.internal',
      username: 'robertsale',
    );
    await tester.pumpAndSettle();

    expect(find.text('Bridge host'), findsNothing);
    expect(find.text('Username'), findsNothing);
    expect(find.text('Connect'), findsNothing);
    expect(find.text('Connected to robertsale@bridge.internal'), findsNothing);
    expect(find.byType(TerminalView), findsOneWidget);

    debugDefaultTargetPlatformOverride = null;
    controller.dispose();
  });

  testWidgets('terminal composer button toggles drawer without closing session', (
    WidgetTester tester,
  ) async {
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    SharedPreferences.setMockInitialValues(<String, Object>{});
    final controller = IntegratedTerminalController();

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: Column(
            children: [
              Expanded(
                child: ChatTimeline(
                  threadId: 'config-operator',
                  entries: const [],
                  title: 'Config Operator',
                  contextWindowRemainingPercent: 92,
                  onSend: (_) {},
                  onInterrupt: () {},
                  composerEnabled: true,
                  isRunning: false,
                  showComposer: true,
                  selection: mockWorkbenchData.selection,
                  availableModels: mockWorkbenchData.availableModels,
                  onSettingsChanged: (_) {},
                  terminalAvailable: controller.isAvailable,
                  onTerminalPressed: controller.toggleDrawer,
                ),
              ),
              IntegratedTerminalDrawer(
                controller: controller,
                host: 'bridge.internal',
              ),
            ],
          ),
        ),
      ),
    );

    controller.markConnectedForTest(
      sessionId: 'ssh-test',
      host: 'bridge.internal',
      username: 'robertsale',
    );
    await tester.pumpAndSettle();
    expect(controller.isOpen, true);
    expect(controller.isDrawerVisible, true);
    expect(find.byType(TerminalView), findsOneWidget);

    await tester.tap(find.byTooltip('Open terminal'));
    await tester.pumpAndSettle();
    expect(controller.isOpen, true);
    expect(controller.isDrawerVisible, false);
    expect(find.byType(TerminalView), findsNothing);

    await tester.tap(find.byTooltip('Open terminal'));
    await tester.pumpAndSettle();
    expect(controller.isOpen, true);
    expect(controller.isDrawerVisible, true);
    expect(find.byType(TerminalView), findsOneWidget);
    expect(find.text('Bridge host'), findsNothing);
    expect(find.text('Username'), findsNothing);
    expect(find.text('Connect'), findsNothing);

    debugDefaultTargetPlatformOverride = null;
    controller.dispose();
  });

  testWidgets('requirements commentary packet renders summary without raw json', (
    WidgetTester tester,
  ) async {
    const commentaryJson = '''
{"summary":"Still validating bridge health before final review.","requirements":null}
''';

    await tester.pumpWidget(
      MaterialApp(
        theme: buildRobdexTheme(),
        home: Scaffold(
          body: ChatTimeline(
            threadId: 'worker',
            entries: const [
              ChatEntry(
                id: 'claim-commentary',
                author: 'Assistant',
                displayLabel: 'Assistant',
                timestamp: null,
                body: commentaryJson,
                semanticCard: ChatSemanticCard(
                  kind: 'requirementsClaim',
                  title: 'Requirements Commentary',
                  summary: 'Still validating bridge health before final review.',
                  statusLabel: 'commentary',
                  tone: 'secondary',
                  icon: 'notes',
                ),
              ),
            ],
            title: 'Worker',
            contextWindowRemainingPercent: 92,
            onSend: (_) {},
            onInterrupt: () {},
            composerEnabled: false,
            isRunning: false,
            showComposer: false,
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Requirements Commentary'), findsOneWidget);
    expect(find.text('Still validating bridge health before final review.'), findsOneWidget);
    expect(find.text('commentary'), findsOneWidget);
    expect(find.textContaining('"requirements"'), findsNothing);
  });

  testWidgets('waiver required review card is compact amber affordance', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: buildRobdexTheme(),
        home: Scaffold(
          body: ChatTimeline(
            threadId: 'worker',
            entries: const [],
            title: 'Worker',
            contextWindowRemainingPercent: 92,
            onSend: (_) {},
            onInterrupt: () {},
            composerEnabled: false,
            isRunning: false,
            showComposer: false,
            requirementReview: _waiverRequiredReviewSummary(),
            onOpenThread: (_) {},
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Human waiver required'), findsOneWidget);
    expect(find.text('Waiver needed · 1 active'), findsOneWidget);
    expect(find.text('Open review thread'), findsOneWidget);
    expect(find.byKey(const ValueKey('semantic.requirementsReview.inline')), findsOneWidget);
  });

  testWidgets('thread list uses distinct waiver required requirements badge', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: buildRobdexTheme(),
        home: Scaffold(
          body: ThreadListPanel(
            selection: mockWorkbenchData.selection,
            projects: mockWorkbenchData.projects,
            threads: [
              ThreadItem(
                id: 'worker-waiver',
                title: 'Worker Waiver',
                role: 'worker',
                projectName: mockWorkbenchData.projects.first.name,
                preview: 'Waiting for owner waiver.',
                isRunning: false,
                unreadCount: 0,
                requirementReview: _waiverRequiredReviewSummary(),
              ),
              ThreadItem(
                id: 'worker-review',
                title: 'Worker Review',
                role: 'worker',
                projectName: mockWorkbenchData.projects.first.name,
                preview: 'In review.',
                isRunning: false,
                unreadCount: 0,
                requirementReview: _reviewSummary(status: 'inReview'),
              ),
              ThreadItem(
                id: 'worker-passed',
                title: 'Worker Passed',
                role: 'worker',
                projectName: mockWorkbenchData.projects.first.name,
                preview: 'Passed.',
                isRunning: false,
                unreadCount: 0,
                requirementReview: _reviewSummary(status: 'passed'),
              ),
              ThreadItem(
                id: 'worker-failed',
                title: 'Worker Failed',
                role: 'worker',
                projectName: mockWorkbenchData.projects.first.name,
                preview: 'Failed.',
                isRunning: false,
                unreadCount: 0,
                requirementReview: _reviewSummary(status: 'failed'),
              ),
              ThreadItem(
                id: 'worker-blocked',
                title: 'Worker Blocked',
                role: 'worker',
                projectName: mockWorkbenchData.projects.first.name,
                preview: 'Blocked.',
                isRunning: false,
                unreadCount: 0,
                requirementReview: _reviewSummary(status: 'blocked'),
              ),
              ThreadItem(
                id: 'worker-active',
                title: 'Worker Active',
                role: 'worker',
                projectName: mockWorkbenchData.projects.first.name,
                preview: 'Active requirements.',
                isRunning: false,
                unreadCount: 0,
                requirementReview: _reviewSummary(status: null),
              ),
              ThreadItem(
                id: 'worker-none',
                title: 'Worker None',
                role: 'worker',
                projectName: mockWorkbenchData.projects.first.name,
                preview: 'No review.',
                isRunning: false,
                unreadCount: 0,
                requirementReview: null,
              ),
            ],
            pendingApprovals: const [],
            onDisconnect: () {},
            onGlobalSettings: () {},
            onThreadSelected: (_) {},
            onCreateProject: () {},
            onProjectSettings: (_) {},
            onCreateThread: (_) {},
            onSpawnAgent: () {},
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.byKey(const ValueKey('semantic.thread.requirements.waiverRequired')), findsOneWidget);
    expect(find.byKey(const ValueKey('semantic.thread.requirements.inReview')), findsOneWidget);
    expect(find.byKey(const ValueKey('semantic.thread.requirements.passed')), findsOneWidget);
    expect(find.byKey(const ValueKey('semantic.thread.requirements.failed')), findsOneWidget);
    expect(find.byKey(const ValueKey('semantic.thread.requirements.blocked')), findsOneWidget);
    expect(find.byKey(const ValueKey('semantic.thread.requirements.active')), findsOneWidget);
    expect(find.byTooltip('Requirements: Human waiver required'), findsOneWidget);
    expect(find.byTooltip('Requirements: In review'), findsOneWidget);
    expect(find.byTooltip('Requirements: Passed'), findsOneWidget);
    expect(find.byTooltip('Requirements: Failed'), findsOneWidget);
    expect(find.byTooltip('Requirements: Blocked'), findsOneWidget);
    expect(find.byTooltip('Requirements: Requirements active'), findsOneWidget);
  });

  testWidgets('thread list shows planner threads with planner role badge', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: buildRobdexTheme(),
        home: Scaffold(
          body: ThreadListPanel(
            selection: mockWorkbenchData.selection,
            projects: mockWorkbenchData.projects,
            threads: [
              ThreadItem(
                id: 'planner-visible',
                title: 'App Planner',
                role: 'planner',
                projectId: mockWorkbenchData.projects.first.id,
                projectRootPath: mockWorkbenchData.projects.first.rootPath,
                projectName: 'stale-display-name',
                preview: 'Planning product scope.',
                isRunning: false,
                unreadCount: 0,
                requirementReview: null,
              ),
            ],
            pendingApprovals: const [],
            onDisconnect: () {},
            onGlobalSettings: () {},
            onThreadSelected: (_) {},
            onCreateProject: () {},
            onProjectSettings: (_) {},
            onCreateThread: (_) {},
            onSpawnAgent: () {},
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('App Planner'), findsOneWidget);
    expect(find.byKey(const ValueKey('semantic.thread.roleBadge.planner')), findsOneWidget);
    expect(find.byTooltip('Planner'), findsOneWidget);
    expect(find.byIcon(Icons.psychology_alt_outlined), findsOneWidget);
    expect(find.byIcon(Icons.build_circle_outlined), findsNothing);
  });

  testWidgets('thread list long press copies thread name', (
    WidgetTester tester,
  ) async {
    String? copiedText;
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger.setMockMethodCallHandler(
      SystemChannels.platform,
      (call) async {
        if (call.method == 'Clipboard.setData') {
          copiedText = (call.arguments as Map<Object?, Object?>)['text'] as String?;
        }
        return null;
      },
    );
    addTearDown(
      () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(SystemChannels.platform, null),
    );

    await tester.pumpWidget(
      MaterialApp(
        theme: buildRobdexTheme(),
        home: ScaffoldMessenger(
          child: Scaffold(
            body: ThreadListPanel(
              selection: mockWorkbenchData.selection,
              projects: mockWorkbenchData.projects,
              threads: [
                ThreadItem(
                  id: 'worker-copy',
                  title: 'Worker Copy Target',
                  role: 'worker',
                  projectName: mockWorkbenchData.projects.first.name,
                  preview: 'Copy me.',
                  isRunning: false,
                  unreadCount: 0,
                  requirementReview: null,
                ),
              ],
              pendingApprovals: const [],
              onDisconnect: () {},
              onGlobalSettings: () {},
              onThreadSelected: (_) {},
              onCreateProject: () {},
              onProjectSettings: (_) {},
              onCreateThread: (_) {},
              onSpawnAgent: () {},
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    await tester.longPress(find.text('Worker Copy Target'));
    await tester.pumpAndSettle();
    expect(find.text('Copy name'), findsOneWidget);

    await tester.tap(find.text('Copy name'));
    await tester.pump();

    expect(copiedText, 'Worker Copy Target');
    expect(find.text('Copied "Worker Copy Target"'), findsOneWidget);
  });

  testWidgets('composer sets requirements on the thread without sending a message', (
    WidgetTester tester,
  ) async {
    Map<String, dynamic>? setRequestBody;
    var sendCount = 0;
    Future<void> setThreadRequirements(String requirementSetJson) async {
      setRequestBody = <String, dynamic>{
        'recipientThreadId': mockWorkbenchData.selection.threadId,
        'requirementSet': requirementSetJson.trim().isEmpty
            ? null
            : jsonDecode(requirementSetJson) as Map<String, dynamic>,
      };
    }

    await tester.pumpWidget(
      MaterialApp(
        theme: buildRobdexTheme(),
        home: ScaffoldMessenger(
          child: Scaffold(
            body: ComposerPanel(
              enabled: true,
              isRunning: false,
              selection: mockWorkbenchData.selection,
              availableModels: mockWorkbenchData.availableModels,
              requirementReview: null,
              loadRequirementComposables: ({senderThreadId, recipientThreadId, projectPath}) async => const <Map<String, dynamic>>[],
              setThreadRequirements: setThreadRequirements,
              onSettingsChanged: (_) {},
              onCompactThread: () {},
              onSend: (_) => sendCount += 1,
              onInterrupt: () {},
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byTooltip('Add'));
    await tester.pumpAndSettle();
    expect(find.text('Add requirements'), findsOneWidget);

    await tester.tap(find.text('Add requirements'));
    await tester.pumpAndSettle();
    expect(find.text('Set Requirements'), findsOneWidget);
    expect(find.text('Activate'), findsNothing);
    expect(find.text('Deactivate'), findsNothing);

    await tester.enterText(find.widgetWithText(TextField, 'Title'), 'Composer Set');
    await tester.enterText(
      find.widgetWithText(TextField, 'Statement'),
      'The composer must set stored active Requirements without sending a chat message.',
    );
    await tester.tap(find.text('Set'));
    await tester.pumpAndSettle();

    expect(sendCount, 0);
    expect(setRequestBody?.containsKey('senderThreadId'), isFalse);
    expect(setRequestBody?['recipientThreadId'], mockWorkbenchData.selection.threadId);
    final requirementSet = setRequestBody?['requirementSet'] as Map<String, dynamic>?;
    expect(requirementSet?['active'], isTrue);
    expect(requirementSet?['enforceOnTurns'], isTrue);
    expect(requirementSet?['requirements'], isNotEmpty);
    expect(find.text('Requirements updated.'), findsOneWidget);
  });

  testWidgets('composer replaces, clears, and deactivates stored requirements', (
    WidgetTester tester,
  ) async {
    final requestBodies = <Map<String, dynamic>>[];
    Future<void> setThreadRequirements(String requirementSetJson) async {
      requestBodies.add(<String, dynamic>{
        'recipientThreadId': mockWorkbenchData.selection.threadId,
        'requirementSet': requirementSetJson.trim().isEmpty
            ? null
            : jsonDecode(requirementSetJson) as Map<String, dynamic>,
      });
    }

    await tester.pumpWidget(
      MaterialApp(
        theme: buildRobdexTheme(),
        home: ScaffoldMessenger(
          child: Scaffold(
            body: ComposerPanel(
              enabled: true,
              isRunning: false,
              selection: mockWorkbenchData.selection,
              availableModels: mockWorkbenchData.availableModels,
              requirementReview: _reviewSummary(status: 'inReview'),
              loadRequirementComposables: ({senderThreadId, recipientThreadId, projectPath}) async => const <Map<String, dynamic>>[],
              setThreadRequirements: setThreadRequirements,
              onSettingsChanged: (_) {},
              onCompactThread: () {},
              onSend: (_) {},
              onInterrupt: () {},
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byTooltip('Add'));
    await tester.pumpAndSettle();
    expect(find.text('Replace Requirements'), findsOneWidget);

    await tester.tap(find.text('Replace Requirements'));
    await tester.pumpAndSettle();
    expect(find.text('Replace Requirements'), findsOneWidget);
    expect(find.text('Owner decision required.'), findsOneWidget);
    expect(find.text('Deactivate'), findsOneWidget);

    await tester.tap(find.text('Clear'));
    await tester.pumpAndSettle();

    expect(requestBodies.last.containsKey('senderThreadId'), isFalse);
    expect(requestBodies.last['recipientThreadId'], mockWorkbenchData.selection.threadId);
    expect(requestBodies.last['requirementSet'], isNull);
    expect(find.text('Requirements updated.'), findsOneWidget);

    await tester.tap(find.byTooltip('Add'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Replace Requirements'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Deactivate'));
    await tester.pumpAndSettle();

    final deactivated = requestBodies.last['requirementSet'] as Map<String, dynamic>?;
    expect(deactivated?['active'], isFalse);
    expect(deactivated?['enforceOnTurns'], isFalse);
    expect(deactivated?['requirements'], isNotEmpty);
  });

  testWidgets('composer activates inactive stored requirements from modal', (
    WidgetTester tester,
  ) async {
    Map<String, dynamic>? requestBody;
    Future<void> setThreadRequirements(String requirementSetJson) async {
      requestBody = <String, dynamic>{
        'recipientThreadId': mockWorkbenchData.selection.threadId,
        'requirementSet': requirementSetJson.trim().isEmpty
            ? null
            : jsonDecode(requirementSetJson) as Map<String, dynamic>,
      };
    }

    await tester.pumpWidget(
      MaterialApp(
        theme: buildRobdexTheme(),
        home: ScaffoldMessenger(
          child: Scaffold(
            body: ComposerPanel(
              enabled: true,
              isRunning: false,
              selection: mockWorkbenchData.selection,
              availableModels: mockWorkbenchData.availableModels,
              requirementReview: _reviewSummary(
                status: null,
                activeRequirementCount: 0,
                storedRequirementCount: 1,
                requirementSetActive: false,
              ),
              loadRequirementComposables: ({senderThreadId, recipientThreadId, projectPath}) async => const <Map<String, dynamic>>[],
              setThreadRequirements: setThreadRequirements,
              onSettingsChanged: (_) {},
              onCompactThread: () {},
              onSend: (_) {},
              onInterrupt: () {},
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byTooltip('Add'));
    await tester.pumpAndSettle();
    expect(find.text('Replace Requirements'), findsOneWidget);
    await tester.tap(find.text('Replace Requirements'));
    await tester.pumpAndSettle();
    expect(find.text('Activate'), findsOneWidget);
    expect(find.text('Deactivate'), findsNothing);

    await tester.tap(find.text('Activate'));
    await tester.pumpAndSettle();

    final requirementSet = requestBody?['requirementSet'] as Map<String, dynamic>?;
    expect(requirementSet?['active'], isTrue);
    expect(requirementSet?['enforceOnTurns'], isTrue);
    expect(requirementSet?['requirements'], isNotEmpty);
  });

  testWidgets('composer primary Replace submits active stored requirements without sending', (
    WidgetTester tester,
  ) async {
    Map<String, dynamic>? requestBody;
    var sendCount = 0;
    Future<void> setThreadRequirements(String requirementSetJson) async {
      requestBody = <String, dynamic>{
        'recipientThreadId': mockWorkbenchData.selection.threadId,
        'requirementSet': requirementSetJson.trim().isEmpty
            ? null
            : jsonDecode(requirementSetJson) as Map<String, dynamic>,
      };
    }

    await tester.pumpWidget(
      MaterialApp(
        theme: buildRobdexTheme(),
        home: ScaffoldMessenger(
          child: Scaffold(
            body: ComposerPanel(
              enabled: true,
              isRunning: false,
              selection: mockWorkbenchData.selection,
              availableModels: mockWorkbenchData.availableModels,
              requirementReview: _reviewSummary(status: 'inReview'),
              loadRequirementComposables: ({senderThreadId, recipientThreadId, projectPath}) async => const <Map<String, dynamic>>[],
              setThreadRequirements: setThreadRequirements,
              onSettingsChanged: (_) {},
              onCompactThread: () {},
              onSend: (_) => sendCount += 1,
              onInterrupt: () {},
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byTooltip('Add'));
    await tester.pumpAndSettle();
    expect(find.text('Replace Requirements'), findsOneWidget);
    await tester.tap(find.text('Replace Requirements'));
    await tester.pumpAndSettle();

    expect(find.text('Replace Requirements'), findsOneWidget);
    expect(find.text('Owner decision required.'), findsOneWidget);
    await tester.tap(find.text('Replace'));
    await tester.pumpAndSettle();

    expect(sendCount, 0);
    expect(requestBody?.containsKey('senderThreadId'), isFalse);
    expect(requestBody?['recipientThreadId'], mockWorkbenchData.selection.threadId);
    final requirementSet = requestBody?['requirementSet'] as Map<String, dynamic>?;
    expect(requirementSet?['active'], isTrue);
    expect(requirementSet?['enforceOnTurns'], isTrue);
    expect(requirementSet?['requirements'], isNotEmpty);
    expect(find.text('Requirements updated.'), findsOneWidget);
  });

  test('requirements JSON remains parseable for Rust-owned submission', () async {
    final jsonText = const JsonEncoder.withIndent('  ').convert({
      'id': 'requirements',
      'active': false,
      'enforceOnTurns': false,
      'requirements': [
        {'key': 'stored', 'statement': 'Stored but inactive.'},
      ],
    });
    final decoded = jsonDecode(jsonText) as Map<String, dynamic>;
    expect(decoded['active'], isFalse);
    expect(decoded['requirements'], isNotEmpty);
  });

  testWidgets('nested requirements claim packet renders claim rows without raw json', (
    WidgetTester tester,
  ) async {
    const claimJson = '''
{"summary":"Frontend rendering now understands nested Requirements packets.","requirements":{"chatRendersNullPacket":{"claim":"satisfied","evidence":["Widget test covers requirements:null rendering."],"justification":"The card renders the summary and hides raw JSON.","risk":"low"},"chatRendersClaimObject":{"claim":"satisfied","evidence":["Widget test covers nested claim rows."],"justification":"The nested requirements object supplies the displayed claim entries.","risk":"low"}}}
''';

    await tester.pumpWidget(
      MaterialApp(
        theme: buildRobdexTheme(),
        home: Scaffold(
          body: ChatTimeline(
            threadId: 'worker',
            entries: const [
              ChatEntry(
                id: 'claim-object',
                author: 'Assistant',
                displayLabel: 'Assistant',
                timestamp: null,
                body: claimJson,
                semanticCard: ChatSemanticCard(
                  kind: 'requirementsClaim',
                  title: 'Requirements Claim',
                  summary: 'Frontend rendering now understands nested Requirements packets.',
                  statusLabel: '2 claims',
                  tone: 'success',
                  icon: 'factCheck',
                  rows: [
                    ChatSemanticRow(
                      key: 'chatRendersNullPacket',
                      title: 'chatRendersNullPacket',
                      summary: 'The card renders the summary and hides raw JSON.',
                      trailingLabel: 'Satisfied · risk low',
                      tone: 'success',
                      icon: 'check',
                      bullets: ['Widget test covers requirements:null rendering.'],
                    ),
                    ChatSemanticRow(
                      key: 'chatRendersClaimObject',
                      title: 'chatRendersClaimObject',
                      summary: 'The nested requirements object supplies the displayed claim entries.',
                      trailingLabel: 'Satisfied · risk low',
                      tone: 'success',
                      icon: 'check',
                      bullets: ['Widget test covers nested claim rows.'],
                    ),
                  ],
                ),
              ),
            ],
            title: 'Worker',
            contextWindowRemainingPercent: 92,
            onSend: (_) {},
            onInterrupt: () {},
            composerEnabled: false,
            isRunning: false,
            showComposer: false,
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Requirements Claim'), findsOneWidget);
    expect(find.text('2 claims'), findsOneWidget);
    expect(find.text('Frontend rendering now understands nested Requirements packets.'), findsOneWidget);
    expect(find.text('chatRendersNullPacket'), findsOneWidget);
    expect(find.text('chatRendersClaimObject'), findsOneWidget);
    expect(find.textContaining('Widget test covers nested claim rows.'), findsOneWidget);
    expect(find.textContaining('"requirements"'), findsNothing);
  });

  test('thread stats model parses bridge DTO payload', () {
    final stats = ThreadStatsData.fromJson({
      'threadId': 'thread-a',
      'sessionPath': '/tmp/thread-a.jsonl',
      'generatedAtMs': 42,
      'totals': {
        'inputTokens': 100,
        'uncachedInputTokens': 40,
        'outputTokens': 20,
        'cachedInputTokens': 60,
        'reasoningOutputTokens': 5,
        'totalTokens': 120,
      },
      'estimates': {
        'userMessageInputTokens': 12,
        'toolOutputInputTokens': 30,
        'toolCallOutputTokens': 9,
        'skillInstructionInputTokens': 4,
      },
      'compactionCount': 2,
      'timeline': [
        {
          'index': 1,
          'line': 10,
          'inputTokens': 100,
          'uncachedInputTokens': 40,
          'outputTokens': 20,
          'cachedInputTokens': 60,
          'reasoningOutputTokens': 5,
          'totalTokens': 125,
          'deltaTokens': 125,
        },
      ],
      'categories': [
        {'key': 'tool_output', 'label': 'Tool outputs', 'tokens': 30, 'estimated': true},
        {'key': 'tool_call', 'label': 'Tool call inputs', 'tokens': 9, 'estimated': true},
      ],
      'topItems': [
        {'label': 'Tool output', 'kind': 'tool_output', 'line': 7, 'tokens': 30, 'estimated': true},
      ],
      'warnings': ['estimate only'],
    });
    expect(stats.threadId, 'thread-a');
    expect(stats.totals.inputTokens, 100);
    expect(stats.estimates.toolOutputInputTokens, 30);
    expect(stats.timeline.single.deltaTokens, 125);
  });

  testWidgets('thread stats modal waits for processing before opening rich charts', (
    WidgetTester tester,
  ) async {
    final completer = Completer<ThreadStatsData>();
    await tester.pumpWidget(
      MaterialApp(
        theme: buildRobdexTheme(),
        home: Builder(
          builder: (context) => Scaffold(
            body: TextButton(
              onPressed: () => showThreadStatsModal(
                context: context,
                threadId: 'thread-a',
                loadStats: (_) => completer.future,
              ),
              child: const Text('Open stats'),
            ),
          ),
        ),
      ),
    );

    await tester.tap(find.text('Open stats'));
    await tester.pump();
    expect(find.text('Processing thread statistics...'), findsOneWidget);
    expect(find.text('Thread Statistics'), findsNothing);

    completer.complete(_widgetStats);
    await tester.pumpAndSettle();
    expect(find.text('Thread Statistics'), findsOneWidget);
    expect(find.text('Prompt Input'), findsOneWidget);
    expect(find.text('Uncached Input'), findsOneWidget);
    expect(find.text('Output Tokens'), findsOneWidget);
    expect(find.text('Cached Tokens'), findsOneWidget);
    expect(find.text('Reasoning Tokens'), findsOneWidget);
    expect(find.text('Compactions'), findsOneWidget);
    expect(find.text('Token Timeline'), findsOneWidget);
    expect(find.text('Cumulative Usage'), findsOneWidget);
    expect(find.text('Attribution Breakdown'), findsOneWidget);
    expect(find.text('Top Expensive Items'), findsOneWidget);
  });

  testWidgets('thread stats modal reports loading failures without opening stats', (
    WidgetTester tester,
  ) async {
    final completer = Completer<ThreadStatsData>();
    await tester.pumpWidget(
      MaterialApp(
        theme: buildRobdexTheme(),
        home: Builder(
          builder: (context) => Scaffold(
            body: Builder(
              builder: (context) => TextButton(
                onPressed: () => showThreadStatsModal(
                    context: context,
                    threadId: 'thread-a',
                    loadStats: (_) => completer.future,
                  ),
                  child: const Text('Open stats'),
              ),
            ),
          ),
        ),
      ),
    );

    await tester.tap(find.text('Open stats'));
    await tester.pump();
    expect(find.text('Processing thread statistics...'), findsOneWidget);
    completer.completeError(StateError('boom'));
    await tester.pumpAndSettle();
    expect(find.textContaining('Unable to load thread statistics'), findsOneWidget);
    expect(find.text('Thread Statistics'), findsNothing);
  });

  testWidgets('requirements form can select composable packs', (
    WidgetTester tester,
  ) async {
    String? submitted;
    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) => TextButton(
            onPressed: () async {
              submitted = await showRequirementSetFormDialog(
                context,
                title: 'Requirements',
                actionLabel: 'Set',
                initialComposableItems: const [
                  {
                    'id': 'review-evidence',
                    'title': 'Review Evidence',
                    'scope': 'global',
                    'requirements': [
                      {
                        'key': 'reviewableArtifacts',
                        'statement': 'Completion proof must include exact evidence.',
                        'severity': 'high',
                        'verificationMethod': 'manualEvidence',
                      },
                    ],
                  },
                ],
              );
            },
            child: const Text('Open'),
          ),
        ),
      ),
    );

    await tester.tap(find.text('Open'));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('requirements.composable.review-evidence')), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('requirements.composable.review-evidence')));
    await tester.enterText(find.widgetWithText(TextField, 'Title'), 'Task requirements');
    await tester.enterText(find.widgetWithText(TextField, 'Statement'), 'Task-specific statement.');
    await tester.tap(find.text('Set'));
    await tester.pumpAndSettle();

    final payload = jsonDecode(submitted!) as Map<String, dynamic>;
    final requirements = payload['requirements'] as List<dynamic>;
    expect(payload['includeComposables'], contains('review-evidence'));
    expect(requirements.map((item) => item['key']), contains('reviewableArtifacts'));
  });

  testWidgets('requirements form fetches and inspects composable details', (
    WidgetTester tester,
  ) async {
    final requests = <Map<String, String?>>[];

    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) => TextButton(
            onPressed: () {
              showRequirementSetFormDialog(
                context,
                title: 'Requirements',
                actionLabel: 'Set',
                senderThreadId: 'orch-1',
                recipientThreadId: 'worker-1',
                projectPath: '/tmp/project',
                loadComposableItems: ({senderThreadId, recipientThreadId, projectPath}) async {
                  requests.add({
                    'senderThreadId': senderThreadId,
                    'recipientThreadId': recipientThreadId,
                    'projectPath': projectPath,
                  });
                  return [
                    {
                      'id': 'review-evidence',
                      'title': 'Review Evidence',
                      'description': 'Concrete review evidence.',
                      'scope': 'global',
                      'requirements': [
                        {
                          'key': 'reviewableArtifacts',
                          'statement': 'Completion proof must include exact evidence.',
                          'severity': 'high',
                          'verificationMethod': 'manualEvidence',
                        },
                      ],
                    },
                  ];
                },
              );
            },
            child: const Text('Open'),
          ),
        ),
      ),
    );

    await tester.tap(find.text('Open'));
    await tester.pumpAndSettle();
    expect(requests, hasLength(1));
    expect(find.byKey(const ValueKey('requirements.composable.review-evidence')), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('requirements.composable.inspect.review-evidence')));
    await tester.pumpAndSettle();
    expect(find.text('Concrete review evidence.'), findsOneWidget);
    expect(find.text('reviewableArtifacts'), findsOneWidget);
    expect(find.text('Completion proof must include exact evidence.'), findsOneWidget);
  });

  testWidgets('requirements form marks permanent composables as locked and included', (
    WidgetTester tester,
  ) async {
    String? submitted;
    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) => TextButton(
            onPressed: () async {
              submitted = await showRequirementSetFormDialog(
                context,
                title: 'Requirements',
                actionLabel: 'Set',
                initialComposableItems: const [
                  {
                    'id': 'no-legacy',
                    'title': 'No Legacy',
                    'description': 'Clean slate enforcement.',
                    'scope': 'global',
                    'permanent': true,
                    'permanentSource': 'project',
                    'requirements': [
                      {
                        'key': 'noLegacyLeftBehind',
                        'statement': 'Do not leave legacy behavior behind.',
                        'severity': 'blocker',
                        'verificationMethod': 'diffReview',
                      },
                    ],
                  },
                ],
              );
            },
            child: const Text('Open'),
          ),
        ),
      ),
    );

    await tester.tap(find.text('Open'));
    await tester.pumpAndSettle();
    expect(find.textContaining('no-legacy (1) | permanent'), findsOneWidget);
    await tester.enterText(find.widgetWithText(TextField, 'Title'), 'Task requirements');
    await tester.enterText(find.widgetWithText(TextField, 'Statement'), 'Task-specific statement.');
    await tester.tap(find.text('Set'));
    await tester.pumpAndSettle();

    final payload = jsonDecode(submitted!) as Map<String, dynamic>;
    final requirements = payload['requirements'] as List<dynamic>;
    expect(payload['includeComposables'], contains('no-legacy'));
    expect(requirements.map((item) => item['key']), contains('noLegacyLeftBehind'));
  });

  testWidgets('chat timeline preserves scroll position when new entries arrive away from bottom', (
    WidgetTester tester,
  ) async {
    final entries = List<ChatEntry>.generate(
      40,
      (index) => _chatEntry(index),
    );

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SizedBox(
            height: 420,
            child: ChatTimeline(
              threadId: 'thread-a',
              entries: entries,
              title: 'Thread A',
              contextWindowRemainingPercent: 80,
              onSend: (_) {},
              onInterrupt: () {},
              composerEnabled: false,
              isRunning: false,
              showComposer: false,
            ),
          ),
        ),
      ),
    );
    await tester.pump();

    final scrollable = find.byType(Scrollable);
    final initial = tester.state<ScrollableState>(scrollable).position;
    initial.jumpTo(initial.maxScrollExtent);
    await tester.pump();
    await tester.drag(scrollable, const Offset(0, 900));
    await tester.pumpAndSettle();

    final before = tester.state<ScrollableState>(scrollable).position.pixels;

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SizedBox(
            height: 420,
            child: ChatTimeline(
              threadId: 'thread-a',
              entries: [...entries, _chatEntry(40)],
              title: 'Thread A',
              contextWindowRemainingPercent: 79,
              onSend: (_) {},
              onInterrupt: () {},
              composerEnabled: false,
              isRunning: true,
              showComposer: false,
            ),
          ),
        ),
      ),
    );
    await tester.pump();

    final after = tester.state<ScrollableState>(scrollable).position.pixels;
    expect(after, moreOrLessEquals(before, epsilon: 1.0));
  });

  testWidgets('streaming assistant messages bypass markdown renderer', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SizedBox(
            height: 420,
            child: ChatTimeline(
              threadId: 'thread-a',
              entries: const [
                ChatEntry(
                  id: 'streaming-assistant',
                  author: 'Assistant',
                  displayLabel: 'Assistant',
                  timestamp: null,
                  body: 'Streaming ```dart\nvoid main() {}\n```',
                  isStreaming: true,
                ),
              ],
              title: 'Thread A',
              contextWindowRemainingPercent: 80,
              onSend: (_) {},
              onInterrupt: () {},
              composerEnabled: false,
              isRunning: true,
              showComposer: false,
            ),
          ),
        ),
      ),
    );
    await tester.pump();

    expect(
      find.byKey(const ValueKey('chat.streamingPlainText.streaming-assistant')),
      findsOneWidget,
    );
    expect(
      find.byKey(const ValueKey('chat.markdownBody.streaming-assistant')),
      findsNothing,
    );
  });

  testWidgets('completed assistant messages keep markdown renderer', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SizedBox(
            height: 420,
            child: ChatTimeline(
              threadId: 'thread-a',
              entries: const [
                ChatEntry(
                  id: 'completed-assistant',
                  author: 'Assistant',
                  displayLabel: 'Assistant',
                  timestamp: null,
                  body: 'Completed **markdown**',
                ),
              ],
              title: 'Thread A',
              contextWindowRemainingPercent: 80,
              onSend: (_) {},
              onInterrupt: () {},
              composerEnabled: false,
              isRunning: false,
              showComposer: false,
            ),
          ),
        ),
      ),
    );
    await tester.pump();

    expect(
      find.byKey(const ValueKey('chat.markdownBody.completed-assistant')),
      findsOneWidget,
    );
    expect(
      find.byKey(const ValueKey('chat.streamingPlainText.completed-assistant')),
      findsNothing,
    );
  });

  testWidgets('assistant markdown blockquotes use readable themed styling', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: ThemeData.dark(),
        home: Scaffold(
          body: SizedBox(
            height: 420,
            child: ChatTimeline(
              threadId: 'thread-a',
              entries: const [
                ChatEntry(
                  id: 'quoted-assistant',
                  author: 'Assistant',
                  displayLabel: 'Assistant',
                  timestamp: null,
                  body: '> requirements-from-prose converts prose lines.',
                ),
              ],
              title: 'Thread A',
              contextWindowRemainingPercent: 80,
              onSend: (_) {},
              onInterrupt: () {},
              composerEnabled: false,
              isRunning: false,
              showComposer: false,
            ),
          ),
        ),
      ),
    );
    await tester.pump();

    final markdown = tester.widget<MarkdownBody>(
      find.byKey(const ValueKey('chat.markdownBody.quoted-assistant')),
    );
    final blockquoteDecoration =
        markdown.styleSheet!.blockquoteDecoration as BoxDecoration;
    final blockquoteBorder = blockquoteDecoration.border as Border;

    expect(markdown.styleSheet!.blockquote!.color, isNotNull);
    expect(blockquoteDecoration.color, isNotNull);
    expect(blockquoteBorder.left.width, 3);
  });

  testWidgets('multiple assistant commentary entries remain visible with final message', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SizedBox(
            height: 420,
            child: ChatTimeline(
              threadId: 'thread-a',
              entries: const [
                ChatEntry(
                  id: 'commentary-1',
                  author: 'assistant',
                  displayLabel: 'Assistant',
                  timestamp: null,
                  body: 'First commentary.',
                ),
                ChatEntry(
                  id: 'commentary-2',
                  author: 'assistant',
                  displayLabel: 'Assistant',
                  timestamp: null,
                  body: 'Second commentary.',
                ),
                ChatEntry(
                  id: 'final-1',
                  author: 'assistant',
                  displayLabel: 'Assistant',
                  timestamp: null,
                  body: 'Final response.',
                ),
              ],
              title: 'Thread A',
              contextWindowRemainingPercent: 80,
              onSend: (_) {},
              onInterrupt: () {},
              composerEnabled: false,
              isRunning: false,
              showComposer: false,
            ),
          ),
        ),
      ),
    );
    await tester.pump();

    expect(find.text('First commentary.'), findsOneWidget);
    expect(find.text('Second commentary.'), findsOneWidget);
    expect(find.text('Final response.'), findsOneWidget);
  });

  testWidgets('chat timeline sticks to bottom when new entries arrive near bottom', (
    WidgetTester tester,
  ) async {
    final entries = List<ChatEntry>.generate(
      30,
      (index) => _chatEntry(index),
    );

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SizedBox(
            height: 420,
            child: ChatTimeline(
              threadId: 'thread-a',
              entries: entries,
              title: 'Thread A',
              contextWindowRemainingPercent: 80,
              onSend: (_) {},
              onInterrupt: () {},
              composerEnabled: false,
              isRunning: false,
              showComposer: false,
            ),
          ),
        ),
      ),
    );
    await tester.pump();

    final scrollable = find.byType(Scrollable);
    final position = tester.state<ScrollableState>(scrollable).position;
    position.jumpTo((position.maxScrollExtent - 40).clamp(0.0, position.maxScrollExtent));
    await tester.pump();

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SizedBox(
            height: 420,
            child: ChatTimeline(
              threadId: 'thread-a',
              entries: [...entries, _chatEntry(30)],
              title: 'Thread A',
              contextWindowRemainingPercent: 79,
              onSend: (_) {},
              onInterrupt: () {},
              composerEnabled: false,
              isRunning: true,
              showComposer: false,
            ),
          ),
        ),
      ),
    );
    await tester.pump();

    final after = tester.state<ScrollableState>(scrollable).position;
    expect(after.pixels, moreOrLessEquals(after.maxScrollExtent, epsilon: 1.0));
  });
}

ChatEntry _chatEntry(int index) {
  return ChatEntry(
    id: 'entry-$index',
    author: 'assistant',
    displayLabel: 'Assistant',
    timestamp: null,
    body: 'Entry $index\n${'detail ' * 20}',
  );
}

RequirementReviewSummary _waiverRequiredReviewSummary() {
  return _reviewSummary(
    status: 'waiverRequired',
    waiverRequiredCount: 1,
    verdicts: const [
      RequirementVerdictSummary(
        key: 'ownerDecision',
        verdict: 'waiverRequired',
        reason: 'Owner decision required.',
        evidenceAssessment: 'Reviewer needs human judgement.',
        requiredCorrection: 'Obtain owner waiver.',
      ),
    ],
  );
}

RequirementReviewSummary _reviewSummary({
  required String? status,
  int activeRequirementCount = 1,
  int storedRequirementCount = 1,
  bool requirementSetActive = true,
  int waiverRequiredCount = 0,
  List<RequirementVerdictSummary> verdicts = const [],
}) {
  return RequirementReviewSummary(
    activeRequirementCount: activeRequirementCount,
    storedRequirementCount: storedRequirementCount,
    requirementSetActive: requirementSetActive,
    status: status,
    reviewerThreadId: 'reviewer',
    parentThreadId: 'worker',
    requirementSetId: 'requirements',
    latestClaimPacket: null,
    latestVerdictPacket: null,
    passedCount: status == 'passed' ? 1 : 0,
    failedCount: status == 'failed' ? 1 : 0,
    blockedCount: status == 'blocked' ? 1 : 0,
    waiverRequiredCount: waiverRequiredCount,
    unknownCount: status == null || status == 'inReview' ? 1 : 0,
    updatedAt: null,
    requirements: const [
      RequirementReviewRequirement(
        key: 'ownerDecision',
        statement: 'Owner decision required.',
        severity: 'blocker',
        verificationMethod: 'manualEvidence',
      ),
    ],
    verdicts: verdicts,
  );
}

const _widgetStats = ThreadStatsData(
  threadId: 'thread-a',
  sessionPath: '/tmp/thread-a.jsonl',
  generatedAtMs: 42,
  totals: TokenTotals(
    inputTokens: 1200,
    uncachedInputTokens: 700,
    outputTokens: 300,
    cachedInputTokens: 500,
    reasoningOutputTokens: 90,
    totalTokens: 1500,
  ),
  estimates: TokenEstimates(
    userMessageInputTokens: 120,
    toolOutputInputTokens: 220,
    toolCallOutputTokens: 80,
    skillInstructionInputTokens: 60,
  ),
  compactionCount: 1,
  timeline: [
    TokenTimelinePoint(index: 1, line: 10, inputTokens: 100, uncachedInputTokens: 100, outputTokens: 20, cachedInputTokens: 0, reasoningOutputTokens: 5, totalTokens: 125, deltaTokens: 125),
    TokenTimelinePoint(index: 2, line: 20, inputTokens: 1200, uncachedInputTokens: 700, outputTokens: 300, cachedInputTokens: 500, reasoningOutputTokens: 90, totalTokens: 1215, deltaTokens: 1090),
  ],
  categories: [
    TokenCategoryBreakdown(key: 'tool_output', label: 'Tool outputs', tokens: 220, estimated: true),
    TokenCategoryBreakdown(key: 'tool_call', label: 'Tool call inputs', tokens: 80, estimated: true),
    TokenCategoryBreakdown(key: 'user_message', label: 'User messages', tokens: 120, estimated: true),
  ],
  topItems: [
    TokenTopItem(label: 'Tool output', kind: 'tool_output', line: 7, tokens: 220, estimated: true),
    TokenTopItem(label: 'User message', kind: 'user_message', line: 3, tokens: 120, estimated: true),
  ],
  warnings: ['estimate only'],
);
