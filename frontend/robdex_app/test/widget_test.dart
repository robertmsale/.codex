import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:robdex_app/src/core/models/mock_workbench_data.dart';
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
          onCreateProject: () async {},
          onCreateThread: () async {},
          onSpawnAgent: () async {},
          onSendMessage: (_) {},
          onApprovalDecision: (_, __, ___) async {},
          onSettingsChanged: (ThreadSettingsDraft _) {},
          onRunningStateChanged: (_) {},
          onRenameThread: (_) {},
          onArchiveThread: () {},
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
    expect(find.text('Workspace'), findsOneWidget);
    expect(find.text('Inspector'), findsOneWidget);
  });
}
