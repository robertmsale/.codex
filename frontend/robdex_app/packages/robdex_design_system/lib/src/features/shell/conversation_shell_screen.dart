import 'dart:ui';

import 'package:flutter/material.dart';

import '../../core/models/conversation_shell_models.dart';
import '../../core/models/workbench_models.dart';
import '../chat/chat_timeline.dart';
import '../composer/composer_panel.dart';

class ConversationShellScreen extends StatelessWidget {
  const ConversationShellScreen({
    super.key,
    required this.data,
    required this.onSessionSelected,
    required this.onCreateSession,
    required this.onSendMessage,
    required this.onInterrupt,
    this.onProjectSelected,
    this.onCreateProject,
    this.onEditProject,
    this.onNewSessionInProject,
    this.onArchiveProject,
    this.detailContent,
    this.onCloseSession,
    this.onArchiveSession,
    this.onForkSession,
    this.onSettings,
    this.showPermanentDetail = true,
    this.headerControls,
    this.leftRailWidth = 288,
    this.onLeftRailWidthChanged,
  });

  final ConversationShellData data;
  final ValueChanged<String> onSessionSelected;
  final VoidCallback onCreateSession;
  final ValueChanged<ComposerSubmission> onSendMessage;
  final VoidCallback onInterrupt;
  final ValueChanged<String>? onProjectSelected;
  final VoidCallback? onCreateProject;
  final ValueChanged<String>? onEditProject;
  final ValueChanged<String>? onNewSessionInProject;
  final ValueChanged<String>? onArchiveProject;
  final Widget? detailContent;
  final ValueChanged<String>? onCloseSession;
  final ValueChanged<String>? onArchiveSession;
  final ValueChanged<String>? onForkSession;
  final VoidCallback? onSettings;
  final bool showPermanentDetail;
  final Widget? headerControls;
  final double leftRailWidth;
  final ValueChanged<double>? onLeftRailWidthChanged;

  static const double minLeftRailWidth = 220;
  static const double maxLeftRailWidth = 420;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: const Color(0xFF05090F),
      body: SafeArea(
        child: LayoutBuilder(
          builder: (context, constraints) {
            final compact = constraints.maxWidth < 760;
            final rail = _ConversationRail(
              data: data,
              compact: compact,
              onSessionSelected: onSessionSelected,
              onCreateSession: onCreateSession,
              onProjectSelected: onProjectSelected,
              onCreateProject: onCreateProject,
              onEditProject: onEditProject,
              onNewSessionInProject: onNewSessionInProject,
              onArchiveProject: onArchiveProject,
              onCloseSession: onCloseSession,
              onArchiveSession: onArchiveSession,
              onForkSession: onForkSession,
              onSettings: onSettings,
            );
            final center = _ConversationCenter(
              key: const ValueKey('conversationShell.center'),
              data: data,
              onSendMessage: onSendMessage,
              onInterrupt: onInterrupt,
              headerControls: headerControls,
            );
            final detail = detailContent ?? _ConversationDetail(data: data);
            if (compact) {
              return Column(
                children: [
                  SizedBox(height: 220, child: _RailSurfaceFrame(child: rail)),
                  const Divider(height: 1, color: Color(0xFF263241)),
                  Expanded(child: center),
                ],
              );
            }
            final railWidth = leftRailWidth.clamp(minLeftRailWidth, maxLeftRailWidth).toDouble();
            return Row(
              children: [
                SizedBox(width: railWidth, child: _RailSurfaceFrame(child: rail)),
                MouseRegion(
                  cursor: SystemMouseCursors.resizeColumn,
                  child: GestureDetector(
                    key: const ValueKey('conversationShell.leftRailResizeHandle'),
                    behavior: HitTestBehavior.opaque,
                    onHorizontalDragUpdate: onLeftRailWidthChanged == null
                        ? null
                        : (details) {
                            final next = (railWidth + details.delta.dx).clamp(minLeftRailWidth, maxLeftRailWidth).toDouble();
                            onLeftRailWidthChanged!(next);
                          },
                    child: const SizedBox(
                      width: 8,
                      child: Center(
                        child: VerticalDivider(width: 1, color: Color(0xFF3A4653)),
                      ),
                    ),
                  ),
                ),
                Expanded(child: center),
                if (showPermanentDetail) ...[
                  const VerticalDivider(width: 1, color: Color(0xFF263241)),
                  SizedBox(width: 320, child: detail),
                ],
              ],
            );
          },
        ),
      ),
    );
  }
}

class _RailSurfaceFrame extends StatelessWidget {
  const _RailSurfaceFrame({required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: const BoxDecoration(
        border: Border(right: BorderSide(color: Color(0xFF30343B))),
      ),
      child: Stack(
        children: [
          const Positioned.fill(child: _ConversationBrushedMetalSidebarSurface()),
          Positioned.fill(child: child),
        ],
      ),
    );
  }
}

class _ConversationRail extends StatefulWidget {
  const _ConversationRail({
    required this.data,
    required this.compact,
    required this.onSessionSelected,
    required this.onCreateSession,
    this.onProjectSelected,
    this.onCreateProject,
    this.onEditProject,
    this.onNewSessionInProject,
    this.onArchiveProject,
    this.onCloseSession,
    this.onArchiveSession,
    this.onForkSession,
    this.onSettings,
  });

  final ConversationShellData data;
  final bool compact;
  final ValueChanged<String> onSessionSelected;
  final VoidCallback onCreateSession;
  final ValueChanged<String>? onProjectSelected;
  final VoidCallback? onCreateProject;
  final ValueChanged<String>? onEditProject;
  final ValueChanged<String>? onNewSessionInProject;
  final ValueChanged<String>? onArchiveProject;
  final ValueChanged<String>? onCloseSession;
  final ValueChanged<String>? onArchiveSession;
  final ValueChanged<String>? onForkSession;
  final VoidCallback? onSettings;

  @override
  State<_ConversationRail> createState() => _ConversationRailState();
}

class _ConversationRailState extends State<_ConversationRail> {
  double _projectSectionHeight = 132;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: EdgeInsets.all(widget.compact ? 12 : 16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(widget.data.appTitle, style: theme.textTheme.titleMedium?.copyWith(fontWeight: FontWeight.w800)),
                    const SizedBox(height: 3),
                    Text(widget.data.connectionLabel, maxLines: 1, overflow: TextOverflow.ellipsis, style: theme.textTheme.bodySmall?.copyWith(color: const Color(0xFFAAB6C4))),
                  ],
                ),
              ),
              IconButton(
                key: const ValueKey('conversationShell.globalSettings'),
                onPressed: widget.onSettings,
                tooltip: widget.onSettings == null ? 'Settings are loading' : 'Global settings',
                icon: const Icon(Icons.tune_rounded, size: 18),
              ),
              IconButton(onPressed: widget.onCreateSession, tooltip: 'New session', icon: const Icon(Icons.add_comment_rounded, size: 18)),
            ],
          ),
          const SizedBox(height: 14),
          Expanded(
            child: LayoutBuilder(
              builder: (context, constraints) {
                final minProject = widget.compact ? 56.0 : 96.0;
                final availableHeight = constraints.maxHeight.isFinite ? constraints.maxHeight : 320.0;
                final maxProject = availableHeight <= minProject + 88
                    ? minProject
                    : (availableHeight * 0.65).clamp(minProject, availableHeight - 88).toDouble();
                final projectHeight = widget.data.projects.isEmpty ? 0.0 : _projectSectionHeight.clamp(minProject, maxProject).toDouble();
                return Column(
                  children: [
                    if (widget.data.projects.isNotEmpty)
                      SizedBox(
                        height: projectHeight,
                        child: _RailProjectsSection(
                          data: widget.data,
                          compact: widget.compact,
                          onProjectSelected: widget.onProjectSelected,
                          onCreateProject: widget.onCreateProject,
                          onEditProject: widget.onEditProject,
                          onNewSessionInProject: widget.onNewSessionInProject,
                          onArchiveProject: widget.onArchiveProject,
                        ),
                      ),
                    if (widget.data.projects.isNotEmpty)
                      MouseRegion(
                        cursor: SystemMouseCursors.resizeRow,
                        child: GestureDetector(
                          key: const ValueKey('conversationShell.projectSessionResizeHandle'),
                          behavior: HitTestBehavior.opaque,
                          onVerticalDragUpdate: (details) {
                            setState(() {
                              _projectSectionHeight = (_projectSectionHeight + details.delta.dy).clamp(minProject, maxProject).toDouble();
                            });
                          },
                          child: const SizedBox(
                            height: 12,
                            child: Center(
                              child: Divider(height: 1, color: Color(0xFF2D3744)),
                            ),
                          ),
                        ),
                      ),
                    Expanded(
                      child: _RailSessionsSection(
                        data: widget.data,
                        onSessionSelected: widget.onSessionSelected,
                        onCloseSession: widget.onCloseSession,
                        onArchiveSession: widget.onArchiveSession,
                        onForkSession: widget.onForkSession,
                      ),
                    ),
                  ],
                );
              },
            ),
          ),
        ],
      ),
    );
  }
}

class _RailProjectsSection extends StatelessWidget {
  const _RailProjectsSection({
    required this.data,
    required this.compact,
    this.onProjectSelected,
    this.onCreateProject,
    this.onEditProject,
    this.onNewSessionInProject,
    this.onArchiveProject,
  });

  final ConversationShellData data;
  final bool compact;
  final ValueChanged<String>? onProjectSelected;
  final VoidCallback? onCreateProject;
  final ValueChanged<String>? onEditProject;
  final ValueChanged<String>? onNewSessionInProject;
  final ValueChanged<String>? onArchiveProject;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Expanded(child: Text(data.projectLabel, style: theme.textTheme.labelMedium?.copyWith(color: const Color(0xFFAAB6C4)))),
            if (onCreateProject != null)
              IconButton(
                tooltip: 'Create project',
                onPressed: onCreateProject,
                visualDensity: VisualDensity.compact,
                icon: const Icon(Icons.create_new_folder_rounded, size: 16),
              ),
          ],
        ),
        const SizedBox(height: 4),
        Expanded(
          child: ListView.separated(
            key: const ValueKey('conversationShell.projectsScrollView'),
            scrollDirection: compact ? Axis.horizontal : Axis.vertical,
            itemBuilder: (context, index) {
              final project = data.projects[index];
              final tile = InkWell(
                borderRadius: BorderRadius.circular(6),
                onTap: onProjectSelected == null ? null : () => onProjectSelected!(project.id),
                child: Padding(
                  padding: const EdgeInsets.only(left: 8, right: 2, top: 4, bottom: 4),
                  child: Row(
                    children: [
                      Expanded(child: Text(project.title, maxLines: 1, overflow: TextOverflow.ellipsis)),
                      PopupMenuButton<String>(
                        key: ValueKey('conversationProject.menu.${project.id}'),
                        tooltip: 'Project actions',
                        onSelected: (action) {
                          switch (action) {
                            case 'edit':
                              onEditProject?.call(project.id);
                              break;
                            case 'new':
                              onNewSessionInProject?.call(project.id);
                              break;
                            case 'archive':
                              onArchiveProject?.call(project.id);
                              break;
                          }
                        },
                        itemBuilder: (context) => [
                          if (project.canEdit) const PopupMenuItem(value: 'edit', child: Text('Edit project')),
                          if (project.canCreateSession)
                            PopupMenuItem(
                              value: 'new',
                              child: Text(project.id == '__unassigned__' ? 'New unassigned session' : project.id == '__all__' ? 'New session' : 'New session in project'),
                            ),
                          if (project.canArchive) const PopupMenuItem(value: 'archive', child: Text('Archive project')),
                        ],
                        icon: const Icon(Icons.more_horiz_rounded, size: 16),
                      ),
                    ],
                  ),
                ),
              );
              return compact ? SizedBox(width: 180, child: tile) : tile;
            },
            separatorBuilder: (_, _) => const SizedBox(width: 6, height: 4),
            itemCount: data.projects.length,
          ),
        ),
      ],
    );
  }
}

class _RailSessionsSection extends StatelessWidget {
  const _RailSessionsSection({
    required this.data,
    required this.onSessionSelected,
    this.onCloseSession,
    this.onArchiveSession,
    this.onForkSession,
  });

  final ConversationShellData data;
  final ValueChanged<String> onSessionSelected;
  final ValueChanged<String>? onCloseSession;
  final ValueChanged<String>? onArchiveSession;
  final ValueChanged<String>? onForkSession;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(data.sessionLabel, style: theme.textTheme.labelMedium?.copyWith(color: const Color(0xFFAAB6C4))),
        const SizedBox(height: 6),
        Expanded(
          child: data.sessions.isEmpty
              ? _EmptyRail(title: data.emptyTitle, text: data.emptyText)
              : ListView.separated(
                  key: const ValueKey('conversationShell.sessionsScrollView'),
                  itemCount: data.sessions.length,
                  separatorBuilder: (_, _) => const SizedBox(height: 4),
                  itemBuilder: (context, index) {
                    final session = data.sessions[index];
                    return _SessionTile(
                      session: session,
                      onTap: () => onSessionSelected(session.id),
                      onClose: onCloseSession == null ? null : () => onCloseSession!(session.id),
                      onArchive: onArchiveSession == null ? null : () => onArchiveSession!(session.id),
                      onFork: onForkSession == null ? null : () => onForkSession!(session.id),
                    );
                  },
                ),
        ),
      ],
    );
  }
}

class _SessionTile extends StatelessWidget {
  const _SessionTile({required this.session, required this.onTap, this.onClose, this.onArchive, this.onFork});
  final ConversationSession session;
  final VoidCallback onTap;
  final VoidCallback? onClose;
  final VoidCallback? onArchive;
  final VoidCallback? onFork;

  @override
  Widget build(BuildContext context) {
    final selected = session.selected;
    return Material(
      color: selected ? const Color(0xFF132034) : Colors.transparent,
      borderRadius: BorderRadius.circular(6),
      child: InkWell(
        borderRadius: BorderRadius.circular(6),
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.all(10),
          child: Row(
            children: [
              CircleAvatar(
                radius: 13,
                backgroundColor: _toneColor(session.rolePresentation.tone),
                child: Text(session.rolePresentation.shortLabel, style: const TextStyle(fontSize: 10, fontWeight: FontWeight.w800)),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(session.title, maxLines: 1, overflow: TextOverflow.ellipsis, style: const TextStyle(fontWeight: FontWeight.w700)),
                    const SizedBox(height: 2),
                    Text(session.subtitle, maxLines: 1, overflow: TextOverflow.ellipsis, style: const TextStyle(color: Color(0xFFAAB6C4), fontSize: 12)),
                  ],
                ),
              ),
              PopupMenuButton<String>(
                tooltip: 'Session actions',
                onSelected: (value) {
                  switch (value) {
                    case 'close':
                      onClose?.call();
                    case 'archive':
                      onArchive?.call();
                    case 'fork':
                      onFork?.call();
                  }
                },
                itemBuilder: (context) => [
                  PopupMenuItem(value: 'close', enabled: onClose != null, child: const Text('Close')),
                  PopupMenuItem(value: 'archive', enabled: onArchive != null, child: const Text('Archive')),
                  PopupMenuItem(value: 'fork', enabled: onFork != null, child: const Text('Fork')),
                ],
                child: const Icon(Icons.more_horiz_rounded, size: 18, color: Color(0xFFAAB6C4)),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _ConversationCenter extends StatelessWidget {
  const _ConversationCenter({super.key, required this.data, required this.onSendMessage, required this.onInterrupt, this.headerControls});
  final ConversationShellData data;
  final ValueChanged<ComposerSubmission> onSendMessage;
  final VoidCallback onInterrupt;
  final Widget? headerControls;

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: const BoxDecoration(color: Color(0xFF171C22)),
      child: Stack(
        children: [
          const Positioned.fill(child: _ConversationPaperSurface()),
          Padding(
            padding: const EdgeInsets.fromLTRB(28, 16, 28, 22),
            child: ChatTimeline(
              threadId: data.selectedSessionId,
              entries: data.entries,
              title: data.timelineTitle,
              contextWindowRemainingPercent: null,
              onSend: onSendMessage,
              onInterrupt: onInterrupt,
              composerEnabled: data.composerEnabled,
              composerDisabledHint: data.composerDisabledHint,
              composerPlaceholder: data.composerPlaceholder,
              isRunning: data.isRunning,
              selection: WorkspaceSelection(
                projectId: data.projects.isEmpty ? null : data.projects.first.id,
                projectRootPath: null,
                projectOrchestratorThreadId: null,
                projectOrchestratorName: null,
                threadId: data.selectedSessionId,
                threadRole: _selectedRole(data),
                projectName: data.projects.isEmpty ? 'Agent Runtime' : data.projects.first.title,
                threadName: data.timelineTitle,
                connectionLabel: data.connectionLabel,
                isRunning: data.isRunning,
              ),
              availableModels: const [ModelItem(id: 'runtime-default', name: 'Runtime default', hidden: false)],
              onSettingsChanged: (_) {},
              onCompactThread: () {},
              headerControls: headerControls,
            ),
          ),
          if (data.inlineErrorMessage case final message?)
            Positioned(
              left: 28,
              right: 28,
              bottom: 94,
              child: _ConversationInlineError(message: message),
            ),
        ],
      ),
    );
  }
}

class _ConversationInlineError extends StatelessWidget {
  const _ConversationInlineError({required this.message});

  final String message;

  @override
  Widget build(BuildContext context) {
    return Semantics(
      liveRegion: true,
      label: 'Agent Runtime error',
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: const Color(0xFF351D24),
          border: Border.all(color: const Color(0xFFB85C6A)),
          borderRadius: BorderRadius.circular(12),
        ),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
          child: Row(
            children: [
              const Icon(Icons.error_outline_rounded, color: Color(0xFFFFB4BF), size: 18),
              const SizedBox(width: 10),
              Expanded(
                child: Text(
                  message,
                  style: Theme.of(context).textTheme.bodyMedium?.copyWith(color: const Color(0xFFFFD9DE)),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _ConversationPaperSurface extends StatefulWidget {
  const _ConversationPaperSurface();

  @override
  State<_ConversationPaperSurface> createState() => _ConversationPaperSurfaceState();
}

class _ConversationPaperSurfaceState extends State<_ConversationPaperSurface> {
  FragmentShader? _shader;

  @override
  void initState() {
    super.initState();
    _loadShader();
  }

  Future<void> _loadShader() async {
    try {
      final program = await FragmentProgram.fromAsset(
        'packages/robdex_design_system/shaders/timeline_paper_surface.frag',
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
          painter: _ConversationPaperSurfacePainter(shader: _shader),
        ),
      ),
    );
  }
}

class _ConversationPaperSurfacePainter extends CustomPainter {
  const _ConversationPaperSurfacePainter({required this.shader});

  final FragmentShader? shader;

  @override
  void paint(Canvas canvas, Size size) {
    final rect = Offset.zero & size;
    canvas.drawRect(
      rect,
      Paint()
        ..shader = const LinearGradient(
          begin: Alignment.topCenter,
          end: Alignment.bottomCenter,
          colors: [Color(0xFF101820), Color(0xFF0D151C), Color(0xFF0A1117)],
        ).createShader(rect),
    );
    final activeShader = shader;
    if (activeShader != null) {
      activeShader.setFloat(0, size.width);
      activeShader.setFloat(1, size.height);
      activeShader.setFloat(2, 0.0);
      canvas.drawRect(rect, Paint()..shader = activeShader);
    }
    final hairline = Paint()
      ..color = const Color(0x12FFFFFF)
      ..strokeWidth = 0.5;
    for (double y = 0; y < size.height; y += 18) {
      canvas.drawLine(Offset(0, y), Offset(size.width, y), hairline);
    }
  }

  @override
  bool shouldRepaint(covariant _ConversationPaperSurfacePainter oldDelegate) => oldDelegate.shader != shader;
}

class _ConversationBrushedMetalSidebarSurface extends StatefulWidget {
  const _ConversationBrushedMetalSidebarSurface();

  @override
  State<_ConversationBrushedMetalSidebarSurface> createState() => _ConversationBrushedMetalSidebarSurfaceState();
}

class _ConversationBrushedMetalSidebarSurfaceState extends State<_ConversationBrushedMetalSidebarSurface> {
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
          painter: _ConversationBrushedMetalSidebarPainter(shader: _shader),
        ),
      ),
    );
  }
}

class _ConversationBrushedMetalSidebarPainter extends CustomPainter {
  const _ConversationBrushedMetalSidebarPainter({required this.shader});

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
          colors: [Color(0xFF151A20), Color(0xFF0D131A), Color(0xFF171C22)],
          stops: [0.0, 0.58, 1.0],
        ).createShader(rect),
    );
    final activeShader = shader;
    if (activeShader != null) {
      activeShader.setFloat(0, size.width);
      activeShader.setFloat(1, size.height);
      activeShader.setFloat(2, 0.0);
      canvas.drawRect(rect, Paint()..shader = activeShader);
    }
    canvas.drawRect(
      rect,
      Paint()
        ..shader = LinearGradient(
          begin: Alignment.centerLeft,
          end: Alignment.centerRight,
          colors: [
            Colors.white.withValues(alpha: 0.045),
            Colors.transparent,
            Colors.black.withValues(alpha: 0.18),
          ],
        ).createShader(rect),
    );
  }

  @override
  bool shouldRepaint(covariant _ConversationBrushedMetalSidebarPainter oldDelegate) => oldDelegate.shader != shader;
}

class _ConversationDetail extends StatelessWidget {
  const _ConversationDetail({required this.data});
  final ConversationShellData data;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.all(16),
      child: ListView(
        children: [
          Text(data.detailTitle, style: theme.textTheme.titleSmall?.copyWith(fontWeight: FontWeight.w800)),
          const SizedBox(height: 12),
          for (final section in data.detailSections) ...[
            Text(section.title, style: theme.textTheme.labelLarge?.copyWith(color: const Color(0xFFAAB6C4))),
            const SizedBox(height: 6),
            for (final row in section.rows)
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 4),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    SizedBox(width: 112, child: Text(row.label, style: const TextStyle(color: Color(0xFFAAB6C4), fontSize: 12))),
                    Expanded(child: Text(row.value, style: const TextStyle(fontSize: 12))),
                  ],
                ),
              ),
            const SizedBox(height: 14),
          ],
        ],
      ),
    );
  }
}

class _EmptyRail extends StatelessWidget {
  const _EmptyRail({required this.title, required this.text});
  final String title;
  final String text;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(title, style: const TextStyle(fontWeight: FontWeight.w700)),
          const SizedBox(height: 4),
          Text(text, textAlign: TextAlign.center, style: const TextStyle(color: Color(0xFFAAB6C4), fontSize: 12)),
        ],
      ),
    );
  }
}

Color _toneColor(String tone) {
  return switch (tone) {
    'success' => const Color(0xFF1F7A4D),
    'warning' => const Color(0xFFA66A00),
    'danger' || 'error' => const Color(0xFF963D4A),
    _ => const Color(0xFF2C5D8F),
  };
}

String? _selectedRole(ConversationShellData data) {
  for (final session in data.sessions) {
    if (session.id == data.selectedSessionId) {
      return session.role;
    }
  }
  return null;
}
