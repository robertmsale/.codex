// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeRoleEditorDraftView {
  const AgentRuntimeRoleEditorDraftView({
    required this.roleId,
    required this.version,
    required this.displayName,
    required this.model,
    required this.reasoningEffort,
    required this.instructionText,
    required this.capabilities,
    required this.policyRows,
    required this.routingMode,
    required this.defaultRecipient,
    required this.allowedRecipients,
    required this.listed,
    required this.ownerVisible,
    required this.canSpawnAgents,
    required this.canArchiveAgents,
    required this.canValidate,
    required this.canCreate,
    required this.canUpdate,
  });

  static AgentRuntimeRoleEditorDraftView deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeRoleEditorDraftView(
      roleId: deserializer.deserializeString(),
      version: deserializer.deserializeString(),
      displayName: deserializer.deserializeString(),
      model: deserializer.deserializeString(),
      reasoningEffort: deserializer.deserializeString(),
      instructionText: deserializer.deserializeString(),
      capabilities: TraitHelpers.deserializeVectorStr(deserializer),
      policyRows: TraitHelpers.deserializeVectorAgentRuntimeRolePolicyRow(deserializer),
      routingMode: deserializer.deserializeString(),
      defaultRecipient: deserializer.deserializeString(),
      allowedRecipients: TraitHelpers.deserializeVectorStr(deserializer),
      listed: deserializer.deserializeBool(),
      ownerVisible: deserializer.deserializeBool(),
      canSpawnAgents: deserializer.deserializeBool(),
      canArchiveAgents: deserializer.deserializeBool(),
      canValidate: deserializer.deserializeBool(),
      canCreate: deserializer.deserializeBool(),
      canUpdate: deserializer.deserializeBool(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeRoleEditorDraftView bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeRoleEditorDraftView.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String roleId;
  final String version;
  final String displayName;
  final String model;
  final String reasoningEffort;
  final String instructionText;
  final List<String> capabilities;
  final List<AgentRuntimeRolePolicyRow> policyRows;
  final String routingMode;
  final String defaultRecipient;
  final List<String> allowedRecipients;
  final bool listed;
  final bool ownerVisible;
  final bool canSpawnAgents;
  final bool canArchiveAgents;
  final bool canValidate;
  final bool canCreate;
  final bool canUpdate;

  AgentRuntimeRoleEditorDraftView copyWith({
    String? roleId,
    String? version,
    String? displayName,
    String? model,
    String? reasoningEffort,
    String? instructionText,
    List<String>? capabilities,
    List<AgentRuntimeRolePolicyRow>? policyRows,
    String? routingMode,
    String? defaultRecipient,
    List<String>? allowedRecipients,
    bool? listed,
    bool? ownerVisible,
    bool? canSpawnAgents,
    bool? canArchiveAgents,
    bool? canValidate,
    bool? canCreate,
    bool? canUpdate,
  }) {
    return AgentRuntimeRoleEditorDraftView(
      roleId: roleId ?? this.roleId,
      version: version ?? this.version,
      displayName: displayName ?? this.displayName,
      model: model ?? this.model,
      reasoningEffort: reasoningEffort ?? this.reasoningEffort,
      instructionText: instructionText ?? this.instructionText,
      capabilities: capabilities ?? this.capabilities,
      policyRows: policyRows ?? this.policyRows,
      routingMode: routingMode ?? this.routingMode,
      defaultRecipient: defaultRecipient ?? this.defaultRecipient,
      allowedRecipients: allowedRecipients ?? this.allowedRecipients,
      listed: listed ?? this.listed,
      ownerVisible: ownerVisible ?? this.ownerVisible,
      canSpawnAgents: canSpawnAgents ?? this.canSpawnAgents,
      canArchiveAgents: canArchiveAgents ?? this.canArchiveAgents,
      canValidate: canValidate ?? this.canValidate,
      canCreate: canCreate ?? this.canCreate,
      canUpdate: canUpdate ?? this.canUpdate,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(roleId);
    serializer.serializeString(version);
    serializer.serializeString(displayName);
    serializer.serializeString(model);
    serializer.serializeString(reasoningEffort);
    serializer.serializeString(instructionText);
    TraitHelpers.serializeVectorStr(capabilities, serializer);
    TraitHelpers.serializeVectorAgentRuntimeRolePolicyRow(policyRows, serializer);
    serializer.serializeString(routingMode);
    serializer.serializeString(defaultRecipient);
    TraitHelpers.serializeVectorStr(allowedRecipients, serializer);
    serializer.serializeBool(listed);
    serializer.serializeBool(ownerVisible);
    serializer.serializeBool(canSpawnAgents);
    serializer.serializeBool(canArchiveAgents);
    serializer.serializeBool(canValidate);
    serializer.serializeBool(canCreate);
    serializer.serializeBool(canUpdate);
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

    return other is AgentRuntimeRoleEditorDraftView
      && roleId == other.roleId
      && version == other.version
      && displayName == other.displayName
      && model == other.model
      && reasoningEffort == other.reasoningEffort
      && instructionText == other.instructionText
      && listEquals(capabilities, other.capabilities)
      && listEquals(policyRows, other.policyRows)
      && routingMode == other.routingMode
      && defaultRecipient == other.defaultRecipient
      && listEquals(allowedRecipients, other.allowedRecipients)
      && listed == other.listed
      && ownerVisible == other.ownerVisible
      && canSpawnAgents == other.canSpawnAgents
      && canArchiveAgents == other.canArchiveAgents
      && canValidate == other.canValidate
      && canCreate == other.canCreate
      && canUpdate == other.canUpdate;
  }

  @override
  int get hashCode => Object.hash(
        roleId,
        version,
        displayName,
        model,
        reasoningEffort,
        instructionText,
        capabilities,
        policyRows,
        routingMode,
        defaultRecipient,
        allowedRecipients,
        listed,
        ownerVisible,
        canSpawnAgents,
        canArchiveAgents,
        canValidate,
        canCreate,
        canUpdate,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'roleId: $roleId, '
        'version: $version, '
        'displayName: $displayName, '
        'model: $model, '
        'reasoningEffort: $reasoningEffort, '
        'instructionText: $instructionText, '
        'capabilities: $capabilities, '
        'policyRows: $policyRows, '
        'routingMode: $routingMode, '
        'defaultRecipient: $defaultRecipient, '
        'allowedRecipients: $allowedRecipients, '
        'listed: $listed, '
        'ownerVisible: $ownerVisible, '
        'canSpawnAgents: $canSpawnAgents, '
        'canArchiveAgents: $canArchiveAgents, '
        'canValidate: $canValidate, '
        'canCreate: $canCreate, '
        'canUpdate: $canUpdate'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeRoleEditorDraftView';
  }
}
