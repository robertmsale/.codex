// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeRoleEditorRoutingMetadata {
  const AgentRuntimeRoleEditorRoutingMetadata({
    required this.mode,
    required this.defaultRecipient,
    required this.hasDefaultRecipient,
    required this.allowedRecipients,
    required this.reservedActions,
  });

  static AgentRuntimeRoleEditorRoutingMetadata deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeRoleEditorRoutingMetadata(
      mode: deserializer.deserializeString(),
      defaultRecipient: deserializer.deserializeString(),
      hasDefaultRecipient: deserializer.deserializeBool(),
      allowedRecipients: TraitHelpers.deserializeVectorStr(deserializer),
      reservedActions: TraitHelpers.deserializeVectorStr(deserializer),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeRoleEditorRoutingMetadata bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeRoleEditorRoutingMetadata.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String mode;
  final String defaultRecipient;
  final bool hasDefaultRecipient;
  final List<String> allowedRecipients;
  final List<String> reservedActions;

  AgentRuntimeRoleEditorRoutingMetadata copyWith({
    String? mode,
    String? defaultRecipient,
    bool? hasDefaultRecipient,
    List<String>? allowedRecipients,
    List<String>? reservedActions,
  }) {
    return AgentRuntimeRoleEditorRoutingMetadata(
      mode: mode ?? this.mode,
      defaultRecipient: defaultRecipient ?? this.defaultRecipient,
      hasDefaultRecipient: hasDefaultRecipient ?? this.hasDefaultRecipient,
      allowedRecipients: allowedRecipients ?? this.allowedRecipients,
      reservedActions: reservedActions ?? this.reservedActions,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(mode);
    serializer.serializeString(defaultRecipient);
    serializer.serializeBool(hasDefaultRecipient);
    TraitHelpers.serializeVectorStr(allowedRecipients, serializer);
    TraitHelpers.serializeVectorStr(reservedActions, serializer);
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

    return other is AgentRuntimeRoleEditorRoutingMetadata
      && mode == other.mode
      && defaultRecipient == other.defaultRecipient
      && hasDefaultRecipient == other.hasDefaultRecipient
      && listEquals(allowedRecipients, other.allowedRecipients)
      && listEquals(reservedActions, other.reservedActions);
  }

  @override
  int get hashCode => Object.hash(
        mode,
        defaultRecipient,
        hasDefaultRecipient,
        allowedRecipients,
        reservedActions,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'mode: $mode, '
        'defaultRecipient: $defaultRecipient, '
        'hasDefaultRecipient: $hasDefaultRecipient, '
        'allowedRecipients: $allowedRecipients, '
        'reservedActions: $reservedActions'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeRoleEditorRoutingMetadata';
  }
}
