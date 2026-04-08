// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class ThreadHistoryStateSignal {
  /// An async broadcast stream that listens for signals from Rust.
  /// It supports multiple subscriptions.
  /// Make sure to cancel the subscription when it's no longer needed,
  /// such as when a widget is disposed.
  static final rustSignalStream =
      _threadHistoryStateSignalStreamController.stream.asBroadcastStream();
        
  /// The latest signal value received from Rust.
  /// This is updated every time a new signal is received.
  /// It can be null if no signals have been received yet.
  static RustSignalPack<ThreadHistoryStateSignal>? latestRustSignal = null;

  const ThreadHistoryStateSignal({
    required this.entriesJson,
    required this.isLoading,
    required this.errorMessage,
  });

  static ThreadHistoryStateSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = ThreadHistoryStateSignal(
      entriesJson: deserializer.deserializeString(),
      isLoading: deserializer.deserializeBool(),
      errorMessage: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static ThreadHistoryStateSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = ThreadHistoryStateSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String entriesJson;
  final bool isLoading;
  final String errorMessage;

  ThreadHistoryStateSignal copyWith({
    String? entriesJson,
    bool? isLoading,
    String? errorMessage,
  }) {
    return ThreadHistoryStateSignal(
      entriesJson: entriesJson ?? this.entriesJson,
      isLoading: isLoading ?? this.isLoading,
      errorMessage: errorMessage ?? this.errorMessage,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(entriesJson);
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

    return other is ThreadHistoryStateSignal
      && entriesJson == other.entriesJson
      && isLoading == other.isLoading
      && errorMessage == other.errorMessage;
  }

  @override
  int get hashCode => Object.hash(
        entriesJson,
        isLoading,
        errorMessage,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'entriesJson: $entriesJson, '
        'isLoading: $isLoading, '
        'errorMessage: $errorMessage'
        ')';
      return true;
    }());

    return fullString ?? 'ThreadHistoryStateSignal';
  }
}

final _threadHistoryStateSignalStreamController =
    StreamController<RustSignalPack<ThreadHistoryStateSignal>>();
