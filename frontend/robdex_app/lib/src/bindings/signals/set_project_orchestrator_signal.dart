// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class SetProjectOrchestratorSignal {
  const SetProjectOrchestratorSignal({
    required this.projectId,
    required this.projectPath,
    required this.threadId,
  });

  static SetProjectOrchestratorSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = SetProjectOrchestratorSignal(
      projectId: deserializer.deserializeString(),
      projectPath: deserializer.deserializeString(),
      threadId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static SetProjectOrchestratorSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = SetProjectOrchestratorSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String projectId;
  final String projectPath;
  final String threadId;

  SetProjectOrchestratorSignal copyWith({
    String? projectId,
    String? projectPath,
    String? threadId,
  }) {
    return SetProjectOrchestratorSignal(
      projectId: projectId ?? this.projectId,
      projectPath: projectPath ?? this.projectPath,
      threadId: threadId ?? this.threadId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(projectId);
    serializer.serializeString(projectPath);
    serializer.serializeString(threadId);
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

    return other is SetProjectOrchestratorSignal
      && projectId == other.projectId
      && projectPath == other.projectPath
      && threadId == other.threadId;
  }

  @override
  int get hashCode => Object.hash(
        projectId,
        projectPath,
        threadId,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'projectId: $projectId, '
        'projectPath: $projectPath, '
        'threadId: $threadId'
        ')';
      return true;
    }());

    return fullString ?? 'SetProjectOrchestratorSignal';
  }
}

extension SetProjectOrchestratorSignalDartSignalExt on SetProjectOrchestratorSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_set_project_orchestrator_signal',
      messageBytes,
      binary,
    );
  }
}
