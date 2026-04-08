// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class WarmHandoffSignal {
  const WarmHandoffSignal({
    required this.prompt,
  });

  static WarmHandoffSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = WarmHandoffSignal(
      prompt: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static WarmHandoffSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = WarmHandoffSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String prompt;

  WarmHandoffSignal copyWith({
    String? prompt,
  }) {
    return WarmHandoffSignal(
      prompt: prompt ?? this.prompt,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(prompt);
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

    return other is WarmHandoffSignal
      && prompt == other.prompt;
  }

  @override
  int get hashCode => prompt.hashCode;

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'prompt: $prompt'
        ')';
      return true;
    }());

    return fullString ?? 'WarmHandoffSignal';
  }
}

extension WarmHandoffSignalDartSignalExt on WarmHandoffSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_warm_handoff_signal',
      messageBytes,
      binary,
    );
  }
}
