// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class LoadRequirementComposablesSignal {
  const LoadRequirementComposablesSignal({
    required this.requestId,
    required this.senderThreadId,
    required this.recipientThreadId,
    required this.projectPath,
  });

  static LoadRequirementComposablesSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = LoadRequirementComposablesSignal(
      requestId: deserializer.deserializeString(),
      senderThreadId: deserializer.deserializeString(),
      recipientThreadId: deserializer.deserializeString(),
      projectPath: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static LoadRequirementComposablesSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = LoadRequirementComposablesSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String requestId;
  final String senderThreadId;
  final String recipientThreadId;
  final String projectPath;

  LoadRequirementComposablesSignal copyWith({
    String? requestId,
    String? senderThreadId,
    String? recipientThreadId,
    String? projectPath,
  }) {
    return LoadRequirementComposablesSignal(
      requestId: requestId ?? this.requestId,
      senderThreadId: senderThreadId ?? this.senderThreadId,
      recipientThreadId: recipientThreadId ?? this.recipientThreadId,
      projectPath: projectPath ?? this.projectPath,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(requestId);
    serializer.serializeString(senderThreadId);
    serializer.serializeString(recipientThreadId);
    serializer.serializeString(projectPath);
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

    return other is LoadRequirementComposablesSignal
      && requestId == other.requestId
      && senderThreadId == other.senderThreadId
      && recipientThreadId == other.recipientThreadId
      && projectPath == other.projectPath;
  }

  @override
  int get hashCode => Object.hash(
        requestId,
        senderThreadId,
        recipientThreadId,
        projectPath,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'requestId: $requestId, '
        'senderThreadId: $senderThreadId, '
        'recipientThreadId: $recipientThreadId, '
        'projectPath: $projectPath'
        ')';
      return true;
    }());

    return fullString ?? 'LoadRequirementComposablesSignal';
  }
}

extension LoadRequirementComposablesSignalDartSignalExt on LoadRequirementComposablesSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_load_requirement_composables_signal',
      messageBytes,
      binary,
    );
  }
}
