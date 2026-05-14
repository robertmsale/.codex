import 'package:flutter/material.dart';
import 'package:robdex_design_system/robdex_design_system.dart';

void main() {
  runApp(const RobdexDesignLabApp());
}

class RobdexDesignLabApp extends StatelessWidget {
  const RobdexDesignLabApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      debugShowCheckedModeBanner: false,
      title: 'Robdex Design Lab',
      theme: buildRobdexTheme(),
      home: const RobdexDesignLabHome(),
    );
  }
}

class RobdexDesignLabHome extends StatelessWidget {
  const RobdexDesignLabHome({super.key});

  @override
  Widget build(BuildContext context) {
    return RobdexShellScreen(
      enableGraphics: true,
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
    );
  }
}
