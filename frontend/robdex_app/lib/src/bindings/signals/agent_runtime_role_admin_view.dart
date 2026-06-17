// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeRoleAdminView {
  const AgentRuntimeRoleAdminView({
    required this.title,
    required this.subtitle,
    required this.emptyTitle,
    required this.emptyText,
    required this.rows,
    required this.selectedDetail,
    required this.hasSelectedDetail,
    required this.versionRows,
    required this.editorDraft,
    required this.hasEditorDraft,
    required this.validationErrors,
    required this.actionStates,
  });

  static AgentRuntimeRoleAdminView deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeRoleAdminView(
      title: deserializer.deserializeString(),
      subtitle: deserializer.deserializeString(),
      emptyTitle: deserializer.deserializeString(),
      emptyText: deserializer.deserializeString(),
      rows: TraitHelpers.deserializeVectorAgentRuntimeRoleRow(deserializer),
      selectedDetail: AgentRuntimeRoleDetail.deserialize(deserializer),
      hasSelectedDetail: deserializer.deserializeBool(),
      versionRows: TraitHelpers.deserializeVectorAgentRuntimeRoleVersionRow(deserializer),
      editorDraft: AgentRuntimeRoleEditorDraftView.deserialize(deserializer),
      hasEditorDraft: deserializer.deserializeBool(),
      validationErrors: TraitHelpers.deserializeVectorStr(deserializer),
      actionStates: TraitHelpers.deserializeVectorAgentRuntimeActionRow(deserializer),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeRoleAdminView bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeRoleAdminView.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String title;
  final String subtitle;
  final String emptyTitle;
  final String emptyText;
  final List<AgentRuntimeRoleRow> rows;
  final AgentRuntimeRoleDetail selectedDetail;
  final bool hasSelectedDetail;
  final List<AgentRuntimeRoleVersionRow> versionRows;
  final AgentRuntimeRoleEditorDraftView editorDraft;
  final bool hasEditorDraft;
  final List<String> validationErrors;
  final List<AgentRuntimeActionRow> actionStates;

  AgentRuntimeRoleAdminView copyWith({
    String? title,
    String? subtitle,
    String? emptyTitle,
    String? emptyText,
    List<AgentRuntimeRoleRow>? rows,
    AgentRuntimeRoleDetail? selectedDetail,
    bool? hasSelectedDetail,
    List<AgentRuntimeRoleVersionRow>? versionRows,
    AgentRuntimeRoleEditorDraftView? editorDraft,
    bool? hasEditorDraft,
    List<String>? validationErrors,
    List<AgentRuntimeActionRow>? actionStates,
  }) {
    return AgentRuntimeRoleAdminView(
      title: title ?? this.title,
      subtitle: subtitle ?? this.subtitle,
      emptyTitle: emptyTitle ?? this.emptyTitle,
      emptyText: emptyText ?? this.emptyText,
      rows: rows ?? this.rows,
      selectedDetail: selectedDetail ?? this.selectedDetail,
      hasSelectedDetail: hasSelectedDetail ?? this.hasSelectedDetail,
      versionRows: versionRows ?? this.versionRows,
      editorDraft: editorDraft ?? this.editorDraft,
      hasEditorDraft: hasEditorDraft ?? this.hasEditorDraft,
      validationErrors: validationErrors ?? this.validationErrors,
      actionStates: actionStates ?? this.actionStates,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(title);
    serializer.serializeString(subtitle);
    serializer.serializeString(emptyTitle);
    serializer.serializeString(emptyText);
    TraitHelpers.serializeVectorAgentRuntimeRoleRow(rows, serializer);
    selectedDetail.serialize(serializer);
    serializer.serializeBool(hasSelectedDetail);
    TraitHelpers.serializeVectorAgentRuntimeRoleVersionRow(versionRows, serializer);
    editorDraft.serialize(serializer);
    serializer.serializeBool(hasEditorDraft);
    TraitHelpers.serializeVectorStr(validationErrors, serializer);
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

    return other is AgentRuntimeRoleAdminView
      && title == other.title
      && subtitle == other.subtitle
      && emptyTitle == other.emptyTitle
      && emptyText == other.emptyText
      && listEquals(rows, other.rows)
      && selectedDetail == other.selectedDetail
      && hasSelectedDetail == other.hasSelectedDetail
      && listEquals(versionRows, other.versionRows)
      && editorDraft == other.editorDraft
      && hasEditorDraft == other.hasEditorDraft
      && listEquals(validationErrors, other.validationErrors)
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
        versionRows,
        editorDraft,
        hasEditorDraft,
        validationErrors,
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
        'versionRows: $versionRows, '
        'editorDraft: $editorDraft, '
        'hasEditorDraft: $hasEditorDraft, '
        'validationErrors: $validationErrors, '
        'actionStates: $actionStates'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeRoleAdminView';
  }
}
