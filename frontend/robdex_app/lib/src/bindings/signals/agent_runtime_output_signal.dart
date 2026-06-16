// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeOutputSignal {
  /// An async broadcast stream that listens for signals from Rust.
  /// It supports multiple subscriptions.
  /// Make sure to cancel the subscription when it's no longer needed,
  /// such as when a widget is disposed.
  static final rustSignalStream =
      _agentRuntimeOutputSignalStreamController.stream.asBroadcastStream();

  /// The latest signal value received from Rust.
  /// This is updated every time a new signal is received.
  /// It can be null if no signals have been received yet.
  static RustSignalPack<AgentRuntimeOutputSignal>? latestRustSignal = null;

  const AgentRuntimeOutputSignal({
    required this.requestId,
    required this.outputJson,
  });

  static AgentRuntimeOutputSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeOutputSignal(
      requestId: deserializer.deserializeString(),
      outputJson: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeOutputSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeOutputSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String requestId;
  final String outputJson;

  AgentRuntimeOutputSignal copyWith({
    String? requestId,
    String? outputJson,
  }) {
    return AgentRuntimeOutputSignal(
      requestId: requestId ?? this.requestId,
      outputJson: outputJson ?? this.outputJson,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(requestId);
    serializer.serializeString(outputJson);
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

    return other is AgentRuntimeOutputSignal
      && requestId == other.requestId
      && outputJson == other.outputJson;
  }

  @override
  int get hashCode => Object.hash(
        requestId,
        outputJson,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'requestId: $requestId, '
        'outputJson: $outputJson'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeOutputSignal';
  }
}

final _agentRuntimeOutputSignalStreamController =
    StreamController<RustSignalPack<AgentRuntimeOutputSignal>>();
