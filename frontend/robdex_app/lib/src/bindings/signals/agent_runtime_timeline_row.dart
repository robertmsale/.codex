// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeTimelineRow {
  const AgentRuntimeTimelineRow({
    required this.id,
    required this.title,
    required this.subtitle,
    required this.status,
    required this.tone,
  });

  static AgentRuntimeTimelineRow deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeTimelineRow(
      id: deserializer.deserializeString(),
      title: deserializer.deserializeString(),
      subtitle: deserializer.deserializeString(),
      status: deserializer.deserializeString(),
      tone: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeTimelineRow bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeTimelineRow.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String id;
  final String title;
  final String subtitle;
  final String status;
  final String tone;

  AgentRuntimeTimelineRow copyWith({
    String? id,
    String? title,
    String? subtitle,
    String? status,
    String? tone,
  }) {
    return AgentRuntimeTimelineRow(
      id: id ?? this.id,
      title: title ?? this.title,
      subtitle: subtitle ?? this.subtitle,
      status: status ?? this.status,
      tone: tone ?? this.tone,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(id);
    serializer.serializeString(title);
    serializer.serializeString(subtitle);
    serializer.serializeString(status);
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

    return other is AgentRuntimeTimelineRow
      && id == other.id
      && title == other.title
      && subtitle == other.subtitle
      && status == other.status
      && tone == other.tone;
  }

  @override
  int get hashCode => Object.hash(
        id,
        title,
        subtitle,
        status,
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
        'status: $status, '
        'tone: $tone'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeTimelineRow';
  }
}
