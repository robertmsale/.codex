import 'workbench_models.dart';

class ConversationShellData {
  const ConversationShellData({
    required this.appTitle,
    required this.connectionLabel,
    required this.projects,
    required this.sessions,
    required this.selectedSessionId,
    required this.timelineTitle,
    required this.entries,
    required this.composerEnabled,
    required this.isRunning,
    required this.detailTitle,
    required this.detailSections,
    this.emptyTitle = 'Select a session',
    this.emptyText = 'Choose or create a session to start.',
    this.projectLabel = 'Projects',
    this.sessionLabel = 'Sessions',
    this.composerPlaceholder = 'Message selected session...',
    this.composerDisabledHint = 'Select a session to enable the composer.',
    this.composerStatusMessage,
    this.inlineErrorMessage,
  });

  final String appTitle;
  final String connectionLabel;
  final List<ConversationProject> projects;
  final List<ConversationSession> sessions;
  final String? selectedSessionId;
  final String timelineTitle;
  final List<ChatEntry> entries;
  final bool composerEnabled;
  final bool isRunning;
  final String detailTitle;
  final List<ConversationDetailSection> detailSections;
  final String emptyTitle;
  final String emptyText;
  final String projectLabel;
  final String sessionLabel;
  final String composerPlaceholder;
  final String composerDisabledHint;
  final String? composerStatusMessage;
  final String? inlineErrorMessage;
}

class ConversationProject {
  const ConversationProject({
    required this.id,
    required this.title,
    this.subtitle = '',
    this.canEdit = false,
    this.canArchive = false,
    this.canCreateSession = true,
    this.defaultWorkdir = '',
    this.defaultWorktreeRoot = '',
    this.defaultRoleId = '',
    this.defaultModel = '',
    this.archived = false,
  });
  final String id;
  final String title;
  final String subtitle;
  final bool canEdit;
  final bool canArchive;
  final bool canCreateSession;
  final String defaultWorkdir;
  final String defaultWorktreeRoot;
  final String defaultRoleId;
  final String defaultModel;
  final bool archived;
}

class ConversationSession {
  const ConversationSession({
    required this.id,
    required this.title,
    required this.subtitle,
    required this.role,
    required this.rolePresentation,
    this.selected = false,
  });

  final String id;
  final String title;
  final String subtitle;
  final String role;
  final ConversationRolePresentation rolePresentation;
  final bool selected;
}

class ConversationRolePresentation {
  const ConversationRolePresentation({
    required this.roleId,
    required this.displayLabel,
    required this.shortLabel,
    required this.iconKey,
    required this.tone,
    required this.statusLabel,
    required this.description,
  });

  final String roleId;
  final String displayLabel;
  final String shortLabel;
  final String iconKey;
  final String tone;
  final String statusLabel;
  final String description;
}

class ConversationDetailSection {
  const ConversationDetailSection({required this.title, required this.rows});
  final String title;
  final List<ConversationDetailRow> rows;
}

class ConversationDetailRow {
  const ConversationDetailRow({required this.label, required this.value});
  final String label;
  final String value;
}
