// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeShellRolePresentation {
  const AgentRuntimeShellRolePresentation({
    required this.roleId,
    required this.displayLabel,
    required this.shortLabel,
    required this.tone,
    required this.description,
  });

  static AgentRuntimeShellRolePresentation deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeShellRolePresentation(
      roleId: deserializer.deserializeString(),
      displayLabel: deserializer.deserializeString(),
      shortLabel: deserializer.deserializeString(),
      tone: deserializer.deserializeString(),
      description: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeShellRolePresentation bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeShellRolePresentation.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String roleId;
  final String displayLabel;
  final String shortLabel;
  final String tone;
  final String description;

  AgentRuntimeShellRolePresentation copyWith({
    String? roleId,
    String? displayLabel,
    String? shortLabel,
    String? tone,
    String? description,
  }) {
    return AgentRuntimeShellRolePresentation(
      roleId: roleId ?? this.roleId,
      displayLabel: displayLabel ?? this.displayLabel,
      shortLabel: shortLabel ?? this.shortLabel,
      tone: tone ?? this.tone,
      description: description ?? this.description,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(roleId);
    serializer.serializeString(displayLabel);
    serializer.serializeString(shortLabel);
    serializer.serializeString(tone);
    serializer.serializeString(description);
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

    return other is AgentRuntimeShellRolePresentation
      && roleId == other.roleId
      && displayLabel == other.displayLabel
      && shortLabel == other.shortLabel
      && tone == other.tone
      && description == other.description;
  }

  @override
  int get hashCode => Object.hash(
        roleId,
        displayLabel,
        shortLabel,
        tone,
        description,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'roleId: $roleId, '
        'displayLabel: $displayLabel, '
        'shortLabel: $shortLabel, '
        'tone: $tone, '
        'description: $description'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeShellRolePresentation';
  }
}
