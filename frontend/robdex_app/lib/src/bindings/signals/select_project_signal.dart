// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class SelectProjectSignal {
  const SelectProjectSignal({
    required this.projectId,
  });

  static SelectProjectSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = SelectProjectSignal(
      projectId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static SelectProjectSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = SelectProjectSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String projectId;

  SelectProjectSignal copyWith({
    String? projectId,
  }) {
    return SelectProjectSignal(
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

    return other is SelectProjectSignal
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

    return fullString ?? 'SelectProjectSignal';
  }
}

extension SelectProjectSignalDartSignalExt on SelectProjectSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_select_project_signal',
      messageBytes,
      binary,
    );
  }
}
