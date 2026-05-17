import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:robdex_app/src/app/robdex_app.dart';
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
          enableGraphics: true,
        ),
      ),
    );
    await tester.pump(const Duration(milliseconds: 250));

    expect(find.text('Codex Control Plane'), findsAtLeastNWidgets(1));
    expect(find.text('Config Operator'), findsAtLeastNWidgets(1));
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
    await tester.pumpAndSettle();

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
    await tester.drag(scrollable, const Offset(0, -900));
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
    position.jumpTo(position.maxScrollExtent);
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
  int waiverRequiredCount = 0,
  List<RequirementVerdictSummary> verdicts = const [],
}) {
  return RequirementReviewSummary(
    activeRequirementCount: 1,
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
