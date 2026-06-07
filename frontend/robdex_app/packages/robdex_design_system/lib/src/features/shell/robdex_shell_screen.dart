import 'dart:ui';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

import '../../core/models/workbench_view_data.dart';
import '../../core/models/workbench_models.dart';
import '../../core/models/thread_stats_models.dart';
import '../chat/chat_timeline.dart';
import '../composer/composer_panel.dart';
import '../inspector/inspector_panel.dart';
import '../requirements/requirement_set_form.dart';
import '../sidebar/thread_list_panel.dart';
import '../stats/thread_stats_modal.dart';

class RobdexShellScreen extends StatelessWidget {
  const RobdexShellScreen({
    super.key,
    required this.enableGraphics,
    required this.workbench,
    required this.onThreadSelected,
    required this.onProjectSelected,
    required this.onDisconnect,
    required this.onGlobalSettings,
    required this.onCreateProject,
    required this.onProjectSettings,
    required this.onCreateThread,
    required this.onSpawnAgent,
    required this.onSendMessage,
    required this.onOpenHistory,
    required this.onCompactThread,
    required this.onTerminateCommandExecution,
    required this.onInterruptThread,
    required this.onApprovalDecision,
    required this.onSettingsChanged,
    required this.onRunningStateChanged,
    required this.onRenameThread,
    required this.onArchiveThread,
    required this.onWarmHandoff,
    required this.onSetProjectOrchestrator,
    required this.onCreateThreadGroup,
    required this.onRenameThreadGroup,
    required this.onDeleteThreadGroup,
    required this.onArchiveThreadGroup,
    required this.onMoveSelectedThreadToGroup,
    required this.onUpdateWorkerMetadata,
    required this.loadThreadStats,
    this.loadRequirementComposables,
    this.setThreadRequirements,
    this.uploadImageBytes,
    this.onOpenLink,
    this.chatBottomDrawer,
    this.terminalAvailable = false,
    this.onTerminalPressed,
  });

  final bool enableGraphics;
  final WorkbenchViewData workbench;
  final ValueChanged<String> onThreadSelected;
  final ValueChanged<String> onProjectSelected;
  final VoidCallback onDisconnect;
  final VoidCallback onGlobalSettings;
  final VoidCallback onCreateProject;
  final ValueChanged<ProjectItem> onProjectSettings;
  final ValueChanged<ProjectItem> onCreateThread;
  final VoidCallback onSpawnAgent;
  final ValueChanged<ComposerSubmission> onSendMessage;
  final VoidCallback onOpenHistory;
  final VoidCallback onCompactThread;
  final ValueChanged<String> onTerminateCommandExecution;
  final VoidCallback onInterruptThread;
  final Future<void> Function(PendingApprovalItem approval, String decision, String? message)
      onApprovalDecision;
  final ValueChanged<ThreadSettingsDraft> onSettingsChanged;
  final ValueChanged<bool> onRunningStateChanged;
  final ValueChanged<String> onRenameThread;
  final VoidCallback onArchiveThread;
  final ValueChanged<String> onWarmHandoff;
  final VoidCallback onSetProjectOrchestrator;
  final ValueChanged<String> onCreateThreadGroup;
  final Future<void> Function(ThreadGroupItem group) onRenameThreadGroup;
  final ValueChanged<String> onDeleteThreadGroup;
  final ValueChanged<String> onArchiveThreadGroup;
  final ValueChanged<String?> onMoveSelectedThreadToGroup;
  final ValueChanged<WorkerMetadataDraft> onUpdateWorkerMetadata;
  final Future<ThreadStatsData> Function(String threadId) loadThreadStats;
  final RequirementComposableLoader? loadRequirementComposables;
  final Future<void> Function(String recipientThreadId, String requirementSetJson)? setThreadRequirements;
  final ImageBytesUploader? uploadImageBytes;
  final ValueChanged<String>? onOpenLink;
  final Widget? chatBottomDrawer;
  final bool terminalAvailable;
  final VoidCallback? onTerminalPressed;

  @override
  Widget build(BuildContext context) {
    final isIOS = defaultTargetPlatform == TargetPlatform.iOS;
    return Scaffold(
      body: DecoratedBox(
        decoration: const BoxDecoration(color: Color(0xFF05090F)),
        child: SafeArea(
          child: Stack(
            children: [
              if (enableGraphics && !isIOS && !kIsWeb)
                const Positioned.fill(child: _ShellNebulaBackdrop()),
              LayoutBuilder(
                builder: (context, constraints) {
                  final isCompact = constraints.maxWidth < 860;
                  return isCompact
                        ? RepaintBoundary(
                            child: _CompactShell(
                              workbench: workbench,
                              onThreadSelected: onThreadSelected,
                              onDisconnect: onDisconnect,
                              onGlobalSettings: onGlobalSettings,
                              onCreateProject: onCreateProject,
                              onProjectSettings: onProjectSettings,
                              onCreateThread: onCreateThread,
                              onSpawnAgent: onSpawnAgent,
                              onSendMessage: onSendMessage,
                              onOpenHistory: onOpenHistory,
                              onCompactThread: onCompactThread,
                              onTerminateCommandExecution: onTerminateCommandExecution,
                              onInterruptThread: onInterruptThread,
                              onApprovalDecision: onApprovalDecision,
                              onSettingsChanged: onSettingsChanged,
                              onRunningStateChanged: onRunningStateChanged,
                              onRenameThread: onRenameThread,
                              onArchiveThread: onArchiveThread,
                              onWarmHandoff: onWarmHandoff,
                              onSetProjectOrchestrator: onSetProjectOrchestrator,
                              onCreateThreadGroup: onCreateThreadGroup,
                              onRenameThreadGroup: onRenameThreadGroup,
                              onDeleteThreadGroup: onDeleteThreadGroup,
                              onArchiveThreadGroup: onArchiveThreadGroup,
                              onMoveSelectedThreadToGroup: onMoveSelectedThreadToGroup,
                              onUpdateWorkerMetadata: onUpdateWorkerMetadata,
                              loadThreadStats: loadThreadStats,
                              loadRequirementComposables: loadRequirementComposables,
                              setThreadRequirements: setThreadRequirements,
                              uploadImageBytes: uploadImageBytes,
                              onOpenLink: onOpenLink,
                              terminalAvailable: terminalAvailable,
                              onTerminalPressed: onTerminalPressed,
                            ),
                          )
                        : RepaintBoundary(
                            child: _WideShell(
                              workbench: workbench,
                              onThreadSelected: onThreadSelected,
                              onDisconnect: onDisconnect,
                              onGlobalSettings: onGlobalSettings,
                              onCreateProject: onCreateProject,
                              onProjectSettings: onProjectSettings,
                              onCreateThread: onCreateThread,
                              onSpawnAgent: onSpawnAgent,
                              onSendMessage: onSendMessage,
                              onOpenHistory: onOpenHistory,
                              onCompactThread: onCompactThread,
                              onTerminateCommandExecution: onTerminateCommandExecution,
                              onInterruptThread: onInterruptThread,
                              onApprovalDecision: onApprovalDecision,
                              onSettingsChanged: onSettingsChanged,
                              onRunningStateChanged: onRunningStateChanged,
                              onRenameThread: onRenameThread,
                              onArchiveThread: onArchiveThread,
                              onWarmHandoff: onWarmHandoff,
                              onSetProjectOrchestrator: onSetProjectOrchestrator,
                              onCreateThreadGroup: onCreateThreadGroup,
                              onRenameThreadGroup: onRenameThreadGroup,
                              onDeleteThreadGroup: onDeleteThreadGroup,
                              onArchiveThreadGroup: onArchiveThreadGroup,
                              onMoveSelectedThreadToGroup: onMoveSelectedThreadToGroup,
                              onUpdateWorkerMetadata: onUpdateWorkerMetadata,
                              loadThreadStats: loadThreadStats,
                              loadRequirementComposables: loadRequirementComposables,
                              setThreadRequirements: setThreadRequirements,
                              uploadImageBytes: uploadImageBytes,
                              onOpenLink: onOpenLink,
                              chatBottomDrawer: chatBottomDrawer,
                              terminalAvailable: terminalAvailable,
                              onTerminalPressed: onTerminalPressed,
                            ),
                          );
                },
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _WideShell extends StatefulWidget {
  const _WideShell({
    required this.workbench,
    required this.onThreadSelected,
    required this.onDisconnect,
    required this.onGlobalSettings,
    required this.onCreateProject,
    required this.onProjectSettings,
    required this.onCreateThread,
    required this.onSpawnAgent,
    required this.onSendMessage,
    required this.onOpenHistory,
    required this.onCompactThread,
    required this.onTerminateCommandExecution,
    required this.onInterruptThread,
    required this.onApprovalDecision,
    required this.onSettingsChanged,
    required this.onRunningStateChanged,
    required this.onRenameThread,
    required this.onArchiveThread,
    required this.onWarmHandoff,
    required this.onSetProjectOrchestrator,
    required this.onCreateThreadGroup,
    required this.onRenameThreadGroup,
    required this.onDeleteThreadGroup,
    required this.onArchiveThreadGroup,
    required this.onMoveSelectedThreadToGroup,
    required this.onUpdateWorkerMetadata,
    required this.loadThreadStats,
    required this.loadRequirementComposables,
    required this.setThreadRequirements,
    required this.uploadImageBytes,
    required this.onOpenLink,
    required this.chatBottomDrawer,
    required this.terminalAvailable,
    required this.onTerminalPressed,
  });

  final WorkbenchViewData workbench;
  final ValueChanged<String> onThreadSelected;
  final VoidCallback onDisconnect;
  final VoidCallback onGlobalSettings;
  final VoidCallback onCreateProject;
  final ValueChanged<ProjectItem> onProjectSettings;
  final ValueChanged<ProjectItem> onCreateThread;
  final VoidCallback onSpawnAgent;
  final ValueChanged<ComposerSubmission> onSendMessage;
  final VoidCallback onOpenHistory;
  final VoidCallback onCompactThread;
  final ValueChanged<String> onTerminateCommandExecution;
  final VoidCallback onInterruptThread;
  final Future<void> Function(PendingApprovalItem approval, String decision, String? message)
      onApprovalDecision;
  final ValueChanged<ThreadSettingsDraft> onSettingsChanged;
  final ValueChanged<bool> onRunningStateChanged;
  final ValueChanged<String> onRenameThread;
  final VoidCallback onArchiveThread;
  final ValueChanged<String> onWarmHandoff;
  final VoidCallback onSetProjectOrchestrator;
  final ValueChanged<String> onCreateThreadGroup;
  final Future<void> Function(ThreadGroupItem group) onRenameThreadGroup;
  final ValueChanged<String> onDeleteThreadGroup;
  final ValueChanged<String> onArchiveThreadGroup;
  final ValueChanged<String?> onMoveSelectedThreadToGroup;
  final ValueChanged<WorkerMetadataDraft> onUpdateWorkerMetadata;
  final Future<ThreadStatsData> Function(String threadId) loadThreadStats;
  final RequirementComposableLoader? loadRequirementComposables;
  final Future<void> Function(String recipientThreadId, String requirementSetJson)? setThreadRequirements;
  final ImageBytesUploader? uploadImageBytes;
  final ValueChanged<String>? onOpenLink;
  final Widget? chatBottomDrawer;
  final bool terminalAvailable;
  final VoidCallback? onTerminalPressed;

  @override
  State<_WideShell> createState() => _WideShellState();
}

class _WideShellState extends State<_WideShell> {
  double _sidebarWidth = 294;

  void _resizeSidebar(double delta) {
    setState(() {
      _sidebarWidth = (_sidebarWidth + delta).clamp(260, 420);
    });
  }

  @override
  Widget build(BuildContext context) {
    final workbench = widget.workbench;
    final shell = Row(
      children: [
          AnimatedContainer(
            duration: const Duration(milliseconds: 180),
            curve: Curves.easeOutCubic,
            width: _sidebarWidth,
            child: DecoratedBox(
              decoration: const BoxDecoration(
                color: Color(0xCC12161D),
                border: Border(
                  right: BorderSide(color: Color(0xFF30343B)),
                ),
              ),
              child: Padding(
                padding: const EdgeInsets.fromLTRB(10, 10, 10, 12),
                child: RepaintBoundary(
                  child: ThreadListPanel(
                    selection: workbench.selection,
                    projects: workbench.projects,
                    threads: workbench.threads,
                    pendingApprovals: workbench.pendingApprovals,
                    onDisconnect: widget.onDisconnect,
                    onGlobalSettings: widget.onGlobalSettings,
                    onThreadSelected: widget.onThreadSelected,
                    onCreateProject: widget.onCreateProject,
                    onProjectSettings: widget.onProjectSettings,
                    onCreateThread: widget.onCreateThread,
                    onSpawnAgent: widget.onSpawnAgent,
                  ),
                ),
              ),
            ),
          ),
          _SidebarResizeHandle(onDrag: _resizeSidebar),
          Expanded(
            child: DecoratedBox(
              decoration: const BoxDecoration(color: Color(0xFF171C22)),
              child: Padding(
                padding: const EdgeInsets.fromLTRB(28, 16, 62, 22),
                child: RepaintBoundary(
                  child: Column(
                    children: [
                      Expanded(
                        child: ChatTimeline(
                          threadId: workbench.selection.threadId,
                          entries: workbench.chatEntries,
                          title: workbench.selection.threadName,
                          contextWindowRemainingPercent:
                              workbench.contextWindowRemainingPercent,
                          onSend: widget.onSendMessage,
                          onInterrupt: widget.onInterruptThread,
                          onTerminateCommandExecution:
                              widget.onTerminateCommandExecution,
                          loadRequirementComposables: widget.loadRequirementComposables,
                          setThreadRequirements: widget.setThreadRequirements == null
                              ? null
                              : (requirementSetJson) => widget.setThreadRequirements!(
                                    workbench.selection.threadId ?? '',
                                    requirementSetJson,
                                  ),
                          uploadImageBytes: widget.uploadImageBytes,
                          onOpenLink: widget.onOpenLink,
                          composerEnabled: workbench.selection.threadId != null,
                          isRunning: workbench.selection.isRunning,
                          selection: workbench.selection,
                          availableModels: workbench.availableModels,
                          onSettingsChanged: widget.onSettingsChanged,
                          onCompactThread: widget.onCompactThread,
                          requirementReview: workbench.requirementReview,
                          onOpenThread: widget.onThreadSelected,
                          terminalAvailable: widget.terminalAvailable,
                          onTerminalPressed: widget.onTerminalPressed,
                          headerControls: _DesktopThreadControls(
                            selection: workbench.selection,
                            liveProcesses: workbench.liveProcesses,
                            pendingApprovalCount:
                                workbench.pendingApprovals.length,
                            onOpenHistory: widget.onOpenHistory,
                            loadThreadStats: widget.loadThreadStats,
                            onCompactThread: widget.onCompactThread,
                            onTerminateCommandExecution:
                                widget.onTerminateCommandExecution,
                            onMore: () => _showInspectorDialog(
                              context,
                              workbench: workbench,
                              onThreadSelected: widget.onThreadSelected,
                              loadRequirementComposables: widget.loadRequirementComposables,
                              setThreadRequirements: widget.setThreadRequirements,
                              uploadImageBytes: widget.uploadImageBytes,
                              onApprovalDecision: widget.onApprovalDecision,
                              onSettingsChanged: widget.onSettingsChanged,
                              onRunningStateChanged:
                                  widget.onRunningStateChanged,
                              onRenameThread: widget.onRenameThread,
                              onArchiveThread: widget.onArchiveThread,
                              onWarmHandoff: widget.onWarmHandoff,
                              onSetProjectOrchestrator:
                                  widget.onSetProjectOrchestrator,
                              onCreateThreadGroup: widget.onCreateThreadGroup,
                              onRenameThreadGroup:
                                  widget.onRenameThreadGroup,
                              onDeleteThreadGroup:
                                  widget.onDeleteThreadGroup,
                              onArchiveThreadGroup:
                                  widget.onArchiveThreadGroup,
                              onMoveSelectedThreadToGroup:
                                  widget.onMoveSelectedThreadToGroup,
                              onUpdateWorkerMetadata:
                                  widget.onUpdateWorkerMetadata,
                            ),
                          ),
                          overlay: _ApprovalOverlay(
                            selection: workbench.selection,
                            pendingApprovals: workbench.pendingApprovals,
                            onApprovalDecision: widget.onApprovalDecision,
                          ),
                        ),
                      ),
                      if (widget.chatBottomDrawer != null)
                        widget.chatBottomDrawer!,
                    ],
                  ),
                ),
              ),
            ),
          ),
      ],
    );
    return shell;
  }
}

class _CompactShell extends StatefulWidget {
  const _CompactShell({
    required this.workbench,
    required this.onThreadSelected,
    required this.onDisconnect,
    required this.onGlobalSettings,
    required this.onCreateProject,
    required this.onProjectSettings,
    required this.onCreateThread,
    required this.onSpawnAgent,
    required this.onSendMessage,
    required this.onOpenHistory,
    required this.onCompactThread,
    required this.onTerminateCommandExecution,
    required this.onInterruptThread,
    required this.onApprovalDecision,
    required this.onSettingsChanged,
    required this.onRunningStateChanged,
    required this.onRenameThread,
    required this.onArchiveThread,
    required this.onWarmHandoff,
    required this.onSetProjectOrchestrator,
    required this.onCreateThreadGroup,
    required this.onRenameThreadGroup,
    required this.onDeleteThreadGroup,
    required this.onArchiveThreadGroup,
    required this.onMoveSelectedThreadToGroup,
    required this.onUpdateWorkerMetadata,
    required this.loadThreadStats,
    required this.loadRequirementComposables,
    required this.setThreadRequirements,
    required this.uploadImageBytes,
    required this.onOpenLink,
    required this.terminalAvailable,
    required this.onTerminalPressed,
  });

  final WorkbenchViewData workbench;
  final ValueChanged<String> onThreadSelected;
  final VoidCallback onDisconnect;
  final VoidCallback onGlobalSettings;
  final VoidCallback onCreateProject;
  final ValueChanged<ProjectItem> onProjectSettings;
  final ValueChanged<ProjectItem> onCreateThread;
  final VoidCallback onSpawnAgent;
  final ValueChanged<ComposerSubmission> onSendMessage;
  final VoidCallback onOpenHistory;
  final VoidCallback onCompactThread;
  final ValueChanged<String> onTerminateCommandExecution;
  final VoidCallback onInterruptThread;
  final Future<void> Function(PendingApprovalItem approval, String decision, String? message)
      onApprovalDecision;
  final ValueChanged<ThreadSettingsDraft> onSettingsChanged;
  final ValueChanged<bool> onRunningStateChanged;
  final ValueChanged<String> onRenameThread;
  final VoidCallback onArchiveThread;
  final ValueChanged<String> onWarmHandoff;
  final VoidCallback onSetProjectOrchestrator;
  final ValueChanged<String> onCreateThreadGroup;
  final Future<void> Function(ThreadGroupItem group) onRenameThreadGroup;
  final ValueChanged<String> onDeleteThreadGroup;
  final ValueChanged<String> onArchiveThreadGroup;
  final ValueChanged<String?> onMoveSelectedThreadToGroup;
  final ValueChanged<WorkerMetadataDraft> onUpdateWorkerMetadata;
  final Future<ThreadStatsData> Function(String threadId) loadThreadStats;
  final RequirementComposableLoader? loadRequirementComposables;
  final Future<void> Function(String recipientThreadId, String requirementSetJson)? setThreadRequirements;
  final ImageBytesUploader? uploadImageBytes;
  final ValueChanged<String>? onOpenLink;
  final bool terminalAvailable;
  final VoidCallback? onTerminalPressed;

  @override
  State<_CompactShell> createState() => _CompactShellState();
}

class _ShellNebulaBackdrop extends StatefulWidget {
  const _ShellNebulaBackdrop();

  @override
  State<_ShellNebulaBackdrop> createState() => _ShellNebulaBackdropState();
}

class _ShellNebulaBackdropState extends State<_ShellNebulaBackdrop>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller;
  FragmentShader? _shader;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      vsync: this,
      duration: const Duration(days: 1),
    )..repeat();
    _loadShader();
  }

  Future<void> _loadShader() async {
    try {
      final program = await FragmentProgram.fromAsset(
        'packages/robdex_design_system/shaders/connection_nebula.frag',
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
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return IgnorePointer(
      child: Stack(
        fit: StackFit.expand,
        children: [
          DecoratedBox(
            decoration: BoxDecoration(
              gradient: LinearGradient(
                begin: Alignment.topLeft,
                end: Alignment.bottomRight,
                colors: [
                  const Color(0xFF071018).withValues(alpha: 0.48),
                  const Color(0xFF0C1622).withValues(alpha: 0.34),
                  const Color(0xFF101D2B).withValues(alpha: 0.4),
                ],
                stops: const [0.0, 0.44, 1.0],
              ),
            ),
          ),
          if (_shader != null)
            RepaintBoundary(
              child: CustomPaint(
                painter: _ShellNebulaPainter(
                  animation: _controller,
                  shader: _shader!,
                ),
              ),
            ),
          DecoratedBox(
            decoration: BoxDecoration(
              gradient: RadialGradient(
                center: const Alignment(0, -0.08),
                radius: 0.88,
                colors: [
                  const Color(0xFF2A5E9B).withValues(alpha: 0.035),
                  Colors.transparent,
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _ShellNebulaPainter extends CustomPainter {
  const _ShellNebulaPainter({
    required this.animation,
    required this.shader,
  }) : super(repaint: animation);

  final AnimationController animation;
  final FragmentShader shader;

  @override
  void paint(Canvas canvas, Size size) {
    final elapsedSeconds =
        (animation.lastElapsedDuration?.inMilliseconds ?? 0) / 1000.0;
    shader.setFloat(0, size.width);
    shader.setFloat(1, size.height);
    shader.setFloat(2, elapsedSeconds);
    shader.setFloat(3, 0.0);
    canvas.drawRect(
      Offset.zero & size,
      Paint()..shader = shader,
    );
  }

  @override
  bool shouldRepaint(covariant _ShellNebulaPainter oldDelegate) {
    return oldDelegate.shader != shader ||
        oldDelegate.animation != animation;
  }
}

class _SidebarResizeHandle extends StatelessWidget {
  const _SidebarResizeHandle({required this.onDrag});

  final ValueChanged<double> onDrag;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return MouseRegion(
      cursor: SystemMouseCursors.resizeColumn,
      child: GestureDetector(
        behavior: HitTestBehavior.translucent,
        onHorizontalDragUpdate: (details) => onDrag(details.delta.dx),
        child: SizedBox(
          width: 6,
          child: Center(
            child: Container(
              width: 1,
              height: double.infinity,
              decoration: BoxDecoration(
                color: theme.colorScheme.outline.withValues(alpha: 0.18),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _CompactShellState extends State<_CompactShell> {
  bool _showThread = false;

  @override
  void initState() {
    super.initState();
    _showThread = widget.workbench.selection.threadId != null;
  }

  @override
  void didUpdateWidget(covariant _CompactShell oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.workbench.selection.threadId != oldWidget.workbench.selection.threadId &&
        widget.workbench.selection.threadId != null) {
      _showThread = true;
    }
  }

  @override
  Widget build(BuildContext context) {
    final hasThread = widget.workbench.selection.threadId != null;
    if (!_showThread || !hasThread) {
        return ThreadListPanel(
          selection: widget.workbench.selection,
          projects: widget.workbench.projects,
          threads: widget.workbench.threads,
          pendingApprovals: widget.workbench.pendingApprovals,
          onDisconnect: widget.onDisconnect,
          onGlobalSettings: widget.onGlobalSettings,
          onThreadSelected: (threadId) {
            widget.onThreadSelected(threadId);
            setState(() {
              _showThread = true;
            });
          },
          onCreateProject: widget.onCreateProject,
          onProjectSettings: widget.onProjectSettings,
          onCreateThread: widget.onCreateThread,
        onSpawnAgent: widget.onSpawnAgent,
      );
    }

    return ChatTimeline(
      threadId: widget.workbench.selection.threadId,
      entries: widget.workbench.chatEntries,
      title: widget.workbench.selection.threadName,
      contextWindowRemainingPercent:
          widget.workbench.contextWindowRemainingPercent,
      onSend: widget.onSendMessage,
      onInterrupt: widget.onInterruptThread,
      onTerminateCommandExecution: widget.onTerminateCommandExecution,
      loadRequirementComposables: widget.loadRequirementComposables,
      setThreadRequirements: widget.setThreadRequirements == null
          ? null
          : (requirementSetJson) => widget.setThreadRequirements!(
                widget.workbench.selection.threadId ?? '',
                requirementSetJson,
              ),
      uploadImageBytes: widget.uploadImageBytes,
      onOpenLink: widget.onOpenLink,
      composerEnabled: true,
      isRunning: widget.workbench.selection.isRunning,
      selection: widget.workbench.selection,
      availableModels: widget.workbench.availableModels,
      onSettingsChanged: widget.onSettingsChanged,
      requirementReview: widget.workbench.requirementReview,
      onOpenThread: widget.onThreadSelected,
      terminalAvailable: widget.terminalAvailable,
      onTerminalPressed: widget.onTerminalPressed,
      overlay: _ApprovalOverlay(
        selection: widget.workbench.selection,
        pendingApprovals: widget.workbench.pendingApprovals,
        onApprovalDecision: widget.onApprovalDecision,
      ),
      leading: Semantics(
        key: const ValueKey('semantic.thread.backToThreads'),
        container: true,
        button: true,
        label: 'Back to thread list',
        child: ExcludeSemantics(
          child: IconButton(
            icon: const Icon(Icons.arrow_back),
            onPressed: () {
              setState(() {
                _showThread = false;
              });
            },
          ),
        ),
      ),
      headerControls: Align(
        alignment: Alignment.centerRight,
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            _ProcessManagerButton(
              liveProcesses: widget.workbench.liveProcesses,
              onTerminateCommandExecution: widget.onTerminateCommandExecution,
            ),
            Semantics(
              key: const ValueKey('semantic.thread.stats'),
              container: true,
              button: true,
              label: 'Open thread statistics',
              child: ExcludeSemantics(
                child: IconButton(
                  onPressed: widget.workbench.selection.threadId == null
                      ? null
                      : () => showThreadStatsModal(
                            context: context,
                            threadId: widget.workbench.selection.threadId!,
                            loadStats: widget.loadThreadStats,
                          ),
                  tooltip: 'Thread statistics',
                  icon: const Icon(Icons.query_stats_rounded),
                ),
              ),
            ),
            Semantics(
              key: const ValueKey('semantic.thread.history'),
              container: true,
              button: true,
              label: 'Open thread history',
              child: ExcludeSemantics(
                child: IconButton(
                  onPressed: widget.onOpenHistory,
                  tooltip: 'History',
                  icon: const Icon(Icons.history),
                ),
              ),
            ),
            Semantics(
              key: const ValueKey('semantic.thread.compact'),
              container: true,
              button: true,
              label: 'Compact selected thread',
              child: ExcludeSemantics(
                child: IconButton(
                  icon: const Icon(Icons.compress_rounded),
                  tooltip: 'Compact thread',
                  onPressed: widget.onCompactThread,
                ),
              ),
            ),
            _HeaderIconButton(
              tooltip: 'Thread settings',
              icon: const Icon(Icons.tune),
              badgeCount: widget.workbench.pendingApprovals.length,
              onPressed: () => _showInspectorSheet(
                context,
                workbench: widget.workbench,
                onThreadSelected: widget.onThreadSelected,
                loadRequirementComposables: widget.loadRequirementComposables,
                setThreadRequirements: widget.setThreadRequirements,
                uploadImageBytes: widget.uploadImageBytes,
                onApprovalDecision: widget.onApprovalDecision,
                onSettingsChanged: widget.onSettingsChanged,
                onRunningStateChanged: widget.onRunningStateChanged,
                onRenameThread: widget.onRenameThread,
                onArchiveThread: widget.onArchiveThread,
                onWarmHandoff: widget.onWarmHandoff,
                onSetProjectOrchestrator: widget.onSetProjectOrchestrator,
                onCreateThreadGroup: widget.onCreateThreadGroup,
                onRenameThreadGroup: widget.onRenameThreadGroup,
                onDeleteThreadGroup: widget.onDeleteThreadGroup,
                onArchiveThreadGroup: widget.onArchiveThreadGroup,
                onMoveSelectedThreadToGroup: widget.onMoveSelectedThreadToGroup,
                onUpdateWorkerMetadata: widget.onUpdateWorkerMetadata,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _HeaderIconButton extends StatelessWidget {
  const _HeaderIconButton({
    required this.tooltip,
    required this.icon,
    required this.onPressed,
    this.badgeCount = 0,
  });

  final String tooltip;
  final Widget icon;
  final VoidCallback? onPressed;
  final int badgeCount;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final semanticId = tooltip.toLowerCase().replaceAll(RegExp(r'[^a-z0-9]+'), '.');
    return Semantics(
      key: ValueKey('semantic.headerButton.$semanticId'),
      container: true,
      button: true,
      enabled: onPressed != null,
      label: tooltip,
      value: badgeCount > 0 ? '$badgeCount pending item${badgeCount == 1 ? '' : 's'}' : null,
      child: ExcludeSemantics(
        child: Stack(
          clipBehavior: Clip.none,
          children: [
            IconButton(
              onPressed: onPressed,
              tooltip: tooltip,
              icon: icon,
            ),
            if (badgeCount > 0)
              Positioned(
                right: 2,
                top: 2,
                child: Container(
                  padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 1),
                  decoration: BoxDecoration(
                    color: theme.colorScheme.error,
                    borderRadius: BorderRadius.circular(999),
                  ),
                  constraints: const BoxConstraints(minWidth: 16, minHeight: 16),
                  child: Center(
                    child: Text(
                      '$badgeCount',
                      style: theme.textTheme.labelSmall?.copyWith(
                        color: theme.colorScheme.onError,
                        fontSize: 9,
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                  ),
                ),
              ),
          ],
        ),
      ),
    );
  }
}

class _DesktopThreadControls extends StatelessWidget {
  const _DesktopThreadControls({
    required this.selection,
    required this.liveProcesses,
    required this.pendingApprovalCount,
    required this.onOpenHistory,
    required this.loadThreadStats,
    required this.onCompactThread,
    required this.onTerminateCommandExecution,
    required this.onMore,
  });

  final WorkspaceSelection selection;
  final List<LiveProcessItem> liveProcesses;
  final int pendingApprovalCount;
  final VoidCallback onOpenHistory;
  final Future<ThreadStatsData> Function(String threadId) loadThreadStats;
  final VoidCallback onCompactThread;
  final ValueChanged<String> onTerminateCommandExecution;
  final VoidCallback onMore;

  @override
  Widget build(BuildContext context) {
    final enabled = selection.threadId != null;
    return Wrap(
      spacing: 12,
      runSpacing: 8,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: [
        Semantics(
          key: const ValueKey('semantic.thread.history'),
          container: true,
          button: true,
          enabled: enabled,
          label: 'Open thread history',
          child: ExcludeSemantics(
            child: IconButton(
              onPressed: enabled ? onOpenHistory : null,
              tooltip: 'History',
              icon: const Icon(Icons.history),
            ),
          ),
        ),
        Semantics(
          key: const ValueKey('semantic.thread.stats'),
          container: true,
          button: true,
          enabled: enabled,
          label: 'Open thread statistics',
          child: ExcludeSemantics(
            child: IconButton(
              onPressed: enabled
                  ? () => showThreadStatsModal(
                        context: context,
                        threadId: selection.threadId!,
                        loadStats: loadThreadStats,
                      )
                  : null,
              tooltip: 'Thread statistics',
              icon: const Icon(Icons.query_stats_rounded),
            ),
          ),
        ),
        _ProcessManagerButton(
          liveProcesses: liveProcesses,
          onTerminateCommandExecution: onTerminateCommandExecution,
          enabled: enabled,
        ),
        Semantics(
          key: const ValueKey('semantic.thread.compact'),
          container: true,
          button: true,
          enabled: enabled,
          label: 'Compact selected thread',
          child: ExcludeSemantics(
            child: IconButton.outlined(
              onPressed: enabled ? onCompactThread : null,
              tooltip: 'Compact thread',
              icon: const Icon(Icons.compress_rounded),
            ),
          ),
        ),
        _HeaderIconButton(
          tooltip: 'Thread settings',
          icon: const Icon(Icons.settings_outlined),
          badgeCount: pendingApprovalCount,
          onPressed: enabled ? onMore : null,
        ),
      ],
    );
  }
}

class _ProcessManagerButton extends StatelessWidget {
  const _ProcessManagerButton({
    required this.liveProcesses,
    required this.onTerminateCommandExecution,
    this.enabled = true,
  });

  final List<LiveProcessItem> liveProcesses;
  final ValueChanged<String> onTerminateCommandExecution;
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    final count = liveProcesses.length;
    return _HeaderIconButton(
      tooltip: 'Processes',
      icon: const Icon(Icons.developer_board_outlined),
      badgeCount: count,
      onPressed: enabled
          ? () => _showProcessManagerSheet(
                context,
                liveProcesses: liveProcesses,
                onTerminateCommandExecution: onTerminateCommandExecution,
              )
          : null,
    );
  }
}

Future<void> _showProcessManagerSheet(
  BuildContext context, {
  required List<LiveProcessItem> liveProcesses,
  required ValueChanged<String> onTerminateCommandExecution,
}) {
  final theme = Theme.of(context);
  return showModalBottomSheet<void>(
    context: context,
    showDragHandle: true,
    builder: (context) => SafeArea(
      child: SizedBox(
        height: 360,
        child: Padding(
          padding: const EdgeInsets.all(12),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                liveProcesses.isEmpty
                    ? 'No Active Processes'
                    : '${liveProcesses.length} Active Process${liveProcesses.length == 1 ? '' : 'es'}',
                style: theme.textTheme.titleMedium?.copyWith(fontWeight: FontWeight.w700),
              ),
              const SizedBox(height: 12),
              if (liveProcesses.isEmpty)
                Text(
                  'This thread has no registered live shell processes.',
                  style: theme.textTheme.bodyMedium,
                )
              else
                Expanded(
                  child: ListView.separated(
                    itemCount: liveProcesses.length,
                    separatorBuilder: (_, _) => const Divider(height: 16),
                    itemBuilder: (context, index) {
                      final process = liveProcesses[index];
                      final subtitleParts = <String>[
                        'pid=${process.pid ?? process.processId}',
                        if (process.processGroupId != null) 'pgid=${process.processGroupId}',
                      ];
                      final commandLabel = process.command.trim().isEmpty
                          ? '(unknown command)'
                          : process.command.trim();
                      return Row(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Expanded(
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                SelectableText(
                                  commandLabel,
                                  style: theme.textTheme.bodySmall?.copyWith(
                                    fontFamily: 'monospace',
                                    fontWeight: FontWeight.w600,
                                  ),
                                ),
                                const SizedBox(height: 4),
                                Text(
                                  subtitleParts.join('  ·  '),
                                  style: theme.textTheme.labelSmall,
                                ),
                              ],
                            ),
                          ),
                          const SizedBox(width: 12),
                          Semantics(
                            key: ValueKey('semantic.process.terminate.${process.processId}'),
                            container: true,
                            button: true,
                            label: 'Terminate process ${process.processId}',
                            child: ExcludeSemantics(
                              child: IconButton.outlined(
                                onPressed: () => onTerminateCommandExecution(process.processId),
                                tooltip: 'Terminate process',
                                icon: const Icon(Icons.stop_circle_outlined),
                                color: theme.colorScheme.error,
                              ),
                            ),
                          ),
                        ],
                      );
                    },
                  ),
                ),
            ],
          ),
        ),
      ),
    ),
  );
}

class _ApprovalOverlay extends StatelessWidget {
  const _ApprovalOverlay({
    required this.selection,
    required this.pendingApprovals,
    required this.onApprovalDecision,
  });

  final WorkspaceSelection selection;
  final List<PendingApprovalItem> pendingApprovals;
  final Future<void> Function(PendingApprovalItem approval, String decision, String? message)
      onApprovalDecision;

  @override
  Widget build(BuildContext context) {
    final threadId = selection.threadId;
    if (threadId == null) {
      return const SizedBox.shrink();
    }
    final items = pendingApprovals
        .where((approval) => approval.threadId == threadId)
        .toList(growable: false);
    if (items.isEmpty) {
      return const SizedBox.shrink();
    }

    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: theme.colorScheme.surface.withValues(alpha: 0.96),
          border: Border(
            left: BorderSide(color: Colors.amber.shade700, width: 3),
            bottom: BorderSide(color: theme.colorScheme.outline.withValues(alpha: 0.5)),
          ),
        ),
        child: Padding(
          padding: const EdgeInsets.fromLTRB(10, 8, 10, 8),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                items.length == 1 ? 'Pending Approval' : '${items.length} Pending Approvals',
                style: theme.textTheme.bodySmall?.copyWith(fontWeight: FontWeight.w700),
              ),
              const SizedBox(height: 6),
              ...items.map(
                (approval) => Padding(
                  padding: const EdgeInsets.only(bottom: 8),
                  child: _ApprovalRow(
                    approval: approval,
                    onApprovalDecision: onApprovalDecision,
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _ApprovalRow extends StatelessWidget {
  const _ApprovalRow({
    required this.approval,
    required this.onApprovalDecision,
  });

  final PendingApprovalItem approval;
  final Future<void> Function(PendingApprovalItem approval, String decision, String? message)
      onApprovalDecision;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          approval.title,
          style: theme.textTheme.bodySmall?.copyWith(fontWeight: FontWeight.w700),
        ),
        if (approval.detail != null && approval.detail!.isNotEmpty) ...[
          const SizedBox(height: 3),
          Text(approval.detail!, style: theme.textTheme.bodySmall),
        ],
        if (approval.command != null && approval.command!.isNotEmpty) ...[
          const SizedBox(height: 4),
          Text(
            approval.command!,
            style: theme.textTheme.labelSmall?.copyWith(fontFamily: 'monospace'),
          ),
        ],
        if (approval.commandCwd != null && approval.commandCwd!.isNotEmpty) ...[
          const SizedBox(height: 2),
          Text(
            approval.commandCwd!,
            style: theme.textTheme.labelSmall?.copyWith(fontFamily: 'monospace'),
          ),
        ],
        if (approval.filePaths.isNotEmpty) ...[
          const SizedBox(height: 4),
          ...approval.filePaths.map(
            (path) => Text(
              path,
              style: theme.textTheme.labelSmall?.copyWith(fontFamily: 'monospace'),
            ),
          ),
        ],
        const SizedBox(height: 6),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [
            FilledButton.tonal(
              onPressed: () => onApprovalDecision(approval, 'accept', null),
              child: const Text('Approve'),
            ),
            OutlinedButton(
              onPressed: () async {
                final message = await _promptDeclineMessage(context);
                await onApprovalDecision(approval, 'decline', message);
              },
              child: const Text('Decline'),
            ),
          ],
        ),
      ],
    );
  }
}

Future<String?> _promptDeclineMessage(BuildContext context) async {
  final controller = TextEditingController();
  final result = await showDialog<String?>(
    context: context,
    builder: (context) => AlertDialog(
      title: const Text('Decline Approval'),
      content: TextField(
        controller: controller,
        minLines: 2,
        maxLines: 6,
        decoration: const InputDecoration(
          labelText: 'Optional follow-up message',
          border: OutlineInputBorder(),
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('Cancel'),
        ),
        FilledButton(
          onPressed: () => Navigator.of(context).pop(controller.text),
          child: const Text('Decline'),
        ),
      ],
    ),
  );
  controller.dispose();
  return result;
}

Future<void> _showInspectorDialog(
  BuildContext context, {
  required WorkbenchViewData workbench,
  required ValueChanged<String> onThreadSelected,
  required RequirementComposableLoader? loadRequirementComposables,
  required Future<void> Function(String recipientThreadId, String requirementSetJson)? setThreadRequirements,
  required ImageBytesUploader? uploadImageBytes,
  required Future<void> Function(PendingApprovalItem approval, String decision, String? message)
      onApprovalDecision,
  required ValueChanged<ThreadSettingsDraft> onSettingsChanged,
  required ValueChanged<bool> onRunningStateChanged,
  required ValueChanged<String> onRenameThread,
  required VoidCallback onArchiveThread,
  required ValueChanged<String> onWarmHandoff,
  required VoidCallback onSetProjectOrchestrator,
  required ValueChanged<String> onCreateThreadGroup,
  required Future<void> Function(ThreadGroupItem group) onRenameThreadGroup,
  required ValueChanged<String> onDeleteThreadGroup,
  required ValueChanged<String> onArchiveThreadGroup,
  required ValueChanged<String?> onMoveSelectedThreadToGroup,
  required ValueChanged<WorkerMetadataDraft> onUpdateWorkerMetadata,
}) {
  return showDialog<void>(
    context: context,
    builder: (context) => Dialog(
      child: SizedBox(
        width: 520,
        height: 680,
        child: Padding(
          padding: const EdgeInsets.all(12),
          child: InspectorPanel(
            selection: workbench.selection,
            availableModels: workbench.availableModels,
            threadGroups: workbench.threadGroups,
            workerMetadata: workbench.workerMetadata,
            requirementReview: workbench.requirementReview,
            loadRequirementComposables: loadRequirementComposables,
            setThreadRequirements: setThreadRequirements,
            uploadImageBytes: uploadImageBytes,
            onOpenThread: onThreadSelected,
            onSettingsChanged: onSettingsChanged,
            onRunningStateChanged: onRunningStateChanged,
            onRenameThread: onRenameThread,
            onArchiveThread: onArchiveThread,
            onWarmHandoff: onWarmHandoff,
            onCreateThreadGroup: onCreateThreadGroup,
            onRenameThreadGroup: onRenameThreadGroup,
            onDeleteThreadGroup: onDeleteThreadGroup,
            onArchiveThreadGroup: onArchiveThreadGroup,
            onMoveSelectedThreadToGroup: onMoveSelectedThreadToGroup,
            onUpdateWorkerMetadata: onUpdateWorkerMetadata,
          ),
        ),
      ),
    ),
  );
}

Future<void> _showInspectorSheet(
  BuildContext context, {
  required WorkbenchViewData workbench,
  required ValueChanged<String> onThreadSelected,
  required RequirementComposableLoader? loadRequirementComposables,
  required Future<void> Function(String recipientThreadId, String requirementSetJson)? setThreadRequirements,
  required ImageBytesUploader? uploadImageBytes,
  required Future<void> Function(PendingApprovalItem approval, String decision, String? message)
      onApprovalDecision,
  required ValueChanged<ThreadSettingsDraft> onSettingsChanged,
  required ValueChanged<bool> onRunningStateChanged,
  required ValueChanged<String> onRenameThread,
  required VoidCallback onArchiveThread,
  required ValueChanged<String> onWarmHandoff,
  required VoidCallback onSetProjectOrchestrator,
  required ValueChanged<String> onCreateThreadGroup,
  required Future<void> Function(ThreadGroupItem group) onRenameThreadGroup,
  required ValueChanged<String> onDeleteThreadGroup,
  required ValueChanged<String> onArchiveThreadGroup,
  required ValueChanged<String?> onMoveSelectedThreadToGroup,
  required ValueChanged<WorkerMetadataDraft> onUpdateWorkerMetadata,
}) {
  return showModalBottomSheet<void>(
    context: context,
    isScrollControlled: true,
    builder: (context) => SafeArea(
      child: SizedBox(
        height: MediaQuery.of(context).size.height * 0.82,
        child: Padding(
          padding: const EdgeInsets.all(12),
          child: InspectorPanel(
            selection: workbench.selection,
            availableModels: workbench.availableModels,
            threadGroups: workbench.threadGroups,
            workerMetadata: workbench.workerMetadata,
            requirementReview: workbench.requirementReview,
            loadRequirementComposables: loadRequirementComposables,
            setThreadRequirements: setThreadRequirements,
            uploadImageBytes: uploadImageBytes,
            onOpenThread: onThreadSelected,
            onSettingsChanged: onSettingsChanged,
            onRunningStateChanged: onRunningStateChanged,
            onRenameThread: onRenameThread,
            onArchiveThread: onArchiveThread,
            onWarmHandoff: onWarmHandoff,
            onCreateThreadGroup: onCreateThreadGroup,
            onRenameThreadGroup: onRenameThreadGroup,
            onDeleteThreadGroup: onDeleteThreadGroup,
            onArchiveThreadGroup: onArchiveThreadGroup,
            onMoveSelectedThreadToGroup: onMoveSelectedThreadToGroup,
            onUpdateWorkerMetadata: onUpdateWorkerMetadata,
          ),
        ),
      ),
    ),
  );
}
