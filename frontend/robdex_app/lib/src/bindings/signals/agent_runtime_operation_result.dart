// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeOperationResult {
  const AgentRuntimeOperationResult({
    required this.operation,
    required this.outcome,
    required this.message,
    this.valueJson = '',
    this.hasValueJson = false,
  });

  static AgentRuntimeOperationResult deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeOperationResult(
      operation: deserializer.deserializeString(),
      outcome: deserializer.deserializeString(),
      message: deserializer.deserializeString(),
      valueJson: deserializer.deserializeString(),
      hasValueJson: deserializer.deserializeBool(),
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
  final String valueJson;
  final bool hasValueJson;

  AgentRuntimeOperationResult copyWith({
    String? operation,
    String? outcome,
    String? message,
    String? valueJson,
    bool? hasValueJson,
  }) {
    return AgentRuntimeOperationResult(
      operation: operation ?? this.operation,
      outcome: outcome ?? this.outcome,
      message: message ?? this.message,
      valueJson: valueJson ?? this.valueJson,
      hasValueJson: hasValueJson ?? this.hasValueJson,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(operation);
    serializer.serializeString(outcome);
    serializer.serializeString(message);
    serializer.serializeString(valueJson);
    serializer.serializeBool(hasValueJson);
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
      && message == other.message
      && valueJson == other.valueJson
      && hasValueJson == other.hasValueJson;
  }

  @override
  int get hashCode => Object.hash(
        operation,
        outcome,
        message,
        valueJson,
        hasValueJson,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'operation: $operation, '
        'outcome: $outcome, '
        'message: $message, '
        'valueJson: $valueJson, '
        'hasValueJson: $hasValueJson'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeOperationResult';
  }
}
