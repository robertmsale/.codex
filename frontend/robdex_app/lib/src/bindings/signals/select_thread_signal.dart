// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class SelectThreadSignal {
  const SelectThreadSignal({
    required this.threadId,
  });

  static SelectThreadSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = SelectThreadSignal(
      threadId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static SelectThreadSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = SelectThreadSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String threadId;

  SelectThreadSignal copyWith({
    String? threadId,
  }) {
    return SelectThreadSignal(
      threadId: threadId ?? this.threadId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
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

    return other is SelectThreadSignal
      && threadId == other.threadId;
  }

  @override
  int get hashCode => threadId.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'threadId: $threadId'
        ')';
      return true;
    }());

    return fullString ?? 'SelectThreadSignal';
  }
}

extension SelectThreadSignalDartSignalExt on SelectThreadSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_select_thread_signal',
      messageBytes,
      binary,
    );
  }
}
