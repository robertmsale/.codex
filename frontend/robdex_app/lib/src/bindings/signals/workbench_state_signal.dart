// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class WorkbenchStateSignal {
  /// An async broadcast stream that listens for signals from Rust.
  /// It supports multiple subscriptions.
  /// Make sure to cancel the subscription when it's no longer needed,
  /// such as when a widget is disposed.
  static final rustSignalStream =
      _workbenchStateSignalStreamController.stream.asBroadcastStream();
        
  /// The latest signal value received from Rust.
  /// This is updated every time a new signal is received.
  /// It can be null if no signals have been received yet.
  static RustSignalPack<WorkbenchStateSignal>? latestRustSignal = null;

  const WorkbenchStateSignal({
    required this.viewJson,
    required this.isLoading,
    required this.errorMessage,
  });

  static WorkbenchStateSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = WorkbenchStateSignal(
      viewJson: deserializer.deserializeString(),
      isLoading: deserializer.deserializeBool(),
      errorMessage: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static WorkbenchStateSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = WorkbenchStateSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String viewJson;
  final bool isLoading;
  final String errorMessage;

  WorkbenchStateSignal copyWith({
    String? viewJson,
    bool? isLoading,
    String? errorMessage,
  }) {
    return WorkbenchStateSignal(
      viewJson: viewJson ?? this.viewJson,
      isLoading: isLoading ?? this.isLoading,
      errorMessage: errorMessage ?? this.errorMessage,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(viewJson);
    serializer.serializeBool(isLoading);
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

    return other is WorkbenchStateSignal
      && viewJson == other.viewJson
      && isLoading == other.isLoading
      && errorMessage == other.errorMessage;
  }

  @override
  int get hashCode => Object.hash(
        viewJson,
        isLoading,
        errorMessage,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'viewJson: $viewJson, '
        'isLoading: $isLoading, '
        'errorMessage: $errorMessage'
        ')';
      return true;
    }());

    return fullString ?? 'WorkbenchStateSignal';
  }
}

final _workbenchStateSignalStreamController =
    StreamController<RustSignalPack<WorkbenchStateSignal>>();
