// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class WorkbenchSelectedChatDeltaSignal {
  /// An async broadcast stream that listens for signals from Rust.
  /// It supports multiple subscriptions.
  /// Make sure to cancel the subscription when it's no longer needed,
  /// such as when a widget is disposed.
  static final rustSignalStream =
      _workbenchSelectedChatDeltaSignalStreamController.stream.asBroadcastStream();
        
  /// The latest signal value received from Rust.
  /// This is updated every time a new signal is received.
  /// It can be null if no signals have been received yet.
  static RustSignalPack<WorkbenchSelectedChatDeltaSignal>? latestRustSignal = null;

  const WorkbenchSelectedChatDeltaSignal({
    required this.threadId,
    required this.messageId,
    required this.appendedText,
    required this.replacementText,
    required this.deliveryState,
    required this.isFinal,
    required this.sequence,
    required this.metadataJson,
    required this.selectedEntryCount,
    required this.coalescedStreamUpdateCount,
    required this.droppedIntermediateStreamUpdateCount,
  });

  static WorkbenchSelectedChatDeltaSignal deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = WorkbenchSelectedChatDeltaSignal(
      threadId: deserializer.deserializeString(),
      messageId: deserializer.deserializeString(),
      appendedText: deserializer.deserializeString(),
      replacementText: deserializer.deserializeString(),
      deliveryState: deserializer.deserializeString(),
      isFinal: deserializer.deserializeBool(),
      sequence: deserializer.deserializeUint64(),
      metadataJson: deserializer.deserializeString(),
      selectedEntryCount: deserializer.deserializeUint32(),
      coalescedStreamUpdateCount: deserializer.deserializeUint32(),
      droppedIntermediateStreamUpdateCount: deserializer.deserializeUint32(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static WorkbenchSelectedChatDeltaSignal bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = WorkbenchSelectedChatDeltaSignal.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String threadId;
  final String messageId;
  final String appendedText;
  final String replacementText;
  final String deliveryState;
  final bool isFinal;
  final Uint64 sequence;
  final String metadataJson;
  final int selectedEntryCount;
  final int coalescedStreamUpdateCount;
  final int droppedIntermediateStreamUpdateCount;

  WorkbenchSelectedChatDeltaSignal copyWith({
    String? threadId,
    String? messageId,
    String? appendedText,
    String? replacementText,
    String? deliveryState,
    bool? isFinal,
    Uint64? sequence,
    String? metadataJson,
    int? selectedEntryCount,
    int? coalescedStreamUpdateCount,
    int? droppedIntermediateStreamUpdateCount,
  }) {
    return WorkbenchSelectedChatDeltaSignal(
      threadId: threadId ?? this.threadId,
      messageId: messageId ?? this.messageId,
      appendedText: appendedText ?? this.appendedText,
      replacementText: replacementText ?? this.replacementText,
      deliveryState: deliveryState ?? this.deliveryState,
      isFinal: isFinal ?? this.isFinal,
      sequence: sequence ?? this.sequence,
      metadataJson: metadataJson ?? this.metadataJson,
      selectedEntryCount: selectedEntryCount ?? this.selectedEntryCount,
      coalescedStreamUpdateCount: coalescedStreamUpdateCount ?? this.coalescedStreamUpdateCount,
      droppedIntermediateStreamUpdateCount: droppedIntermediateStreamUpdateCount ?? this.droppedIntermediateStreamUpdateCount,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(threadId);
    serializer.serializeString(messageId);
    serializer.serializeString(appendedText);
    serializer.serializeString(replacementText);
    serializer.serializeString(deliveryState);
    serializer.serializeBool(isFinal);
    serializer.serializeUint64(sequence);
    serializer.serializeString(metadataJson);
    serializer.serializeUint32(selectedEntryCount);
    serializer.serializeUint32(coalescedStreamUpdateCount);
    serializer.serializeUint32(droppedIntermediateStreamUpdateCount);
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

    return other is WorkbenchSelectedChatDeltaSignal
      && threadId == other.threadId
      && messageId == other.messageId
      && appendedText == other.appendedText
      && replacementText == other.replacementText
      && deliveryState == other.deliveryState
      && isFinal == other.isFinal
      && sequence == other.sequence
      && metadataJson == other.metadataJson
      && selectedEntryCount == other.selectedEntryCount
      && coalescedStreamUpdateCount == other.coalescedStreamUpdateCount
      && droppedIntermediateStreamUpdateCount == other.droppedIntermediateStreamUpdateCount;
  }

  @override
  int get hashCode => Object.hash(
        threadId,
        messageId,
        appendedText,
        replacementText,
        deliveryState,
        isFinal,
        sequence,
        metadataJson,
        selectedEntryCount,
        coalescedStreamUpdateCount,
        droppedIntermediateStreamUpdateCount,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'threadId: $threadId, '
        'messageId: $messageId, '
        'appendedText: $appendedText, '
        'replacementText: $replacementText, '
        'deliveryState: $deliveryState, '
        'isFinal: $isFinal, '
        'sequence: $sequence, '
        'metadataJson: $metadataJson, '
        'selectedEntryCount: $selectedEntryCount, '
        'coalescedStreamUpdateCount: $coalescedStreamUpdateCount, '
        'droppedIntermediateStreamUpdateCount: $droppedIntermediateStreamUpdateCount'
        ')';
      return true;
    }());

    return fullString ?? 'WorkbenchSelectedChatDeltaSignal';
  }
}

final _workbenchSelectedChatDeltaSignalStreamController =
    StreamController<RustSignalPack<WorkbenchSelectedChatDeltaSignal>>();
