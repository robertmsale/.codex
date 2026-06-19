// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeRoleEditorOptionsView {
  const AgentRuntimeRoleEditorOptionsView({
    required this.models,
    required this.reasoningEfforts,
    required this.capabilities,
    required this.policyActions,
    required this.policyDecisions,
    required this.routingModes,
    required this.recipients,
    required this.reservedActions,
  });

  static AgentRuntimeRoleEditorOptionsView deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeRoleEditorOptionsView(
      models: TraitHelpers.deserializeVectorStr(deserializer),
      reasoningEfforts: TraitHelpers.deserializeVectorStr(deserializer),
      capabilities: TraitHelpers.deserializeVectorStr(deserializer),
      policyActions: TraitHelpers.deserializeVectorStr(deserializer),
      policyDecisions: TraitHelpers.deserializeVectorStr(deserializer),
      routingModes: TraitHelpers.deserializeVectorStr(deserializer),
      recipients: TraitHelpers.deserializeVectorStr(deserializer),
      reservedActions: TraitHelpers.deserializeVectorStr(deserializer),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeRoleEditorOptionsView bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeRoleEditorOptionsView.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final List<String> models;
  final List<String> reasoningEfforts;
  final List<String> capabilities;
  final List<String> policyActions;
  final List<String> policyDecisions;
  final List<String> routingModes;
  final List<String> recipients;
  final List<String> reservedActions;

  AgentRuntimeRoleEditorOptionsView copyWith({
    List<String>? models,
    List<String>? reasoningEfforts,
    List<String>? capabilities,
    List<String>? policyActions,
    List<String>? policyDecisions,
    List<String>? routingModes,
    List<String>? recipients,
    List<String>? reservedActions,
  }) {
    return AgentRuntimeRoleEditorOptionsView(
      models: models ?? this.models,
      reasoningEfforts: reasoningEfforts ?? this.reasoningEfforts,
      capabilities: capabilities ?? this.capabilities,
      policyActions: policyActions ?? this.policyActions,
      policyDecisions: policyDecisions ?? this.policyDecisions,
      routingModes: routingModes ?? this.routingModes,
      recipients: recipients ?? this.recipients,
      reservedActions: reservedActions ?? this.reservedActions,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    TraitHelpers.serializeVectorStr(models, serializer);
    TraitHelpers.serializeVectorStr(reasoningEfforts, serializer);
    TraitHelpers.serializeVectorStr(capabilities, serializer);
    TraitHelpers.serializeVectorStr(policyActions, serializer);
    TraitHelpers.serializeVectorStr(policyDecisions, serializer);
    TraitHelpers.serializeVectorStr(routingModes, serializer);
    TraitHelpers.serializeVectorStr(recipients, serializer);
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

    return other is AgentRuntimeRoleEditorOptionsView
      && listEquals(models, other.models)
      && listEquals(reasoningEfforts, other.reasoningEfforts)
      && listEquals(capabilities, other.capabilities)
      && listEquals(policyActions, other.policyActions)
      && listEquals(policyDecisions, other.policyDecisions)
      && listEquals(routingModes, other.routingModes)
      && listEquals(recipients, other.recipients)
      && listEquals(reservedActions, other.reservedActions);
  }

  @override
  int get hashCode => Object.hash(
        models,
        reasoningEfforts,
        capabilities,
        policyActions,
        policyDecisions,
        routingModes,
        recipients,
        reservedActions,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'models: $models, '
        'reasoningEfforts: $reasoningEfforts, '
        'capabilities: $capabilities, '
        'policyActions: $policyActions, '
        'policyDecisions: $policyDecisions, '
        'routingModes: $routingModes, '
        'recipients: $recipients, '
        'reservedActions: $reservedActions'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeRoleEditorOptionsView';
  }
}
