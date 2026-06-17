// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeRoleEditorDraft {
  const AgentRuntimeRoleEditorDraft({
    required this.id,
    required this.version,
    required this.displayName,
    required this.modelDefaults,
    required this.instructionText,
    required this.capabilities,
    required this.policyEntries,
    required this.routing,
    required this.visibility,
    required this.lifecycleAuthority,
  });

  static AgentRuntimeRoleEditorDraft deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeRoleEditorDraft(
      id: deserializer.deserializeString(),
      version: deserializer.deserializeString(),
      displayName: deserializer.deserializeString(),
      modelDefaults: AgentRuntimeRoleEditorModelDefaults.deserialize(deserializer),
      instructionText: deserializer.deserializeString(),
      capabilities: TraitHelpers.deserializeVectorStr(deserializer),
      policyEntries: TraitHelpers.deserializeVectorAgentRuntimeRolePolicyEntry(deserializer),
      routing: AgentRuntimeRoleEditorRoutingMetadata.deserialize(deserializer),
      visibility: AgentRuntimeRoleEditorVisibilityMetadata.deserialize(deserializer),
      lifecycleAuthority: AgentRuntimeRoleEditorLifecycleAuthorityMetadata.deserialize(deserializer),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeRoleEditorDraft bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeRoleEditorDraft.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String id;
  final String version;
  final String displayName;
  final AgentRuntimeRoleEditorModelDefaults modelDefaults;
  final String instructionText;
  final List<String> capabilities;
  final List<AgentRuntimeRolePolicyEntry> policyEntries;
  final AgentRuntimeRoleEditorRoutingMetadata routing;
  final AgentRuntimeRoleEditorVisibilityMetadata visibility;
  final AgentRuntimeRoleEditorLifecycleAuthorityMetadata lifecycleAuthority;

  AgentRuntimeRoleEditorDraft copyWith({
    String? id,
    String? version,
    String? displayName,
    AgentRuntimeRoleEditorModelDefaults? modelDefaults,
    String? instructionText,
    List<String>? capabilities,
    List<AgentRuntimeRolePolicyEntry>? policyEntries,
    AgentRuntimeRoleEditorRoutingMetadata? routing,
    AgentRuntimeRoleEditorVisibilityMetadata? visibility,
    AgentRuntimeRoleEditorLifecycleAuthorityMetadata? lifecycleAuthority,
  }) {
    return AgentRuntimeRoleEditorDraft(
      id: id ?? this.id,
      version: version ?? this.version,
      displayName: displayName ?? this.displayName,
      modelDefaults: modelDefaults ?? this.modelDefaults,
      instructionText: instructionText ?? this.instructionText,
      capabilities: capabilities ?? this.capabilities,
      policyEntries: policyEntries ?? this.policyEntries,
      routing: routing ?? this.routing,
      visibility: visibility ?? this.visibility,
      lifecycleAuthority: lifecycleAuthority ?? this.lifecycleAuthority,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(id);
    serializer.serializeString(version);
    serializer.serializeString(displayName);
    modelDefaults.serialize(serializer);
    serializer.serializeString(instructionText);
    TraitHelpers.serializeVectorStr(capabilities, serializer);
    TraitHelpers.serializeVectorAgentRuntimeRolePolicyEntry(policyEntries, serializer);
    routing.serialize(serializer);
    visibility.serialize(serializer);
    lifecycleAuthority.serialize(serializer);
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

    return other is AgentRuntimeRoleEditorDraft
      && id == other.id
      && version == other.version
      && displayName == other.displayName
      && modelDefaults == other.modelDefaults
      && instructionText == other.instructionText
      && listEquals(capabilities, other.capabilities)
      && listEquals(policyEntries, other.policyEntries)
      && routing == other.routing
      && visibility == other.visibility
      && lifecycleAuthority == other.lifecycleAuthority;
  }

  @override
  int get hashCode => Object.hash(
        id,
        version,
        displayName,
        modelDefaults,
        instructionText,
        capabilities,
        policyEntries,
        routing,
        visibility,
        lifecycleAuthority,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'id: $id, '
        'version: $version, '
        'displayName: $displayName, '
        'modelDefaults: $modelDefaults, '
        'instructionText: $instructionText, '
        'capabilities: $capabilities, '
        'policyEntries: $policyEntries, '
        'routing: $routing, '
        'visibility: $visibility, '
        'lifecycleAuthority: $lifecycleAuthority'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeRoleEditorDraft';
  }
}
