// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class SetThreadRunningStateSignal {
  const SetThreadRunningStateSignal({
    required this.running,
  });

  static SetThreadRunningStateSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = SetThreadRunningStateSignal(
      running: deserializer.deserializeBool(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static SetThreadRunningStateSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = SetThreadRunningStateSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final bool running;

  SetThreadRunningStateSignal copyWith({
    bool? running,
  }) {
    return SetThreadRunningStateSignal(
      running: running ?? this.running,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeBool(running);
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

    return other is SetThreadRunningStateSignal
      && running == other.running;
  }

  @override
  int get hashCode => running.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'running: $running'
        ')';
      return true;
    }());

    return fullString ?? 'SetThreadRunningStateSignal';
  }
}

extension SetThreadRunningStateSignalDartSignalExt on SetThreadRunningStateSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_set_thread_running_state_signal',
      messageBytes,
      binary,
    );
  }
}
