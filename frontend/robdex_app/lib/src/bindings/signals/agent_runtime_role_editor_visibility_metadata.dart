// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeRoleEditorVisibilityMetadata {
  const AgentRuntimeRoleEditorVisibilityMetadata({
    required this.listed,
    required this.ownerVisible,
  });

  static AgentRuntimeRoleEditorVisibilityMetadata deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeRoleEditorVisibilityMetadata(
      listed: deserializer.deserializeBool(),
      ownerVisible: deserializer.deserializeBool(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeRoleEditorVisibilityMetadata bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeRoleEditorVisibilityMetadata.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final bool listed;
  final bool ownerVisible;

  AgentRuntimeRoleEditorVisibilityMetadata copyWith({
    bool? listed,
    bool? ownerVisible,
  }) {
    return AgentRuntimeRoleEditorVisibilityMetadata(
      listed: listed ?? this.listed,
      ownerVisible: ownerVisible ?? this.ownerVisible,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeBool(listed);
    serializer.serializeBool(ownerVisible);
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

    return other is AgentRuntimeRoleEditorVisibilityMetadata
      && listed == other.listed
      && ownerVisible == other.ownerVisible;
  }

  @override
  int get hashCode => Object.hash(
        listed,
        ownerVisible,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'listed: $listed, '
        'ownerVisible: $ownerVisible'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeRoleEditorVisibilityMetadata';
  }
}
