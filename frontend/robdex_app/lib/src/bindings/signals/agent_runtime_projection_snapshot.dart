// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeProjectionSnapshot {
  const AgentRuntimeProjectionSnapshot({
    required this.watermark,
    required this.sessionCount,
    required this.timelineCount,
    required this.actionCount,
    required this.roleCount,
    required this.workflowMemoryCount,
    this.selectedChatEntries = const [],
  });

  static AgentRuntimeProjectionSnapshot deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeProjectionSnapshot(
      watermark: deserializer.deserializeInt64(),
      sessionCount: deserializer.deserializeInt64(),
      timelineCount: deserializer.deserializeInt64(),
      actionCount: deserializer.deserializeInt64(),
      roleCount: deserializer.deserializeInt64(),
      workflowMemoryCount: deserializer.deserializeInt64(),
      selectedChatEntries: TraitHelpers.deserializeVectorAgentRuntimeChatEntry(deserializer),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeProjectionSnapshot bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeProjectionSnapshot.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final int watermark;
  final int sessionCount;
  final int timelineCount;
  final int actionCount;
  final int roleCount;
  final int workflowMemoryCount;
  final List<AgentRuntimeChatEntry> selectedChatEntries;

  AgentRuntimeProjectionSnapshot copyWith({
    int? watermark,
    int? sessionCount,
    int? timelineCount,
    int? actionCount,
    int? roleCount,
    int? workflowMemoryCount,
    List<AgentRuntimeChatEntry>? selectedChatEntries,
  }) {
    return AgentRuntimeProjectionSnapshot(
      watermark: watermark ?? this.watermark,
      sessionCount: sessionCount ?? this.sessionCount,
      timelineCount: timelineCount ?? this.timelineCount,
      actionCount: actionCount ?? this.actionCount,
      roleCount: roleCount ?? this.roleCount,
      workflowMemoryCount: workflowMemoryCount ?? this.workflowMemoryCount,
      selectedChatEntries: selectedChatEntries ?? this.selectedChatEntries,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeInt64(watermark);
    serializer.serializeInt64(sessionCount);
    serializer.serializeInt64(timelineCount);
    serializer.serializeInt64(actionCount);
    serializer.serializeInt64(roleCount);
    serializer.serializeInt64(workflowMemoryCount);
    TraitHelpers.serializeVectorAgentRuntimeChatEntry(selectedChatEntries, serializer);
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

    return other is AgentRuntimeProjectionSnapshot
      && watermark == other.watermark
      && sessionCount == other.sessionCount
      && timelineCount == other.timelineCount
      && actionCount == other.actionCount
      && roleCount == other.roleCount
      && workflowMemoryCount == other.workflowMemoryCount
      && selectedChatEntries.length == other.selectedChatEntries.length
      && Iterable.generate(selectedChatEntries.length).every((index) => selectedChatEntries[index] == other.selectedChatEntries[index]);
  }

  @override
  int get hashCode => Object.hash(
        watermark,
        sessionCount,
        timelineCount,
        actionCount,
        roleCount,
        workflowMemoryCount,
        Object.hashAll(selectedChatEntries),
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'watermark: $watermark, '
        'sessionCount: $sessionCount, '
        'timelineCount: $timelineCount, '
        'actionCount: $actionCount, '
        'roleCount: $roleCount, '
        'workflowMemoryCount: $workflowMemoryCount, '
        'selectedChatEntries: $selectedChatEntries'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeProjectionSnapshot';
  }
}
