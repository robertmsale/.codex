// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class SendThreadMessageSignal {
  const SendThreadMessageSignal({
    required this.text,
  });

  static SendThreadMessageSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = SendThreadMessageSignal(
      text: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static SendThreadMessageSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = SendThreadMessageSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String text;

  SendThreadMessageSignal copyWith({
    String? text,
  }) {
    return SendThreadMessageSignal(
      text: text ?? this.text,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(text);
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

    return other is SendThreadMessageSignal
      && text == other.text;
  }

  @override
  int get hashCode => text.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'text: $text'
        ')';
      return true;
    }());

    return fullString ?? 'SendThreadMessageSignal';
  }
}

extension SendThreadMessageSignalDartSignalExt on SendThreadMessageSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_send_thread_message_signal',
      messageBytes,
      binary,
    );
  }
}
