// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class UploadImageBytesSignal {
  const UploadImageBytesSignal({
    required this.requestId,
    required this.filename,
    required this.contentType,
    required this.bytes,
  });

  static UploadImageBytesSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = UploadImageBytesSignal(
      requestId: deserializer.deserializeString(),
      filename: deserializer.deserializeString(),
      contentType: deserializer.deserializeString(),
      bytes: TraitHelpers.deserializeVectorU8(deserializer),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static UploadImageBytesSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = UploadImageBytesSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String requestId;
  final String filename;
  final String contentType;
  final List<int> bytes;

  UploadImageBytesSignal copyWith({
    String? requestId,
    String? filename,
    String? contentType,
    List<int>? bytes,
  }) {
    return UploadImageBytesSignal(
      requestId: requestId ?? this.requestId,
      filename: filename ?? this.filename,
      contentType: contentType ?? this.contentType,
      bytes: bytes ?? this.bytes,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(requestId);
    serializer.serializeString(filename);
    serializer.serializeString(contentType);
    TraitHelpers.serializeVectorU8(bytes, serializer);
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

    return other is UploadImageBytesSignal
      && requestId == other.requestId
      && filename == other.filename
      && contentType == other.contentType
      && listEquals(bytes, other.bytes);
  }

  @override
  int get hashCode => Object.hash(
        requestId,
        filename,
        contentType,
        bytes,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'requestId: $requestId, '
        'filename: $filename, '
        'contentType: $contentType, '
        'bytes: $bytes'
        ')';
      return true;
    }());

    return fullString ?? 'UploadImageBytesSignal';
  }
}

extension UploadImageBytesSignalDartSignalExt on UploadImageBytesSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_upload_image_bytes_signal',
      messageBytes,
      binary,
    );
  }
}
