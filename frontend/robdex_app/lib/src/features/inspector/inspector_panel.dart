import 'package:flutter/material.dart';

import '../../core/models/workbench_models.dart';

class InspectorPanel extends StatelessWidget {
  const InspectorPanel({
    super.key,
    required this.selection,
    required this.availableModels,
    required this.threadGroups,
    required this.workerMetadata,
    required this.onSettingsChanged,
    required this.onRunningStateChanged,
    required this.onRenameThread,
    required this.onArchiveThread,
    required this.onCreateThreadGroup,
    required this.onRenameThreadGroup,
    required this.onDeleteThreadGroup,
    required this.onArchiveThreadGroup,
    required this.onMoveSelectedThreadToGroup,
    required this.onUpdateWorkerMetadata,
  });

  final WorkspaceSelection selection;
  final List<ModelItem> availableModels;
  final List<ThreadGroupItem> threadGroups;
  final WorkerMetadata? workerMetadata;
  final ValueChanged<ThreadSettingsDraft> onSettingsChanged;
  final ValueChanged<bool> onRunningStateChanged;
  final ValueChanged<String> onRenameThread;
  final VoidCallback onArchiveThread;
  final ValueChanged<String> onCreateThreadGroup;
  final Future<void> Function(ThreadGroupItem group) onRenameThreadGroup;
  final ValueChanged<String> onDeleteThreadGroup;
  final ValueChanged<String> onArchiveThreadGroup;
  final ValueChanged<String?> onMoveSelectedThreadToGroup;
  final ValueChanged<WorkerMetadataDraft> onUpdateWorkerMetadata;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          'Inspector',
          style: theme.textTheme.bodyMedium?.copyWith(
            fontWeight: FontWeight.w700,
          ),
        ),
        const SizedBox(height: 8),
        Expanded(
          child: ListView(
            children: [
              _ThreadSettingsCard(
                selection: selection,
                availableModels: availableModels,
                onSettingsChanged: onSettingsChanged,
                onRunningStateChanged: onRunningStateChanged,
                onRenameThread: onRenameThread,
                onArchiveThread: onArchiveThread,
              ),
              const SizedBox(height: 10),
              _ProjectGroupsCard(
                selection: selection,
                threadGroups: threadGroups,
                onCreateThreadGroup: onCreateThreadGroup,
                onRenameThreadGroup: onRenameThreadGroup,
                onDeleteThreadGroup: onDeleteThreadGroup,
                onArchiveThreadGroup: onArchiveThreadGroup,
                onMoveSelectedThreadToGroup: onMoveSelectedThreadToGroup,
              ),
              if (workerMetadata != null) ...[
                const SizedBox(height: 10),
                _WorkerMetadataCard(
                  workerMetadata: workerMetadata!,
                  onUpdateWorkerMetadata: onUpdateWorkerMetadata,
                ),
              ],
            ],
          ),
        ),
      ],
    );
  }
}

class ThreadSettingsDraft {
  const ThreadSettingsDraft({
    required this.role,
    required this.approvalPolicy,
    required this.sandboxMode,
    required this.networkAccessMode,
    required this.modelId,
    required this.reasoningEffort,
  });

  final String role;
  final String approvalPolicy;
  final String sandboxMode;
  final String networkAccessMode;
  final String modelId;
  final String reasoningEffort;
}

class WorkerMetadataDraft {
  const WorkerMetadataDraft({
    required this.issueNumber,
    required this.pullRequestNumber,
    required this.blockedReason,
    required this.unblockWhen,
    required this.clearBlocked,
  });

  final String issueNumber;
  final String pullRequestNumber;
  final String blockedReason;
  final String unblockWhen;
  final bool clearBlocked;
}

class _ThreadSettingsCard extends StatefulWidget {
  const _ThreadSettingsCard({
    required this.selection,
    required this.availableModels,
    required this.onSettingsChanged,
    required this.onRunningStateChanged,
    required this.onRenameThread,
    required this.onArchiveThread,
  });

  final WorkspaceSelection selection;
  final List<ModelItem> availableModels;
  final ValueChanged<ThreadSettingsDraft> onSettingsChanged;
  final ValueChanged<bool> onRunningStateChanged;
  final ValueChanged<String> onRenameThread;
  final VoidCallback onArchiveThread;

  @override
  State<_ThreadSettingsCard> createState() => _ThreadSettingsCardState();
}

class _ProjectGroupsCard extends StatelessWidget {
  const _ProjectGroupsCard({
    required this.selection,
    required this.threadGroups,
    required this.onCreateThreadGroup,
    required this.onRenameThreadGroup,
    required this.onDeleteThreadGroup,
    required this.onArchiveThreadGroup,
    required this.onMoveSelectedThreadToGroup,
  });

  final WorkspaceSelection selection;
  final List<ThreadGroupItem> threadGroups;
  final ValueChanged<String> onCreateThreadGroup;
  final Future<void> Function(ThreadGroupItem group) onRenameThreadGroup;
  final ValueChanged<String> onDeleteThreadGroup;
  final ValueChanged<String> onArchiveThreadGroup;
  final ValueChanged<String?> onMoveSelectedThreadToGroup;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final enabled = selection.projectId != null;
    final canAssign = enabled && selection.threadId != null;
    return DecoratedBox(
      decoration: BoxDecoration(
        border: Border(
          top: BorderSide(color: theme.colorScheme.outline),
        ),
      ),
      child: Padding(
        padding: const EdgeInsets.only(top: 8),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              'Project Controls',
              style: theme.textTheme.bodyMedium?.copyWith(fontWeight: FontWeight.w700),
            ),
            const SizedBox(height: 4),
            Text(
              selection.projectOrchestratorName == null
                  ? 'No orchestrator assigned'
                  : 'Orchestrator: ${selection.projectOrchestratorName}',
              style: theme.textTheme.labelSmall,
            ),
            const SizedBox(height: 8),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                OutlinedButton(
                  onPressed: enabled
                      ? () async {
                          final title = await _promptInline(context, 'New Thread Group', '');
                          if (title != null && title.trim().isNotEmpty) {
                            onCreateThreadGroup(title);
                          }
                        }
                      : null,
                  child: const Text('Create Group'),
                ),
                OutlinedButton(
                  onPressed: canAssign ? () => onMoveSelectedThreadToGroup(null) : null,
                  child: const Text('Ungroup Selected'),
                ),
              ],
            ),
            const SizedBox(height: 8),
            if (threadGroups.isEmpty)
              Text(
                'No thread groups yet.',
                style: theme.textTheme.labelSmall,
              )
            else
              ...threadGroups.map(
                (group) => Padding(
                    padding: const EdgeInsets.only(bottom: 6),
                    child: _ThreadGroupTile(
                    group: group,
                    canAssignSelected: canAssign,
                    onAssignSelected: () => onMoveSelectedThreadToGroup(group.id),
                    onRename: () => onRenameThreadGroup(group),
                    onDelete: () => onDeleteThreadGroup(group.id),
                    onArchive: () => onArchiveThreadGroup(group.id),
                  ),
                ),
              ),
          ],
        ),
      ),
    );
  }
}

class _ThreadGroupTile extends StatelessWidget {
  const _ThreadGroupTile({
    required this.group,
    required this.canAssignSelected,
    required this.onAssignSelected,
    required this.onRename,
    required this.onDelete,
    required this.onArchive,
  });

  final ThreadGroupItem group;
  final bool canAssignSelected;
  final VoidCallback onAssignSelected;
  final VoidCallback onRename;
  final VoidCallback onDelete;
  final VoidCallback onArchive;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return DecoratedBox(
      decoration: BoxDecoration(
        border: Border(
          left: BorderSide(color: theme.colorScheme.outline, width: 2),
        ),
      ),
      child: Padding(
        padding: const EdgeInsets.fromLTRB(8, 4, 0, 4),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Expanded(
                  child: Text(
                    group.title,
                    style: theme.textTheme.bodySmall?.copyWith(fontWeight: FontWeight.w700),
                  ),
                ),
                Text(
                  '${group.threadIds.length} threads',
                  style: theme.textTheme.labelSmall,
                ),
              ],
            ),
            const SizedBox(height: 4),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                OutlinedButton(
                  onPressed: canAssignSelected ? onAssignSelected : null,
                  child: const Text('Assign Selected'),
                ),
                TextButton(
                  onPressed: onRename,
                  child: const Text('Rename'),
                ),
                TextButton(
                  onPressed: onDelete,
                  child: const Text('Delete'),
                ),
                TextButton(
                  onPressed: onArchive,
                  child: const Text('Archive'),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

class _WorkerMetadataCard extends StatefulWidget {
  const _WorkerMetadataCard({
    required this.workerMetadata,
    required this.onUpdateWorkerMetadata,
  });

  final WorkerMetadata workerMetadata;
  final ValueChanged<WorkerMetadataDraft> onUpdateWorkerMetadata;

  @override
  State<_WorkerMetadataCard> createState() => _WorkerMetadataCardState();
}

class _WorkerMetadataCardState extends State<_WorkerMetadataCard> {
  late TextEditingController _issueController;
  late TextEditingController _pullRequestController;
  late TextEditingController _blockedReasonController;
  late TextEditingController _unblockWhenController;

  @override
  void initState() {
    super.initState();
    _syncControllers();
  }

  @override
  void didUpdateWidget(covariant _WorkerMetadataCard oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.workerMetadata.threadId != widget.workerMetadata.threadId ||
        oldWidget.workerMetadata.issueNumber != widget.workerMetadata.issueNumber ||
        oldWidget.workerMetadata.pullRequestNumber != widget.workerMetadata.pullRequestNumber ||
        oldWidget.workerMetadata.blockedReason != widget.workerMetadata.blockedReason ||
        oldWidget.workerMetadata.unblockWhen != widget.workerMetadata.unblockWhen) {
      _issueController.dispose();
      _pullRequestController.dispose();
      _blockedReasonController.dispose();
      _unblockWhenController.dispose();
      _syncControllers();
    }
  }

  void _syncControllers() {
    _issueController = TextEditingController(
      text: widget.workerMetadata.issueNumber?.toString() ?? '',
    );
    _pullRequestController = TextEditingController(
      text: widget.workerMetadata.pullRequestNumber?.toString() ?? '',
    );
    _blockedReasonController = TextEditingController(
      text: widget.workerMetadata.blockedReason ?? '',
    );
    _unblockWhenController = TextEditingController(
      text: widget.workerMetadata.unblockWhen ?? '',
    );
  }

  @override
  void dispose() {
    _issueController.dispose();
    _pullRequestController.dispose();
    _blockedReasonController.dispose();
    _unblockWhenController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return DecoratedBox(
      decoration: BoxDecoration(
        border: Border(
          top: BorderSide(color: theme.colorScheme.outline),
        ),
      ),
      child: Padding(
        padding: const EdgeInsets.only(top: 8),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              'Worker Metadata',
              style: theme.textTheme.bodyMedium?.copyWith(fontWeight: FontWeight.w700),
            ),
            const SizedBox(height: 8),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                SizedBox(
                  width: 128,
                  child: TextField(
                    controller: _issueController,
                    decoration: const InputDecoration(labelText: 'Issue'),
                    keyboardType: TextInputType.number,
                  ),
                ),
                SizedBox(
                  width: 128,
                  child: TextField(
                    controller: _pullRequestController,
                    decoration: const InputDecoration(labelText: 'PR'),
                    keyboardType: TextInputType.number,
                  ),
                ),
              ],
            ),
            const SizedBox(height: 8),
            TextField(
              controller: _blockedReasonController,
              minLines: 1,
              maxLines: 3,
              decoration: const InputDecoration(labelText: 'Blocked Reason'),
            ),
            const SizedBox(height: 8),
            TextField(
              controller: _unblockWhenController,
              decoration: const InputDecoration(labelText: 'Unblock When'),
            ),
            const SizedBox(height: 8),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                FilledButton.tonal(
                  onPressed: () {
                    widget.onUpdateWorkerMetadata(
                      WorkerMetadataDraft(
                        issueNumber: _issueController.text,
                        pullRequestNumber: _pullRequestController.text,
                        blockedReason: _blockedReasonController.text,
                        unblockWhen: _unblockWhenController.text,
                        clearBlocked: false,
                      ),
                    );
                  },
                  child: const Text('Save Metadata'),
                ),
                OutlinedButton(
                  onPressed: () {
                    _blockedReasonController.clear();
                    _unblockWhenController.clear();
                    widget.onUpdateWorkerMetadata(
                      WorkerMetadataDraft(
                        issueNumber: _issueController.text,
                        pullRequestNumber: _pullRequestController.text,
                        blockedReason: '',
                        unblockWhen: '',
                        clearBlocked: true,
                      ),
                    );
                  },
                  child: const Text('Clear Blocked'),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

Future<String?> _promptInline(BuildContext context, String title, String initialValue) async {
  final controller = TextEditingController(text: initialValue);
  final result = await showDialog<String>(
    context: context,
    builder: (context) => AlertDialog(
      title: Text(title),
      content: TextField(
        controller: controller,
        decoration: const InputDecoration(labelText: 'Title'),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('Cancel'),
        ),
        FilledButton(
          onPressed: () => Navigator.of(context).pop(controller.text),
          child: const Text('Save'),
        ),
      ],
    ),
  );
  controller.dispose();
  return result;
}

class _ThreadSettingsCardState extends State<_ThreadSettingsCard> {
  late TextEditingController _nameController;
  late String _role;
  late String _approvalPolicy;
  late String _sandboxMode;
  late String _networkAccessMode;
  late String _modelId;
  late String _reasoningEffort;

  @override
  void initState() {
    super.initState();
    _syncFromSelection();
  }

  @override
  void didUpdateWidget(covariant _ThreadSettingsCard oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.selection.threadId != widget.selection.threadId ||
        oldWidget.selection.threadRole != widget.selection.threadRole ||
        oldWidget.selection.approvalPolicy != widget.selection.approvalPolicy ||
        oldWidget.selection.sandboxMode != widget.selection.sandboxMode ||
        oldWidget.selection.networkAccess != widget.selection.networkAccess ||
        oldWidget.selection.model != widget.selection.model ||
        oldWidget.selection.reasoningEffort != widget.selection.reasoningEffort ||
        oldWidget.selection.threadName != widget.selection.threadName) {
      _syncFromSelection();
    }
  }

  void _syncFromSelection() {
    if (_isNameControllerInitialized) {
      _nameController.dispose();
    }
    _nameController = TextEditingController(text: widget.selection.threadName);
    _role = widget.selection.threadRole ?? 'worker';
    _approvalPolicy = widget.selection.approvalPolicy ?? '';
    _sandboxMode = widget.selection.sandboxMode ?? '';
    _networkAccessMode = switch (widget.selection.networkAccess) {
      true => 'enabled',
      false => 'disabled',
      null => 'default',
    };
    _modelId = widget.selection.model ?? '';
    _reasoningEffort = widget.selection.reasoningEffort ?? '';
  }

  bool get _isNameControllerInitialized {
    try {
      _nameController;
      return true;
    } catch (_) {
      return false;
    }
  }

  @override
  void dispose() {
    _nameController.dispose();
    super.dispose();
  }

  void _emitSettings() {
    widget.onSettingsChanged(
      ThreadSettingsDraft(
        role: _role,
        approvalPolicy: _approvalPolicy,
        sandboxMode: _sandboxMode,
        networkAccessMode: _networkAccessMode,
        modelId: _modelId,
        reasoningEffort: _reasoningEffort,
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final enabled = widget.selection.threadId != null;
    final modelItems = [
      const DropdownMenuItem<String>(value: '', child: Text('Default')),
      ...widget.availableModels
          .where((model) => !model.hidden || model.id == _modelId)
          .map(
            (model) => DropdownMenuItem<String>(
              value: model.id,
              child: Text(model.name?.trim().isNotEmpty == true ? model.name! : model.id),
            ),
          ),
    ];
    return DecoratedBox(
      decoration: BoxDecoration(
        border: Border(
          top: BorderSide(color: theme.colorScheme.outline),
        ),
      ),
      child: Padding(
        padding: const EdgeInsets.only(top: 8),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              'Thread Controls',
              style: theme.textTheme.bodyMedium?.copyWith(fontWeight: FontWeight.w700),
            ),
            const SizedBox(height: 8),
            TextField(
              controller: _nameController,
              enabled: enabled,
              decoration: const InputDecoration(labelText: 'Agent name'),
              onSubmitted: enabled
                  ? (value) {
                      final trimmed = value.trim();
                      if (trimmed.isNotEmpty) {
                        widget.onRenameThread(trimmed);
                      }
                    }
                  : null,
            ),
            const SizedBox(height: 8),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                SizedBox(
                  width: 128,
                  child: DropdownButtonFormField<String>(
                    initialValue: _role,
                    isExpanded: true,
                    decoration: const InputDecoration(labelText: 'Role'),
                    items: const [
                      DropdownMenuItem(value: 'worker', child: Text('worker')),
                      DropdownMenuItem(value: 'qa', child: Text('qa')),
                      DropdownMenuItem(value: 'operator', child: Text('operator')),
                      DropdownMenuItem(value: 'orchestrator', child: Text('orchestrator')),
                      DropdownMenuItem(value: 'hidden', child: Text('hidden')),
                    ],
                    onChanged: enabled
                        ? (value) {
                            if (value == null) return;
                            setState(() => _role = value);
                            _emitSettings();
                          }
                        : null,
                  ),
                ),
                SizedBox(
                  width: 128,
                  child: DropdownButtonFormField<String>(
                    initialValue: _approvalPolicy,
                    isExpanded: true,
                    decoration: const InputDecoration(labelText: 'Approval'),
                    items: const [
                      DropdownMenuItem(value: '', child: Text('Default')),
                      DropdownMenuItem(value: 'untrusted', child: Text('untrusted')),
                      DropdownMenuItem(value: 'on-failure', child: Text('on-failure')),
                      DropdownMenuItem(value: 'on-request', child: Text('on-request')),
                      DropdownMenuItem(value: 'never', child: Text('never')),
                    ],
                    onChanged: enabled
                        ? (value) {
                            if (value == null) return;
                            setState(() => _approvalPolicy = value);
                            _emitSettings();
                          }
                        : null,
                  ),
                ),
                SizedBox(
                  width: 128,
                  child: DropdownButtonFormField<String>(
                    initialValue: _sandboxMode,
                    isExpanded: true,
                    decoration: const InputDecoration(labelText: 'Sandbox'),
                    items: const [
                      DropdownMenuItem(value: '', child: Text('Default')),
                      DropdownMenuItem(value: 'workspace-write', child: Text('workspace-write')),
                      DropdownMenuItem(value: 'danger-full-access', child: Text('danger-full-access')),
                    ],
                    onChanged: enabled
                        ? (value) {
                            if (value == null) return;
                            setState(() => _sandboxMode = value);
                            _emitSettings();
                          }
                        : null,
                  ),
                ),
                SizedBox(
                  width: 128,
                  child: DropdownButtonFormField<String>(
                    initialValue: _networkAccessMode,
                    isExpanded: true,
                    decoration: const InputDecoration(labelText: 'Network'),
                    items: const [
                      DropdownMenuItem(value: 'default', child: Text('Default')),
                      DropdownMenuItem(value: 'enabled', child: Text('Enabled')),
                      DropdownMenuItem(value: 'disabled', child: Text('Disabled')),
                    ],
                    onChanged: enabled
                        ? (value) {
                            if (value == null) return;
                            setState(() => _networkAccessMode = value);
                            _emitSettings();
                          }
                        : null,
                  ),
                ),
                SizedBox(
                  width: 128,
                  child: DropdownButtonFormField<String>(
                    initialValue: modelItems.any((item) => item.value == _modelId) ? _modelId : '',
                    isExpanded: true,
                    decoration: const InputDecoration(labelText: 'Model'),
                    items: modelItems,
                    onChanged: enabled
                        ? (value) {
                            if (value == null) return;
                            setState(() => _modelId = value);
                            _emitSettings();
                          }
                        : null,
                  ),
                ),
                SizedBox(
                  width: 128,
                  child: DropdownButtonFormField<String>(
                    initialValue: _reasoningEffort,
                    isExpanded: true,
                    decoration: const InputDecoration(labelText: 'Reasoning'),
                    items: const [
                      DropdownMenuItem(value: '', child: Text('Default')),
                      DropdownMenuItem(value: 'low', child: Text('low')),
                      DropdownMenuItem(value: 'medium', child: Text('medium')),
                      DropdownMenuItem(value: 'high', child: Text('high')),
                    ],
                    onChanged: enabled
                        ? (value) {
                            if (value == null) return;
                            setState(() => _reasoningEffort = value);
                            _emitSettings();
                          }
                        : null,
                  ),
                ),
              ],
            ),
            const SizedBox(height: 8),
            OutlinedButton.icon(
              onPressed: enabled
                  ? () => widget.onRunningStateChanged(!widget.selection.isRunning)
                  : null,
              icon: Icon(widget.selection.isRunning ? Icons.pause_circle : Icons.play_circle),
              label: Text(widget.selection.isRunning ? 'Mark Idle' : 'Mark Running'),
            ),
            const SizedBox(height: 8),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                OutlinedButton(
                  onPressed: enabled
                      ? () {
                          final trimmed = _nameController.text.trim();
                          if (trimmed.isNotEmpty) {
                            widget.onRenameThread(trimmed);
                          }
                        }
                      : null,
                  child: const Text('Save Name'),
                ),
                FilledButton.tonal(
                  onPressed: enabled ? widget.onArchiveThread : null,
                  child: const Text('Archive'),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}
