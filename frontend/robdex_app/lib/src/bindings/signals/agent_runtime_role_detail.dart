// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeRoleDetail {
  const AgentRuntimeRoleDetail({
    required this.id,
    required this.title,
    required this.displayName,
    required this.version,
    required this.status,
    required this.instructionsPreview,
    required this.modelLabel,
    required this.routingLabel,
    required this.visibilityLabel,
    required this.lifecycleLabel,
    required this.policyRows,
  });

  static AgentRuntimeRoleDetail deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeRoleDetail(
      id: deserializer.deserializeString(),
      title: deserializer.deserializeString(),
      displayName: deserializer.deserializeString(),
      version: deserializer.deserializeString(),
      status: deserializer.deserializeString(),
      instructionsPreview: deserializer.deserializeString(),
      modelLabel: deserializer.deserializeString(),
      routingLabel: deserializer.deserializeString(),
      visibilityLabel: deserializer.deserializeString(),
      lifecycleLabel: deserializer.deserializeString(),
      policyRows: TraitHelpers.deserializeVectorAgentRuntimeRolePolicyRow(deserializer),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeRoleDetail bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeRoleDetail.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String id;
  final String title;
  final String displayName;
  final String version;
  final String status;
  final String instructionsPreview;
  final String modelLabel;
  final String routingLabel;
  final String visibilityLabel;
  final String lifecycleLabel;
  final List<AgentRuntimeRolePolicyRow> policyRows;

  AgentRuntimeRoleDetail copyWith({
    String? id,
    String? title,
    String? displayName,
    String? version,
    String? status,
    String? instructionsPreview,
    String? modelLabel,
    String? routingLabel,
    String? visibilityLabel,
    String? lifecycleLabel,
    List<AgentRuntimeRolePolicyRow>? policyRows,
  }) {
    return AgentRuntimeRoleDetail(
      id: id ?? this.id,
      title: title ?? this.title,
      displayName: displayName ?? this.displayName,
      version: version ?? this.version,
      status: status ?? this.status,
      instructionsPreview: instructionsPreview ?? this.instructionsPreview,
      modelLabel: modelLabel ?? this.modelLabel,
      routingLabel: routingLabel ?? this.routingLabel,
      visibilityLabel: visibilityLabel ?? this.visibilityLabel,
      lifecycleLabel: lifecycleLabel ?? this.lifecycleLabel,
      policyRows: policyRows ?? this.policyRows,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(id);
    serializer.serializeString(title);
    serializer.serializeString(displayName);
    serializer.serializeString(version);
    serializer.serializeString(status);
    serializer.serializeString(instructionsPreview);
    serializer.serializeString(modelLabel);
    serializer.serializeString(routingLabel);
    serializer.serializeString(visibilityLabel);
    serializer.serializeString(lifecycleLabel);
    TraitHelpers.serializeVectorAgentRuntimeRolePolicyRow(policyRows, serializer);
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

    return other is AgentRuntimeRoleDetail
      && id == other.id
      && title == other.title
      && displayName == other.displayName
      && version == other.version
      && status == other.status
      && instructionsPreview == other.instructionsPreview
      && modelLabel == other.modelLabel
      && routingLabel == other.routingLabel
      && visibilityLabel == other.visibilityLabel
      && lifecycleLabel == other.lifecycleLabel
      && listEquals(policyRows, other.policyRows);
  }

  @override
  int get hashCode => Object.hash(
        id,
        title,
        displayName,
        version,
        status,
        instructionsPreview,
        modelLabel,
        routingLabel,
        visibilityLabel,
        lifecycleLabel,
        policyRows,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'id: $id, '
        'title: $title, '
        'displayName: $displayName, '
        'version: $version, '
        'status: $status, '
        'instructionsPreview: $instructionsPreview, '
        'modelLabel: $modelLabel, '
        'routingLabel: $routingLabel, '
        'visibilityLabel: $visibilityLabel, '
        'lifecycleLabel: $lifecycleLabel, '
        'policyRows: $policyRows'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeRoleDetail';
  }
}
