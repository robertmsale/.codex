// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class TerminateCommandExecutionSignal {
  const TerminateCommandExecutionSignal({
    required this.processId,
  });

  static TerminateCommandExecutionSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = TerminateCommandExecutionSignal(
      processId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static TerminateCommandExecutionSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = TerminateCommandExecutionSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String processId;

  TerminateCommandExecutionSignal copyWith({
    String? processId,
  }) {
    return TerminateCommandExecutionSignal(
      processId: processId ?? this.processId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(processId);
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

    return other is TerminateCommandExecutionSignal
      && processId == other.processId;
  }

  @override
  int get hashCode => processId.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'processId: $processId'
        ')';
      return true;
    }());

    return fullString ?? 'TerminateCommandExecutionSignal';
  }
}

extension TerminateCommandExecutionSignalDartSignalExt on TerminateCommandExecutionSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_terminate_command_execution_signal',
      messageBytes,
      binary,
    );
  }
}
