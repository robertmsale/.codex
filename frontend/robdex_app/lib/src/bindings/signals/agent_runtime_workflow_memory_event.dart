// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeWorkflowMemoryEvent {
  const AgentRuntimeWorkflowMemoryEvent({
    required this.id,
    required this.title,
    required this.subtitle,
    required this.createdAt,
    required this.tone,
  });

  static AgentRuntimeWorkflowMemoryEvent deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeWorkflowMemoryEvent(
      id: deserializer.deserializeString(),
      title: deserializer.deserializeString(),
      subtitle: deserializer.deserializeString(),
      createdAt: deserializer.deserializeString(),
      tone: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeWorkflowMemoryEvent bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeWorkflowMemoryEvent.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String id;
  final String title;
  final String subtitle;
  final String createdAt;
  final String tone;

  AgentRuntimeWorkflowMemoryEvent copyWith({
    String? id,
    String? title,
    String? subtitle,
    String? createdAt,
    String? tone,
  }) {
    return AgentRuntimeWorkflowMemoryEvent(
      id: id ?? this.id,
      title: title ?? this.title,
      subtitle: subtitle ?? this.subtitle,
      createdAt: createdAt ?? this.createdAt,
      tone: tone ?? this.tone,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(id);
    serializer.serializeString(title);
    serializer.serializeString(subtitle);
    serializer.serializeString(createdAt);
    serializer.serializeString(tone);
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

    return other is AgentRuntimeWorkflowMemoryEvent
      && id == other.id
      && title == other.title
      && subtitle == other.subtitle
      && createdAt == other.createdAt
      && tone == other.tone;
  }

  @override
  int get hashCode => Object.hash(
        id,
        title,
        subtitle,
        createdAt,
        tone,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'id: $id, '
        'title: $title, '
        'subtitle: $subtitle, '
        'createdAt: $createdAt, '
        'tone: $tone'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeWorkflowMemoryEvent';
  }
}
