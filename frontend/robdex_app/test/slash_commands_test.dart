import 'package:flutter_test/flutter_test.dart';
import 'package:robdex_design_system/robdex_design_system.dart';
import 'package:robdex_design_system/src/features/composer/slash_commands.dart';

void main() {
  const selection = WorkspaceSelection(
    projectId: 'project',
    projectRootPath: '/tmp/project',
    projectOrchestratorThreadId: 'orch',
    projectOrchestratorName: 'Orchestrator',
    threadId: 'thread',
    threadRole: 'worker',
    projectName: 'Project',
    threadName: 'Worker',
    connectionLabel: 'Bridge',
    approvalPolicy: 'never',
    sandboxMode: 'workspace-write',
    networkAccess: true,
    model: 'gpt-5.4',
    serviceTier: 'auto',
    reasoningEffort: 'medium',
  );

  const models = [
    ModelItem(id: 'gpt-5.4', name: 'GPT 5.4', hidden: false),
    ModelItem(id: 'gpt-5.4-mini', name: 'GPT 5.4 Mini', hidden: false),
  ];

  test('parses only exact complete slash commands', () {
    expect(
      parseCompleteSlashCommand(
        '/reasoning high',
        selection: selection,
        availableModels: models,
      )?.kind,
      SlashCommandKind.reasoning,
    );
    expect(
      parseCompleteSlashCommand(
        '/reasoning high',
        selection: selection,
        availableModels: models,
      )?.argument,
      'high',
    );
    expect(
      parseCompleteSlashCommand(
        'Please switch to /reasoning high',
        selection: selection,
        availableModels: models,
      ),
      isNull,
    );
    expect(
      parseCompleteSlashCommand(
        '/reasoning high please',
        selection: selection,
        availableModels: models,
      ),
      isNull,
    );
    expect(
      parseCompleteSlashCommand(
        ' /reasoning high',
        selection: selection,
        availableModels: models,
      ),
      isNull,
    );
  });

  test('suggests top-level and argument options with current markers', () {
    final topLevel = slashCommandSuggestions(
      '/',
      selection: selection,
      availableModels: models,
    );
    expect(topLevel?.options.map((option) => option.value), contains('reasoning'));
    expect(topLevel?.options.map((option) => option.value), contains('compact'));

    final reasoning = slashCommandSuggestions(
      '/reasoning ',
      selection: selection,
      availableModels: models,
    );
    expect(reasoning?.command?.kind, SlashCommandKind.reasoning);
    expect(reasoning?.options.map((option) => option.value), ['low', 'medium', 'high']);
    expect(reasoning?.options.singleWhere((option) => option.value == 'medium').current, true);
  });
}
