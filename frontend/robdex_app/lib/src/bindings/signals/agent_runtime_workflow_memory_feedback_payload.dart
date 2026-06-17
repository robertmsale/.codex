// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeWorkflowMemoryFeedbackPayload {
  const AgentRuntimeWorkflowMemoryFeedbackPayload({
    required this.source,
    required this.reason,
    required this.variant,
    required this.hasVariant,
  });

  static AgentRuntimeWorkflowMemoryFeedbackPayload deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeWorkflowMemoryFeedbackPayload(
      source: deserializer.deserializeString(),
      reason: deserializer.deserializeString(),
      variant: deserializer.deserializeBool(),
      hasVariant: deserializer.deserializeBool(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeWorkflowMemoryFeedbackPayload bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeWorkflowMemoryFeedbackPayload.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String source;
  final String reason;
  final bool variant;
  final bool hasVariant;

  AgentRuntimeWorkflowMemoryFeedbackPayload copyWith({
    String? source,
    String? reason,
    bool? variant,
    bool? hasVariant,
  }) {
    return AgentRuntimeWorkflowMemoryFeedbackPayload(
      source: source ?? this.source,
      reason: reason ?? this.reason,
      variant: variant ?? this.variant,
      hasVariant: hasVariant ?? this.hasVariant,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(source);
    serializer.serializeString(reason);
    serializer.serializeBool(variant);
    serializer.serializeBool(hasVariant);
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

    return other is AgentRuntimeWorkflowMemoryFeedbackPayload
      && source == other.source
      && reason == other.reason
      && variant == other.variant
      && hasVariant == other.hasVariant;
  }

  @override
  int get hashCode => Object.hash(
        source,
        reason,
        variant,
        hasVariant,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'source: $source, '
        'reason: $reason, '
        'variant: $variant, '
        'hasVariant: $hasVariant'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeWorkflowMemoryFeedbackPayload';
  }
}
