// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeFinalExecutionPolicy {
  const AgentRuntimeFinalExecutionPolicy({
    required this.decision,
    required this.reason,
  });

  static AgentRuntimeFinalExecutionPolicy deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeFinalExecutionPolicy(
      decision: deserializer.deserializeString(),
      reason: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeFinalExecutionPolicy bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeFinalExecutionPolicy.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String decision;
  final String reason;

  AgentRuntimeFinalExecutionPolicy copyWith({
    String? decision,
    String? reason,
  }) {
    return AgentRuntimeFinalExecutionPolicy(
      decision: decision ?? this.decision,
      reason: reason ?? this.reason,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(decision);
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

    return other is AgentRuntimeFinalExecutionPolicy
      && decision == other.decision
      && reason == other.reason;
  }

  @override
  int get hashCode => Object.hash(
        decision,
        reason,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'decision: $decision, '
        'reason: $reason'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeFinalExecutionPolicy';
  }
}
