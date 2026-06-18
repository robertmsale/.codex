// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class WorkbenchDiagnosticsSignal {
  /// An async broadcast stream that listens for signals from Rust.
  /// It supports multiple subscriptions.
  /// Make sure to cancel the subscription when it's no longer needed,
  /// such as when a widget is disposed.
  static final rustSignalStream =
      _workbenchDiagnosticsSignalStreamController.stream.asBroadcastStream();
        
  /// The latest signal value received from Rust.
  /// This is updated every time a new signal is received.
  /// It can be null if no signals have been received yet.
  static RustSignalPack<WorkbenchDiagnosticsSignal>? latestRustSignal = null;

  const WorkbenchDiagnosticsSignal({
    required this.websocketEventCountsJson,
    required this.websocketPayloadBytesJson,
    required this.nativeSignalCount,
    required this.serializedPayloadBytes,
    required this.dartFullSnapshotDecodeMicros,
    required this.dartSelectedChatDeltaApplyCount,
    required this.coalescedStreamUpdateCount,
    required this.droppedIntermediateStreamUpdateCount,
    required this.selectedTimelineEntryCount,
  });

  static WorkbenchDiagnosticsSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = WorkbenchDiagnosticsSignal(
      websocketEventCountsJson: deserializer.deserializeString(),
      websocketPayloadBytesJson: deserializer.deserializeString(),
      nativeSignalCount: deserializer.deserializeUint64(),
      serializedPayloadBytes: deserializer.deserializeUint64(),
      dartFullSnapshotDecodeMicros: deserializer.deserializeUint64(),
      dartSelectedChatDeltaApplyCount: deserializer.deserializeUint64(),
      coalescedStreamUpdateCount: deserializer.deserializeUint64(),
      droppedIntermediateStreamUpdateCount: deserializer.deserializeUint64(),
      selectedTimelineEntryCount: deserializer.deserializeUint32(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static WorkbenchDiagnosticsSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = WorkbenchDiagnosticsSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String websocketEventCountsJson;
  final String websocketPayloadBytesJson;
  final Uint64 nativeSignalCount;
  final Uint64 serializedPayloadBytes;
  final Uint64 dartFullSnapshotDecodeMicros;
  final Uint64 dartSelectedChatDeltaApplyCount;
  final Uint64 coalescedStreamUpdateCount;
  final Uint64 droppedIntermediateStreamUpdateCount;
  final int selectedTimelineEntryCount;

  WorkbenchDiagnosticsSignal copyWith({
    String? websocketEventCountsJson,
    String? websocketPayloadBytesJson,
    Uint64? nativeSignalCount,
    Uint64? serializedPayloadBytes,
    Uint64? dartFullSnapshotDecodeMicros,
    Uint64? dartSelectedChatDeltaApplyCount,
    Uint64? coalescedStreamUpdateCount,
    Uint64? droppedIntermediateStreamUpdateCount,
    int? selectedTimelineEntryCount,
  }) {
    return WorkbenchDiagnosticsSignal(
      websocketEventCountsJson: websocketEventCountsJson ?? this.websocketEventCountsJson,
      websocketPayloadBytesJson: websocketPayloadBytesJson ?? this.websocketPayloadBytesJson,
      nativeSignalCount: nativeSignalCount ?? this.nativeSignalCount,
      serializedPayloadBytes: serializedPayloadBytes ?? this.serializedPayloadBytes,
      dartFullSnapshotDecodeMicros: dartFullSnapshotDecodeMicros ?? this.dartFullSnapshotDecodeMicros,
      dartSelectedChatDeltaApplyCount: dartSelectedChatDeltaApplyCount ?? this.dartSelectedChatDeltaApplyCount,
      coalescedStreamUpdateCount: coalescedStreamUpdateCount ?? this.coalescedStreamUpdateCount,
      droppedIntermediateStreamUpdateCount: droppedIntermediateStreamUpdateCount ?? this.droppedIntermediateStreamUpdateCount,
      selectedTimelineEntryCount: selectedTimelineEntryCount ?? this.selectedTimelineEntryCount,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(websocketEventCountsJson);
    serializer.serializeString(websocketPayloadBytesJson);
    serializer.serializeUint64(nativeSignalCount);
    serializer.serializeUint64(serializedPayloadBytes);
    serializer.serializeUint64(dartFullSnapshotDecodeMicros);
    serializer.serializeUint64(dartSelectedChatDeltaApplyCount);
    serializer.serializeUint64(coalescedStreamUpdateCount);
    serializer.serializeUint64(droppedIntermediateStreamUpdateCount);
    serializer.serializeUint32(selectedTimelineEntryCount);
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

    return other is WorkbenchDiagnosticsSignal
      && websocketEventCountsJson == other.websocketEventCountsJson
      && websocketPayloadBytesJson == other.websocketPayloadBytesJson
      && nativeSignalCount == other.nativeSignalCount
      && serializedPayloadBytes == other.serializedPayloadBytes
      && dartFullSnapshotDecodeMicros == other.dartFullSnapshotDecodeMicros
      && dartSelectedChatDeltaApplyCount == other.dartSelectedChatDeltaApplyCount
      && coalescedStreamUpdateCount == other.coalescedStreamUpdateCount
      && droppedIntermediateStreamUpdateCount == other.droppedIntermediateStreamUpdateCount
      && selectedTimelineEntryCount == other.selectedTimelineEntryCount;
  }

  @override
  int get hashCode => Object.hash(
        websocketEventCountsJson,
        websocketPayloadBytesJson,
        nativeSignalCount,
        serializedPayloadBytes,
        dartFullSnapshotDecodeMicros,
        dartSelectedChatDeltaApplyCount,
        coalescedStreamUpdateCount,
        droppedIntermediateStreamUpdateCount,
        selectedTimelineEntryCount,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'websocketEventCountsJson: $websocketEventCountsJson, '
        'websocketPayloadBytesJson: $websocketPayloadBytesJson, '
        'nativeSignalCount: $nativeSignalCount, '
        'serializedPayloadBytes: $serializedPayloadBytes, '
        'dartFullSnapshotDecodeMicros: $dartFullSnapshotDecodeMicros, '
        'dartSelectedChatDeltaApplyCount: $dartSelectedChatDeltaApplyCount, '
        'coalescedStreamUpdateCount: $coalescedStreamUpdateCount, '
        'droppedIntermediateStreamUpdateCount: $droppedIntermediateStreamUpdateCount, '
        'selectedTimelineEntryCount: $selectedTimelineEntryCount'
        ')';
      return true;
    }());

    return fullString ?? 'WorkbenchDiagnosticsSignal';
  }
}

final _workbenchDiagnosticsSignalStreamController =
    StreamController<RustSignalPack<WorkbenchDiagnosticsSignal>>();
