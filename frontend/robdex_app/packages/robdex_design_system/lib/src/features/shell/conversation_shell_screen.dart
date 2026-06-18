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
    this.detailContent,
    this.onCloseSession,
    this.onArchiveSession,
    this.onForkSession,
    this.onSettings,
    this.showPermanentDetail = true,
    this.headerControls,
  });

  final ConversationShellData data;
  final ValueChanged<String> onSessionSelected;
  final VoidCallback onCreateSession;
  final ValueChanged<ComposerSubmission> onSendMessage;
  final VoidCallback onInterrupt;
  final ValueChanged<String>? onProjectSelected;
  final Widget? detailContent;
  final ValueChanged<String>? onCloseSession;
  final ValueChanged<String>? onArchiveSession;
  final ValueChanged<String>? onForkSession;
  final VoidCallback? onSettings;
  final bool showPermanentDetail;
  final Widget? headerControls;

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
                  SizedBox(height: 220, child: rail),
                  const Divider(height: 1, color: Color(0xFF263241)),
                  Expanded(child: center),
                ],
              );
            }
            return Row(
              children: [
                DecoratedBox(
                  decoration: const BoxDecoration(
                    gradient: LinearGradient(
                      begin: Alignment.topLeft,
                      end: Alignment.bottomRight,
                      colors: [Color(0xFF161A20), Color(0xFF0E1319), Color(0xFF1A1D22)],
                    ),
                    border: Border(right: BorderSide(color: Color(0xFF30343B))),
                  ),
                  child: SizedBox(width: 288, child: rail),
                ),
                const VerticalDivider(width: 1, color: Color(0xFF263241)),
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

class _ConversationRail extends StatelessWidget {
  const _ConversationRail({
    required this.data,
    required this.compact,
    required this.onSessionSelected,
    required this.onCreateSession,
    this.onProjectSelected,
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
  final ValueChanged<String>? onCloseSession;
  final ValueChanged<String>? onArchiveSession;
  final ValueChanged<String>? onForkSession;
  final VoidCallback? onSettings;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: EdgeInsets.all(compact ? 12 : 16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(data.appTitle, style: theme.textTheme.titleMedium?.copyWith(fontWeight: FontWeight.w800)),
                    const SizedBox(height: 3),
                    Text(data.connectionLabel, maxLines: 1, overflow: TextOverflow.ellipsis, style: theme.textTheme.bodySmall?.copyWith(color: const Color(0xFFAAB6C4))),
                  ],
                ),
              ),
              IconButton(onPressed: onSettings, tooltip: onSettings == null ? 'Settings are loading' : 'Settings', icon: const Icon(Icons.tune_rounded, size: 18)),
              TextButton(onPressed: onCreateSession, child: const Text('New')),
            ],
          ),
          const SizedBox(height: 14),
          if (data.projects.isNotEmpty) ...[
            Text(data.projectLabel, style: theme.textTheme.labelMedium?.copyWith(color: const Color(0xFFAAB6C4))),
            const SizedBox(height: 6),
            SizedBox(
              height: compact ? 38 : 74,
              child: ListView.separated(
                scrollDirection: compact ? Axis.horizontal : Axis.vertical,
                itemBuilder: (context, index) {
                  final project = data.projects[index];
                  return InkWell(
                    borderRadius: BorderRadius.circular(6),
                    onTap: onProjectSelected == null ? null : () => onProjectSelected!(project.id),
                    child: Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
                      child: Text(project.title, maxLines: 1, overflow: TextOverflow.ellipsis),
                    ),
                  );
                },
                separatorBuilder: (_, _) => const SizedBox(width: 6, height: 4),
                itemCount: data.projects.length,
              ),
            ),
            const SizedBox(height: 10),
          ],
          Text(data.sessionLabel, style: theme.textTheme.labelMedium?.copyWith(color: const Color(0xFFAAB6C4))),
          const SizedBox(height: 6),
          Expanded(
            child: data.sessions.isEmpty
                ? _EmptyRail(title: data.emptyTitle, text: data.emptyText)
                : ListView.separated(
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
      ),
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
        ],
      ),
    );
  }
}

class _ConversationPaperSurface extends StatelessWidget {
  const _ConversationPaperSurface();

  @override
  Widget build(BuildContext context) {
    return IgnorePointer(
      child: CustomPaint(
        painter: _ConversationPaperSurfacePainter(),
      ),
    );
  }
}

class _ConversationPaperSurfacePainter extends CustomPainter {
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
    final hairline = Paint()
      ..color = const Color(0x22FFFFFF)
      ..strokeWidth = 0.5;
    for (double y = 0; y < size.height; y += 11) {
      canvas.drawLine(Offset(0, y), Offset(size.width, y), hairline);
    }
  }

  @override
  bool shouldRepaint(covariant _ConversationPaperSurfacePainter oldDelegate) => false;
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
