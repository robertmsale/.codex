// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class LoadProjectHookLogsSignal {
  const LoadProjectHookLogsSignal({
    required this.requestId,
    required this.projectId,
  });

  static LoadProjectHookLogsSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = LoadProjectHookLogsSignal(
      requestId: deserializer.deserializeString(),
      projectId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static LoadProjectHookLogsSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = LoadProjectHookLogsSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String requestId;
  final String projectId;

  LoadProjectHookLogsSignal copyWith({
    String? requestId,
    String? projectId,
  }) {
    return LoadProjectHookLogsSignal(
      requestId: requestId ?? this.requestId,
      projectId: projectId ?? this.projectId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(requestId);
    serializer.serializeString(projectId);
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

    return other is LoadProjectHookLogsSignal
      && requestId == other.requestId
      && projectId == other.projectId;
  }

  @override
  int get hashCode => Object.hash(
        requestId,
        projectId,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'requestId: $requestId, '
        'projectId: $projectId'
        ')';
      return true;
    }());

    return fullString ?? 'LoadProjectHookLogsSignal';
  }
}

extension LoadProjectHookLogsSignalDartSignalExt on LoadProjectHookLogsSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_load_project_hook_logs_signal',
      messageBytes,
      binary,
    );
  }
}
