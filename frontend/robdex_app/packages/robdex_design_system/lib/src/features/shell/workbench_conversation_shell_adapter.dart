import '../../core/models/conversation_shell_models.dart';
import '../../core/models/workbench_view_data.dart';

ConversationShellData workbenchConversationShellData(WorkbenchViewData workbench) {
  return ConversationShellData(
    appTitle: 'Robdex',
    connectionLabel: workbench.selection.connectionLabel,
    projects: workbench.projects
        .map((project) => ConversationProject(id: project.id, title: project.name, subtitle: project.rootPath))
        .toList(growable: false),
    sessions: workbench.threads
        .map((thread) => ConversationSession(
              id: thread.id,
              title: thread.title,
              subtitle: thread.preview,
              role: thread.role,
              selected: thread.id == workbench.selection.threadId,
              rolePresentation: ConversationRolePresentation(
                roleId: thread.role,
                displayLabel: thread.role,
                shortLabel: _shortLabel(thread.role),
                iconKey: thread.role,
                tone: thread.isRunning ? 'warning' : 'info',
                statusLabel: thread.isRunning ? 'Running' : 'Idle',
                description: thread.projectName,
              ),
            ))
        .toList(growable: false),
    selectedSessionId: workbench.selection.threadId,
    timelineTitle: workbench.selection.threadName,
    entries: workbench.chatEntries,
    composerEnabled: workbench.selection.threadId != null,
    isRunning: workbench.selection.isRunning,
    detailTitle: 'Inspector',
    detailSections: [
      ConversationDetailSection(
        title: 'Selection',
        rows: [
          ConversationDetailRow(label: 'Project', value: workbench.selection.projectName),
          ConversationDetailRow(label: 'Thread', value: workbench.selection.threadName),
        ],
      ),
      ConversationDetailSection(
        title: 'Bridge',
        rows: [
          ConversationDetailRow(label: 'Status', value: workbench.statusHeadline),
          ConversationDetailRow(label: 'Detail', value: workbench.statusDetail),
        ],
      ),
    ],
    emptyTitle: 'No thread selected',
    emptyText: 'Select or create a thread to begin.',
    projectLabel: 'Projects',
    sessionLabel: 'Threads',
    composerPlaceholder: 'Message selected thread...',
    composerDisabledHint: 'Select a thread to enable the composer.',
  );
}

String _shortLabel(String value) {
  final words = value.split(RegExp(r'\\s+')).where((word) => word.isNotEmpty).toList();
  if (words.isEmpty) return 'RB';
  if (words.length == 1) {
    final word = words.single;
    return word.substring(0, word.length < 2 ? word.length : 2).toUpperCase();
  }
  return '${words[0][0]}${words[1][0]}'.toUpperCase();
}
