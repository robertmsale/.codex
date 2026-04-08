// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class InitializeWorkbenchSignal {
  const InitializeWorkbenchSignal({
    required this.host,
    required this.port,
  });

  static InitializeWorkbenchSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = InitializeWorkbenchSignal(
      host: deserializer.deserializeString(),
      port: deserializer.deserializeUint32(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static InitializeWorkbenchSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = InitializeWorkbenchSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String host;
  final int port;

  InitializeWorkbenchSignal copyWith({
    String? host,
    int? port,
  }) {
    return InitializeWorkbenchSignal(
      host: host ?? this.host,
      port: port ?? this.port,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(host);
    serializer.serializeUint32(port);
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

    return other is InitializeWorkbenchSignal
      && host == other.host
      && port == other.port;
  }

  @override
  int get hashCode => Object.hash(
        host,
        port,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'host: $host, '
        'port: $port'
        ')';
      return true;
    }());

    return fullString ?? 'InitializeWorkbenchSignal';
  }
}

extension InitializeWorkbenchSignalDartSignalExt on InitializeWorkbenchSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_initialize_workbench_signal',
      messageBytes,
      binary,
    );
  }
}
