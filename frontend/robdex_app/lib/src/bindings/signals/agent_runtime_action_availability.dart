// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeActionAvailability {
  const AgentRuntimeActionAvailability({
    required this.id,
    required this.label,
    required this.available,
    required this.reason,
  });

  static AgentRuntimeActionAvailability deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeActionAvailability(
      id: deserializer.deserializeString(),
      label: deserializer.deserializeString(),
      available: deserializer.deserializeBool(),
      reason: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeActionAvailability bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeActionAvailability.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String id;
  final String label;
  final bool available;
  final String reason;

  AgentRuntimeActionAvailability copyWith({
    String? id,
    String? label,
    bool? available,
    String? reason,
  }) {
    return AgentRuntimeActionAvailability(
      id: id ?? this.id,
      label: label ?? this.label,
      available: available ?? this.available,
      reason: reason ?? this.reason,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(id);
    serializer.serializeString(label);
    serializer.serializeBool(available);
    serializer.serializeString(reason);
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

    return other is AgentRuntimeActionAvailability
      && id == other.id
      && label == other.label
      && available == other.available
      && reason == other.reason;
  }

  @override
  int get hashCode => Object.hash(
        id,
        label,
        available,
        reason,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'id: $id, '
        'label: $label, '
        'available: $available, '
        'reason: $reason'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeActionAvailability';
  }
}
