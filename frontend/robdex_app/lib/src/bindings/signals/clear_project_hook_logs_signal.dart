// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class ClearProjectHookLogsSignal {
  const ClearProjectHookLogsSignal({
    required this.requestId,
    required this.projectId,
  });

  static ClearProjectHookLogsSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = ClearProjectHookLogsSignal(
      requestId: deserializer.deserializeString(),
      projectId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static ClearProjectHookLogsSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = ClearProjectHookLogsSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String requestId;
  final String projectId;

  ClearProjectHookLogsSignal copyWith({
    String? requestId,
    String? projectId,
  }) {
    return ClearProjectHookLogsSignal(
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

    return other is ClearProjectHookLogsSignal
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

    return fullString ?? 'ClearProjectHookLogsSignal';
  }
}

extension ClearProjectHookLogsSignalDartSignalExt on ClearProjectHookLogsSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_clear_project_hook_logs_signal',
      messageBytes,
      binary,
    );
  }
}
