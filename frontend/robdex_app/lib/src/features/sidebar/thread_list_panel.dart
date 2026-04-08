import 'package:flutter/material.dart';

import '../../core/models/workbench_models.dart';

class ThreadListPanel extends StatelessWidget {
  const ThreadListPanel({
    super.key,
    required this.selection,
    required this.projects,
    required this.threads,
    required this.pendingApprovals,
    required this.onDisconnect,
    required this.onThreadSelected,
    required this.onCreateProject,
    required this.onProjectSettings,
    required this.onCreateThread,
    required this.onSpawnAgent,
  });

  final WorkspaceSelection selection;
  final List<ProjectItem> projects;
  final List<ThreadItem> threads;
  final List<PendingApprovalItem> pendingApprovals;
  final VoidCallback onDisconnect;
  final ValueChanged<String> onThreadSelected;
  final VoidCallback onCreateProject;
  final ValueChanged<ProjectItem> onProjectSettings;
  final ValueChanged<ProjectItem> onCreateThread;
  final VoidCallback onSpawnAgent;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final grouped = <String, List<ThreadItem>>{
      for (final project in projects) project.id: <ThreadItem>[],
    };
    for (final thread in threads) {
      final project = projects.where((project) => project.name == thread.projectName).cast<ProjectItem?>().firstWhere(
            (project) => project != null,
            orElse: () => null,
          );
      if (project != null) {
        grouped.putIfAbsent(project.id, () => <ThreadItem>[]).add(thread);
      }
    }

    final orderedProjects = [...projects]
      ..sort((a, b) => a.name.toLowerCase().compareTo(b.name.toLowerCase()));

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Expanded(
              child: Text(
                'Threads',
                style: theme.textTheme.bodyMedium?.copyWith(fontWeight: FontWeight.w800),
              ),
            ),
            TextButton.icon(
              onPressed: onCreateProject,
              icon: const Icon(Icons.create_new_folder_outlined, size: 14),
              label: const Text('Project'),
            ),
            const SizedBox(width: 4),
            IconButton(
              onPressed: onDisconnect,
              tooltip: 'Disconnect',
              icon: const Icon(Icons.link_off, size: 18),
            ),
          ],
        ),
        const SizedBox(height: 8),
        Expanded(
          child: ListView.builder(
            itemCount: orderedProjects.length,
            itemBuilder: (context, index) {
              final project = orderedProjects[index];
              final projectThreads = grouped[project.id] ?? const <ThreadItem>[];
              return Padding(
                padding: EdgeInsets.only(bottom: index == orderedProjects.length - 1 ? 0 : 12),
                child: _ProjectSection(
                  project: project,
                  threads: projectThreads,
                  pendingApprovals: pendingApprovals,
                  selectedThreadId: selection.threadId,
                  onProjectSettings: () => onProjectSettings(project),
                  onCreateThread: () => onCreateThread(project),
                  onThreadSelected: onThreadSelected,
                ),
              );
            },
          ),
        ),
      ],
    );
  }
}

class _ProjectSection extends StatelessWidget {
  const _ProjectSection({
    required this.project,
    required this.threads,
    required this.pendingApprovals,
    required this.selectedThreadId,
    required this.onProjectSettings,
    required this.onCreateThread,
    required this.onThreadSelected,
  });

  final ProjectItem project;
  final List<ThreadItem> threads;
  final List<PendingApprovalItem> pendingApprovals;
  final String? selectedThreadId;
  final VoidCallback onProjectSettings;
  final VoidCallback onCreateThread;
  final ValueChanged<String> onThreadSelected;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Expanded(
              child: RichText(
                text: TextSpan(
                  style: theme.textTheme.labelSmall?.copyWith(
                    color: theme.colorScheme.onSurface.withValues(alpha: 0.82),
                  ),
                  children: [
                    TextSpan(
                      text: project.name,
                      style: const TextStyle(fontWeight: FontWeight.w800),
                    ),
                    TextSpan(
                      text: '  ${project.defaultCwd}',
                      style: TextStyle(
                        fontFamily: 'monospace',
                        color: theme.colorScheme.onSurface.withValues(alpha: 0.62),
                      ),
                    ),
                  ],
                ),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
            ),
            IconButton(
              onPressed: onProjectSettings,
              tooltip: 'Project settings for ${project.name}',
              icon: const Icon(Icons.settings_outlined, size: 16),
              visualDensity: VisualDensity.compact,
            ),
            IconButton(
              onPressed: onCreateThread,
              tooltip: 'Create thread in ${project.name}',
              icon: const Icon(Icons.add, size: 16),
              visualDensity: VisualDensity.compact,
            ),
          ],
        ),
        const SizedBox(height: 4),
        ...threads.map(
          (thread) => _ThreadTile(
            thread: thread,
            hasPendingApproval: pendingApprovals.any((approval) => approval.threadId == thread.id),
            isSelected: thread.id == selectedThreadId,
            onTap: () => onThreadSelected(thread.id),
          ),
        ),
      ],
    );
  }
}

class _ThreadTile extends StatelessWidget {
  const _ThreadTile({
    required this.thread,
    required this.hasPendingApproval,
    required this.isSelected,
    required this.onTap,
  });

  final ThreadItem thread;
  final bool hasPendingApproval;
  final bool isSelected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final foreground = isSelected ? theme.colorScheme.primary : theme.colorScheme.onSurface;

    return Material(
      color: Colors.transparent,
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(8),
        child: Container(
          margin: const EdgeInsets.only(bottom: 4),
          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(8),
            color: isSelected ? theme.colorScheme.primary.withValues(alpha: 0.1) : Colors.transparent,
            border: Border(
              left: BorderSide(
                color: isSelected ? theme.colorScheme.primary : Colors.transparent,
                width: 2,
              ),
            ),
          ),
          child: Row(
            children: [
              Expanded(
                child: Text(
                  thread.title,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: theme.textTheme.bodySmall?.copyWith(
                    fontWeight: FontWeight.w700,
                    color: foreground,
                  ),
                ),
              ),
              if (thread.isRunning)
                ...[
                  _RunningBadge(isSelected: isSelected),
                  const SizedBox(width: 6),
                ],
              if (hasPendingApproval) ...[
                const _PendingApprovalBadge(),
                const SizedBox(width: 6),
              ],
              _RoleBadge(role: thread.role),
              if (thread.unreadCount > 0) ...[
                const SizedBox(width: 6),
                Text(
                  '${thread.unreadCount}',
                  style: theme.textTheme.labelSmall?.copyWith(fontWeight: FontWeight.w700),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

class _PendingApprovalBadge extends StatelessWidget {
  const _PendingApprovalBadge();

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: 'Pending approval',
      child: Container(
        width: 8,
        height: 8,
        decoration: BoxDecoration(
          color: Colors.amber.shade700,
          shape: BoxShape.circle,
        ),
      ),
    );
  }
}

class _RunningBadge extends StatelessWidget {
  const _RunningBadge({required this.isSelected});

  final bool isSelected;

  @override
  Widget build(BuildContext context) {
    final color = Colors.green.shade700;
    return Tooltip(
      message: 'Running',
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: color.withValues(alpha: 0.12),
          borderRadius: BorderRadius.circular(999),
          border: Border.all(color: color.withValues(alpha: 0.45)),
        ),
        child: Padding(
          padding: const EdgeInsets.all(5),
          child: Icon(
            Icons.bolt_rounded,
            size: 11,
            color: color,
          ),
        ),
      ),
    );
  }
}

class _RoleBadge extends StatelessWidget {
  const _RoleBadge({required this.role});

  final String role;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final (background, foreground, icon, tooltip) = switch (role) {
      'operator' => (
          theme.colorScheme.primary.withValues(alpha: 0.12),
          theme.colorScheme.primary,
          Icons.verified_user_outlined,
          'Operator',
        ),
      'orchestrator' => (
          theme.colorScheme.secondary.withValues(alpha: 0.14),
          theme.colorScheme.secondary,
          Icons.account_tree_outlined,
          'Orchestrator',
        ),
      'qa' => (
          theme.colorScheme.tertiary.withValues(alpha: 0.14),
          theme.colorScheme.tertiary,
          Icons.fact_check_outlined,
          'QA',
        ),
      'hidden' => (
          theme.colorScheme.outline.withValues(alpha: 0.14),
          theme.colorScheme.outline,
          Icons.visibility_off_outlined,
          'Hidden',
        ),
      _ => (
          theme.colorScheme.onSurface.withValues(alpha: 0.08),
          theme.colorScheme.onSurface.withValues(alpha: 0.75),
          Icons.build_circle_outlined,
          'Worker',
        ),
    };

    return Tooltip(
      message: tooltip,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: background,
          borderRadius: BorderRadius.circular(999),
        ),
        child: Padding(
          padding: const EdgeInsets.all(4),
          child: Icon(icon, size: 12, color: foreground),
        ),
      ),
    );
  }
}
