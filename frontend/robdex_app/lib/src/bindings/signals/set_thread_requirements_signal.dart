// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class SetThreadRequirementsSignal {
  const SetThreadRequirementsSignal({
    required this.requestId,
    required this.senderThreadId,
    required this.recipientThreadId,
    required this.projectPath,
    required this.requirementSetJson,
  });

  static SetThreadRequirementsSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = SetThreadRequirementsSignal(
      requestId: deserializer.deserializeString(),
      senderThreadId: deserializer.deserializeString(),
      recipientThreadId: deserializer.deserializeString(),
      projectPath: deserializer.deserializeString(),
      requirementSetJson: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static SetThreadRequirementsSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = SetThreadRequirementsSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String requestId;
  final String senderThreadId;
  final String recipientThreadId;
  final String projectPath;
  final String requirementSetJson;

  SetThreadRequirementsSignal copyWith({
    String? requestId,
    String? senderThreadId,
    String? recipientThreadId,
    String? projectPath,
    String? requirementSetJson,
  }) {
    return SetThreadRequirementsSignal(
      requestId: requestId ?? this.requestId,
      senderThreadId: senderThreadId ?? this.senderThreadId,
      recipientThreadId: recipientThreadId ?? this.recipientThreadId,
      projectPath: projectPath ?? this.projectPath,
      requirementSetJson: requirementSetJson ?? this.requirementSetJson,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(requestId);
    serializer.serializeString(senderThreadId);
    serializer.serializeString(recipientThreadId);
    serializer.serializeString(projectPath);
    serializer.serializeString(requirementSetJson);
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

    return other is SetThreadRequirementsSignal
      && requestId == other.requestId
      && senderThreadId == other.senderThreadId
      && recipientThreadId == other.recipientThreadId
      && projectPath == other.projectPath
      && requirementSetJson == other.requirementSetJson;
  }

  @override
  int get hashCode => Object.hash(
        requestId,
        senderThreadId,
        recipientThreadId,
        projectPath,
        requirementSetJson,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'requestId: $requestId, '
        'senderThreadId: $senderThreadId, '
        'recipientThreadId: $recipientThreadId, '
        'projectPath: $projectPath, '
        'requirementSetJson: $requirementSetJson'
        ')';
      return true;
    }());

    return fullString ?? 'SetThreadRequirementsSignal';
  }
}

extension SetThreadRequirementsSignalDartSignalExt on SetThreadRequirementsSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_set_thread_requirements_signal',
      messageBytes,
      binary,
    );
  }
}
