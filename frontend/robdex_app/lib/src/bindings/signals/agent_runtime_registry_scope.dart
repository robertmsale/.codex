// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeRegistryScope {
  const AgentRuntimeRegistryScope({
    required this.scopeType,
    required this.projectKey,
  });

  static AgentRuntimeRegistryScope deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeRegistryScope(
      scopeType: deserializer.deserializeString(),
      projectKey: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeRegistryScope bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeRegistryScope.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String scopeType;
  final String projectKey;

  AgentRuntimeRegistryScope copyWith({
    String? scopeType,
    String? projectKey,
  }) {
    return AgentRuntimeRegistryScope(
      scopeType: scopeType ?? this.scopeType,
      projectKey: projectKey ?? this.projectKey,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(scopeType);
    serializer.serializeString(projectKey);
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

    return other is AgentRuntimeRegistryScope
      && scopeType == other.scopeType
      && projectKey == other.projectKey;
  }

  @override
  int get hashCode => Object.hash(
        scopeType,
        projectKey,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'scopeType: $scopeType, '
        'projectKey: $projectKey'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeRegistryScope';
  }
}
