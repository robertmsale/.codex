import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:robdex_design_system/robdex_design_system.dart';

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
