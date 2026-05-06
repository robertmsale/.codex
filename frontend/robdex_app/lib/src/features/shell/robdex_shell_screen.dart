import 'dart:ui';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

import '../../core/models/workbench_view_data.dart';
import '../../core/models/workbench_models.dart';
import '../chat/chat_timeline.dart';
import '../composer/composer_panel.dart';
import '../inspector/inspector_panel.dart';
import '../sidebar/thread_list_panel.dart';

class RobdexShellScreen extends StatelessWidget {
  const RobdexShellScreen({
    super.key,
    required this.enableGraphics,
    required this.workbench,
    required this.onThreadSelected,
    required this.onProjectSelected,
    required this.onDisconnect,
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
    this.bridgeBaseUri,
  });

  final bool enableGraphics;
  final WorkbenchViewData workbench;
  final ValueChanged<String> onThreadSelected;
  final ValueChanged<String> onProjectSelected;
  final VoidCallback onDisconnect;
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
  final Uri? bridgeBaseUri;

  @override
  Widget build(BuildContext context) {
    final isIOS = defaultTargetPlatform == TargetPlatform.iOS;
    return Scaffold(
      body: DecoratedBox(
        decoration: const BoxDecoration(color: Color(0xFF05090F)),
        child: SafeArea(
          child: Stack(
            children: [
              if (enableGraphics && !isIOS)
                const Positioned.fill(child: _ShellNebulaBackdrop()),
              LayoutBuilder(
                builder: (context, constraints) {
                  final isCompact = constraints.maxWidth < 860;
                  return Padding(
                    padding: const EdgeInsets.all(12),
                    child: isCompact
                        ? RepaintBoundary(
                            child: _CompactShell(
                              workbench: workbench,
                              onThreadSelected: onThreadSelected,
                              onDisconnect: onDisconnect,
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
                              bridgeBaseUri: bridgeBaseUri,
                            ),
                          )
                        : RepaintBoundary(
                            child: _WideShell(
                              workbench: workbench,
                              onThreadSelected: onThreadSelected,
                              onDisconnect: onDisconnect,
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
                              bridgeBaseUri: bridgeBaseUri,
                            ),
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
    required this.bridgeBaseUri,
  });

  final WorkbenchViewData workbench;
  final ValueChanged<String> onThreadSelected;
  final VoidCallback onDisconnect;
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
  final Uri? bridgeBaseUri;

  @override
  State<_WideShell> createState() => _WideShellState();
}

class _WideShellState extends State<_WideShell> {
  double _sidebarWidth = 360;

  void _resizeSidebar(double delta) {
    setState(() {
      _sidebarWidth = (_sidebarWidth + delta).clamp(280, 520);
    });
  }

  @override
  Widget build(BuildContext context) {
    final workbench = widget.workbench;
    return Row(
      children: [
        AnimatedContainer(
          duration: const Duration(milliseconds: 180),
          curve: Curves.easeOutCubic,
          width: _sidebarWidth,
          child: _PaneSurface(
            accent: const Color(0xFF53C6FF),
            child: RepaintBoundary(
              child: ThreadListPanel(
                selection: workbench.selection,
                projects: workbench.projects,
                threads: workbench.threads,
                pendingApprovals: workbench.pendingApprovals,
                onDisconnect: widget.onDisconnect,
                onThreadSelected: widget.onThreadSelected,
                onCreateProject: widget.onCreateProject,
                onProjectSettings: widget.onProjectSettings,
                onCreateThread: widget.onCreateThread,
                onSpawnAgent: widget.onSpawnAgent,
              ),
            ),
          ),
        ),
        _SidebarResizeHandle(onDrag: _resizeSidebar),
        const SizedBox(width: 12),
        Expanded(
          child: _PaneSurface(
            accent: const Color(0xFFF3A43B),
            child: RepaintBoundary(
              child: ChatTimeline(
                threadId: workbench.selection.threadId,
                entries: workbench.chatEntries,
                title: workbench.selection.threadName,
                contextWindowRemainingPercent:
                    workbench.contextWindowRemainingPercent,
                onSend: widget.onSendMessage,
                onInterrupt: widget.onInterruptThread,
                onTerminateCommandExecution: widget.onTerminateCommandExecution,
                bridgeBaseUri: widget.bridgeBaseUri,
                composerEnabled: workbench.selection.threadId != null,
                isRunning: workbench.selection.isRunning,
                headerControls: _DesktopThreadControls(
                  selection: workbench.selection,
                  availableModels: workbench.availableModels,
                  liveProcesses: workbench.liveProcesses,
                  pendingApprovalCount: workbench.pendingApprovals.length,
                  onOpenHistory: widget.onOpenHistory,
                  onCompactThread: widget.onCompactThread,
                  onTerminateCommandExecution: widget.onTerminateCommandExecution,
                  onSettingsChanged: widget.onSettingsChanged,
                  onRunningStateChanged: widget.onRunningStateChanged,
                  onMore: () => _showInspectorDialog(
                    context,
                    workbench: workbench,
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
                overlay: _ApprovalOverlay(
                  selection: workbench.selection,
                  pendingApprovals: workbench.pendingApprovals,
                  onApprovalDecision: widget.onApprovalDecision,
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }
}

class _CompactShell extends StatefulWidget {
  const _CompactShell({
    required this.workbench,
    required this.onThreadSelected,
    required this.onDisconnect,
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
    required this.bridgeBaseUri,
  });

  final WorkbenchViewData workbench;
  final ValueChanged<String> onThreadSelected;
  final VoidCallback onDisconnect;
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
  final Uri? bridgeBaseUri;

  @override
  State<_CompactShell> createState() => _CompactShellState();
}

class _PaneSurface extends StatelessWidget {
  const _PaneSurface({
    required this.child,
    required this.accent,
  });

  final Widget child;
  final Color accent;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return ClipRRect(
      borderRadius: BorderRadius.circular(22),
      child: BackdropFilter(
        filter: ImageFilter.blur(sigmaX: 12, sigmaY: 12),
        child: DecoratedBox(
          decoration: BoxDecoration(
            gradient: LinearGradient(
              begin: Alignment.topLeft,
              end: Alignment.bottomRight,
              colors: [
                theme.colorScheme.surface.withValues(alpha: 0.5),
                theme.colorScheme.surfaceContainer.withValues(alpha: 0.34),
              ],
            ),
            border: Border.all(
              color: accent.withValues(alpha: 0.14),
            ),
            boxShadow: [
              BoxShadow(
                color: Colors.black.withValues(alpha: 0.08),
                blurRadius: 20,
                offset: const Offset(0, 14),
              ),
            ],
          ),
          child: Stack(
            children: [
              Positioned(
                top: -70,
                right: -30,
                child: IgnorePointer(
                  child: Container(
                    width: 180,
                    height: 180,
                    decoration: BoxDecoration(
                      shape: BoxShape.circle,
                      gradient: RadialGradient(
                        colors: [
                          accent.withValues(alpha: 0.08),
                          accent.withValues(alpha: 0.0),
                        ],
                      ),
                    ),
                  ),
                ),
              ),
              Positioned(
                left: -60,
                bottom: -80,
                child: IgnorePointer(
                  child: Transform.rotate(
                    angle: -0.3,
                    child: Container(
                      width: 180,
                      height: 180,
                      decoration: BoxDecoration(
                        borderRadius: BorderRadius.circular(40),
                        gradient: LinearGradient(
                          colors: [
                            const Color(0xFF1A2B3C).withValues(alpha: 0.1),
                            Colors.transparent,
                          ],
                        ),
                      ),
                    ),
                  ),
                ),
              ),
              Padding(
                padding: const EdgeInsets.all(12),
                child: child,
              ),
            ],
          ),
        ),
      ),
    );
  }
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
        'shaders/connection_nebula.frag',
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
          width: 10,
          child: Center(
            child: Container(
              width: 4,
              height: 72,
              decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(999),
                gradient: LinearGradient(
                  begin: Alignment.topCenter,
                  end: Alignment.bottomCenter,
                  colors: [
                    theme.colorScheme.secondary.withValues(alpha: 0.15),
                    theme.colorScheme.primary.withValues(alpha: 0.5),
                    theme.colorScheme.secondary.withValues(alpha: 0.15),
                  ],
                ),
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
      bridgeBaseUri: widget.bridgeBaseUri,
      composerEnabled: true,
      isRunning: widget.workbench.selection.isRunning,
      overlay: _ApprovalOverlay(
        selection: widget.workbench.selection,
        pendingApprovals: widget.workbench.pendingApprovals,
        onApprovalDecision: widget.onApprovalDecision,
      ),
      leading: IconButton(
        icon: const Icon(Icons.arrow_back),
        onPressed: () {
          setState(() {
            _showThread = false;
          });
        },
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
            IconButton(
              onPressed: widget.onOpenHistory,
              tooltip: 'History',
              icon: const Icon(Icons.history),
            ),
            IconButton(
              icon: const Icon(Icons.compress_rounded),
              tooltip: 'Compact thread',
              onPressed: widget.onCompactThread,
            ),
            _HeaderIconButton(
              tooltip: 'Thread settings',
              icon: const Icon(Icons.tune),
              badgeCount: widget.workbench.pendingApprovals.length,
              onPressed: () => _showInspectorSheet(
                context,
                workbench: widget.workbench,
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
    return Stack(
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
    );
  }
}

class _DesktopThreadControls extends StatelessWidget {
  const _DesktopThreadControls({
    required this.selection,
    required this.availableModels,
    required this.liveProcesses,
    required this.pendingApprovalCount,
    required this.onOpenHistory,
    required this.onCompactThread,
    required this.onTerminateCommandExecution,
    required this.onSettingsChanged,
    required this.onRunningStateChanged,
    required this.onMore,
  });

  final WorkspaceSelection selection;
  final List<ModelItem> availableModels;
  final List<LiveProcessItem> liveProcesses;
  final int pendingApprovalCount;
  final VoidCallback onOpenHistory;
  final VoidCallback onCompactThread;
  final ValueChanged<String> onTerminateCommandExecution;
  final ValueChanged<ThreadSettingsDraft> onSettingsChanged;
  final ValueChanged<bool> onRunningStateChanged;
  final VoidCallback onMore;

  @override
  Widget build(BuildContext context) {
    final enabled = selection.threadId != null;
    String titleCaseWords(String value) => value
        .split(RegExp(r'[\s_-]+'))
        .where((part) => part.isNotEmpty)
        .map((part) => part[0].toUpperCase() + part.substring(1))
        .join(' ');
    String inheritedLabel(String value) => '(${titleCaseWords(value)})';
    String inheritedOrSystem(String? value, {String system = 'System'}) =>
        inheritedLabel((value?.trim().isNotEmpty ?? false) ? value! : system);
    String modelLabel(String? modelId) {
      if (modelId == null || modelId.trim().isEmpty) {
        return inheritedOrSystem(null);
      }
      ModelItem? match;
      for (final model in availableModels) {
        if (model.id == modelId) {
          match = model;
          break;
        }
      }
      final display = match?.name?.trim().isNotEmpty == true ? match!.name! : modelId;
      return inheritedLabel(display);
    }
    bool isOverridden(String? value) => value != null && value.trim().isNotEmpty;
    bool isNetworkOverridden() => selection.networkAccess != null;
    ThreadSettingsDraft draft({
      String? role,
      String? approvalPolicy,
      String? sandboxMode,
      String? networkAccessMode,
      String? modelId,
      String? reasoningEffort,
      String? serviceTier,
    }) {
      return ThreadSettingsDraft(
        role: role ?? (selection.threadRole ?? 'worker'),
        approvalPolicy: approvalPolicy ?? (selection.approvalPolicy ?? ''),
        sandboxMode: sandboxMode ?? (selection.sandboxMode ?? ''),
        networkAccessMode: networkAccessMode ??
            (selection.networkAccess == null
                ? 'default'
                : (selection.networkAccess! ? 'enabled' : 'disabled')),
        modelId: modelId ?? (selection.model ?? ''),
        reasoningEffort: reasoningEffort ?? (selection.reasoningEffort ?? ''),
        serviceTier: serviceTier ?? (selection.serviceTier ?? ''),
      );
    }

    return Wrap(
      spacing: 8,
      runSpacing: 8,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: [
        _CompactDropdown(
          width: 140,
          label: 'Model',
          value: selection.model ?? '',
          enabled: enabled,
          items: [
            DropdownMenuItem(
              value: '',
              child: Text(modelLabel(selection.effectiveModel)),
            ),
            ...availableModels
                .where((model) => !model.hidden || model.id == (selection.model ?? ''))
                .map(
                  (model) => DropdownMenuItem(
                    value: model.id,
                    child: Text(model.name?.trim().isNotEmpty == true ? model.name! : model.id),
                  ),
                ),
          ],
          onChanged: (value) => onSettingsChanged(draft(modelId: value)),
        ),
        _ReasoningControl(
          enabled: enabled,
          overridden: isOverridden(selection.reasoningEffort),
          effectiveValue: selection.effectiveReasoningEffort,
          onSelected: (value) => onSettingsChanged(draft(reasoningEffort: value)),
        ),
        _TierControl(
          enabled: enabled,
          overridden: isOverridden(selection.serviceTier),
          effectiveValue: selection.effectiveServiceTier,
          onSelected: (value) => onSettingsChanged(draft(serviceTier: value)),
        ),
        _SandboxControl(
          enabled: enabled,
          overridden: isOverridden(selection.sandboxMode),
          effectiveValue: selection.effectiveSandboxMode,
          onSelected: (value) => onSettingsChanged(draft(sandboxMode: value)),
        ),
        _NetworkControl(
          enabled: enabled,
          overridden: isNetworkOverridden(),
          effectiveValue: selection.effectiveNetworkAccess,
          onChanged: (value) => onSettingsChanged(draft(networkAccessMode: value)),
        ),
        IconButton.outlined(
          onPressed: enabled ? () => onRunningStateChanged(!selection.isRunning) : null,
          tooltip: selection.isRunning ? 'Mark idle' : 'Mark running',
          icon: Icon(selection.isRunning ? Icons.pause_circle_outline : Icons.play_arrow),
        ),
        _CompactDropdown(
          width: 144,
          label: 'Approval',
          value: selection.approvalPolicy ?? '',
          enabled: enabled,
          items: [
            DropdownMenuItem(
              value: '',
              child: Text(inheritedOrSystem(selection.effectiveApprovalPolicy)),
            ),
            const DropdownMenuItem(value: 'untrusted', child: Text('untrusted')),
            const DropdownMenuItem(value: 'on-failure', child: Text('on-failure')),
            const DropdownMenuItem(value: 'on-request', child: Text('on-request')),
            const DropdownMenuItem(value: 'never', child: Text('never')),
          ],
          onChanged: (value) => onSettingsChanged(draft(approvalPolicy: value)),
        ),
        IconButton(
          onPressed: enabled ? onOpenHistory : null,
          tooltip: 'History',
          icon: const Icon(Icons.history),
        ),
        _ProcessManagerButton(
          liveProcesses: liveProcesses,
          onTerminateCommandExecution: onTerminateCommandExecution,
          enabled: enabled,
        ),
        IconButton.outlined(
          onPressed: enabled ? onCompactThread : null,
          tooltip: 'Compact thread',
          icon: const Icon(Icons.compress_rounded),
        ),
        _HeaderIconButton(
          tooltip: 'Thread settings',
          icon: const Icon(Icons.tune),
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
                          IconButton.outlined(
                            onPressed: () => onTerminateCommandExecution(process.processId),
                            tooltip: 'Terminate process',
                            icon: const Icon(Icons.stop_circle_outlined),
                            color: theme.colorScheme.error,
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


class _VisualSettingFrame extends StatelessWidget {
  const _VisualSettingFrame({
    required this.tooltip,
    required this.enabled,
    required this.overridden,
    required this.child,
    this.onTap,
    this.onLongPress,
  });

  final String tooltip;
  final bool enabled;
  final bool overridden;
  final Widget child;
  final VoidCallback? onTap;
  final VoidCallback? onLongPress;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final tone = overridden ? Colors.green.shade600 : Colors.amber.shade700;
    return Tooltip(
      message: tooltip,
      child: InkWell(
        onTap: enabled ? onTap : null,
        onLongPress: enabled ? onLongPress : null,
        borderRadius: BorderRadius.circular(14),
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 160),
          curve: Curves.easeOutCubic,
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(14),
            color: enabled
                ? tone.withValues(alpha: overridden ? 0.14 : 0.16)
                : theme.colorScheme.surfaceContainerHighest.withValues(alpha: 0.22),
            border: Border.all(
              color: enabled
                  ? tone.withValues(alpha: overridden ? 0.32 : 0.28)
                  : theme.colorScheme.outline.withValues(alpha: 0.12),
            ),
          ),
          child: DefaultTextStyle.merge(
            style: theme.textTheme.labelSmall?.copyWith(
                  color: enabled
                      ? theme.colorScheme.onSurface.withValues(alpha: 0.88)
                      : theme.colorScheme.onSurface.withValues(alpha: 0.42),
                  fontWeight: FontWeight.w700,
                ) ??
                const TextStyle(),
            child: child,
          ),
        ),
      ),
    );
  }
}

class _ReasoningGlyph extends StatelessWidget {
  const _ReasoningGlyph({
    required this.level,
    required this.color,
  });

  final int level;
  final Color color;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: 22,
      height: 14,
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.end,
        children: List.generate(3, (index) {
          final active = index < level;
          final heights = [5.0, 9.0, 13.0];
          return Padding(
            padding: EdgeInsets.only(right: index == 2 ? 0 : 2),
            child: Container(
              width: 3.5,
              height: heights[index],
              decoration: BoxDecoration(
                color: active ? color : color.withValues(alpha: 0.2),
                borderRadius: BorderRadius.circular(999),
              ),
            ),
          );
        }),
      ),
    );
  }
}

class _ReasoningControl extends StatelessWidget {
  const _ReasoningControl({
    required this.enabled,
    required this.overridden,
    required this.effectiveValue,
    required this.onSelected,
  });

  final bool enabled;
  final bool overridden;
  final String? effectiveValue;
  final ValueChanged<String> onSelected;

  int _levelFor(String? value) {
    switch (value?.trim().toLowerCase()) {
      case 'low':
        return 1;
      case 'medium':
        return 2;
      case 'high':
        return 3;
      default:
        return 0;
    }
  }

  @override
  Widget build(BuildContext context) {
    final tone = overridden ? Colors.green.shade600 : Colors.amber.shade700;
    final level = _levelFor(effectiveValue);
    return PopupMenuButton<String>(
      enabled: enabled,
      tooltip: '',
      onSelected: onSelected,
      itemBuilder: (context) => [
        PopupMenuItem(
          value: '',
          child: Row(
            children: [
              _ReasoningGlyph(level: _levelFor(effectiveValue), color: Colors.amber.shade700),
              const SizedBox(width: 8),
              const Text('System'),
            ],
          ),
        ),
        for (final option in [('low', 'Low'), ('medium', 'Medium'), ('high', 'High')])
          PopupMenuItem(
            value: option.$1,
            child: Row(
              children: [
                _ReasoningGlyph(level: _levelFor(option.$1), color: Colors.green.shade600),
                const SizedBox(width: 8),
                Text(option.$2),
              ],
            ),
          ),
      ],
      child: _VisualSettingFrame(
        tooltip: overridden ? 'Reasoning override' : 'Reasoning inherited',
        enabled: enabled,
        overridden: overridden,
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            _ReasoningGlyph(level: level, color: tone),
            const SizedBox(width: 4),
            Icon(Icons.arrow_drop_down_rounded, size: 16, color: tone),
          ],
        ),
      ),
    );
  }
}

class _TierControl extends StatelessWidget {
  const _TierControl({
    required this.enabled,
    required this.overridden,
    required this.effectiveValue,
    required this.onSelected,
  });

  final bool enabled;
  final bool overridden;
  final String? effectiveValue;
  final ValueChanged<String> onSelected;

  @override
  Widget build(BuildContext context) {
    final tone = overridden ? Colors.green.shade600 : Colors.amber.shade700;
    final isFast = effectiveValue?.trim().toLowerCase() == 'fast';
    return PopupMenuButton<String>(
      enabled: enabled,
      tooltip: '',
      onSelected: onSelected,
      itemBuilder: (context) => [
        const PopupMenuItem(value: '', child: Text('System')),
        const PopupMenuItem(value: 'fast', child: Text('⚡ Fast')),
        const PopupMenuItem(value: 'flex', child: Text('🐢 Flex')),
      ],
      child: _VisualSettingFrame(
        tooltip: overridden ? 'Tier override' : 'Tier inherited',
        enabled: enabled,
        overridden: overridden,
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(isFast ? '⚡' : '🐢', style: const TextStyle(fontSize: 16)),
            const SizedBox(width: 2),
            Icon(Icons.arrow_drop_down_rounded, size: 16, color: tone),
          ],
        ),
      ),
    );
  }
}

class _SandboxControl extends StatelessWidget {
  const _SandboxControl({
    required this.enabled,
    required this.overridden,
    required this.effectiveValue,
    required this.onSelected,
  });

  final bool enabled;
  final bool overridden;
  final String? effectiveValue;
  final ValueChanged<String> onSelected;

  String _emojiFor(String? value) {
    switch (value?.trim().toLowerCase()) {
      case 'read-only':
        return '👼';
      case 'danger-full-access':
        return '😈';
      case 'workspace-write':
      default:
        return '👷';
    }
  }

  @override
  Widget build(BuildContext context) {
    final tone = overridden ? Colors.green.shade600 : Colors.amber.shade700;
    return PopupMenuButton<String>(
      enabled: enabled,
      tooltip: '',
      onSelected: onSelected,
      itemBuilder: (context) => [
        PopupMenuItem(value: '', child: Text('System ${_emojiFor(effectiveValue)}')),
        const PopupMenuItem(value: 'read-only', child: Text('👼 Read-only')),
        const PopupMenuItem(value: 'workspace-write', child: Text('👷 Workspace')),
        const PopupMenuItem(value: 'danger-full-access', child: Text('😈 Danger')),
      ],
      child: _VisualSettingFrame(
        tooltip: overridden ? 'Sandbox override' : 'Sandbox inherited',
        enabled: enabled,
        overridden: overridden,
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(_emojiFor(effectiveValue), style: const TextStyle(fontSize: 16)),
            const SizedBox(width: 2),
            Icon(Icons.arrow_drop_down_rounded, size: 16, color: tone),
          ],
        ),
      ),
    );
  }
}

class _NetworkControl extends StatelessWidget {
  const _NetworkControl({
    required this.enabled,
    required this.overridden,
    required this.effectiveValue,
    required this.onChanged,
  });

  final bool enabled;
  final bool overridden;
  final bool? effectiveValue;
  final ValueChanged<String> onChanged;

  @override
  Widget build(BuildContext context) {
    final isOn = effectiveValue ?? false;
    return _VisualSettingFrame(
      tooltip: overridden
          ? 'Network override'
          : 'Network inherited (long-press to restore default after toggle)',
      enabled: enabled,
      overridden: overridden,
      onTap: () => onChanged(isOn ? 'disabled' : 'enabled'),
      onLongPress: () => onChanged('default'),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(
            isOn ? Icons.wifi_rounded : Icons.wifi_off_rounded,
            size: 16,
          ),
        ],
      ),
    );
  }
}

class _CompactDropdown extends StatelessWidget {
  const _CompactDropdown({
    required this.width,
    required this.label,
    required this.value,
    required this.items,
    required this.enabled,
    required this.onChanged,
  });

  final double width;
  final String label;
  final String value;
  final List<DropdownMenuItem<String>> items;
  final bool enabled;
  final ValueChanged<String> onChanged;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final safeValue = items.any((item) => item.value == value) ? value : items.first.value ?? '';
    return SizedBox(
      width: width,
      child: InputDecorator(
        decoration: InputDecoration(
          labelText: label,
          isDense: true,
          contentPadding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
        ),
        child: DropdownButtonHideUnderline(
          child: DropdownButton<String>(
            value: safeValue,
            isDense: true,
            isExpanded: true,
            style: theme.textTheme.labelSmall?.copyWith(
              fontSize: 10,
              fontWeight: FontWeight.w600,
              color: theme.colorScheme.onSurface,
            ),
            menuMaxHeight: 320,
            items: items,
            onChanged: enabled
                ? (value) {
                    if (value != null) {
                      onChanged(value);
                    }
                  }
                : null,
          ),
        ),
      ),
    );
  }
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
