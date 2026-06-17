// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeOperationResult {
  const AgentRuntimeOperationResult({
    required this.operation,
    required this.outcome,
    required this.message,
  });

  static AgentRuntimeOperationResult deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeOperationResult(
      operation: deserializer.deserializeString(),
      outcome: deserializer.deserializeString(),
      message: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeOperationResult bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeOperationResult.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String operation;
  final String outcome;
  final String message;

  AgentRuntimeOperationResult copyWith({
    String? operation,
    String? outcome,
    String? message,
  }) {
    return AgentRuntimeOperationResult(
      operation: operation ?? this.operation,
      outcome: outcome ?? this.outcome,
      message: message ?? this.message,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(operation);
    serializer.serializeString(outcome);
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

    return other is AgentRuntimeOperationResult
      && operation == other.operation
      && outcome == other.outcome
      && message == other.message;
  }

  @override
  int get hashCode => Object.hash(
        operation,
        outcome,
        message,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'operation: $operation, '
        'outcome: $outcome, '
        'message: $message'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeOperationResult';
  }
}
