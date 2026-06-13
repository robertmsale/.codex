import 'dart:ui';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../core/models/workbench_models.dart';

class ThreadListPanel extends StatelessWidget {
  const ThreadListPanel({
    super.key,
    required this.selection,
    required this.projects,
    required this.threads,
    required this.pendingApprovals,
    required this.onDisconnect,
    required this.onGlobalSettings,
    required this.onThreadSelected,
    required this.onCreateProject,
    required this.onProjectSettings,
    required this.onCreateThread,
    required this.onSpawnAgent,
    required this.onWeeklyStats,
  });

  final WorkspaceSelection selection;
  final List<ProjectItem> projects;
  final List<ThreadItem> threads;
  final List<PendingApprovalItem> pendingApprovals;
  final VoidCallback onDisconnect;
  final VoidCallback onGlobalSettings;
  final ValueChanged<String> onThreadSelected;
  final VoidCallback onCreateProject;
  final ValueChanged<ProjectItem> onProjectSettings;
  final ValueChanged<ProjectItem> onCreateThread;
  final VoidCallback onSpawnAgent;
  final VoidCallback onWeeklyStats;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final grouped = <String, List<ThreadItem>>{
      for (final project in projects) project.id: <ThreadItem>[],
    };
    for (final thread in threads) {
      final project = projects.where((project) {
        if (thread.projectId.isNotEmpty && project.id == thread.projectId) {
          return true;
        }
        if (thread.projectRootPath.isNotEmpty && project.rootPath == thread.projectRootPath) {
          return true;
        }
        return project.name == thread.projectName;
      }).cast<ProjectItem?>().firstWhere(
            (project) => project != null,
            orElse: () => null,
          );
      if (project != null) {
        grouped.putIfAbsent(project.id, () => <ThreadItem>[]).add(thread);
      }
    }

    final orderedProjects = [...projects]
      ..sort((a, b) => a.name.toLowerCase().compareTo(b.name.toLowerCase()));

    return Stack(
      fit: StackFit.expand,
      children: [
        const _BrushedMetalSidebarSurface(),
        Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(14, 14, 12, 12),
          child: Row(
            children: [
              const Spacer(),
              _SemanticIconButton(
                id: 'semantic.sidebar.weeklyStats',
                label: 'Open weekly quota attribution',
                onPressed: onWeeklyStats,
                tooltip: 'Weekly quota attribution',
                icon: const Icon(Icons.pie_chart_rounded, size: 17),
              ),
              _SemanticIconButton(
                id: 'semantic.sidebar.globalSettings',
                label: 'Open global settings',
                onPressed: onGlobalSettings,
                tooltip: 'Global settings',
                icon: const Icon(Icons.tune_outlined, size: 17),
              ),
              _SemanticIconButton(
                id: 'semantic.sidebar.newProject',
                label: 'Create new project',
                onPressed: onCreateProject,
                tooltip: 'New project',
                icon: const Icon(Icons.create_new_folder_outlined, size: 16),
              ),
              _SemanticIconButton(
                id: 'semantic.sidebar.disconnect',
                label: 'Disconnect from bridge',
                onPressed: onDisconnect,
                tooltip: 'Disconnect',
                icon: const Icon(Icons.link_off, size: 18),
              ),
            ],
          ),
        ),
        Padding(
          padding: const EdgeInsets.fromLTRB(14, 0, 14, 10),
          child: Row(
            children: [
              Text(
                'Threads',
                style: theme.textTheme.labelLarge?.copyWith(
                  fontWeight: FontWeight.w500,
                  color: theme.colorScheme.onSurface.withValues(alpha: 0.88),
                ),
              ),
              const SizedBox(width: 6),
              DecoratedBox(
                decoration: BoxDecoration(
                  color: theme.colorScheme.surfaceContainerHighest.withValues(alpha: 0.52),
                  borderRadius: BorderRadius.circular(999),
                ),
                child: Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 1),
                  child: Text(
                    '${threads.length}',
                    style: theme.textTheme.labelSmall?.copyWith(
                      color: theme.colorScheme.onSurface.withValues(alpha: 0.62),
                      fontSize: 10,
                    ),
                  ),
                ),
              ),
            ],
          ),
        ),
        Expanded(
          child: ListView.builder(
            itemCount: orderedProjects.length,
            itemBuilder: (context, index) {
              final project = orderedProjects[index];
              final projectThreads = grouped[project.id] ?? const <ThreadItem>[];
              return Padding(
                padding: EdgeInsets.only(bottom: index == orderedProjects.length - 1 ? 0 : 10),
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
        ),
      ],
    );
  }
}

class _BrushedMetalSidebarSurface extends StatefulWidget {
  const _BrushedMetalSidebarSurface();

  @override
  State<_BrushedMetalSidebarSurface> createState() => _BrushedMetalSidebarSurfaceState();
}

class _BrushedMetalSidebarSurfaceState extends State<_BrushedMetalSidebarSurface> {
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
    return IgnorePointer(
      child: RepaintBoundary(
        child: CustomPaint(
          painter: _BrushedMetalSidebarPainter(
            shader: _shader,
            surfaceColor: Theme.of(context).colorScheme.surface,
          ),
        ),
      ),
    );
  }
}

class _BrushedMetalSidebarPainter extends CustomPainter {
  const _BrushedMetalSidebarPainter({
    required this.shader,
    required this.surfaceColor,
  });

  final FragmentShader? shader;
  final Color surfaceColor;

  @override
  void paint(Canvas canvas, Size size) {
    final rect = Offset.zero & size;
    final base = Paint()
      ..shader = LinearGradient(
        begin: Alignment.topLeft,
        end: Alignment.bottomRight,
        colors: [
          surfaceColor.withValues(alpha: 0.78),
          const Color(0xFF0B1219).withValues(alpha: 0.88),
          const Color(0xFF111820).withValues(alpha: 0.82),
        ],
        stops: const [0.0, 0.56, 1.0],
      ).createShader(rect);
    canvas.drawRect(rect, base);

    final activeShader = shader;
    if (activeShader != null) {
      activeShader.setFloat(0, size.width);
      activeShader.setFloat(1, size.height);
      activeShader.setFloat(2, 0.0);
      canvas.drawRect(rect, Paint()..shader = activeShader);
    }

    final edgePaint = Paint()
      ..shader = LinearGradient(
        begin: Alignment.centerLeft,
        end: Alignment.centerRight,
        colors: [
          Colors.white.withValues(alpha: 0.055),
          Colors.transparent,
          Colors.black.withValues(alpha: 0.16),
        ],
        stops: const [0.0, 0.18, 1.0],
      ).createShader(rect);
    canvas.drawRect(rect, edgePaint);

    final topBottomPaint = Paint()
      ..shader = LinearGradient(
        begin: Alignment.topCenter,
        end: Alignment.bottomCenter,
        colors: [
          Colors.white.withValues(alpha: 0.035),
          Colors.transparent,
          Colors.black.withValues(alpha: 0.1),
        ],
        stops: const [0.0, 0.48, 1.0],
      ).createShader(rect);
    canvas.drawRect(rect, topBottomPaint);
  }

  @override
  bool shouldRepaint(covariant _BrushedMetalSidebarPainter oldDelegate) {
    return oldDelegate.shader != shader ||
        oldDelegate.surfaceColor != surfaceColor;
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
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 10),
          child: Row(
            children: [
              Expanded(
              child: Text(
                project.name,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: theme.textTheme.labelSmall?.copyWith(
                  fontWeight: FontWeight.w500,
                  color: theme.colorScheme.onSurface.withValues(alpha: 0.84),
                ),
              ),
            ),
            _SemanticIconButton(
              id: 'semantic.project.settings.${project.id}',
              label: 'Project settings for ${project.name}',
              onPressed: onProjectSettings,
              tooltip: 'Project settings for ${project.name}',
              icon: const Icon(Icons.settings_outlined, size: 15),
              visualDensity: VisualDensity.compact,
            ),
            _SemanticIconButton(
              id: 'semantic.project.createThread.${project.id}',
              label: 'Create thread in ${project.name}',
              onPressed: onCreateThread,
              tooltip: 'Create thread in ${project.name}',
              icon: const Icon(Icons.add, size: 15),
              visualDensity: VisualDensity.compact,
            ),
          ],
          ),
        ),
        const SizedBox(height: 4),
        ...threads.map(
          (thread) => _ThreadTile(
            thread: thread,
            hasPendingApproval:
                pendingApprovals.any((approval) => approval.threadId == thread.id),
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
    final foreground = theme.colorScheme.onSurface.withValues(
      alpha: isSelected ? 0.94 : 0.82,
    );

    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onSecondaryTapUp: (details) => _showThreadContextMenu(
        context,
        thread,
        details.globalPosition,
      ),
      onLongPressStart: (details) => _showThreadContextMenu(
        context,
        thread,
        details.globalPosition,
      ),
      child: Semantics(
        key: thread.isRunning ? const ValueKey('semantic.thread.runningShimmer') : null,
        container: thread.isRunning,
        label: thread.isRunning ? 'Thread is marked running' : null,
        child: Material(
          color: Colors.transparent,
          child: InkWell(
            onTap: onTap,
            child: Container(
              margin: const EdgeInsets.only(bottom: 2),
              color: isSelected
                  ? Colors.white.withValues(alpha: 0.045)
                  : Colors.transparent,
              child: Stack(
                children: [
                  if (thread.isRunning)
                    Positioned.fill(child: _RunningThreadShimmer(isSelected: isSelected)),
                  Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 18, vertical: 6),
                    child: Row(
                      children: [
                        Expanded(
                          child: Text(
                            thread.title,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: theme.textTheme.bodySmall?.copyWith(
                              fontWeight: isSelected ? FontWeight.w700 : FontWeight.w500,
                              color: foreground,
                            ),
                          ),
                        ),
                if (hasPendingApproval) ...[
                  const _PendingApprovalBadge(),
                  const SizedBox(width: 6),
                ],
                if ((thread.requirementReview?.activeRequirementCount ?? 0) > 0) ...[
                  _RequirementReviewBadge(summary: thread.requirementReview!),
                  const SizedBox(width: 6),
                ],
                _RoleBadge(role: thread.role),
                if (thread.unreadCount > 0) ...[
                  const SizedBox(width: 6),
                  Text(
                    '${thread.unreadCount}',
                    style: theme.textTheme.labelSmall?.copyWith(
                      fontWeight: FontWeight.w600,
                      color: theme.colorScheme.onSurface.withValues(alpha: 0.68),
                    ),
                  ),
                ],
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

Future<void> _showThreadContextMenu(
  BuildContext context,
  ThreadItem thread,
  Offset globalPosition,
) async {
  final overlay = Overlay.of(context).context.findRenderObject() as RenderBox;
  final selected = await showMenu<String>(
    context: context,
    position: RelativeRect.fromLTRB(
      globalPosition.dx,
      globalPosition.dy,
      overlay.size.width - globalPosition.dx,
      overlay.size.height - globalPosition.dy,
    ),
    items: const [
      PopupMenuItem(
        value: 'copyName',
        child: Row(
          children: [
            Icon(Icons.content_copy_rounded, size: 16),
            SizedBox(width: 8),
            Text('Copy name'),
          ],
        ),
      ),
      PopupMenuItem(
        value: 'copyId',
        child: Row(
          children: [
            Icon(Icons.tag_rounded, size: 16),
            SizedBox(width: 8),
            Text('Copy ID'),
          ],
        ),
      ),
    ],
  );
  if ((selected != 'copyName' && selected != 'copyId') || !context.mounted) {
    return;
  }

  final valueToCopy = selected == 'copyName' ? thread.title : thread.id;
  await Clipboard.setData(ClipboardData(text: valueToCopy));
  if (!context.mounted) {
    return;
  }
  ScaffoldMessenger.of(context).hideCurrentSnackBar();
  ScaffoldMessenger.of(context).showSnackBar(
    SnackBar(
      content: Text(
        selected == 'copyName' ? 'Copied "${thread.title}"' : 'Copied thread ID "${thread.id}"',
      ),
      duration: const Duration(milliseconds: 1400),
    ),
  );
}

class _SemanticIconButton extends StatelessWidget {
  const _SemanticIconButton({
    required this.id,
    required this.label,
    required this.onPressed,
    required this.tooltip,
    required this.icon,
    this.visualDensity,
  });

  final String id;
  final String label;
  final VoidCallback? onPressed;
  final String tooltip;
  final Widget icon;
  final VisualDensity? visualDensity;

  @override
  Widget build(BuildContext context) {
    return Semantics(
      key: ValueKey(id),
      container: true,
      button: true,
      enabled: onPressed != null,
      label: label,
      child: ExcludeSemantics(
        child: IconButton(
          onPressed: onPressed,
          tooltip: tooltip,
          icon: icon,
          visualDensity: visualDensity,
        ),
      ),
    );
  }
}

class _RequirementReviewBadge extends StatelessWidget {
  const _RequirementReviewBadge({required this.summary});

  final RequirementReviewSummary summary;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final (color, icon) = switch (summary.status) {
      'passed' => (Colors.green.shade700, Icons.check_rounded),
      'failed' => (theme.colorScheme.error, Icons.close_rounded),
      'blocked' => (Colors.amber.shade800, Icons.warning_amber_rounded),
      'waiverRequired' => (Colors.amber.shade700, Icons.policy_outlined),
      'inReview' => (theme.colorScheme.secondary, Icons.rate_review_outlined),
      _ => (theme.colorScheme.tertiary, Icons.rule_outlined),
    };
    return Semantics(
      key: ValueKey('semantic.thread.requirements.${summary.status ?? 'active'}'),
      container: true,
      button: true,
      label: 'Requirements review ${summary.displayStatus}',
      child: ExcludeSemantics(
        child: Tooltip(
          message: 'Requirements: ${summary.displayStatus}',
          child: IconButton(
            onPressed: () => _showRequirementsReadOnlyDialog(context, summary),
            icon: Icon(icon, size: 13, color: color),
            padding: EdgeInsets.zero,
            constraints: const BoxConstraints.tightFor(width: 24, height: 24),
            visualDensity: VisualDensity.compact,
            style: IconButton.styleFrom(
              backgroundColor: color.withValues(alpha: 0.12),
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(4),
                side: BorderSide(color: color.withValues(alpha: 0.38)),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

Future<void> _showRequirementsReadOnlyDialog(
  BuildContext context,
  RequirementReviewSummary summary,
) {
  final theme = Theme.of(context);
  final requirements = summary.requirements;
  return showDialog<void>(
    context: context,
    builder: (context) {
      return Dialog(
        insetPadding: const EdgeInsets.all(24),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(6)),
        child: ConstrainedBox(
          constraints: BoxConstraints(
            maxWidth: MediaQuery.sizeOf(context).width - 48,
            maxHeight: MediaQuery.sizeOf(context).height - 48,
          ),
          child: Padding(
            padding: const EdgeInsets.fromLTRB(24, 18, 16, 18),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Expanded(
                      child: Text(
                        'Requirements',
                        style: theme.textTheme.titleLarge?.copyWith(fontWeight: FontWeight.w700),
                      ),
                    ),
                    IconButton(
                      onPressed: () => Navigator.of(context).pop(),
                      tooltip: 'Close',
                      icon: const Icon(Icons.close_rounded),
                    ),
                  ],
                ),
                const SizedBox(height: 12),
                Expanded(
                  child: requirements.isEmpty
                      ? Text(
                          'No requirements.',
                          style: theme.textTheme.bodyMedium?.copyWith(
                            color: theme.colorScheme.onSurface.withValues(alpha: 0.68),
                          ),
                        )
                      : Scrollbar(
                          child: SingleChildScrollView(
                            padding: const EdgeInsets.only(right: 12, bottom: 8),
                            child: SelectableText(
                              _plainRequirementsText(requirements),
                              style: theme.textTheme.bodyMedium?.copyWith(
                                height: 1.45,
                                fontFamily: 'monospace',
                                color: theme.colorScheme.onSurface.withValues(alpha: 0.9),
                              ),
                            ),
                          ),
                        ),
                ),
              ],
            ),
          ),
        ),
      );
    },
  );
}

String _plainRequirementsText(List<RequirementReviewRequirement> requirements) {
  return requirements.asMap().entries.map((entry) {
    final index = entry.key + 1;
    final requirement = entry.value;
    return [
      'Requirement $index: ${requirement.statement}',
      '  - Risk: ${requirement.severity}',
      '  - Verification Method: ${requirement.verificationMethod}',
    ].join('\n');
  }).join('\n\n');
}

class _PendingApprovalBadge extends StatelessWidget {
  const _PendingApprovalBadge();

  @override
  Widget build(BuildContext context) {
    return Semantics(
      key: const ValueKey('semantic.thread.pendingApprovalBadge'),
      container: true,
      label: 'Thread has a pending approval request',
      child: ExcludeSemantics(
        child: Tooltip(
          message: 'Pending approval',
          child: Container(
            width: 8,
            height: 8,
            decoration: BoxDecoration(
              color: Colors.amber.shade700,
              shape: BoxShape.circle,
            ),
          ),
        ),
      ),
    );
  }
}

class _RunningThreadShimmer extends StatefulWidget {
  const _RunningThreadShimmer({required this.isSelected});

  final bool isSelected;

  @override
  State<_RunningThreadShimmer> createState() => _RunningThreadShimmerState();
}

class _RunningThreadShimmerState extends State<_RunningThreadShimmer>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 2600),
    )..repeat();
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return RepaintBoundary(
      child: AnimatedBuilder(
        animation: _controller,
        builder: (context, _) {
          return CustomPaint(
            painter: _RunningThreadShimmerPainter(
              progress: Curves.easeInOutCubic.transform(_controller.value),
              isSelected: widget.isSelected,
            ),
          );
        },
      ),
    );
  }
}

class _RunningThreadShimmerPainter extends CustomPainter {
  const _RunningThreadShimmerPainter({required this.progress, required this.isSelected});

  final double progress;
  final bool isSelected;

  @override
  void paint(Canvas canvas, Size size) {
    final rect = Offset.zero & size;
    final baseAlpha = isSelected ? 0.14 : 0.1;
    canvas.drawRect(
      rect,
      Paint()
        ..shader = LinearGradient(
          begin: Alignment.centerLeft,
          end: Alignment.centerRight,
          colors: [
            Colors.transparent,
            const Color(0xFF43FF7A).withValues(alpha: baseAlpha * 0.52),
            const Color(0xFF7BFFAB).withValues(alpha: baseAlpha * 1.18),
            const Color(0xFF31E86F).withValues(alpha: baseAlpha * 0.56),
            Colors.transparent,
          ],
          stops: const [0.0, 0.22, 0.5, 0.78, 1.0],
        ).createShader(rect),
    );

    final shaftCenter = -0.28 + (progress * 1.56);
    final shaft = Rect.fromLTWH((shaftCenter * size.width) - 34, 0, 68, size.height);
    canvas.save();
    canvas.skew(-0.32, 0);
    canvas.drawRect(
      shaft,
      Paint()
        ..shader = LinearGradient(
          begin: Alignment.centerLeft,
          end: Alignment.centerRight,
          colors: [
            Colors.transparent,
            const Color(0xFFA8FFC6).withValues(alpha: 0.0),
            const Color(0xFFB7FFD0).withValues(alpha: isSelected ? 0.24 : 0.19),
            const Color(0xFF72FF9B).withValues(alpha: 0.0),
            Colors.transparent,
          ],
          stops: const [0.0, 0.18, 0.5, 0.82, 1.0],
        ).createShader(shaft),
    );
    canvas.restore();

    final edge = Paint()
      ..shader = LinearGradient(
        begin: Alignment.topCenter,
        end: Alignment.bottomCenter,
        colors: [
          Colors.white.withValues(alpha: 0.018),
          Colors.transparent,
          Colors.black.withValues(alpha: 0.09),
        ],
      ).createShader(rect);
    canvas.drawRect(rect, edge);
  }

  @override
  bool shouldRepaint(covariant _RunningThreadShimmerPainter oldDelegate) {
    return oldDelegate.progress != progress || oldDelegate.isSelected != isSelected;
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
      'designer' => (
          Colors.amber.shade100,
          Colors.amber.shade800,
          Icons.palette_outlined,
          'Designer',
        ),
      'planner' => (
          theme.colorScheme.primaryContainer.withValues(alpha: 0.72),
          theme.colorScheme.onPrimaryContainer,
          Icons.psychology_alt_outlined,
          'Planner',
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

    return Semantics(
      key: ValueKey('semantic.thread.roleBadge.$role'),
      container: true,
      label: 'Thread role: $tooltip',
      child: ExcludeSemantics(
        child: Tooltip(
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
        ),
      ),
    );
  }
}
