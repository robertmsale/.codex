// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeRequestSignal {
  const AgentRuntimeRequestSignal({
    required this.requestId,
    required this.packetJson,
  });

  static AgentRuntimeRequestSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeRequestSignal(
      requestId: deserializer.deserializeString(),
      packetJson: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeRequestSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeRequestSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String requestId;
  final String packetJson;

  AgentRuntimeRequestSignal copyWith({
    String? requestId,
    String? packetJson,
  }) {
    return AgentRuntimeRequestSignal(
      requestId: requestId ?? this.requestId,
      packetJson: packetJson ?? this.packetJson,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(requestId);
    serializer.serializeString(packetJson);
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

    return other is AgentRuntimeRequestSignal
      && requestId == other.requestId
      && packetJson == other.packetJson;
  }

  @override
  int get hashCode => Object.hash(
        requestId,
        packetJson,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'requestId: $requestId, '
        'packetJson: $packetJson'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeRequestSignal';
  }
}

extension AgentRuntimeRequestSignalDartSignalExt on AgentRuntimeRequestSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_agent_runtime_request_signal',
      messageBytes,
      binary,
    );
  }
}
