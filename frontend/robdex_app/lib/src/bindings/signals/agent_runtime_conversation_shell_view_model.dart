// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeConversationShellViewModel {
  const AgentRuntimeConversationShellViewModel({
    required this.projects,
    required this.sessions,
    required this.selectedSessionId,
    required this.hasSelectedSessionId,
    required this.selectedConversation,
    required this.dynamicRoles,
    required this.actions,
    required this.settings,
    required this.roleManagement,
    required this.workflowMemory,
    required this.commandRegistryRequests,
    required this.approvals,
    required this.diagnostics,
    required this.operationSurfaces,
  });

  static AgentRuntimeConversationShellViewModel deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeConversationShellViewModel(
      projects: TraitHelpers.deserializeVectorAgentRuntimeShellProjectRow(deserializer),
      sessions: TraitHelpers.deserializeVectorAgentRuntimeSessionRow(deserializer),
      selectedSessionId: deserializer.deserializeString(),
      hasSelectedSessionId: deserializer.deserializeBool(),
      selectedConversation: TraitHelpers.deserializeVectorAgentRuntimeTimelineRow(deserializer),
      dynamicRoles: TraitHelpers.deserializeVectorAgentRuntimeShellRolePresentation(deserializer),
      actions: TraitHelpers.deserializeVectorAgentRuntimeActionRow(deserializer),
      settings: TraitHelpers.deserializeVectorAgentRuntimeFact(deserializer),
      roleManagement: AgentRuntimeRoleAdminView.deserialize(deserializer),
      workflowMemory: AgentRuntimeWorkflowMemoryView.deserialize(deserializer),
      commandRegistryRequests: TraitHelpers.deserializeVectorAgentRuntimeActionRow(deserializer),
      approvals: TraitHelpers.deserializeVectorAgentRuntimeActionRow(deserializer),
      diagnostics: TraitHelpers.deserializeVectorAgentRuntimeFact(deserializer),
      operationSurfaces: TraitHelpers.deserializeVectorAgentRuntimeOperationSurface(deserializer),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeConversationShellViewModel bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeConversationShellViewModel.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final List<AgentRuntimeShellProjectRow> projects;
  final List<AgentRuntimeSessionRow> sessions;
  final String selectedSessionId;
  final bool hasSelectedSessionId;
  final List<AgentRuntimeTimelineRow> selectedConversation;
  final List<AgentRuntimeShellRolePresentation> dynamicRoles;
  final List<AgentRuntimeActionRow> actions;
  final List<AgentRuntimeFact> settings;
  final AgentRuntimeRoleAdminView roleManagement;
  final AgentRuntimeWorkflowMemoryView workflowMemory;
  final List<AgentRuntimeActionRow> commandRegistryRequests;
  final List<AgentRuntimeActionRow> approvals;
  final List<AgentRuntimeFact> diagnostics;
  final List<AgentRuntimeOperationSurface> operationSurfaces;

  AgentRuntimeConversationShellViewModel copyWith({
    List<AgentRuntimeShellProjectRow>? projects,
    List<AgentRuntimeSessionRow>? sessions,
    String? selectedSessionId,
    bool? hasSelectedSessionId,
    List<AgentRuntimeTimelineRow>? selectedConversation,
    List<AgentRuntimeShellRolePresentation>? dynamicRoles,
    List<AgentRuntimeActionRow>? actions,
    List<AgentRuntimeFact>? settings,
    AgentRuntimeRoleAdminView? roleManagement,
    AgentRuntimeWorkflowMemoryView? workflowMemory,
    List<AgentRuntimeActionRow>? commandRegistryRequests,
    List<AgentRuntimeActionRow>? approvals,
    List<AgentRuntimeFact>? diagnostics,
    List<AgentRuntimeOperationSurface>? operationSurfaces,
  }) {
    return AgentRuntimeConversationShellViewModel(
      projects: projects ?? this.projects,
      sessions: sessions ?? this.sessions,
      selectedSessionId: selectedSessionId ?? this.selectedSessionId,
      hasSelectedSessionId: hasSelectedSessionId ?? this.hasSelectedSessionId,
      selectedConversation: selectedConversation ?? this.selectedConversation,
      dynamicRoles: dynamicRoles ?? this.dynamicRoles,
      actions: actions ?? this.actions,
      settings: settings ?? this.settings,
      roleManagement: roleManagement ?? this.roleManagement,
      workflowMemory: workflowMemory ?? this.workflowMemory,
      commandRegistryRequests: commandRegistryRequests ?? this.commandRegistryRequests,
      approvals: approvals ?? this.approvals,
      diagnostics: diagnostics ?? this.diagnostics,
      operationSurfaces: operationSurfaces ?? this.operationSurfaces,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    TraitHelpers.serializeVectorAgentRuntimeShellProjectRow(projects, serializer);
    TraitHelpers.serializeVectorAgentRuntimeSessionRow(sessions, serializer);
    serializer.serializeString(selectedSessionId);
    serializer.serializeBool(hasSelectedSessionId);
    TraitHelpers.serializeVectorAgentRuntimeTimelineRow(selectedConversation, serializer);
    TraitHelpers.serializeVectorAgentRuntimeShellRolePresentation(dynamicRoles, serializer);
    TraitHelpers.serializeVectorAgentRuntimeActionRow(actions, serializer);
    TraitHelpers.serializeVectorAgentRuntimeFact(settings, serializer);
    roleManagement.serialize(serializer);
    workflowMemory.serialize(serializer);
    TraitHelpers.serializeVectorAgentRuntimeActionRow(commandRegistryRequests, serializer);
    TraitHelpers.serializeVectorAgentRuntimeActionRow(approvals, serializer);
    TraitHelpers.serializeVectorAgentRuntimeFact(diagnostics, serializer);
    TraitHelpers.serializeVectorAgentRuntimeOperationSurface(operationSurfaces, serializer);
    serializer.decreaseContainerDepth();
  }

  Uint8List bincodeSerialize() {
      final serializer = BincodeSerializer();
      serialize(serializer);
      return serializer.bytes;
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeConversationShellViewModel
      && listEquals(projects, other.projects)
      && listEquals(sessions, other.sessions)
      && selectedSessionId == other.selectedSessionId
      && hasSelectedSessionId == other.hasSelectedSessionId
      && listEquals(selectedConversation, other.selectedConversation)
      && listEquals(dynamicRoles, other.dynamicRoles)
      && listEquals(actions, other.actions)
      && listEquals(settings, other.settings)
      && roleManagement == other.roleManagement
      && workflowMemory == other.workflowMemory
      && listEquals(commandRegistryRequests, other.commandRegistryRequests)
      && listEquals(approvals, other.approvals)
      && listEquals(diagnostics, other.diagnostics)
      && listEquals(operationSurfaces, other.operationSurfaces);
  }

  @override
  int get hashCode => Object.hash(
        projects,
        sessions,
        selectedSessionId,
        hasSelectedSessionId,
        selectedConversation,
        dynamicRoles,
        actions,
        settings,
        roleManagement,
        workflowMemory,
        commandRegistryRequests,
        approvals,
        diagnostics,
        operationSurfaces,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'projects: $projects, '
        'sessions: $sessions, '
        'selectedSessionId: $selectedSessionId, '
        'hasSelectedSessionId: $hasSelectedSessionId, '
        'selectedConversation: $selectedConversation, '
        'dynamicRoles: $dynamicRoles, '
        'actions: $actions, '
        'settings: $settings, '
        'roleManagement: $roleManagement, '
        'workflowMemory: $workflowMemory, '
        'commandRegistryRequests: $commandRegistryRequests, '
        'approvals: $approvals, '
        'diagnostics: $diagnostics, '
        'operationSurfaces: $operationSurfaces'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeConversationShellViewModel';
  }
}
