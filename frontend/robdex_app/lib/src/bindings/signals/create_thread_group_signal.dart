// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class CreateThreadGroupSignal {
  const CreateThreadGroupSignal({
    required this.title,
  });

  static CreateThreadGroupSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = CreateThreadGroupSignal(
      title: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static CreateThreadGroupSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = CreateThreadGroupSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String title;

  CreateThreadGroupSignal copyWith({
    String? title,
  }) {
    return CreateThreadGroupSignal(
      title: title ?? this.title,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
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

    return other is CreateThreadGroupSignal
      && title == other.title;
  }

  @override
  int get hashCode => title.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'title: $title'
        ')';
      return true;
    }());

    return fullString ?? 'CreateThreadGroupSignal';
  }
}

extension CreateThreadGroupSignalDartSignalExt on CreateThreadGroupSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_create_thread_group_signal',
      messageBytes,
      binary,
    );
  }
}
