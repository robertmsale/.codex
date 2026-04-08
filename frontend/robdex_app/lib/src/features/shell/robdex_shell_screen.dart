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
    required this.onInterruptThread,
    required this.onApprovalDecision,
    required this.onSettingsChanged,
    required this.onRunningStateChanged,
    required this.onRenameThread,
    required this.onArchiveThread,
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
  final VoidCallback onInterruptThread;
  final Future<void> Function(PendingApprovalItem approval, String decision, String? message)
      onApprovalDecision;
  final ValueChanged<ThreadSettingsDraft> onSettingsChanged;
  final ValueChanged<bool> onRunningStateChanged;
  final ValueChanged<String> onRenameThread;
  final VoidCallback onArchiveThread;
  final VoidCallback onSetProjectOrchestrator;
  final ValueChanged<String> onCreateThreadGroup;
  final Future<void> Function(ThreadGroupItem group) onRenameThreadGroup;
  final ValueChanged<String> onDeleteThreadGroup;
  final ValueChanged<String> onArchiveThreadGroup;
  final ValueChanged<String?> onMoveSelectedThreadToGroup;
  final ValueChanged<WorkerMetadataDraft> onUpdateWorkerMetadata;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Scaffold(
      body: ColoredBox(
        color: theme.colorScheme.surface,
        child: SafeArea(
          child: LayoutBuilder(
            builder: (context, constraints) {
              final isCompact = constraints.maxWidth < 860;
              return Padding(
                padding: const EdgeInsets.all(8),
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
                            onInterruptThread: onInterruptThread,
                            onApprovalDecision: onApprovalDecision,
                            onSettingsChanged: onSettingsChanged,
                            onRunningStateChanged: onRunningStateChanged,
                            onRenameThread: onRenameThread,
                            onArchiveThread: onArchiveThread,
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
                            onInterruptThread: onInterruptThread,
                            onApprovalDecision: onApprovalDecision,
                            onSettingsChanged: onSettingsChanged,
                            onRunningStateChanged: onRunningStateChanged,
                            onRenameThread: onRenameThread,
                            onArchiveThread: onArchiveThread,
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
        ),
      ),
    );
  }
}

class _WideShell extends StatelessWidget {
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
    required this.onInterruptThread,
    required this.onApprovalDecision,
    required this.onSettingsChanged,
    required this.onRunningStateChanged,
    required this.onRenameThread,
    required this.onArchiveThread,
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
  final VoidCallback onInterruptThread;
  final Future<void> Function(PendingApprovalItem approval, String decision, String? message)
      onApprovalDecision;
  final ValueChanged<ThreadSettingsDraft> onSettingsChanged;
  final ValueChanged<bool> onRunningStateChanged;
  final ValueChanged<String> onRenameThread;
  final VoidCallback onArchiveThread;
  final VoidCallback onSetProjectOrchestrator;
  final ValueChanged<String> onCreateThreadGroup;
  final Future<void> Function(ThreadGroupItem group) onRenameThreadGroup;
  final ValueChanged<String> onDeleteThreadGroup;
  final ValueChanged<String> onArchiveThreadGroup;
  final ValueChanged<String?> onMoveSelectedThreadToGroup;
  final ValueChanged<WorkerMetadataDraft> onUpdateWorkerMetadata;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        SizedBox(
          width: 340,
          child: RepaintBoundary(
              child: ThreadListPanel(
              selection: workbench.selection,
              projects: workbench.projects,
              threads: workbench.threads,
              pendingApprovals: workbench.pendingApprovals,
              onDisconnect: onDisconnect,
              onThreadSelected: onThreadSelected,
              onCreateProject: onCreateProject,
              onProjectSettings: onProjectSettings,
              onCreateThread: onCreateThread,
              onSpawnAgent: onSpawnAgent,
            ),
          ),
        ),
        const SizedBox(width: 10),
        Expanded(
          child: RepaintBoundary(
            child: ChatTimeline(
              threadId: workbench.selection.threadId,
              entries: workbench.chatEntries,
              title: workbench.selection.threadName,
              contextWindowRemainingPercent:
                  workbench.contextWindowRemainingPercent,
              onSend: onSendMessage,
              onInterrupt: onInterruptThread,
              composerEnabled: workbench.selection.threadId != null,
              isRunning: workbench.selection.isRunning,
              headerControls: _DesktopThreadControls(
                selection: workbench.selection,
                availableModels: workbench.availableModels,
                pendingApprovalCount: workbench.pendingApprovals.length,
                onOpenHistory: onOpenHistory,
                onSettingsChanged: onSettingsChanged,
                onRunningStateChanged: onRunningStateChanged,
                onMore: () => _showInspectorDialog(
                  context,
                  workbench: workbench,
                  onApprovalDecision: onApprovalDecision,
                  onSettingsChanged: onSettingsChanged,
                  onRunningStateChanged: onRunningStateChanged,
                  onRenameThread: onRenameThread,
                  onArchiveThread: onArchiveThread,
                  onSetProjectOrchestrator: onSetProjectOrchestrator,
                  onCreateThreadGroup: onCreateThreadGroup,
                  onRenameThreadGroup: onRenameThreadGroup,
                  onDeleteThreadGroup: onDeleteThreadGroup,
                  onArchiveThreadGroup: onArchiveThreadGroup,
                  onMoveSelectedThreadToGroup: onMoveSelectedThreadToGroup,
                  onUpdateWorkerMetadata: onUpdateWorkerMetadata,
                ),
              ),
              overlay: _ApprovalOverlay(
                selection: workbench.selection,
                pendingApprovals: workbench.pendingApprovals,
                onApprovalDecision: onApprovalDecision,
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
    required this.onInterruptThread,
    required this.onApprovalDecision,
    required this.onSettingsChanged,
    required this.onRunningStateChanged,
    required this.onRenameThread,
    required this.onArchiveThread,
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
  final VoidCallback onInterruptThread;
  final Future<void> Function(PendingApprovalItem approval, String decision, String? message)
      onApprovalDecision;
  final ValueChanged<ThreadSettingsDraft> onSettingsChanged;
  final ValueChanged<bool> onRunningStateChanged;
  final ValueChanged<String> onRenameThread;
  final VoidCallback onArchiveThread;
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
    required this.onSettingsChanged,
    required this.onRunningStateChanged,
    required this.onMore,
  });

  final WorkspaceSelection selection;
  final List<ModelItem> availableModels;
  final int pendingApprovalCount;
  final VoidCallback onOpenHistory;
  final ValueChanged<ThreadSettingsDraft> onSettingsChanged;
  final ValueChanged<bool> onRunningStateChanged;
  final VoidCallback onMore;

  @override
  Widget build(BuildContext context) {
    final enabled = selection.threadId != null;
    ThreadSettingsDraft draft({
      String? role,
      String? approvalPolicy,
      String? sandboxMode,
      String? networkAccessMode,
      String? modelId,
      String? reasoningEffort,
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
            const DropdownMenuItem(value: '', child: Text('Default')),
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
          items: const [
            DropdownMenuItem(value: '', child: Text('Default')),
            DropdownMenuItem(value: 'low', child: Text('Low')),
            DropdownMenuItem(value: 'medium', child: Text('Medium')),
            DropdownMenuItem(value: 'high', child: Text('High')),
          ],
          onChanged: (value) => onSettingsChanged(draft(reasoningEffort: value)),
        ),
        _CompactDropdown(
          width: 142,
          label: 'Sandbox',
          value: selection.sandboxMode ?? '',
          enabled: enabled,
          items: const [
            DropdownMenuItem(value: '', child: Text('Default')),
            DropdownMenuItem(value: 'workspace-write', child: Text('Workspace')),
            DropdownMenuItem(value: 'danger-full-access', child: Text('Danger')),
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
          items: const [
            DropdownMenuItem(value: 'default', child: Text('Default')),
            DropdownMenuItem(value: 'enabled', child: Text('Enabled')),
            DropdownMenuItem(value: 'disabled', child: Text('Disabled')),
          ],
          onChanged: (value) => onSettingsChanged(draft(networkAccessMode: value)),
        ),
        IconButton.outlined(
          onPressed: enabled ? () => onRunningStateChanged(!selection.isRunning) : null,
          tooltip: selection.isRunning ? 'Mark idle' : 'Mark running',
          icon: Icon(selection.isRunning ? Icons.pause_circle_outline : Icons.play_arrow),
        ),
        TextButton(
          onPressed: enabled ? onOpenHistory : null,
          child: const Text('History'),
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
