// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class TerminalOpenSignal {
  const TerminalOpenSignal({
    required this.requestId,
    required this.host,
    required this.username,
    required this.cols,
    required this.rows,
  });

  static TerminalOpenSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = TerminalOpenSignal(
      requestId: deserializer.deserializeString(),
      host: deserializer.deserializeString(),
      username: deserializer.deserializeString(),
      cols: deserializer.deserializeUint32(),
      rows: deserializer.deserializeUint32(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static TerminalOpenSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = TerminalOpenSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String requestId;
  final String host;
  final String username;
  final int cols;
  final int rows;

  TerminalOpenSignal copyWith({
    String? requestId,
    String? host,
    String? username,
    int? cols,
    int? rows,
  }) {
    return TerminalOpenSignal(
      requestId: requestId ?? this.requestId,
      host: host ?? this.host,
      username: username ?? this.username,
      cols: cols ?? this.cols,
      rows: rows ?? this.rows,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(requestId);
    serializer.serializeString(host);
    serializer.serializeString(username);
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

    return other is TerminalOpenSignal
      && requestId == other.requestId
      && host == other.host
      && username == other.username
      && cols == other.cols
      && rows == other.rows;
  }

  @override
  int get hashCode => Object.hash(
        requestId,
        host,
        username,
        cols,
        rows,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'requestId: $requestId, '
        'host: $host, '
        'username: $username, '
        'cols: $cols, '
        'rows: $rows'
        ')';
      return true;
    }());

    return fullString ?? 'TerminalOpenSignal';
  }
}

extension TerminalOpenSignalDartSignalExt on TerminalOpenSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_terminal_open_signal',
      messageBytes,
      binary,
    );
  }
}
