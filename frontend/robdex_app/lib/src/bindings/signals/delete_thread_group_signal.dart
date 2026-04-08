// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class DeleteThreadGroupSignal {
  const DeleteThreadGroupSignal({
    required this.groupId,
  });

  static DeleteThreadGroupSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = DeleteThreadGroupSignal(
      groupId: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static DeleteThreadGroupSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = DeleteThreadGroupSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String groupId;

  DeleteThreadGroupSignal copyWith({
    String? groupId,
  }) {
    return DeleteThreadGroupSignal(
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

    return other is DeleteThreadGroupSignal
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

    return fullString ?? 'DeleteThreadGroupSignal';
  }
}

extension DeleteThreadGroupSignalDartSignalExt on DeleteThreadGroupSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_delete_thread_group_signal',
      messageBytes,
      binary,
    );
  }
}
