// ignore_for_file: type=lint, type=warning
part of 'signals.dart';

@immutable
class ThreadCompactSignal {
  const ThreadCompactSignal();

  static ThreadCompactSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = ThreadCompactSignal();
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static ThreadCompactSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = ThreadCompactSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
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

    return other is ThreadCompactSignal;
  }

  @override
  int get hashCode => runtimeType.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType()';
      return true;
    }());

    return fullString ?? 'ThreadCompactSignal';
  }
}

extension ThreadCompactSignalDartSignalExt on ThreadCompactSignal {
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_thread_compact_signal',
      messageBytes,
      binary,
    );
  }
}
