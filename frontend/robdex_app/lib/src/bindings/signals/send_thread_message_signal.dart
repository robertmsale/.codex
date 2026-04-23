// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class SendThreadMessageSignal {
  const SendThreadMessageSignal({
    required this.text,
    required this.localImagePaths,
  });

  static SendThreadMessageSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final text = deserializer.deserializeString();
    final localImagePaths = <String>[];
    final localImagePathsLength = deserializer.deserializeLength();
    for (var i = 0; i < localImagePathsLength; i++) {
      localImagePaths.add(deserializer.deserializeString());
    }
    final instance = SendThreadMessageSignal(
      text: text,
      localImagePaths: localImagePaths,
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static SendThreadMessageSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = SendThreadMessageSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String text;
  final List<String> localImagePaths;

  SendThreadMessageSignal copyWith({
    String? text,
    List<String>? localImagePaths,
  }) {
    return SendThreadMessageSignal(
      text: text ?? this.text,
      localImagePaths: localImagePaths ?? this.localImagePaths,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(text);
    serializer.serializeLength(localImagePaths.length);
    for (final item in localImagePaths) {
      serializer.serializeString(item);
    }
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

    return other is SendThreadMessageSignal
      && text == other.text
      && _sameStringList(localImagePaths, other.localImagePaths);
  }

  @override
  int get hashCode => Object.hash(text, Object.hashAll(localImagePaths));

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'text: $text, '
        'localImagePaths: $localImagePaths'
        ')';
      return true;
    }());

    return fullString ?? 'SendThreadMessageSignal';
  }
}

bool _sameStringList(List<String> a, List<String> b) {
  if (a.length != b.length) {
    return false;
  }
  for (var i = 0; i < a.length; i++) {
    if (a[i] != b[i]) {
      return false;
    }
  }
  return true;
}

extension SendThreadMessageSignalDartSignalExt on SendThreadMessageSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_send_thread_message_signal',
      messageBytes,
      binary,
    );
  }
}
