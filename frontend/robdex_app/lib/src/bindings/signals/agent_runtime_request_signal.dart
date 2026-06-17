// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeRequestSignal {
  const AgentRuntimeRequestSignal({
    required this.requestId,
    required this.request,
  });

  static AgentRuntimeRequestSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeRequestSignal(
      requestId: deserializer.deserializeString(),
      request: AgentRuntimeRequest.deserialize(deserializer),
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
  final AgentRuntimeRequest request;

  AgentRuntimeRequestSignal copyWith({
    String? requestId,
    AgentRuntimeRequest? request,
  }) {
    return AgentRuntimeRequestSignal(
      requestId: requestId ?? this.requestId,
      request: request ?? this.request,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(requestId);
    request.serialize(serializer);
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
      && request == other.request;
  }

  @override
  int get hashCode => Object.hash(
        requestId,
        request,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'requestId: $requestId, '
        'request: $request'
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
