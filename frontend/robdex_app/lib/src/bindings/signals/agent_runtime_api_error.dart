// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeApiError {
  const AgentRuntimeApiError({
    required this.code,
    required this.message,
    required this.details,
  });

  static AgentRuntimeApiError deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeApiError(
      code: deserializer.deserializeString(),
      message: deserializer.deserializeString(),
      details: TraitHelpers.deserializeVectorAgentRuntimeFact(deserializer),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeApiError bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeApiError.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String code;
  final String message;
  final List<AgentRuntimeFact> details;

  AgentRuntimeApiError copyWith({
    String? code,
    String? message,
    List<AgentRuntimeFact>? details,
  }) {
    return AgentRuntimeApiError(
      code: code ?? this.code,
      message: message ?? this.message,
      details: details ?? this.details,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(code);
    serializer.serializeString(message);
    TraitHelpers.serializeVectorAgentRuntimeFact(details, serializer);
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

    return other is AgentRuntimeApiError
      && code == other.code
      && message == other.message
      && listEquals(details, other.details);
  }

  @override
  int get hashCode => Object.hash(
        code,
        message,
        details,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'code: $code, '
        'message: $message, '
        'details: $details'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeApiError';
  }
}
