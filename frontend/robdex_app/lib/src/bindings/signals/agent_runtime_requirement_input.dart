// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeRequirementInput {
  const AgentRuntimeRequirementInput({
    required this.key,
    required this.statement,
    required this.severity,
    required this.verificationMethod,
  });

  static AgentRuntimeRequirementInput deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeRequirementInput(
      key: deserializer.deserializeString(),
      statement: deserializer.deserializeString(),
      severity: deserializer.deserializeString(),
      verificationMethod: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeRequirementInput bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeRequirementInput.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String key;
  final String statement;
  final String severity;
  final String verificationMethod;

  AgentRuntimeRequirementInput copyWith({
    String? key,
    String? statement,
    String? severity,
    String? verificationMethod,
  }) {
    return AgentRuntimeRequirementInput(
      key: key ?? this.key,
      statement: statement ?? this.statement,
      severity: severity ?? this.severity,
      verificationMethod: verificationMethod ?? this.verificationMethod,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(key);
    serializer.serializeString(statement);
    serializer.serializeString(severity);
    serializer.serializeString(verificationMethod);
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

    return other is AgentRuntimeRequirementInput
      && key == other.key
      && statement == other.statement
      && severity == other.severity
      && verificationMethod == other.verificationMethod;
  }

  @override
  int get hashCode => Object.hash(
        key,
        statement,
        severity,
        verificationMethod,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'key: $key, '
        'statement: $statement, '
        'severity: $severity, '
        'verificationMethod: $verificationMethod'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeRequirementInput';
  }
}
