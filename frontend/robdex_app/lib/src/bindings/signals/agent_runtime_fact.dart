// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeFact {
  const AgentRuntimeFact({
    required this.label,
    required this.value,
  });

  static AgentRuntimeFact deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeFact(
      label: deserializer.deserializeString(),
      value: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeFact bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeFact.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String label;
  final String value;

  AgentRuntimeFact copyWith({
    String? label,
    String? value,
  }) {
    return AgentRuntimeFact(
      label: label ?? this.label,
      value: value ?? this.value,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(label);
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

    return other is AgentRuntimeFact
      && label == other.label
      && value == other.value;
  }

  @override
  int get hashCode => Object.hash(
        label,
        value,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'label: $label, '
        'value: $value'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeFact';
  }
}
