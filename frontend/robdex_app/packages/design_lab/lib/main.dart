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
    if (surface == 'weeklyStats') {
      return const Scaffold(
        body: Center(
          child: SizedBox(
            width: 920,
            child: PeriodStatsView(stats: _mockPeriodStats),
          ),
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
        loadThreadStats: (_) async => _mockThreadStats,
        loadPeriodStats: (_) async => _mockPeriodStats,
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

const _mockPeriodStats = PeriodStatsData(
  label: 'Weekly quota attribution',
  startMs: 1780446600000,
  endMs: 1781029200000,
  generatedAtMs: 1781029200000,
  sessionCount: 28,
  totals: TokenTotals(
    inputTokens: 4200000,
    uncachedInputTokens: 2180000,
    outputTokens: 720000,
    cachedInputTokens: 2020000,
    reasoningOutputTokens: 210000,
    totalTokens: 5130000,
  ),
  estimates: TokenEstimates(
    userMessageInputTokens: 260000,
    toolOutputInputTokens: 1460000,
    toolCallOutputTokens: 210000,
    skillInstructionInputTokens: 88000,
  ),
  compactionCount: 11,
  categories: [
    TokenCategoryBreakdown(key: 'tool_output', label: 'Tool outputs', tokens: 1460000, estimated: true),
    TokenCategoryBreakdown(key: 'user_message', label: 'User messages', tokens: 260000, estimated: true),
    TokenCategoryBreakdown(key: 'tool_call', label: 'Tool call inputs', tokens: 210000, estimated: true),
    TokenCategoryBreakdown(key: 'assistant_message', label: 'Assistant messages', tokens: 190000, estimated: true),
    TokenCategoryBreakdown(key: 'skill_instruction', label: 'Skill instructions', tokens: 88000, estimated: true),
  ],
  topItems: [
    TokenTopItem(label: 'worker-session.jsonl · Tool output flutter test', kind: 'tool_output', line: 1204, tokens: 142000, estimated: true),
    TokenTopItem(label: 'qa-session.jsonl · Tool output design lab sweep', kind: 'tool_output', line: 841, tokens: 118000, estimated: true),
  ],
  warnings: [],
  quota: WeeklyQuotaData(
    resetAtMs: 1781051400000,
    remainingPercent: 30,
    usedPercent: 70,
    inferredStartMs: 1780446600000,
  ),
);
