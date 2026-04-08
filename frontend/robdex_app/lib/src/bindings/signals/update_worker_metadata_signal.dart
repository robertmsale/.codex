// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class UpdateWorkerMetadataSignal {
  const UpdateWorkerMetadataSignal({
    required this.issueNumber,
    required this.pullRequestNumber,
    required this.blockedReason,
    required this.unblockWhen,
    required this.clearBlocked,
  });

  static UpdateWorkerMetadataSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = UpdateWorkerMetadataSignal(
      issueNumber: deserializer.deserializeString(),
      pullRequestNumber: deserializer.deserializeString(),
      blockedReason: deserializer.deserializeString(),
      unblockWhen: deserializer.deserializeString(),
      clearBlocked: deserializer.deserializeBool(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static UpdateWorkerMetadataSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = UpdateWorkerMetadataSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String issueNumber;
  final String pullRequestNumber;
  final String blockedReason;
  final String unblockWhen;
  final bool clearBlocked;

  UpdateWorkerMetadataSignal copyWith({
    String? issueNumber,
    String? pullRequestNumber,
    String? blockedReason,
    String? unblockWhen,
    bool? clearBlocked,
  }) {
    return UpdateWorkerMetadataSignal(
      issueNumber: issueNumber ?? this.issueNumber,
      pullRequestNumber: pullRequestNumber ?? this.pullRequestNumber,
      blockedReason: blockedReason ?? this.blockedReason,
      unblockWhen: unblockWhen ?? this.unblockWhen,
      clearBlocked: clearBlocked ?? this.clearBlocked,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(issueNumber);
    serializer.serializeString(pullRequestNumber);
    serializer.serializeString(blockedReason);
    serializer.serializeString(unblockWhen);
    serializer.serializeBool(clearBlocked);
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

    return other is UpdateWorkerMetadataSignal
      && issueNumber == other.issueNumber
      && pullRequestNumber == other.pullRequestNumber
      && blockedReason == other.blockedReason
      && unblockWhen == other.unblockWhen
      && clearBlocked == other.clearBlocked;
  }

  @override
  int get hashCode => Object.hash(
        issueNumber,
        pullRequestNumber,
        blockedReason,
        unblockWhen,
        clearBlocked,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'issueNumber: $issueNumber, '
        'pullRequestNumber: $pullRequestNumber, '
        'blockedReason: $blockedReason, '
        'unblockWhen: $unblockWhen, '
        'clearBlocked: $clearBlocked'
        ')';
      return true;
    }());

    return fullString ?? 'UpdateWorkerMetadataSignal';
  }
}

extension UpdateWorkerMetadataSignalDartSignalExt on UpdateWorkerMetadataSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_update_worker_metadata_signal',
      messageBytes,
      binary,
    );
  }
}
