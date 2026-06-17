// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeCommandRegistryDecisionInput {
  const AgentRuntimeCommandRegistryDecisionInput({
    required this.sessionId,
    required this.status,
    required this.finalScope,
    required this.hasFinalScope,
    required this.finalExecutionPolicy,
    required this.hasFinalExecutionPolicy,
    required this.finalCommand,
    required this.hasFinalCommand,
  });

  static AgentRuntimeCommandRegistryDecisionInput deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeCommandRegistryDecisionInput(
      sessionId: deserializer.deserializeString(),
      status: deserializer.deserializeString(),
      finalScope: AgentRuntimeRegistryScope.deserialize(deserializer),
      hasFinalScope: deserializer.deserializeBool(),
      finalExecutionPolicy: AgentRuntimeFinalExecutionPolicy.deserialize(deserializer),
      hasFinalExecutionPolicy: deserializer.deserializeBool(),
      finalCommand: AgentRuntimeCommandSeed.deserialize(deserializer),
      hasFinalCommand: deserializer.deserializeBool(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeCommandRegistryDecisionInput bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeCommandRegistryDecisionInput.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String sessionId;
  final String status;
  final AgentRuntimeRegistryScope finalScope;
  final bool hasFinalScope;
  final AgentRuntimeFinalExecutionPolicy finalExecutionPolicy;
  final bool hasFinalExecutionPolicy;
  final AgentRuntimeCommandSeed finalCommand;
  final bool hasFinalCommand;

  AgentRuntimeCommandRegistryDecisionInput copyWith({
    String? sessionId,
    String? status,
    AgentRuntimeRegistryScope? finalScope,
    bool? hasFinalScope,
    AgentRuntimeFinalExecutionPolicy? finalExecutionPolicy,
    bool? hasFinalExecutionPolicy,
    AgentRuntimeCommandSeed? finalCommand,
    bool? hasFinalCommand,
  }) {
    return AgentRuntimeCommandRegistryDecisionInput(
      sessionId: sessionId ?? this.sessionId,
      status: status ?? this.status,
      finalScope: finalScope ?? this.finalScope,
      hasFinalScope: hasFinalScope ?? this.hasFinalScope,
      finalExecutionPolicy: finalExecutionPolicy ?? this.finalExecutionPolicy,
      hasFinalExecutionPolicy: hasFinalExecutionPolicy ?? this.hasFinalExecutionPolicy,
      finalCommand: finalCommand ?? this.finalCommand,
      hasFinalCommand: hasFinalCommand ?? this.hasFinalCommand,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(sessionId);
    serializer.serializeString(status);
    finalScope.serialize(serializer);
    serializer.serializeBool(hasFinalScope);
    finalExecutionPolicy.serialize(serializer);
    serializer.serializeBool(hasFinalExecutionPolicy);
    finalCommand.serialize(serializer);
    serializer.serializeBool(hasFinalCommand);
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

    return other is AgentRuntimeCommandRegistryDecisionInput
      && sessionId == other.sessionId
      && status == other.status
      && finalScope == other.finalScope
      && hasFinalScope == other.hasFinalScope
      && finalExecutionPolicy == other.finalExecutionPolicy
      && hasFinalExecutionPolicy == other.hasFinalExecutionPolicy
      && finalCommand == other.finalCommand
      && hasFinalCommand == other.hasFinalCommand;
  }

  @override
  int get hashCode => Object.hash(
        sessionId,
        status,
        finalScope,
        hasFinalScope,
        finalExecutionPolicy,
        hasFinalExecutionPolicy,
        finalCommand,
        hasFinalCommand,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'sessionId: $sessionId, '
        'status: $status, '
        'finalScope: $finalScope, '
        'hasFinalScope: $hasFinalScope, '
        'finalExecutionPolicy: $finalExecutionPolicy, '
        'hasFinalExecutionPolicy: $hasFinalExecutionPolicy, '
        'finalCommand: $finalCommand, '
        'hasFinalCommand: $hasFinalCommand'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeCommandRegistryDecisionInput';
  }
}
