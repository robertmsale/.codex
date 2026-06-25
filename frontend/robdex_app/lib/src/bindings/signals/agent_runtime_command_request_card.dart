// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeCommandRequestCard {
  const AgentRuntimeCommandRequestCard({
    required this.id,
    required this.actionId,
    required this.title,
    required this.operation,
    required this.status,
    required this.scopeSummary,
    required this.policySummary,
    required this.previewStatus,
    required this.applyStatus,
    required this.canPreview,
    required this.canDecide,
    required this.canApply,
    required this.commandSummary,
  });

  static AgentRuntimeCommandRequestCard deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeCommandRequestCard(
      id: deserializer.deserializeString(),
      actionId: deserializer.deserializeString(),
      title: deserializer.deserializeString(),
      operation: deserializer.deserializeString(),
      status: deserializer.deserializeString(),
      scopeSummary: deserializer.deserializeString(),
      policySummary: deserializer.deserializeString(),
      previewStatus: deserializer.deserializeString(),
      applyStatus: deserializer.deserializeString(),
      canPreview: deserializer.deserializeBool(),
      canDecide: deserializer.deserializeBool(),
      canApply: deserializer.deserializeBool(),
      commandSummary: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeCommandRequestCard bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeCommandRequestCard.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String id;
  final String actionId;
  final String title;
  final String operation;
  final String status;
  final String scopeSummary;
  final String policySummary;
  final String previewStatus;
  final String applyStatus;
  final bool canPreview;
  final bool canDecide;
  final bool canApply;
  final String commandSummary;

  AgentRuntimeCommandRequestCard copyWith({
    String? id,
    String? actionId,
    String? title,
    String? operation,
    String? status,
    String? scopeSummary,
    String? policySummary,
    String? previewStatus,
    String? applyStatus,
    bool? canPreview,
    bool? canDecide,
    bool? canApply,
    String? commandSummary,
  }) {
    return AgentRuntimeCommandRequestCard(
      id: id ?? this.id,
      actionId: actionId ?? this.actionId,
      title: title ?? this.title,
      operation: operation ?? this.operation,
      status: status ?? this.status,
      scopeSummary: scopeSummary ?? this.scopeSummary,
      policySummary: policySummary ?? this.policySummary,
      previewStatus: previewStatus ?? this.previewStatus,
      applyStatus: applyStatus ?? this.applyStatus,
      canPreview: canPreview ?? this.canPreview,
      canDecide: canDecide ?? this.canDecide,
      canApply: canApply ?? this.canApply,
      commandSummary: commandSummary ?? this.commandSummary,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(id);
    serializer.serializeString(actionId);
    serializer.serializeString(title);
    serializer.serializeString(operation);
    serializer.serializeString(status);
    serializer.serializeString(scopeSummary);
    serializer.serializeString(policySummary);
    serializer.serializeString(previewStatus);
    serializer.serializeString(applyStatus);
    serializer.serializeBool(canPreview);
    serializer.serializeBool(canDecide);
    serializer.serializeBool(canApply);
    serializer.serializeString(commandSummary);
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

    return other is AgentRuntimeCommandRequestCard
      && id == other.id
      && actionId == other.actionId
      && title == other.title
      && operation == other.operation
      && status == other.status
      && scopeSummary == other.scopeSummary
      && policySummary == other.policySummary
      && previewStatus == other.previewStatus
      && applyStatus == other.applyStatus
      && canPreview == other.canPreview
      && canDecide == other.canDecide
      && canApply == other.canApply
      && commandSummary == other.commandSummary;
  }

  @override
  int get hashCode => Object.hash(
        id,
        actionId,
        title,
        operation,
        status,
        scopeSummary,
        policySummary,
        previewStatus,
        applyStatus,
        canPreview,
        canDecide,
        canApply,
        commandSummary,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'id: $id, '
        'actionId: $actionId, '
        'title: $title, '
        'operation: $operation, '
        'status: $status, '
        'scopeSummary: $scopeSummary, '
        'policySummary: $policySummary, '
        'previewStatus: $previewStatus, '
        'applyStatus: $applyStatus, '
        'canPreview: $canPreview, '
        'canDecide: $canDecide, '
        'canApply: $canApply, '
        'commandSummary: $commandSummary'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeCommandRequestCard';
  }
}
