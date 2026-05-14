// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class TerminalInputSignal {
  const TerminalInputSignal({
    required this.sessionId,
    required this.data,
  });

  static TerminalInputSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = TerminalInputSignal(
      sessionId: deserializer.deserializeString(),
      data: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static TerminalInputSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = TerminalInputSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String sessionId;
  final String data;

  TerminalInputSignal copyWith({
    String? sessionId,
    String? data,
  }) {
    return TerminalInputSignal(
      sessionId: sessionId ?? this.sessionId,
      data: data ?? this.data,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(sessionId);
    serializer.serializeString(data);
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

    return other is TerminalInputSignal
      && sessionId == other.sessionId
      && data == other.data;
  }

  @override
  int get hashCode => Object.hash(
        sessionId,
        data,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'sessionId: $sessionId, '
        'data: $data'
        ')';
      return true;
    }());

    return fullString ?? 'TerminalInputSignal';
  }
}

extension TerminalInputSignalDartSignalExt on TerminalInputSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_terminal_input_signal',
      messageBytes,
      binary,
    );
  }
}
