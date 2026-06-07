// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class LoadThreadStatsSignal {
  const LoadThreadStatsSignal({
    required this.requestId,
    required this.threadId,
  });

  static LoadThreadStatsSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = LoadThreadStatsSignal(
      requestId: deserializer.deserializeString(),
      threadId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static LoadThreadStatsSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = LoadThreadStatsSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String requestId;
  final String threadId;

  LoadThreadStatsSignal copyWith({
    String? requestId,
    String? threadId,
  }) {
    return LoadThreadStatsSignal(
      requestId: requestId ?? this.requestId,
      threadId: threadId ?? this.threadId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(requestId);
    serializer.serializeString(threadId);
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

    return other is LoadThreadStatsSignal
      && requestId == other.requestId
      && threadId == other.threadId;
  }

  @override
  int get hashCode => Object.hash(
        requestId,
        threadId,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'requestId: $requestId, '
        'threadId: $threadId'
        ')';
      return true;
    }());

    return fullString ?? 'LoadThreadStatsSignal';
  }
}

extension LoadThreadStatsSignalDartSignalExt on LoadThreadStatsSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_load_thread_stats_signal',
      messageBytes,
      binary,
    );
  }
}
