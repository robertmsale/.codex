import 'package:flutter_test/flutter_test.dart';
import 'package:robdex_design_system/robdex_design_system.dart';

void main() {
  test('Robdex workbench data maps into generic conversation shell without changing source data', () {
    final shell = workbenchConversationShellData(mockWorkbenchData);

    expect(shell.appTitle, 'Robdex');
    expect(shell.projects.map((project) => project.id), contains(mockWorkbenchData.projects.first.id));
    expect(shell.sessions.map((session) => session.id), contains(mockWorkbenchData.threads.first.id));
    expect(shell.selectedSessionId, mockWorkbenchData.selection.threadId);
    expect(shell.entries, mockWorkbenchData.chatEntries);
    expect(shell.detailSections.expand((section) => section.rows).any((row) => row.value == mockWorkbenchData.statusHeadline), true);
  });

  test('Agent Runtime generic shell uses dynamic role presentation from view model data', () {
    final custom = mockAgentRuntimeConnected.copyWith(
      sessions: const [
        AgentRuntimeSessionItem(
          id: 'session-custom',
          title: 'Custom role session',
          status: 'open',
          subtitle: 'Project workspace',
          groupLabel: 'Neon Incident Commander',
          tone: 'warning',
        ),
      ],
    );

    final shell = agentRuntimeConversationShellData(custom);

    expect(shell.sessions.single.rolePresentation.roleId, 'Neon Incident Commander');
    expect(shell.sessions.single.rolePresentation.displayLabel, 'Neon Incident Commander');
    expect(shell.sessions.single.rolePresentation.shortLabel, 'NI');
    expect(shell.sessions.single.rolePresentation.tone, 'warning');
  });
}
