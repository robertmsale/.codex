// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class TerminalResizeSignal {
  const TerminalResizeSignal({
    required this.sessionId,
    required this.cols,
    required this.rows,
  });

  static TerminalResizeSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = TerminalResizeSignal(
      sessionId: deserializer.deserializeString(),
      cols: deserializer.deserializeUint32(),
      rows: deserializer.deserializeUint32(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static TerminalResizeSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = TerminalResizeSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String sessionId;
  final int cols;
  final int rows;

  TerminalResizeSignal copyWith({
    String? sessionId,
    int? cols,
    int? rows,
  }) {
    return TerminalResizeSignal(
      sessionId: sessionId ?? this.sessionId,
      cols: cols ?? this.cols,
      rows: rows ?? this.rows,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(sessionId);
    serializer.serializeUint32(cols);
    serializer.serializeUint32(rows);
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

    return other is TerminalResizeSignal
      && sessionId == other.sessionId
      && cols == other.cols
      && rows == other.rows;
  }

  @override
  int get hashCode => Object.hash(
        sessionId,
        cols,
        rows,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'sessionId: $sessionId, '
        'cols: $cols, '
        'rows: $rows'
        ')';
      return true;
    }());

    return fullString ?? 'TerminalResizeSignal';
  }
}

extension TerminalResizeSignalDartSignalExt on TerminalResizeSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_terminal_resize_signal',
      messageBytes,
      binary,
    );
  }
}
