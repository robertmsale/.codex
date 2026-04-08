// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class DecideApprovalSignal {
  const DecideApprovalSignal({
    required this.approvalId,
    required this.decision,
    required this.message,
  });

  static DecideApprovalSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = DecideApprovalSignal(
      approvalId: deserializer.deserializeString(),
      decision: deserializer.deserializeString(),
      message: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static DecideApprovalSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = DecideApprovalSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String approvalId;
  final String decision;
  final String message;

  DecideApprovalSignal copyWith({
    String? approvalId,
    String? decision,
    String? message,
  }) {
    return DecideApprovalSignal(
      approvalId: approvalId ?? this.approvalId,
      decision: decision ?? this.decision,
      message: message ?? this.message,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(approvalId);
    serializer.serializeString(decision);
    serializer.serializeString(message);
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

    return other is DecideApprovalSignal
      && approvalId == other.approvalId
      && decision == other.decision
      && message == other.message;
  }

  @override
  int get hashCode => Object.hash(
        approvalId,
        decision,
        message,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'approvalId: $approvalId, '
        'decision: $decision, '
        'message: $message'
        ')';
      return true;
    }());

    return fullString ?? 'DecideApprovalSignal';
  }
}

extension DecideApprovalSignalDartSignalExt on DecideApprovalSignal {
  /// Sends the signal to Rust.
  /// Passing data from Rust to Dart involves a memory copy
  /// because Rust cannot own data managed by Dart's garbage collector.
  void sendSignalToRust() {
    final messageBytes = bincodeSerialize();
    final binary = Uint8List(0);
    sendDartSignal(
      'rinf_send_dart_signal_decide_approval_signal',
      messageBytes,
      binary,
    );
  }
}
