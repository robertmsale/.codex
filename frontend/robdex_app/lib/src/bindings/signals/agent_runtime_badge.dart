// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeBadge {
  const AgentRuntimeBadge({
    required this.label,
    required this.value,
    required this.tone,
  });

  static AgentRuntimeBadge deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeBadge(
      label: deserializer.deserializeString(),
      value: deserializer.deserializeString(),
      tone: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeBadge bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeBadge.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String label;
  final String value;
  final String tone;

  AgentRuntimeBadge copyWith({
    String? label,
    String? value,
    String? tone,
  }) {
    return AgentRuntimeBadge(
      label: label ?? this.label,
      value: value ?? this.value,
      tone: tone ?? this.tone,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(label);
    serializer.serializeString(value);
    serializer.serializeString(tone);
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

    return other is AgentRuntimeBadge
      && label == other.label
      && value == other.value
      && tone == other.tone;
  }

  @override
  int get hashCode => Object.hash(
        label,
        value,
        tone,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'label: $label, '
        'value: $value, '
        'tone: $tone'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeBadge';
  }
}
