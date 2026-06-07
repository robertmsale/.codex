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
    const defaultSurface = String.fromEnvironment('DESIGN_LAB_INSPECTOR', defaultValue: 'shell');
    final surface = Uri.base.queryParameters['surface'] ?? defaultSurface;
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
    if (surface == 'stats') {
      return const Scaffold(
        body: Center(
          child: ThreadStatsModalView(stats: _mockThreadStats),
        ),
      );
    }
    if (surface == 'inspector') {
      return Scaffold(
        body: Align(
          alignment: Alignment.topCenter,
          child: SizedBox(
            width: 480,
            height: 1800,
            child: InspectorPanel(
              selection: workbench.selection,
              availableModels: workbench.availableModels,
              threadGroups: const [],
              workerMetadata: workbench.workerMetadata,
              requirementReview: workbench.requirementReview,
              loadRequirementComposables: null,
              setThreadRequirements: null,
              uploadImageBytes: null,
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

const _mockThreadStats = ThreadStatsData(
  threadId: 'config-operator',
  sessionPath: '/Users/robertsale/.codex/sessions/2026/05/30/rollout-config-operator.jsonl',
  generatedAtMs: 1780185600000,
  totals: TokenTotals(
    inputTokens: 1842000,
    uncachedInputTokens: 940000,
    outputTokens: 318000,
    cachedInputTokens: 902000,
    reasoningOutputTokens: 74500,
    totalTokens: 2160000,
  ),
  estimates: TokenEstimates(
    userMessageInputTokens: 128000,
    toolOutputInputTokens: 498000,
    toolCallOutputTokens: 62000,
    skillInstructionInputTokens: 39000,
  ),
  compactionCount: 7,
  timeline: [
    TokenTimelinePoint(index: 1, line: 32, inputTokens: 12000, uncachedInputTokens: 12000, outputTokens: 1800, cachedInputTokens: 0, reasoningOutputTokens: 420, totalTokens: 14220, deltaTokens: 14220),
    TokenTimelinePoint(index: 2, line: 104, inputTokens: 54000, uncachedInputTokens: 40000, outputTokens: 9200, cachedInputTokens: 14000, reasoningOutputTokens: 1800, totalTokens: 65220, deltaTokens: 51000),
    TokenTimelinePoint(index: 3, line: 226, inputTokens: 188000, uncachedInputTokens: 111000, outputTokens: 25500, cachedInputTokens: 77000, reasoningOutputTokens: 7200, totalTokens: 208920, deltaTokens: 143700),
    TokenTimelinePoint(index: 4, line: 488, inputTokens: 476000, uncachedInputTokens: 262000, outputTokens: 85000, cachedInputTokens: 214000, reasoningOutputTokens: 22100, totalTokens: 578020, deltaTokens: 369100),
    TokenTimelinePoint(index: 5, line: 772, inputTokens: 988000, uncachedInputTokens: 486000, outputTokens: 176000, cachedInputTokens: 502000, reasoningOutputTokens: 40100, totalTokens: 1280120, deltaTokens: 702100),
    TokenTimelinePoint(index: 6, line: 1008, inputTokens: 1842000, uncachedInputTokens: 940000, outputTokens: 318000, cachedInputTokens: 902000, reasoningOutputTokens: 74500, totalTokens: 2612620, deltaTokens: 1332500),
  ],
  categories: [
    TokenCategoryBreakdown(key: 'tool_output', label: 'Tool outputs', tokens: 498000, estimated: true),
    TokenCategoryBreakdown(key: 'user_message', label: 'User messages', tokens: 128000, estimated: true),
    TokenCategoryBreakdown(key: 'tool_call', label: 'Tool call inputs', tokens: 62000, estimated: true),
    TokenCategoryBreakdown(key: 'assistant_message', label: 'Assistant messages', tokens: 88000, estimated: true),
    TokenCategoryBreakdown(key: 'skill_instruction', label: 'Skill instructions', tokens: 39000, estimated: true),
  ],
  topItems: [
    TokenTopItem(label: 'Tool output cargo test', kind: 'tool_output', line: 772, tokens: 82000, estimated: true),
    TokenTopItem(label: 'Tool output flutter analyze', kind: 'tool_output', line: 1008, tokens: 58000, estimated: true),
    TokenTopItem(label: 'User message', kind: 'user_message', line: 16, tokens: 27000, estimated: true),
    TokenTopItem(label: 'Skill instructions', kind: 'skill_instruction', line: 2, tokens: 18000, estimated: true),
  ],
  warnings: [
    'line 702: token attribution is estimated for tool payloads.',
  ],
);
