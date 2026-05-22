import '../../core/models/workbench_models.dart';

enum SlashCommandKind {
  model,
  reasoning,
  role,
  sandbox,
  approval,
  compact,
  handoff,
}

class SlashCommandDefinition {
  const SlashCommandDefinition({
    required this.kind,
    required this.name,
    required this.description,
    required this.requiresArgument,
  });

  final SlashCommandKind kind;
  final String name;
  final String description;
  final bool requiresArgument;
}

class SlashCommandOption {
  const SlashCommandOption({
    required this.value,
    required this.label,
    this.current = false,
  });

  final String value;
  final String label;
  final bool current;
}

class SlashCommandSuggestionState {
  const SlashCommandSuggestionState({
    required this.command,
    required this.options,
    required this.commandQuery,
    required this.argumentQuery,
    required this.commandComplete,
  });

  final SlashCommandDefinition? command;
  final List<SlashCommandOption> options;
  final String commandQuery;
  final String argumentQuery;
  final bool commandComplete;
}

class ParsedSlashCommand {
  const ParsedSlashCommand({
    required this.kind,
    this.argument,
  });

  final SlashCommandKind kind;
  final String? argument;
}

const slashCommandDefinitions = <SlashCommandDefinition>[
  SlashCommandDefinition(
    kind: SlashCommandKind.model,
    name: 'model',
    description: 'Set model',
    requiresArgument: true,
  ),
  SlashCommandDefinition(
    kind: SlashCommandKind.reasoning,
    name: 'reasoning',
    description: 'Set reasoning',
    requiresArgument: true,
  ),
  SlashCommandDefinition(
    kind: SlashCommandKind.role,
    name: 'role',
    description: 'Set role',
    requiresArgument: true,
  ),
  SlashCommandDefinition(
    kind: SlashCommandKind.sandbox,
    name: 'sandbox',
    description: 'Set sandbox',
    requiresArgument: true,
  ),
  SlashCommandDefinition(
    kind: SlashCommandKind.approval,
    name: 'approval',
    description: 'Set approval',
    requiresArgument: true,
  ),
  SlashCommandDefinition(
    kind: SlashCommandKind.compact,
    name: 'compact',
    description: 'Compact thread',
    requiresArgument: false,
  ),
  SlashCommandDefinition(
    kind: SlashCommandKind.handoff,
    name: 'handoff',
    description: 'Draft handoff',
    requiresArgument: false,
  ),
];

SlashCommandSuggestionState? slashCommandSuggestions(
  String text, {
  required WorkspaceSelection selection,
  required List<ModelItem> availableModels,
}) {
  if (!_isSlashCandidate(text)) {
    return null;
  }
  final raw = text.substring(1);
  if (raw.contains('\n')) {
    return null;
  }
  final firstSpace = raw.indexOf(' ');
  final commandQuery = firstSpace < 0 ? raw : raw.substring(0, firstSpace);
  final argumentQuery = firstSpace < 0 ? '' : raw.substring(firstSpace + 1);
  if (argumentQuery.contains('  ')) {
    return null;
  }

  final matchingCommands = slashCommandDefinitions
      .where((definition) => definition.name.startsWith(commandQuery))
      .toList(growable: false);
  if (matchingCommands.isEmpty) {
    return null;
  }

  final exactCommand = slashCommandDefinitions
      .where((definition) => definition.name == commandQuery)
      .cast<SlashCommandDefinition?>()
      .firstWhere((definition) => definition != null, orElse: () => null);
  if (exactCommand == null || firstSpace < 0) {
    return SlashCommandSuggestionState(
      command: exactCommand,
      options: matchingCommands
          .map(
            (definition) => SlashCommandOption(
              value: definition.name,
              label: '/${definition.name}',
              current: false,
            ),
          )
          .toList(growable: false),
      commandQuery: commandQuery,
      argumentQuery: argumentQuery,
      commandComplete: exactCommand != null && !exactCommand.requiresArgument,
    );
  }

  if (!exactCommand.requiresArgument) {
    return argumentQuery.isEmpty
        ? SlashCommandSuggestionState(
            command: exactCommand,
            options: const [],
            commandQuery: commandQuery,
            argumentQuery: argumentQuery,
            commandComplete: true,
          )
        : null;
  }

  final options = slashCommandArgumentOptions(
    exactCommand.kind,
    selection: selection,
    availableModels: availableModels,
  )
      .where((option) => option.value.startsWith(argumentQuery))
      .toList(growable: false);
  if (options.isEmpty) {
    return null;
  }
  return SlashCommandSuggestionState(
    command: exactCommand,
    options: options,
    commandQuery: commandQuery,
    argumentQuery: argumentQuery,
    commandComplete: options.any((option) => option.value == argumentQuery),
  );
}

ParsedSlashCommand? parseCompleteSlashCommand(
  String text, {
  required WorkspaceSelection selection,
  required List<ModelItem> availableModels,
}) {
  if (!_isSlashCandidate(text) || text.trim() != text || text.contains('\n')) {
    return null;
  }
  final parts = text.substring(1).split(' ');
  if (parts.length > 2 || parts.any((part) => part.isEmpty)) {
    return null;
  }
  final command = slashCommandDefinitions
      .where((definition) => definition.name == parts.first)
      .cast<SlashCommandDefinition?>()
      .firstWhere((definition) => definition != null, orElse: () => null);
  if (command == null) {
    return null;
  }
  if (!command.requiresArgument) {
    return parts.length == 1 ? ParsedSlashCommand(kind: command.kind) : null;
  }
  if (parts.length != 2) {
    return null;
  }
  final argument = parts.last;
  final valid = slashCommandArgumentOptions(
    command.kind,
    selection: selection,
    availableModels: availableModels,
  ).any((option) => option.value == argument);
  return valid ? ParsedSlashCommand(kind: command.kind, argument: argument) : null;
}

List<SlashCommandOption> slashCommandArgumentOptions(
  SlashCommandKind kind, {
  required WorkspaceSelection selection,
  required List<ModelItem> availableModels,
}) {
  switch (kind) {
    case SlashCommandKind.model:
      return availableModels
          .where((model) => !model.hidden)
          .map(
            (model) => SlashCommandOption(
              value: model.id,
              label: (model.name?.trim().isNotEmpty ?? false) ? model.name! : model.id,
              current: model.id == (selection.model ?? selection.effectiveModel),
            ),
          )
          .toList(growable: false);
    case SlashCommandKind.reasoning:
      final current = selection.reasoningEffort ?? selection.effectiveReasoningEffort;
      return const ['low', 'medium', 'high']
          .map(
            (value) => SlashCommandOption(
              value: value,
              label: value,
              current: value == current,
            ),
          )
          .toList(growable: false);
    case SlashCommandKind.role:
      return const ['worker', 'orchestrator', 'operator', 'designer', 'qa', 'hidden']
          .map(
            (value) => SlashCommandOption(
              value: value,
              label: value,
              current: value == selection.threadRole,
            ),
          )
          .toList(growable: false);
    case SlashCommandKind.sandbox:
      final current = selection.sandboxMode ?? selection.effectiveSandboxMode;
      return const ['read-only', 'workspace-write', 'danger-full-access']
          .map(
            (value) => SlashCommandOption(
              value: value,
              label: value,
              current: value == current,
            ),
          )
          .toList(growable: false);
    case SlashCommandKind.approval:
      final current = selection.approvalPolicy ?? selection.effectiveApprovalPolicy;
      return const ['untrusted', 'on-request', 'on-failure', 'never']
          .map(
            (value) => SlashCommandOption(
              value: value,
              label: value,
              current: value == current,
            ),
          )
          .toList(growable: false);
    case SlashCommandKind.compact:
      return const [];
    case SlashCommandKind.handoff:
      return const [];
  }
}

bool _isSlashCandidate(String text) {
  return text.startsWith('/') && !text.startsWith('//');
}
