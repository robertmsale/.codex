// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeGodModeState {
  const AgentRuntimeGodModeState({
    required this.active,
    required this.reason,
    required this.grantedBy,
    required this.grantedAt,
  });

  static AgentRuntimeGodModeState deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeGodModeState(
      active: deserializer.deserializeBool(),
      reason: deserializer.deserializeString(),
      grantedBy: deserializer.deserializeString(),
      grantedAt: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeGodModeState bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeGodModeState.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final bool active;
  final String reason;
  final String grantedBy;
  final String grantedAt;

  AgentRuntimeGodModeState copyWith({
    bool? active,
    String? reason,
    String? grantedBy,
    String? grantedAt,
  }) {
    return AgentRuntimeGodModeState(
      active: active ?? this.active,
      reason: reason ?? this.reason,
      grantedBy: grantedBy ?? this.grantedBy,
      grantedAt: grantedAt ?? this.grantedAt,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeBool(active);
    serializer.serializeString(reason);
    serializer.serializeString(grantedBy);
    serializer.serializeString(grantedAt);
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

    return other is AgentRuntimeGodModeState
      && active == other.active
      && reason == other.reason
      && grantedBy == other.grantedBy
      && grantedAt == other.grantedAt;
  }

  @override
  int get hashCode => Object.hash(
        active,
        reason,
        grantedBy,
        grantedAt,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'active: $active, '
        'reason: $reason, '
        'grantedBy: $grantedBy, '
        'grantedAt: $grantedAt'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeGodModeState';
  }
}
