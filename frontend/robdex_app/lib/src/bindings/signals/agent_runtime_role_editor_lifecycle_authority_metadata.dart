// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeRoleEditorLifecycleAuthorityMetadata {
  const AgentRuntimeRoleEditorLifecycleAuthorityMetadata({
    required this.canSpawnAgents,
    required this.canArchiveAgents,
    required this.reservedActions,
  });

  static AgentRuntimeRoleEditorLifecycleAuthorityMetadata deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeRoleEditorLifecycleAuthorityMetadata(
      canSpawnAgents: deserializer.deserializeBool(),
      canArchiveAgents: deserializer.deserializeBool(),
      reservedActions: TraitHelpers.deserializeVectorStr(deserializer),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeRoleEditorLifecycleAuthorityMetadata bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeRoleEditorLifecycleAuthorityMetadata.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final bool canSpawnAgents;
  final bool canArchiveAgents;
  final List<String> reservedActions;

  AgentRuntimeRoleEditorLifecycleAuthorityMetadata copyWith({
    bool? canSpawnAgents,
    bool? canArchiveAgents,
    List<String>? reservedActions,
  }) {
    return AgentRuntimeRoleEditorLifecycleAuthorityMetadata(
      canSpawnAgents: canSpawnAgents ?? this.canSpawnAgents,
      canArchiveAgents: canArchiveAgents ?? this.canArchiveAgents,
      reservedActions: reservedActions ?? this.reservedActions,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeBool(canSpawnAgents);
    serializer.serializeBool(canArchiveAgents);
    TraitHelpers.serializeVectorStr(reservedActions, serializer);
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

    return other is AgentRuntimeRoleEditorLifecycleAuthorityMetadata
      && canSpawnAgents == other.canSpawnAgents
      && canArchiveAgents == other.canArchiveAgents
      && listEquals(reservedActions, other.reservedActions);
  }

  @override
  int get hashCode => Object.hash(
        canSpawnAgents,
        canArchiveAgents,
        reservedActions,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'canSpawnAgents: $canSpawnAgents, '
        'canArchiveAgents: $canArchiveAgents, '
        'reservedActions: $reservedActions'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeRoleEditorLifecycleAuthorityMetadata';
  }
}
