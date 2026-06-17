// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeWorkflowMemoryView {
  const AgentRuntimeWorkflowMemoryView({
    required this.title,
    required this.subtitle,
    required this.emptyTitle,
    required this.emptyText,
    required this.rows,
    required this.selectedDetail,
    required this.hasSelectedDetail,
    required this.actionStates,
  });

  static AgentRuntimeWorkflowMemoryView deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeWorkflowMemoryView(
      title: deserializer.deserializeString(),
      subtitle: deserializer.deserializeString(),
      emptyTitle: deserializer.deserializeString(),
      emptyText: deserializer.deserializeString(),
      rows: TraitHelpers.deserializeVectorAgentRuntimeWorkflowMemoryRow(deserializer),
      selectedDetail: AgentRuntimeWorkflowMemoryDetail.deserialize(deserializer),
      hasSelectedDetail: deserializer.deserializeBool(),
      actionStates: TraitHelpers.deserializeVectorAgentRuntimeActionRow(deserializer),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeWorkflowMemoryView bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeWorkflowMemoryView.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String title;
  final String subtitle;
  final String emptyTitle;
  final String emptyText;
  final List<AgentRuntimeWorkflowMemoryRow> rows;
  final AgentRuntimeWorkflowMemoryDetail selectedDetail;
  final bool hasSelectedDetail;
  final List<AgentRuntimeActionRow> actionStates;

  AgentRuntimeWorkflowMemoryView copyWith({
    String? title,
    String? subtitle,
    String? emptyTitle,
    String? emptyText,
    List<AgentRuntimeWorkflowMemoryRow>? rows,
    AgentRuntimeWorkflowMemoryDetail? selectedDetail,
    bool? hasSelectedDetail,
    List<AgentRuntimeActionRow>? actionStates,
  }) {
    return AgentRuntimeWorkflowMemoryView(
      title: title ?? this.title,
      subtitle: subtitle ?? this.subtitle,
      emptyTitle: emptyTitle ?? this.emptyTitle,
      emptyText: emptyText ?? this.emptyText,
      rows: rows ?? this.rows,
      selectedDetail: selectedDetail ?? this.selectedDetail,
      hasSelectedDetail: hasSelectedDetail ?? this.hasSelectedDetail,
      actionStates: actionStates ?? this.actionStates,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(title);
    serializer.serializeString(subtitle);
    serializer.serializeString(emptyTitle);
    serializer.serializeString(emptyText);
    TraitHelpers.serializeVectorAgentRuntimeWorkflowMemoryRow(rows, serializer);
    selectedDetail.serialize(serializer);
    serializer.serializeBool(hasSelectedDetail);
    TraitHelpers.serializeVectorAgentRuntimeActionRow(actionStates, serializer);
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

    return other is AgentRuntimeWorkflowMemoryView
      && title == other.title
      && subtitle == other.subtitle
      && emptyTitle == other.emptyTitle
      && emptyText == other.emptyText
      && listEquals(rows, other.rows)
      && selectedDetail == other.selectedDetail
      && hasSelectedDetail == other.hasSelectedDetail
      && listEquals(actionStates, other.actionStates);
  }

  @override
  int get hashCode => Object.hash(
        title,
        subtitle,
        emptyTitle,
        emptyText,
        rows,
        selectedDetail,
        hasSelectedDetail,
        actionStates,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'title: $title, '
        'subtitle: $subtitle, '
        'emptyTitle: $emptyTitle, '
        'emptyText: $emptyText, '
        'rows: $rows, '
        'selectedDetail: $selectedDetail, '
        'hasSelectedDetail: $hasSelectedDetail, '
        'actionStates: $actionStates'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeWorkflowMemoryView';
  }
}
