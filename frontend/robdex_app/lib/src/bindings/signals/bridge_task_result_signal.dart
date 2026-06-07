// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class BridgeTaskResultSignal {
  /// An async broadcast stream that listens for signals from Rust.
  /// It supports multiple subscriptions.
  /// Make sure to cancel the subscription when it's no longer needed,
  /// such as when a widget is disposed.
  static final rustSignalStream =
      _bridgeTaskResultSignalStreamController.stream.asBroadcastStream();
        
  /// The latest signal value received from Rust.
  /// This is updated every time a new signal is received.
  /// It can be null if no signals have been received yet.
  static RustSignalPack<BridgeTaskResultSignal>? latestRustSignal = null;

  const BridgeTaskResultSignal({
    required this.requestId,
    required this.task,
    required this.payloadJson,
    required this.errorMessage,
  });

  static BridgeTaskResultSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = BridgeTaskResultSignal(
      requestId: deserializer.deserializeString(),
      task: deserializer.deserializeString(),
      payloadJson: deserializer.deserializeString(),
      errorMessage: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static BridgeTaskResultSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = BridgeTaskResultSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String requestId;
  final String task;
  final String payloadJson;
  final String errorMessage;

  BridgeTaskResultSignal copyWith({
    String? requestId,
    String? task,
    String? payloadJson,
    String? errorMessage,
  }) {
    return BridgeTaskResultSignal(
      requestId: requestId ?? this.requestId,
      task: task ?? this.task,
      payloadJson: payloadJson ?? this.payloadJson,
      errorMessage: errorMessage ?? this.errorMessage,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(requestId);
    serializer.serializeString(task);
    serializer.serializeString(payloadJson);
    serializer.serializeString(errorMessage);
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

    return other is BridgeTaskResultSignal
      && requestId == other.requestId
      && task == other.task
      && payloadJson == other.payloadJson
      && errorMessage == other.errorMessage;
  }

  @override
  int get hashCode => Object.hash(
        requestId,
        task,
        payloadJson,
        errorMessage,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'requestId: $requestId, '
        'task: $task, '
        'payloadJson: $payloadJson, '
        'errorMessage: $errorMessage'
        ')';
      return true;
    }());

    return fullString ?? 'BridgeTaskResultSignal';
  }
}

final _bridgeTaskResultSignalStreamController =
    StreamController<RustSignalPack<BridgeTaskResultSignal>>();
