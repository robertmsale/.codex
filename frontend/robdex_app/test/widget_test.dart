import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:robdex_app/src/terminal/integrated_terminal.dart';
import 'package:robdex_design_system/robdex_design_system.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:xterm/xterm.dart';

void main() {
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

  testWidgets('terminal composer button is icon-only and placed after network', (
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

    final networkFinder = find.byKey(const ValueKey('semantic.composer.networkDropdown'));
    final terminalFinder = find.byKey(const ValueKey('semantic.composer.terminal'));
    expect(networkFinder, findsOneWidget);
    expect(terminalFinder, findsOneWidget);
    expect(find.widgetWithText(IconButton, 'Terminal'), findsNothing);
    expect(tester.getTopLeft(terminalFinder).dx, greaterThan(tester.getTopLeft(networkFinder).dx));

    await tester.tap(find.byTooltip('Open terminal'));
    await tester.pump();
    expect(pressed, 1);
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
