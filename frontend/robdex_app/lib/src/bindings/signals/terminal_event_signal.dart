// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class TerminalEventSignal {
  /// An async broadcast stream that listens for signals from Rust.
  /// It supports multiple subscriptions.
  /// Make sure to cancel the subscription when it's no longer needed,
  /// such as when a widget is disposed.
  static final rustSignalStream =
      _terminalEventSignalStreamController.stream.asBroadcastStream();
        
  /// The latest signal value received from Rust.
  /// This is updated every time a new signal is received.
  /// It can be null if no signals have been received yet.
  static RustSignalPack<TerminalEventSignal>? latestRustSignal = null;

  const TerminalEventSignal({
    required this.requestId,
    required this.sessionId,
    required this.kind,
    required this.data,
    required this.host,
    required this.username,
  });

  static TerminalEventSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = TerminalEventSignal(
      requestId: deserializer.deserializeString(),
      sessionId: deserializer.deserializeString(),
      kind: deserializer.deserializeString(),
      data: deserializer.deserializeString(),
      host: deserializer.deserializeString(),
      username: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static TerminalEventSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = TerminalEventSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String requestId;
  final String sessionId;
  final String kind;
  final String data;
  final String host;
  final String username;

  TerminalEventSignal copyWith({
    String? requestId,
    String? sessionId,
    String? kind,
    String? data,
    String? host,
    String? username,
  }) {
    return TerminalEventSignal(
      requestId: requestId ?? this.requestId,
      sessionId: sessionId ?? this.sessionId,
      kind: kind ?? this.kind,
      data: data ?? this.data,
      host: host ?? this.host,
      username: username ?? this.username,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(requestId);
    serializer.serializeString(sessionId);
    serializer.serializeString(kind);
    serializer.serializeString(data);
    serializer.serializeString(host);
    serializer.serializeString(username);
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

    return other is TerminalEventSignal
      && requestId == other.requestId
      && sessionId == other.sessionId
      && kind == other.kind
      && data == other.data
      && host == other.host
      && username == other.username;
  }

  @override
  int get hashCode => Object.hash(
        requestId,
        sessionId,
        kind,
        data,
        host,
        username,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'requestId: $requestId, '
        'sessionId: $sessionId, '
        'kind: $kind, '
        'data: $data, '
        'host: $host, '
        'username: $username'
        ')';
      return true;
    }());

    return fullString ?? 'TerminalEventSignal';
  }
}

final _terminalEventSignalStreamController =
    StreamController<RustSignalPack<TerminalEventSignal>>();
