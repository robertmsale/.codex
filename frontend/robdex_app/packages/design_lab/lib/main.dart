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
    final surface = Uri.base.queryParameters['surface'] ?? 'shell';
    final workbench = mockWorkbenchData.copyWith(
      selection: const WorkspaceSelection(
        projectId: 'project-codex-home',
        projectRootPath: '/Users/robertsale/.codex',
        projectOrchestratorThreadId: null,
        projectOrchestratorName: null,
        threadId: 'config-operator',
        threadRole: 'operator',
        projectName: '.codex',
        threadName: 'Codex Config Operator',
        connectionLabel: 'Bridge Connected',
        sandboxMode: 'danger-full-access',
        networkAccess: true,
        approvalPolicy: 'on-request',
        model: 'gpt-5',
        reasoningEffort: 'medium',
        serviceTier: null,
        isRunning: false,
      ),
      workerMetadata: const WorkerMetadata(threadId: 'config-operator'),
    );
    if (surface == 'inspector') {
      return Scaffold(
        body: Align(
          alignment: Alignment.topCenter,
          child: SizedBox(
            width: 480,
            height: 1000,
            child: InspectorPanel(
              selection: workbench.selection,
              availableModels: workbench.availableModels,
              threadGroups: const [],
              workerMetadata: workbench.workerMetadata,
              requirementReview: workbench.requirementReview,
              bridgeBaseUri: null,
              onOpenThread: (_) {},
              onSettingsChanged: (_) {},
              onRunningStateChanged: (_) {},
              onRenameThread: (_) {},
              onArchiveThread: () {},
              onWarmHandoff: (_) {},
              onCreateThreadGroup: (_) {},
              onRenameThreadGroup: (_) async {},
              onDeleteThreadGroup: (_) {},
              onArchiveThreadGroup: (_) {},
              onMoveSelectedThreadToGroup: (_) {},
              onUpdateWorkerMetadata: (_) {},
            ),
          ),
        ),
      );
    }
    return Scaffold(
      body: RobdexShellScreen(
        enableGraphics: true,
        workbench: workbench,
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
      ),
    );
  }
}
