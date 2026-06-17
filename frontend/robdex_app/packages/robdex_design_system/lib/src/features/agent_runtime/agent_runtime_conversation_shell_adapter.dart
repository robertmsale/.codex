import '../../core/models/agent_runtime_control_tower_models.dart';
import '../../core/models/conversation_shell_models.dart';
import '../../core/models/workbench_models.dart';

ConversationShellData agentRuntimeConversationShellData(AgentRuntimeControlTowerData data) {
  final selected = _selectedSessionId(data);
  final entries = data.timeline
      .map((item) => ChatEntry(
            id: item.id,
            author: item.tone == 'success' ? 'assistant' : 'system',
            displayLabel: _displayCopy(item.title),
            timestamp: null,
            body: _displayCopy(item.subtitle.isEmpty ? item.status : item.subtitle),
            subtitle: _displayCopy(item.status),
            status: _displayCopy(item.status),
            isTool: item.tone == 'warning',
          ))
      .toList(growable: false);
  return ConversationShellData(
    appTitle: 'Agent Runtime',
    connectionLabel: data.statusLabel,
    projects: const [ConversationProject(id: 'runtime', title: 'Runtime', subtitle: 'Agent sessions')],
    sessions: data.sessions
        .map((session) => ConversationSession(
              id: session.id,
              title: _displayCopy(session.title),
              subtitle: _displayCopy(session.subtitle),
              role: _displayCopy(session.groupLabel),
              selected: session.id == selected,
              rolePresentation: ConversationRolePresentation(
                roleId: _displayCopy(session.groupLabel),
                displayLabel: _displayCopy(session.groupLabel),
                shortLabel: _shortLabel(_displayCopy(session.groupLabel)),
                iconKey: 'runtime',
                tone: session.tone,
                statusLabel: _displayCopy(session.status),
                description: _displayCopy(session.subtitle),
              ),
            ))
        .toList(growable: false),
    selectedSessionId: selected,
    timelineTitle: data.selectedSessionLabel == 'none selected' ? 'Select a session' : data.selectedSessionLabel,
    entries: entries,
    composerEnabled: _hasConnectedRuntime(data) && selected != null,
    isRunning: data.timeline.any((entry) => entry.status.toLowerCase().contains('running')),
    detailTitle: 'Details',
    detailSections: [
      ConversationDetailSection(
        title: 'Operations',
        rows: [
          for (final action in data.actions.take(5)) ConversationDetailRow(label: _displayCopy(action.title), value: _displayCopy(action.stateText)),
          if (data.actions.isEmpty) const ConversationDetailRow(label: 'Queue', value: 'No action needed'),
        ],
      ),
      ConversationDetailSection(
        title: 'Runtime',
        rows: [
          for (final fact in data.controllerFacts.take(6)) ConversationDetailRow(label: _displayCopy(fact.label), value: _displayCopy(fact.value)),
          ConversationDetailRow(label: 'Status', value: data.statusLabel),
        ],
      ),
      ConversationDetailSection(
        title: 'Settings',
        rows: const [
          ConversationDetailRow(label: 'Runtime settings', value: 'Loaded from the runtime'),
          ConversationDetailRow(label: 'Project selection', value: 'Runtime project scope'),
        ],
      ),
      ConversationDetailSection(
        title: 'Role Admin',
        rows: [
          ConversationDetailRow(label: 'Roles', value: '${data.roleAdmin.rows.length} available'),
          if (data.roleAdmin.selectedDetail != null) ConversationDetailRow(label: 'Selected', value: data.roleAdmin.selectedDetail!.displayName),
        ],
      ),
      ConversationDetailSection(
        title: 'Workflow Memory',
        rows: [
          ConversationDetailRow(label: 'Memories', value: '${data.workflowMemory.rows.length} visible'),
          if (data.workflowMemory.selectedDetail != null) ConversationDetailRow(label: 'Selected', value: data.workflowMemory.selectedDetail!.title),
        ],
      ),
    ],
    emptyTitle: data.sessionsEmptyTitle,
    emptyText: data.sessionsEmptyText,
  );
}

bool _hasConnectedRuntime(AgentRuntimeControlTowerData data) {
  return data.connectionState != 'disconnected' && data.connectionState != 'connecting' && data.connectionState != 'failed';
}

String _displayCopy(String value) {
  return value
      .replaceAll(RegExp(r'/Users/[^ ]+'), 'Project workspace')
      .replaceAll('tool.call execute_code', 'Execute code')
      .replaceAll('tool.call', 'Tool work')
      .replaceAll('approval.requested', 'Approval requested')
      .replaceAll('cmd.rg.audit', 'Command review')
      .replaceAll('rg · audit', 'Search audit')
      .replaceAll('runtime-allow', 'Runtime allow')
      .replaceAll('Starlark completed', 'Code run completed')
      .replaceAll('projectionSnapshot', 'Runtime update')
      .replaceAll('controllerState', 'Connection')
      .trim();
}

String? _selectedSessionId(AgentRuntimeControlTowerData data) {
  if (data.sessions.isEmpty) {
    return null;
  }
  for (final session in data.sessions) {
    if (session.title == data.selectedSessionLabel) {
      return session.id;
    }
  }
  return data.sessions.first.id;
}

String _shortLabel(String value) {
  final words = value.split(RegExp(r'\s+')).where((word) => word.isNotEmpty).toList();
  if (words.isEmpty) return 'AR';
  if (words.length == 1) {
    final word = words.single;
    return word.substring(0, word.length < 2 ? word.length : 2).toUpperCase();
  }
  return '${words[0][0]}${words[1][0]}'.toUpperCase();
}
