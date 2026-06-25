// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeApprovalCard {
  const AgentRuntimeApprovalCard({
    required this.id,
    required this.title,
    required this.status,
    required this.requiredApprover,
    required this.requestedAt,
    required this.contextSummary,
    required this.canDecide,
    required this.canResume,
    required this.decisionSummary,
  });

  static AgentRuntimeApprovalCard deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeApprovalCard(
      id: deserializer.deserializeString(),
      title: deserializer.deserializeString(),
      status: deserializer.deserializeString(),
      requiredApprover: deserializer.deserializeString(),
      requestedAt: deserializer.deserializeString(),
      contextSummary: deserializer.deserializeString(),
      canDecide: deserializer.deserializeBool(),
      canResume: deserializer.deserializeBool(),
      decisionSummary: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeApprovalCard bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeApprovalCard.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String id;
  final String title;
  final String status;
  final String requiredApprover;
  final String requestedAt;
  final String contextSummary;
  final bool canDecide;
  final bool canResume;
  final String decisionSummary;

  AgentRuntimeApprovalCard copyWith({
    String? id,
    String? title,
    String? status,
    String? requiredApprover,
    String? requestedAt,
    String? contextSummary,
    bool? canDecide,
    bool? canResume,
    String? decisionSummary,
  }) {
    return AgentRuntimeApprovalCard(
      id: id ?? this.id,
      title: title ?? this.title,
      status: status ?? this.status,
      requiredApprover: requiredApprover ?? this.requiredApprover,
      requestedAt: requestedAt ?? this.requestedAt,
      contextSummary: contextSummary ?? this.contextSummary,
      canDecide: canDecide ?? this.canDecide,
      canResume: canResume ?? this.canResume,
      decisionSummary: decisionSummary ?? this.decisionSummary,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(id);
    serializer.serializeString(title);
    serializer.serializeString(status);
    serializer.serializeString(requiredApprover);
    serializer.serializeString(requestedAt);
    serializer.serializeString(contextSummary);
    serializer.serializeBool(canDecide);
    serializer.serializeBool(canResume);
    serializer.serializeString(decisionSummary);
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

    return other is AgentRuntimeApprovalCard
      && id == other.id
      && title == other.title
      && status == other.status
      && requiredApprover == other.requiredApprover
      && requestedAt == other.requestedAt
      && contextSummary == other.contextSummary
      && canDecide == other.canDecide
      && canResume == other.canResume
      && decisionSummary == other.decisionSummary;
  }

  @override
  int get hashCode => Object.hash(
        id,
        title,
        status,
        requiredApprover,
        requestedAt,
        contextSummary,
        canDecide,
        canResume,
        decisionSummary,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'id: $id, '
        'title: $title, '
        'status: $status, '
        'requiredApprover: $requiredApprover, '
        'requestedAt: $requestedAt, '
        'contextSummary: $contextSummary, '
        'canDecide: $canDecide, '
        'canResume: $canResume, '
        'decisionSummary: $decisionSummary'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeApprovalCard';
  }
}
