// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class RenameThreadGroupSignal {
  const RenameThreadGroupSignal({
    required this.groupId,
    required this.title,
  });

  static RenameThreadGroupSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = RenameThreadGroupSignal(
      groupId: deserializer.deserializeString(),
      title: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static RenameThreadGroupSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = RenameThreadGroupSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String groupId;
  final String title;

  RenameThreadGroupSignal copyWith({
    String? groupId,
    String? title,
  }) {
    return RenameThreadGroupSignal(
      groupId: groupId ?? this.groupId,
      title: title ?? this.title,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(groupId);
    serializer.serializeString(title);
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

    return other is RenameThreadGroupSignal
      && groupId == other.groupId
      && title == other.title;
  }

  @override
  int get hashCode => Object.hash(
        groupId,
        title,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'groupId: $groupId, '
        'title: $title'
        ')';
      return true;
    }());

    return fullString ?? 'RenameThreadGroupSignal';
  }
}

extension RenameThreadGroupSignalDartSignalExt on RenameThreadGroupSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_rename_thread_group_signal',
      messageBytes,
      binary,
    );
  }
}
