// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class LoadImageBytesSignal {
  const LoadImageBytesSignal({
    required this.requestId,
    required this.path,
  });

  static LoadImageBytesSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = LoadImageBytesSignal(
      requestId: deserializer.deserializeString(),
      path: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static LoadImageBytesSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = LoadImageBytesSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String requestId;
  final String path;

  LoadImageBytesSignal copyWith({
    String? requestId,
    String? path,
  }) {
    return LoadImageBytesSignal(
      requestId: requestId ?? this.requestId,
      path: path ?? this.path,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(requestId);
    serializer.serializeString(path);
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

    return other is LoadImageBytesSignal
      && requestId == other.requestId
      && path == other.path;
  }

  @override
  int get hashCode => Object.hash(
        requestId,
        path,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'requestId: $requestId, '
        'path: $path'
        ')';
      return true;
    }());

    return fullString ?? 'LoadImageBytesSignal';
  }
}

extension LoadImageBytesSignalDartSignalExt on LoadImageBytesSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_load_image_bytes_signal',
      messageBytes,
      binary,
    );
  }
}
