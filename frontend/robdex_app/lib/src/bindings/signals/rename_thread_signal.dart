// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class RenameThreadSignal {
  const RenameThreadSignal({
    required this.name,
  });

  static RenameThreadSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = RenameThreadSignal(
      name: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static RenameThreadSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = RenameThreadSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String name;

  RenameThreadSignal copyWith({
    String? name,
  }) {
    return RenameThreadSignal(
      name: name ?? this.name,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(name);
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

    return other is RenameThreadSignal
      && name == other.name;
  }

  @override
  int get hashCode => name.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'name: $name'
        ')';
      return true;
    }());

    return fullString ?? 'RenameThreadSignal';
  }
}

extension RenameThreadSignalDartSignalExt on RenameThreadSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_rename_thread_signal',
      messageBytes,
      binary,
    );
  }
}
