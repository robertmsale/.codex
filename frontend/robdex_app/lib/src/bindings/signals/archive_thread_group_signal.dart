// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class ArchiveThreadGroupSignal {
  const ArchiveThreadGroupSignal({
    required this.groupId,
  });

  static ArchiveThreadGroupSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = ArchiveThreadGroupSignal(
      groupId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static ArchiveThreadGroupSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = ArchiveThreadGroupSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String groupId;

  ArchiveThreadGroupSignal copyWith({
    String? groupId,
  }) {
    return ArchiveThreadGroupSignal(
      groupId: groupId ?? this.groupId,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(groupId);
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

    return other is ArchiveThreadGroupSignal
      && groupId == other.groupId;
  }

  @override
  int get hashCode => groupId.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'groupId: $groupId'
        ')';
      return true;
    }());

    return fullString ?? 'ArchiveThreadGroupSignal';
  }
}

extension ArchiveThreadGroupSignalDartSignalExt on ArchiveThreadGroupSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_archive_thread_group_signal',
      messageBytes,
      binary,
    );
  }
}
