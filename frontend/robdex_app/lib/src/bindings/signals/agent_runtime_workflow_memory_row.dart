// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeWorkflowMemoryRow {
  const AgentRuntimeWorkflowMemoryRow({
    required this.id,
    required this.title,
    required this.scopeLabel,
    required this.projectKey,
    required this.hasProjectKey,
    required this.helpfulScore,
    required this.promotedAt,
    required this.hasPromotedAt,
    required this.sourceSessionId,
    required this.reason,
    required this.tone,
    required this.isSelected,
  });

  static AgentRuntimeWorkflowMemoryRow deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeWorkflowMemoryRow(
      id: deserializer.deserializeString(),
      title: deserializer.deserializeString(),
      scopeLabel: deserializer.deserializeString(),
      projectKey: deserializer.deserializeString(),
      hasProjectKey: deserializer.deserializeBool(),
      helpfulScore: deserializer.deserializeString(),
      promotedAt: deserializer.deserializeString(),
      hasPromotedAt: deserializer.deserializeBool(),
      sourceSessionId: deserializer.deserializeString(),
      reason: deserializer.deserializeString(),
      tone: deserializer.deserializeString(),
      isSelected: deserializer.deserializeBool(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeWorkflowMemoryRow bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeWorkflowMemoryRow.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String id;
  final String title;
  final String scopeLabel;
  final String projectKey;
  final bool hasProjectKey;
  final String helpfulScore;
  final String promotedAt;
  final bool hasPromotedAt;
  final String sourceSessionId;
  final String reason;
  final String tone;
  final bool isSelected;

  AgentRuntimeWorkflowMemoryRow copyWith({
    String? id,
    String? title,
    String? scopeLabel,
    String? projectKey,
    bool? hasProjectKey,
    String? helpfulScore,
    String? promotedAt,
    bool? hasPromotedAt,
    String? sourceSessionId,
    String? reason,
    String? tone,
    bool? isSelected,
  }) {
    return AgentRuntimeWorkflowMemoryRow(
      id: id ?? this.id,
      title: title ?? this.title,
      scopeLabel: scopeLabel ?? this.scopeLabel,
      projectKey: projectKey ?? this.projectKey,
      hasProjectKey: hasProjectKey ?? this.hasProjectKey,
      helpfulScore: helpfulScore ?? this.helpfulScore,
      promotedAt: promotedAt ?? this.promotedAt,
      hasPromotedAt: hasPromotedAt ?? this.hasPromotedAt,
      sourceSessionId: sourceSessionId ?? this.sourceSessionId,
      reason: reason ?? this.reason,
      tone: tone ?? this.tone,
      isSelected: isSelected ?? this.isSelected,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(id);
    serializer.serializeString(title);
    serializer.serializeString(scopeLabel);
    serializer.serializeString(projectKey);
    serializer.serializeBool(hasProjectKey);
    serializer.serializeString(helpfulScore);
    serializer.serializeString(promotedAt);
    serializer.serializeBool(hasPromotedAt);
    serializer.serializeString(sourceSessionId);
    serializer.serializeString(reason);
    serializer.serializeString(tone);
    serializer.serializeBool(isSelected);
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

    return other is AgentRuntimeWorkflowMemoryRow
      && id == other.id
      && title == other.title
      && scopeLabel == other.scopeLabel
      && projectKey == other.projectKey
      && hasProjectKey == other.hasProjectKey
      && helpfulScore == other.helpfulScore
      && promotedAt == other.promotedAt
      && hasPromotedAt == other.hasPromotedAt
      && sourceSessionId == other.sourceSessionId
      && reason == other.reason
      && tone == other.tone
      && isSelected == other.isSelected;
  }

  @override
  int get hashCode => Object.hash(
        id,
        title,
        scopeLabel,
        projectKey,
        hasProjectKey,
        helpfulScore,
        promotedAt,
        hasPromotedAt,
        sourceSessionId,
        reason,
        tone,
        isSelected,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'id: $id, '
        'title: $title, '
        'scopeLabel: $scopeLabel, '
        'projectKey: $projectKey, '
        'hasProjectKey: $hasProjectKey, '
        'helpfulScore: $helpfulScore, '
        'promotedAt: $promotedAt, '
        'hasPromotedAt: $hasPromotedAt, '
        'sourceSessionId: $sourceSessionId, '
        'reason: $reason, '
        'tone: $tone, '
        'isSelected: $isSelected'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeWorkflowMemoryRow';
  }
}
