// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeChatEntry {
  const AgentRuntimeChatEntry({
    required this.id,
    required this.author,
    required this.displayLabel,
    required this.timestamp,
    required this.hasTimestamp,
    required this.body,
    required this.subtitle,
    required this.kind,
    required this.status,
    required this.processId,
    required this.hasProcessId,
    required this.command,
    required this.output,
    required this.deliveryState,
    required this.isStreaming,
    required this.isTool,
  });

  static AgentRuntimeChatEntry deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeChatEntry(
      id: deserializer.deserializeString(),
      author: deserializer.deserializeString(),
      displayLabel: deserializer.deserializeString(),
      timestamp: deserializer.deserializeString(),
      hasTimestamp: deserializer.deserializeBool(),
      body: deserializer.deserializeString(),
      subtitle: deserializer.deserializeString(),
      kind: deserializer.deserializeString(),
      status: deserializer.deserializeString(),
      processId: deserializer.deserializeString(),
      hasProcessId: deserializer.deserializeBool(),
      command: deserializer.deserializeString(),
      output: deserializer.deserializeString(),
      deliveryState: deserializer.deserializeString(),
      isStreaming: deserializer.deserializeBool(),
      isTool: deserializer.deserializeBool(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeChatEntry bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeChatEntry.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String id;
  final String author;
  final String displayLabel;
  final String timestamp;
  final bool hasTimestamp;
  final String body;
  final String subtitle;
  final String kind;
  final String status;
  final String processId;
  final bool hasProcessId;
  final String command;
  final String output;
  final String deliveryState;
  final bool isStreaming;
  final bool isTool;

  AgentRuntimeChatEntry copyWith({
    String? id,
    String? author,
    String? displayLabel,
    String? timestamp,
    bool? hasTimestamp,
    String? body,
    String? subtitle,
    String? kind,
    String? status,
    String? processId,
    bool? hasProcessId,
    String? command,
    String? output,
    String? deliveryState,
    bool? isStreaming,
    bool? isTool,
  }) {
    return AgentRuntimeChatEntry(
      id: id ?? this.id,
      author: author ?? this.author,
      displayLabel: displayLabel ?? this.displayLabel,
      timestamp: timestamp ?? this.timestamp,
      hasTimestamp: hasTimestamp ?? this.hasTimestamp,
      body: body ?? this.body,
      subtitle: subtitle ?? this.subtitle,
      kind: kind ?? this.kind,
      status: status ?? this.status,
      processId: processId ?? this.processId,
      hasProcessId: hasProcessId ?? this.hasProcessId,
      command: command ?? this.command,
      output: output ?? this.output,
      deliveryState: deliveryState ?? this.deliveryState,
      isStreaming: isStreaming ?? this.isStreaming,
      isTool: isTool ?? this.isTool,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(id);
    serializer.serializeString(author);
    serializer.serializeString(displayLabel);
    serializer.serializeString(timestamp);
    serializer.serializeBool(hasTimestamp);
    serializer.serializeString(body);
    serializer.serializeString(subtitle);
    serializer.serializeString(kind);
    serializer.serializeString(status);
    serializer.serializeString(processId);
    serializer.serializeBool(hasProcessId);
    serializer.serializeString(command);
    serializer.serializeString(output);
    serializer.serializeString(deliveryState);
    serializer.serializeBool(isStreaming);
    serializer.serializeBool(isTool);
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

    return other is AgentRuntimeChatEntry
      && id == other.id
      && author == other.author
      && displayLabel == other.displayLabel
      && timestamp == other.timestamp
      && hasTimestamp == other.hasTimestamp
      && body == other.body
      && subtitle == other.subtitle
      && kind == other.kind
      && status == other.status
      && processId == other.processId
      && hasProcessId == other.hasProcessId
      && command == other.command
      && output == other.output
      && deliveryState == other.deliveryState
      && isStreaming == other.isStreaming
      && isTool == other.isTool;
  }

  @override
  int get hashCode => Object.hash(
        id,
        author,
        displayLabel,
        timestamp,
        hasTimestamp,
        body,
        subtitle,
        kind,
        status,
        processId,
        hasProcessId,
        command,
        output,
        deliveryState,
        isStreaming,
        isTool,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'id: $id, '
        'author: $author, '
        'displayLabel: $displayLabel, '
        'timestamp: $timestamp, '
        'hasTimestamp: $hasTimestamp, '
        'body: $body, '
        'subtitle: $subtitle, '
        'kind: $kind, '
        'status: $status, '
        'processId: $processId, '
        'hasProcessId: $hasProcessId, '
        'command: $command, '
        'output: $output, '
        'deliveryState: $deliveryState, '
        'isStreaming: $isStreaming, '
        'isTool: $isTool'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeChatEntry';
  }
}
