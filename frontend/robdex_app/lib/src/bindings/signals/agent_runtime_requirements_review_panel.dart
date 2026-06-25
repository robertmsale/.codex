// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeRequirementsReviewPanel {
  const AgentRuntimeRequirementsReviewPanel({
    required this.active,
    required this.status,
    required this.progressSummary,
    required this.reviewerStatus,
    required this.ownerActionStatus,
    required this.latestPacketStatus,
  });

  static AgentRuntimeRequirementsReviewPanel deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeRequirementsReviewPanel(
      active: deserializer.deserializeBool(),
      status: deserializer.deserializeString(),
      progressSummary: deserializer.deserializeString(),
      reviewerStatus: deserializer.deserializeString(),
      ownerActionStatus: deserializer.deserializeString(),
      latestPacketStatus: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeRequirementsReviewPanel bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeRequirementsReviewPanel.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final bool active;
  final String status;
  final String progressSummary;
  final String reviewerStatus;
  final String ownerActionStatus;
  final String latestPacketStatus;

  AgentRuntimeRequirementsReviewPanel copyWith({
    bool? active,
    String? status,
    String? progressSummary,
    String? reviewerStatus,
    String? ownerActionStatus,
    String? latestPacketStatus,
  }) {
    return AgentRuntimeRequirementsReviewPanel(
      active: active ?? this.active,
      status: status ?? this.status,
      progressSummary: progressSummary ?? this.progressSummary,
      reviewerStatus: reviewerStatus ?? this.reviewerStatus,
      ownerActionStatus: ownerActionStatus ?? this.ownerActionStatus,
      latestPacketStatus: latestPacketStatus ?? this.latestPacketStatus,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeBool(active);
    serializer.serializeString(status);
    serializer.serializeString(progressSummary);
    serializer.serializeString(reviewerStatus);
    serializer.serializeString(ownerActionStatus);
    serializer.serializeString(latestPacketStatus);
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

    return other is AgentRuntimeRequirementsReviewPanel
      && active == other.active
      && status == other.status
      && progressSummary == other.progressSummary
      && reviewerStatus == other.reviewerStatus
      && ownerActionStatus == other.ownerActionStatus
      && latestPacketStatus == other.latestPacketStatus;
  }

  @override
  int get hashCode => Object.hash(
        active,
        status,
        progressSummary,
        reviewerStatus,
        ownerActionStatus,
        latestPacketStatus,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'active: $active, '
        'status: $status, '
        'progressSummary: $progressSummary, '
        'reviewerStatus: $reviewerStatus, '
        'ownerActionStatus: $ownerActionStatus, '
        'latestPacketStatus: $latestPacketStatus'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeRequirementsReviewPanel';
  }
}
