// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeRolePolicyEntry {
  const AgentRuntimeRolePolicyEntry({
    required this.key,
    required this.value,
  });

  static AgentRuntimeRolePolicyEntry deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeRolePolicyEntry(
      key: deserializer.deserializeString(),
      value: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeRolePolicyEntry bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeRolePolicyEntry.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String key;
  final String value;

  AgentRuntimeRolePolicyEntry copyWith({
    String? key,
    String? value,
  }) {
    return AgentRuntimeRolePolicyEntry(
      key: key ?? this.key,
      value: value ?? this.value,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(key);
    serializer.serializeString(value);
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

    return other is AgentRuntimeRolePolicyEntry
      && key == other.key
      && value == other.value;
  }

  @override
  int get hashCode => Object.hash(
        key,
        value,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'key: $key, '
        'value: $value'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeRolePolicyEntry';
  }
}
