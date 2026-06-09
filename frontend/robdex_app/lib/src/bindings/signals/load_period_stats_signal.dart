// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class LoadPeriodStatsSignal {
  const LoadPeriodStatsSignal({
    required this.requestId,
    required this.startMs,
    required this.endMs,
    required this.label,
    required this.quotaResetAtMs,
    required this.quotaRemainingPercent,
    required this.hasQuota,
  });

  static LoadPeriodStatsSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = LoadPeriodStatsSignal(
      requestId: deserializer.deserializeString(),
      startMs: deserializer.deserializeUint64(),
      endMs: deserializer.deserializeUint64(),
      label: deserializer.deserializeString(),
      quotaResetAtMs: deserializer.deserializeUint64(),
      quotaRemainingPercent: deserializer.deserializeFloat64(),
      hasQuota: deserializer.deserializeBool(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static LoadPeriodStatsSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = LoadPeriodStatsSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String requestId;
  final Uint64 startMs;
  final Uint64 endMs;
  final String label;
  final Uint64 quotaResetAtMs;
  final double quotaRemainingPercent;
  final bool hasQuota;

  LoadPeriodStatsSignal copyWith({
    String? requestId,
    Uint64? startMs,
    Uint64? endMs,
    String? label,
    Uint64? quotaResetAtMs,
    double? quotaRemainingPercent,
    bool? hasQuota,
  }) {
    return LoadPeriodStatsSignal(
      requestId: requestId ?? this.requestId,
      startMs: startMs ?? this.startMs,
      endMs: endMs ?? this.endMs,
      label: label ?? this.label,
      quotaResetAtMs: quotaResetAtMs ?? this.quotaResetAtMs,
      quotaRemainingPercent: quotaRemainingPercent ?? this.quotaRemainingPercent,
      hasQuota: hasQuota ?? this.hasQuota,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(requestId);
    serializer.serializeUint64(startMs);
    serializer.serializeUint64(endMs);
    serializer.serializeString(label);
    serializer.serializeUint64(quotaResetAtMs);
    serializer.serializeFloat64(quotaRemainingPercent);
    serializer.serializeBool(hasQuota);
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

    return other is LoadPeriodStatsSignal
      && requestId == other.requestId
      && startMs == other.startMs
      && endMs == other.endMs
      && label == other.label
      && quotaResetAtMs == other.quotaResetAtMs
      && quotaRemainingPercent == other.quotaRemainingPercent
      && hasQuota == other.hasQuota;
  }

  @override
  int get hashCode => Object.hash(
        requestId,
        startMs,
        endMs,
        label,
        quotaResetAtMs,
        quotaRemainingPercent,
        hasQuota,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'requestId: $requestId, '
        'startMs: $startMs, '
        'endMs: $endMs, '
        'label: $label, '
        'quotaResetAtMs: $quotaResetAtMs, '
        'quotaRemainingPercent: $quotaRemainingPercent, '
        'hasQuota: $hasQuota'
        ')';
      return true;
    }());

    return fullString ?? 'LoadPeriodStatsSignal';
  }
}

extension LoadPeriodStatsSignalDartSignalExt on LoadPeriodStatsSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_load_period_stats_signal',
      messageBytes,
      binary,
    );
  }
}
