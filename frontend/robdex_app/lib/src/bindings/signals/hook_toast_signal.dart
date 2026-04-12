// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class HookToastSignal {
  /// An async broadcast stream that listens for signals from Rust.
  /// It supports multiple subscriptions.
  /// Make sure to cancel the subscription when it's no longer needed,
  /// such as when a widget is disposed.
  static final rustSignalStream =
      _hookToastSignalStreamController.stream.asBroadcastStream();
        
  /// The latest signal value received from Rust.
  /// This is updated every time a new signal is received.
  /// It can be null if no signals have been received yet.
  static RustSignalPack<HookToastSignal>? latestRustSignal = null;

  const HookToastSignal({
    required this.message,
    required this.detail,
    required this.copyText,
    required this.durationMs,
  });

  static HookToastSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = HookToastSignal(
      message: deserializer.deserializeString(),
      detail: deserializer.deserializeString(),
      copyText: deserializer.deserializeString(),
      durationMs: deserializer.deserializeUint32(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static HookToastSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = HookToastSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String message;
  final String detail;
  final String copyText;
  final int durationMs;

  HookToastSignal copyWith({
    String? message,
    String? detail,
    String? copyText,
    int? durationMs,
  }) {
    return HookToastSignal(
      message: message ?? this.message,
      detail: detail ?? this.detail,
      copyText: copyText ?? this.copyText,
      durationMs: durationMs ?? this.durationMs,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(message);
    serializer.serializeString(detail);
    serializer.serializeString(copyText);
    serializer.serializeUint32(durationMs);
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

    return other is HookToastSignal
      && message == other.message
      && detail == other.detail
      && copyText == other.copyText
      && durationMs == other.durationMs;
  }

  @override
  int get hashCode => Object.hash(
        message,
        detail,
        copyText,
        durationMs,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'message: $message, '
        'detail: $detail, '
        'copyText: $copyText, '
        'durationMs: $durationMs'
        ')';
      return true;
    }());

    return fullString ?? 'HookToastSignal';
  }
}

final _hookToastSignalStreamController =
    StreamController<RustSignalPack<HookToastSignal>>();
