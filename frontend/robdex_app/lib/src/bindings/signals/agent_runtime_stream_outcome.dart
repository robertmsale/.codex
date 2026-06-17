// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


abstract class AgentRuntimeStreamOutcome {
  const AgentRuntimeStreamOutcome();

  void serialize(BinarySerializer serializer);

  static AgentRuntimeStreamOutcome deserialize(BinaryDeserializer deserializer) {
    int index = deserializer.deserializeVariantIndex();
    switch (index) {
      case 0: return AgentRuntimeStreamOutcomeHello.load(deserializer);
      case 1: return AgentRuntimeStreamOutcomeDeltaApplied.load(deserializer);
      case 2: return AgentRuntimeStreamOutcomeResyncRequired.load(deserializer);
      case 3: return AgentRuntimeStreamOutcomeServerShutdown.load(deserializer);
      case 4: return AgentRuntimeStreamOutcomeStreamClosed.load(deserializer);
      default: throw Exception('Unknown variant index for AgentRuntimeStreamOutcome: ' + index.toString());
    }
  }

  Uint8List bincodeSerialize() {
      final serializer = BincodeSerializer();
      serialize(serializer);
      return serializer.bytes;
  }

  static AgentRuntimeStreamOutcome bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeStreamOutcome.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }
}


@immutable
class AgentRuntimeStreamOutcomeHello extends AgentRuntimeStreamOutcome {
  const AgentRuntimeStreamOutcomeHello({
    required this.watermark,
    required this.runtimeIdentity,
    required this.hasRuntimeIdentity,
  }) : super();

  static AgentRuntimeStreamOutcomeHello load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeStreamOutcomeHello(
      watermark: deserializer.deserializeInt64(),
      runtimeIdentity: deserializer.deserializeString(),
      hasRuntimeIdentity: deserializer.deserializeBool(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final int watermark;
  final String runtimeIdentity;
  final bool hasRuntimeIdentity;

  AgentRuntimeStreamOutcomeHello copyWith({
    int? watermark,
    String? runtimeIdentity,
    bool? hasRuntimeIdentity,
  }) {
    return AgentRuntimeStreamOutcomeHello(
      watermark: watermark ?? this.watermark,
      runtimeIdentity: runtimeIdentity ?? this.runtimeIdentity,
      hasRuntimeIdentity: hasRuntimeIdentity ?? this.hasRuntimeIdentity,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(0);
    serializer.serializeInt64(watermark);
    serializer.serializeString(runtimeIdentity);
    serializer.serializeBool(hasRuntimeIdentity);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeStreamOutcomeHello
      && watermark == other.watermark
      && runtimeIdentity == other.runtimeIdentity
      && hasRuntimeIdentity == other.hasRuntimeIdentity;
  }

  @override
  int get hashCode => Object.hash(
        watermark,
        runtimeIdentity,
        hasRuntimeIdentity,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'watermark: $watermark, '
        'runtimeIdentity: $runtimeIdentity, '
        'hasRuntimeIdentity: $hasRuntimeIdentity'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeStreamOutcomeHello';
  }
}

@immutable
class AgentRuntimeStreamOutcomeDeltaApplied extends AgentRuntimeStreamOutcome {
  const AgentRuntimeStreamOutcomeDeltaApplied({
    required this.applyOutcome,
  }) : super();

  static AgentRuntimeStreamOutcomeDeltaApplied load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeStreamOutcomeDeltaApplied(
      applyOutcome: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String applyOutcome;

  AgentRuntimeStreamOutcomeDeltaApplied copyWith({
    String? applyOutcome,
  }) {
    return AgentRuntimeStreamOutcomeDeltaApplied(
      applyOutcome: applyOutcome ?? this.applyOutcome,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(1);
    serializer.serializeString(applyOutcome);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeStreamOutcomeDeltaApplied
      && applyOutcome == other.applyOutcome;
  }

  @override
  int get hashCode => applyOutcome.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'applyOutcome: $applyOutcome'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeStreamOutcomeDeltaApplied';
  }
}

@immutable
class AgentRuntimeStreamOutcomeResyncRequired extends AgentRuntimeStreamOutcome {
  const AgentRuntimeStreamOutcomeResyncRequired({
    required this.reason,
    required this.hasReason,
  }) : super();

  static AgentRuntimeStreamOutcomeResyncRequired load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeStreamOutcomeResyncRequired(
      reason: deserializer.deserializeString(),
      hasReason: deserializer.deserializeBool(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  final String reason;
  final bool hasReason;

  AgentRuntimeStreamOutcomeResyncRequired copyWith({
    String? reason,
    bool? hasReason,
  }) {
    return AgentRuntimeStreamOutcomeResyncRequired(
      reason: reason ?? this.reason,
      hasReason: hasReason ?? this.hasReason,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(2);
    serializer.serializeString(reason);
    serializer.serializeBool(hasReason);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeStreamOutcomeResyncRequired
      && reason == other.reason
      && hasReason == other.hasReason;
  }

  @override
  int get hashCode => Object.hash(
        reason,
        hasReason,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'reason: $reason, '
        'hasReason: $hasReason'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeStreamOutcomeResyncRequired';
  }
}

@immutable
class AgentRuntimeStreamOutcomeServerShutdown extends AgentRuntimeStreamOutcome {
  const AgentRuntimeStreamOutcomeServerShutdown(
  ) : super();

  static AgentRuntimeStreamOutcomeServerShutdown load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeStreamOutcomeServerShutdown(
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(3);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeStreamOutcomeServerShutdown;
  }

  @override
  int get hashCode => runtimeType.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeStreamOutcomeServerShutdown';
  }
}

@immutable
class AgentRuntimeStreamOutcomeStreamClosed extends AgentRuntimeStreamOutcome {
  const AgentRuntimeStreamOutcomeStreamClosed(
  ) : super();

  static AgentRuntimeStreamOutcomeStreamClosed load(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeStreamOutcomeStreamClosed(
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeVariantIndex(4);
    serializer.decreaseContainerDepth();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;

    return other is AgentRuntimeStreamOutcomeStreamClosed;
  }

  @override
  int get hashCode => runtimeType.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeStreamOutcomeStreamClosed';
  }
}
