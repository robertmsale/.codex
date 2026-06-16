import 'dart:ui';

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
    if (surface == 'brushedMetalShader') {
      return const Scaffold(
        backgroundColor: Color(0xFF05090F),
        body: Center(
          child: SizedBox(
            width: 520,
            height: 520,
            child: _BrushedMetalShaderSpecimen(),
          ),
        ),
      );
    }
    if (surface == 'sidebar') {
      return Scaffold(
        body: SizedBox.expand(
          child: ThreadListPanel(
            selection: workbench.selection,
            projects: workbench.projects,
            threads: workbench.threads,
            pendingApprovals: workbench.pendingApprovals,
            onDisconnect: () {},
            onGlobalSettings: () {},
            onThreadSelected: (_) {},
            onCreateProject: () {},
            onProjectSettings: (_) {},
            onCreateThread: (_) {},
            onSpawnAgent: () {},
            onWeeklyStats: () {},
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
    if (surface == 'agentRuntimeDisconnected') {
      return _AgentRuntimeScenario(data: mockAgentRuntimeDisconnected);
    }
    if (surface == 'agentRuntimeConnecting') {
      return _AgentRuntimeScenario(data: mockAgentRuntimeConnecting);
    }
    if (surface == 'agentRuntimeConnected') {
      return _AgentRuntimeScenario(data: mockAgentRuntimeConnected);
    }
    if (surface == 'agentRuntimeError') {
      return _AgentRuntimeScenario(data: mockAgentRuntimeError);
    }
    if (surface == 'agentRuntimeEmpty') {
      return _AgentRuntimeScenario(data: mockAgentRuntimeEmpty);
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
        loadFullSizeImage: (_) async => const FullSizeImageData(
          path: '/tmp/robdex-design-lab-static.png',
          bytesBase64: '',
          contentType: 'image/png',
        ),
      ),
    );
  }
}

class _AgentRuntimeScenario extends StatefulWidget {
  const _AgentRuntimeScenario({required this.data});

  final AgentRuntimeControlTowerData data;

  @override
  State<_AgentRuntimeScenario> createState() => _AgentRuntimeScenarioState();
}

class _AgentRuntimeScenarioState extends State<_AgentRuntimeScenario> {
  late final TextEditingController _baseUrlController;

  @override
  void initState() {
    super.initState();
    _baseUrlController = TextEditingController(text: widget.data.baseUrl);
  }

  @override
  void dispose() {
    _baseUrlController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: AgentRuntimeControlTower(
        data: widget.data,
        baseUrlController: _baseUrlController,
        onConnect: () {},
        onRefreshDiscovery: () {},
        onConnectDiscovered: () {},
        onPollStream: () {},
        onDisconnect: () {},
        onRoleValidate: (_) {},
        onRoleCreate: (_) {},
        onRoleUpdate: (_) {},
        onRoleExport: (_) {},
        onRoleArchive: (_) {},
        onRoleUnarchive: (_) {},
        onRoleActivate: (_, _) {},
      ),
    );
  }
}


class _BrushedMetalShaderSpecimen extends StatefulWidget {
  const _BrushedMetalShaderSpecimen();

  @override
  State<_BrushedMetalShaderSpecimen> createState() => _BrushedMetalShaderSpecimenState();
}

class _BrushedMetalShaderSpecimenState extends State<_BrushedMetalShaderSpecimen> {
  FragmentShader? _shader;

  @override
  void initState() {
    super.initState();
    _loadShader();
  }

  Future<void> _loadShader() async {
    try {
      final program = await FragmentProgram.fromAsset(
        'packages/robdex_design_system/shaders/brushed_metal_sidebar.frag',
      );
      if (!mounted) {
        return;
      }
      setState(() {
        _shader = program.fragmentShader();
      });
    } catch (_) {}
  }

  @override
  void dispose() {
    _shader?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(24),
        border: Border.all(color: Colors.white.withValues(alpha: 0.14)),
        boxShadow: [
          BoxShadow(
            color: Colors.black.withValues(alpha: 0.36),
            blurRadius: 36,
            offset: const Offset(0, 18),
          ),
        ],
      ),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(24),
        child: CustomPaint(
          painter: _BrushedMetalShaderSpecimenPainter(shader: _shader),
          child: const SizedBox.expand(),
        ),
      ),
    );
  }
}

class _BrushedMetalShaderSpecimenPainter extends CustomPainter {
  const _BrushedMetalShaderSpecimenPainter({required this.shader});

  final FragmentShader? shader;

  @override
  void paint(Canvas canvas, Size size) {
    final rect = Offset.zero & size;
    canvas.drawRect(
      rect,
      Paint()
        ..shader = const LinearGradient(
          begin: Alignment.topLeft,
          end: Alignment.bottomRight,
          colors: [
            Color(0xFF121923),
            Color(0xFF071018),
            Color(0xFF101821),
          ],
          stops: [0.0, 0.56, 1.0],
        ).createShader(rect),
    );

    final activeShader = shader;
    if (activeShader != null) {
      activeShader.setFloat(0, size.width);
      activeShader.setFloat(1, size.height);
      activeShader.setFloat(2, 0.0);
      canvas.drawRect(rect, Paint()..shader = activeShader);
    }
  }

  @override
  bool shouldRepaint(covariant _BrushedMetalShaderSpecimenPainter oldDelegate) {
    return oldDelegate.shader != shader;
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
