// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeWorkflowMemoryDetail {
  const AgentRuntimeWorkflowMemoryDetail({
    required this.id,
    required this.title,
    required this.reason,
    required this.summary,
    required this.sourceSessionId,
    required this.sourceScriptRunId,
    required this.hasSourceScriptRunId,
    required this.sourcePreview,
    required this.provider,
    required this.model,
    required this.dimensions,
    required this.storageLabel,
    required this.sourceHash,
    required this.commandFingerprint,
    required this.score,
    required this.scopeLabel,
    required this.feedbackEnabled,
    required this.feedbackSessionId,
    required this.hasFeedbackSessionId,
    required this.events,
  });

  static AgentRuntimeWorkflowMemoryDetail deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeWorkflowMemoryDetail(
      id: deserializer.deserializeString(),
      title: deserializer.deserializeString(),
      reason: deserializer.deserializeString(),
      summary: deserializer.deserializeString(),
      sourceSessionId: deserializer.deserializeString(),
      sourceScriptRunId: deserializer.deserializeString(),
      hasSourceScriptRunId: deserializer.deserializeBool(),
      sourcePreview: deserializer.deserializeString(),
      provider: deserializer.deserializeString(),
      model: deserializer.deserializeString(),
      dimensions: deserializer.deserializeString(),
      storageLabel: deserializer.deserializeString(),
      sourceHash: deserializer.deserializeString(),
      commandFingerprint: deserializer.deserializeString(),
      score: deserializer.deserializeString(),
      scopeLabel: deserializer.deserializeString(),
      feedbackEnabled: deserializer.deserializeBool(),
      feedbackSessionId: deserializer.deserializeString(),
      hasFeedbackSessionId: deserializer.deserializeBool(),
      events: TraitHelpers.deserializeVectorAgentRuntimeWorkflowMemoryEvent(deserializer),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeWorkflowMemoryDetail bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeWorkflowMemoryDetail.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String id;
  final String title;
  final String reason;
  final String summary;
  final String sourceSessionId;
  final String sourceScriptRunId;
  final bool hasSourceScriptRunId;
  final String sourcePreview;
  final String provider;
  final String model;
  final String dimensions;
  final String storageLabel;
  final String sourceHash;
  final String commandFingerprint;
  final String score;
  final String scopeLabel;
  final bool feedbackEnabled;
  final String feedbackSessionId;
  final bool hasFeedbackSessionId;
  final List<AgentRuntimeWorkflowMemoryEvent> events;

  AgentRuntimeWorkflowMemoryDetail copyWith({
    String? id,
    String? title,
    String? reason,
    String? summary,
    String? sourceSessionId,
    String? sourceScriptRunId,
    bool? hasSourceScriptRunId,
    String? sourcePreview,
    String? provider,
    String? model,
    String? dimensions,
    String? storageLabel,
    String? sourceHash,
    String? commandFingerprint,
    String? score,
    String? scopeLabel,
    bool? feedbackEnabled,
    String? feedbackSessionId,
    bool? hasFeedbackSessionId,
    List<AgentRuntimeWorkflowMemoryEvent>? events,
  }) {
    return AgentRuntimeWorkflowMemoryDetail(
      id: id ?? this.id,
      title: title ?? this.title,
      reason: reason ?? this.reason,
      summary: summary ?? this.summary,
      sourceSessionId: sourceSessionId ?? this.sourceSessionId,
      sourceScriptRunId: sourceScriptRunId ?? this.sourceScriptRunId,
      hasSourceScriptRunId: hasSourceScriptRunId ?? this.hasSourceScriptRunId,
      sourcePreview: sourcePreview ?? this.sourcePreview,
      provider: provider ?? this.provider,
      model: model ?? this.model,
      dimensions: dimensions ?? this.dimensions,
      storageLabel: storageLabel ?? this.storageLabel,
      sourceHash: sourceHash ?? this.sourceHash,
      commandFingerprint: commandFingerprint ?? this.commandFingerprint,
      score: score ?? this.score,
      scopeLabel: scopeLabel ?? this.scopeLabel,
      feedbackEnabled: feedbackEnabled ?? this.feedbackEnabled,
      feedbackSessionId: feedbackSessionId ?? this.feedbackSessionId,
      hasFeedbackSessionId: hasFeedbackSessionId ?? this.hasFeedbackSessionId,
      events: events ?? this.events,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(id);
    serializer.serializeString(title);
    serializer.serializeString(reason);
    serializer.serializeString(summary);
    serializer.serializeString(sourceSessionId);
    serializer.serializeString(sourceScriptRunId);
    serializer.serializeBool(hasSourceScriptRunId);
    serializer.serializeString(sourcePreview);
    serializer.serializeString(provider);
    serializer.serializeString(model);
    serializer.serializeString(dimensions);
    serializer.serializeString(storageLabel);
    serializer.serializeString(sourceHash);
    serializer.serializeString(commandFingerprint);
    serializer.serializeString(score);
    serializer.serializeString(scopeLabel);
    serializer.serializeBool(feedbackEnabled);
    serializer.serializeString(feedbackSessionId);
    serializer.serializeBool(hasFeedbackSessionId);
    TraitHelpers.serializeVectorAgentRuntimeWorkflowMemoryEvent(events, serializer);
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

    return other is AgentRuntimeWorkflowMemoryDetail
      && id == other.id
      && title == other.title
      && reason == other.reason
      && summary == other.summary
      && sourceSessionId == other.sourceSessionId
      && sourceScriptRunId == other.sourceScriptRunId
      && hasSourceScriptRunId == other.hasSourceScriptRunId
      && sourcePreview == other.sourcePreview
      && provider == other.provider
      && model == other.model
      && dimensions == other.dimensions
      && storageLabel == other.storageLabel
      && sourceHash == other.sourceHash
      && commandFingerprint == other.commandFingerprint
      && score == other.score
      && scopeLabel == other.scopeLabel
      && feedbackEnabled == other.feedbackEnabled
      && feedbackSessionId == other.feedbackSessionId
      && hasFeedbackSessionId == other.hasFeedbackSessionId
      && listEquals(events, other.events);
  }

  @override
  int get hashCode => Object.hash(
        id,
        title,
        reason,
        summary,
        sourceSessionId,
        sourceScriptRunId,
        hasSourceScriptRunId,
        sourcePreview,
        provider,
        model,
        dimensions,
        storageLabel,
        sourceHash,
        commandFingerprint,
        score,
        scopeLabel,
        feedbackEnabled,
        feedbackSessionId,
        hasFeedbackSessionId,
        events,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'id: $id, '
        'title: $title, '
        'reason: $reason, '
        'summary: $summary, '
        'sourceSessionId: $sourceSessionId, '
        'sourceScriptRunId: $sourceScriptRunId, '
        'hasSourceScriptRunId: $hasSourceScriptRunId, '
        'sourcePreview: $sourcePreview, '
        'provider: $provider, '
        'model: $model, '
        'dimensions: $dimensions, '
        'storageLabel: $storageLabel, '
        'sourceHash: $sourceHash, '
        'commandFingerprint: $commandFingerprint, '
        'score: $score, '
        'scopeLabel: $scopeLabel, '
        'feedbackEnabled: $feedbackEnabled, '
        'feedbackSessionId: $feedbackSessionId, '
        'hasFeedbackSessionId: $hasFeedbackSessionId, '
        'events: $events'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeWorkflowMemoryDetail';
  }
}
