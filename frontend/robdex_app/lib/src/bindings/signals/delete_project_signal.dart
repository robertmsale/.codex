// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class DeleteProjectSignal {
  const DeleteProjectSignal({
    required this.projectId,
  });

  static DeleteProjectSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = DeleteProjectSignal(
      projectId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static DeleteProjectSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = DeleteProjectSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String projectId;

  DeleteProjectSignal copyWith({
    String? projectId,
  }) {
    return DeleteProjectSignal(
      projectId: projectId ?? this.projectId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
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

    return other is DeleteProjectSignal
      && projectId == other.projectId;
  }

  @override
  int get hashCode => projectId.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'projectId: $projectId'
        ')';
      return true;
    }());

    return fullString ?? 'DeleteProjectSignal';
  }
}

extension DeleteProjectSignalDartSignalExt on DeleteProjectSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_delete_project_signal',
      messageBytes,
      binary,
    );
  }
}
