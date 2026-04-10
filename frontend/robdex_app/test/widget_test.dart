import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:robdex_app/src/core/models/mock_workbench_data.dart';
import 'package:robdex_app/src/features/chat/chat_timeline.dart';
import 'package:robdex_app/src/features/inspector/inspector_panel.dart';
import 'package:robdex_app/src/features/shell/robdex_shell_screen.dart';

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
          onApprovalDecision: (_, __, ___) async {},
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

    expect(find.text('Resume the three blocked workers with exact scope and proof constraints.'), findsOneWidget);
    expect(find.text('Keep the three QA agents held on warm simulator state pending their paired fixes.'), findsOneWidget);
    expect(find.text('Active'), findsOneWidget);
    expect(find.text('Resuming interrupted QA-driven reliability sweep from existing agents without re-auditing from scratch.'), findsOneWidget);
  });
}
