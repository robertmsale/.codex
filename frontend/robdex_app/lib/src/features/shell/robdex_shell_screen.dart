import 'package:flutter/material.dart';

import '../../core/models/workbench_view_data.dart';
import '../../core/models/workbench_models.dart';
import '../chat/chat_timeline.dart';
import '../inspector/inspector_panel.dart';
import '../sidebar/thread_list_panel.dart';

class RobdexShellScreen extends StatelessWidget {
  const RobdexShellScreen({
    super.key,
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
  });

  final WorkbenchViewData workbench;
  final ValueChanged<String> onThreadSelected;
  final ValueChanged<String> onProjectSelected;
  final VoidCallback onDisconnect;
  final VoidCallback onCreateProject;
  final ValueChanged<ProjectItem> onProjectSettings;
  final ValueChanged<ProjectItem> onCreateThread;
  final VoidCallback onSpawnAgent;
  final ValueChanged<String> onSendMessage;
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

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: DecoratedBox(
        decoration: const BoxDecoration(
          gradient: LinearGradient(
            begin: Alignment.topLeft,
            end: Alignment.bottomRight,
            colors: [
              Color(0xFF071018),
              Color(0xFF0C1622),
              Color(0xFF101D2B),
            ],
            stops: [0.0, 0.44, 1.0],
          ),
        ),
        child: SafeArea(
          child: Stack(
            children: [
              const Positioned(
                top: -120,
                left: -80,
                child: _BackdropOrb(
                  size: 360,
                  colors: [Color(0x6637C8FF), Color(0x00102535)],
                ),
              ),
              const Positioned(
                bottom: -160,
                right: -110,
                child: _BackdropOrb(
                  size: 420,
                  colors: [Color(0x66F3A43B), Color(0x00102535)],
                ),
              ),
              const Positioned(
                top: 150,
                right: 240,
                child: _BackdropOrb(
                  size: 220,
                  colors: [Color(0x331FE0A5), Color(0x00102535)],
                ),
              ),
              LayoutBuilder(
                builder: (context, constraints) {
                  final isCompact = constraints.maxWidth < 860;
                  return Padding(
                    padding: const EdgeInsets.all(10),
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
  });

  final WorkbenchViewData workbench;
  final ValueChanged<String> onThreadSelected;
  final VoidCallback onDisconnect;
  final VoidCallback onCreateProject;
  final ValueChanged<ProjectItem> onProjectSettings;
  final ValueChanged<ProjectItem> onCreateThread;
  final VoidCallback onSpawnAgent;
  final ValueChanged<String> onSendMessage;
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
        const SizedBox(width: 10),
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
                composerEnabled: workbench.selection.threadId != null,
                isRunning: workbench.selection.isRunning,
                headerControls: _DesktopThreadControls(
                  selection: workbench.selection,
                  availableModels: workbench.availableModels,
                  pendingApprovalCount: workbench.pendingApprovals.length,
                  onOpenHistory: widget.onOpenHistory,
                  onCompactThread: widget.onCompactThread,
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
  });

  final WorkbenchViewData workbench;
  final ValueChanged<String> onThreadSelected;
  final VoidCallback onDisconnect;
  final VoidCallback onCreateProject;
  final ValueChanged<ProjectItem> onProjectSettings;
  final ValueChanged<ProjectItem> onCreateThread;
  final VoidCallback onSpawnAgent;
  final ValueChanged<String> onSendMessage;
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
      child: DecoratedBox(
        decoration: BoxDecoration(
          gradient: LinearGradient(
            begin: Alignment.topLeft,
            end: Alignment.bottomRight,
            colors: [
              theme.colorScheme.surface.withValues(alpha: 0.96),
              theme.colorScheme.surfaceContainer.withValues(alpha: 0.9),
            ],
          ),
          border: Border.all(
            color: accent.withValues(alpha: 0.2),
          ),
          boxShadow: [
            BoxShadow(
              color: Colors.black.withValues(alpha: 0.18),
              blurRadius: 28,
              offset: const Offset(0, 18),
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
                        accent.withValues(alpha: 0.2),
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
                          const Color(0xFF1A2B3C).withValues(alpha: 0.36),
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
    );
  }
}

class _BackdropOrb extends StatelessWidget {
  const _BackdropOrb({
    required this.size,
    required this.colors,
  });

  final double size;
  final List<Color> colors;

  @override
  Widget build(BuildContext context) {
    return IgnorePointer(
      child: Container(
        width: size,
        height: size,
        decoration: BoxDecoration(
          shape: BoxShape.circle,
          gradient: RadialGradient(colors: colors),
        ),
      ),
    );
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
            TextButton(
              onPressed: widget.onOpenHistory,
              child: const Text('History'),
            ),
            IconButton(
              icon: const Icon(Icons.compress_rounded),
              tooltip: 'Compact thread',
              onPressed: widget.onCompactThread,
            ),
            IconButton(
              icon: const Icon(Icons.more_horiz),
              tooltip: 'Thread settings',
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

class _DesktopThreadControls extends StatelessWidget {
  const _DesktopThreadControls({
    required this.selection,
    required this.availableModels,
    required this.pendingApprovalCount,
    required this.onOpenHistory,
    required this.onCompactThread,
    required this.onSettingsChanged,
    required this.onRunningStateChanged,
    required this.onMore,
  });

  final WorkspaceSelection selection;
  final List<ModelItem> availableModels;
  final int pendingApprovalCount;
  final VoidCallback onOpenHistory;
  final VoidCallback onCompactThread;
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
    String networkLabel(bool? enabled) => inheritedLabel(
          switch (enabled) {
            true => 'Enabled',
            false => 'Disabled',
            null => 'System',
          },
        );
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
        _CompactDropdown(
          width: 126,
          label: 'Reasoning',
          value: selection.reasoningEffort ?? '',
          enabled: enabled,
          items: [
            DropdownMenuItem(
              value: '',
              child: Text(inheritedOrSystem(selection.effectiveReasoningEffort)),
            ),
            const DropdownMenuItem(value: 'low', child: Text('Low')),
            const DropdownMenuItem(value: 'medium', child: Text('Medium')),
            const DropdownMenuItem(value: 'high', child: Text('High')),
          ],
          onChanged: (value) => onSettingsChanged(draft(reasoningEffort: value)),
        ),
        _CompactDropdown(
          width: 112,
          label: 'Tier',
          value: selection.serviceTier ?? '',
          enabled: enabled,
          items: [
            DropdownMenuItem(
              value: '',
              child: Text(inheritedOrSystem(selection.effectiveServiceTier)),
            ),
            const DropdownMenuItem(value: 'fast', child: Text('Fast')),
            const DropdownMenuItem(value: 'flex', child: Text('Flex')),
          ],
          onChanged: (value) => onSettingsChanged(draft(serviceTier: value)),
        ),
        _CompactDropdown(
          width: 142,
          label: 'Sandbox',
          value: selection.sandboxMode ?? '',
          enabled: enabled,
          items: [
            DropdownMenuItem(
              value: '',
              child: Text(inheritedOrSystem(selection.effectiveSandboxMode)),
            ),
            const DropdownMenuItem(value: 'workspace-write', child: Text('Workspace')),
            const DropdownMenuItem(value: 'danger-full-access', child: Text('Danger')),
          ],
          onChanged: (value) => onSettingsChanged(draft(sandboxMode: value)),
        ),
        _CompactDropdown(
          width: 122,
          label: 'Network',
          value: selection.networkAccess == null
              ? 'default'
              : (selection.networkAccess! ? 'enabled' : 'disabled'),
          enabled: enabled,
          items: [
            DropdownMenuItem(
              value: 'default',
              child: Text(networkLabel(selection.effectiveNetworkAccess)),
            ),
            const DropdownMenuItem(value: 'enabled', child: Text('Enabled')),
            const DropdownMenuItem(value: 'disabled', child: Text('Disabled')),
          ],
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
        TextButton(
          onPressed: enabled ? onOpenHistory : null,
          child: const Text('History'),
        ),
        IconButton.outlined(
          onPressed: enabled ? onCompactThread : null,
          tooltip: 'Compact thread',
          icon: const Icon(Icons.compress_rounded),
        ),
        TextButton.icon(
          onPressed: enabled ? onMore : null,
          icon: const Icon(Icons.tune, size: 16),
          label: Text(pendingApprovalCount > 0 ? 'More · $pendingApprovalCount' : 'More'),
        ),
      ],
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
