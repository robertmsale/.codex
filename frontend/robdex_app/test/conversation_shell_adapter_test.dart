import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:robdex_app/src/bindings/bindings.dart';
import 'package:robdex_app/src/core/state/workbench_controller.dart';
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
          status: 'stopped',
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

  test('Agent Runtime generic shell excludes raw runtime event rows from ChatTimeline entries', () {
  final data = mockAgentRuntimeConnected.copyWith(
    timeline: const [
      AgentRuntimeTimelineItem(id: 'event-1', title: 'role.imported', subtitle: 'raw role event', status: '#1', tone: 'info'),
      AgentRuntimeTimelineItem(id: 'event-2', title: 'turn.started', subtitle: 'raw turn event', status: '#2', tone: 'warning'),
      AgentRuntimeTimelineItem(id: 'chat-1', title: 'Owner', subtitle: 'hello runtime', status: 'sent', tone: 'user'),
      AgentRuntimeTimelineItem(id: 'chat-2', title: 'Assistant', subtitle: 'hello owner', status: 'completed', tone: 'success'),
    ],
    selectedConversation: const [
      ChatEntry(id: 'chat-user', author: 'User', displayLabel: 'User', timestamp: null, body: 'hello runtime'),
      ChatEntry(id: 'chat-assistant', author: 'Assistant', displayLabel: 'Assistant', timestamp: null, body: 'hello user'),
    ],
  );

  final shell = agentRuntimeConversationShellData(data);

  expect(shell.entries.map((entry) => entry.displayLabel), containsAll(<String>['User', 'Assistant']));
  expect(shell.entries.map((entry) => entry.displayLabel), isNot(contains('role.imported')));
  expect(shell.entries.map((entry) => entry.displayLabel), isNot(contains('turn.started')));
  expect(shell.entries.map((entry) => entry.body), isNot(contains('raw role event')));
});
  test('Workbench selected chat delta applies without full snapshot and caps at 50', () {
    final controller = WorkbenchController();
    final entries = List<ChatEntry>.generate(
      50,
      (index) => ChatEntry(
        id: 'message-$index',
        author: index.isEven ? 'User' : 'Assistant',
        displayLabel: index.isEven ? 'User' : 'Assistant',
        timestamp: null,
        body: 'message $index',
      ),
    );
    final base = mockWorkbenchData.copyWith(
      chatEntries: entries,
    );
    controller.applySelectedChatDeltaForTest(
      base,
      WorkbenchSelectedChatDeltaSignal(
        threadId: 'config-operator',
        messageId: 'assistant-streaming',
        appendedText: '',
        replacementText: 'hello',
        deliveryState: 'streaming',
        isFinal: false,
        sequence: Uint64(BigInt.one),
        metadataJson: '{}',
        selectedEntryCount: 50,
        coalescedStreamUpdateCount: 1,
        droppedIntermediateStreamUpdateCount: 0,
      ),
    );
    expect(controller.view!.chatEntries.length, 50);
    expect(controller.view!.chatEntries.last.body, 'hello');
    expect(controller.view!.chatEntries.last.isStreaming, true);

    controller.applySelectedChatDeltaForTest(
      controller.view!,
      WorkbenchSelectedChatDeltaSignal(
        threadId: 'config-operator',
        messageId: 'assistant-streaming',
        appendedText: ' world',
        replacementText: '',
        deliveryState: 'complete',
        isFinal: true,
        sequence: Uint64(BigInt.two),
        metadataJson: '{}',
        selectedEntryCount: 50,
        coalescedStreamUpdateCount: 2,
        droppedIntermediateStreamUpdateCount: 10,
      ),
    );
    expect(controller.view!.chatEntries.length, 50);
    expect(controller.view!.chatEntries.last.body, 'hello world');
    expect(controller.view!.chatEntries.last.isStreaming, false);
    expect(controller.selectedChatDeltaApplyCount, 2);
  });

  testWidgets('Agent Runtime operations detail renders required typed surfaces', (tester) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(
          body: SizedBox(
            height: 3000,
            child: AgentRuntimeOperationsDetail(data: mockAgentRuntimeConnected),
          ),
        ),
      ),
    );

    Future<void> expectVisibleText(String title) async {
      for (var attempt = 0; attempt < 12; attempt += 1) {
        if (find.text(title).evaluate().isNotEmpty) {
          expect(find.text(title), findsWidgets);
          return;
        }
        await tester.drag(find.byType(ListView), const Offset(0, -260));
        await tester.pump();
      }
      expect(find.text(title), findsWidgets);
    }

    for (final title in <String>[
      'Session',
      'Compaction',
      'Process Manager',
      'Approvals',
      'Command Registry',
      'Role Admin',
      'Workflow Memory',
    ]) {
      await expectVisibleText(title);
    }
    expect(mockAgentRuntimeConnected.operationSurfaces.map((surface) => surface.title), isNot(contains('History')));
    expect(mockAgentRuntimeConnected.operationSurfaces.map((surface) => surface.title), isNot(contains('Image artifacts')));
    expect(mockAgentRuntimeConnected.operationSurfaces.map((surface) => surface.title), isNot(contains('Statistics')));
  });

}
