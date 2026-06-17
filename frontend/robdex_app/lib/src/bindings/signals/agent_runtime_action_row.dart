// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeActionRow {
  const AgentRuntimeActionRow({
    required this.id,
    required this.title,
    required this.subtitle,
    required this.kind,
    required this.stateText,
    required this.tone,
  });

  static AgentRuntimeActionRow deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeActionRow(
      id: deserializer.deserializeString(),
      title: deserializer.deserializeString(),
      subtitle: deserializer.deserializeString(),
      kind: deserializer.deserializeString(),
      stateText: deserializer.deserializeString(),
      tone: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeActionRow bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeActionRow.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String id;
  final String title;
  final String subtitle;
  final String kind;
  final String stateText;
  final String tone;

  AgentRuntimeActionRow copyWith({
    String? id,
    String? title,
    String? subtitle,
    String? kind,
    String? stateText,
    String? tone,
  }) {
    return AgentRuntimeActionRow(
      id: id ?? this.id,
      title: title ?? this.title,
      subtitle: subtitle ?? this.subtitle,
      kind: kind ?? this.kind,
      stateText: stateText ?? this.stateText,
      tone: tone ?? this.tone,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(id);
    serializer.serializeString(title);
    serializer.serializeString(subtitle);
    serializer.serializeString(kind);
    serializer.serializeString(stateText);
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

    return other is AgentRuntimeActionRow
      && id == other.id
      && title == other.title
      && subtitle == other.subtitle
      && kind == other.kind
      && stateText == other.stateText
      && tone == other.tone;
  }

  @override
  int get hashCode => Object.hash(
        id,
        title,
        subtitle,
        kind,
        stateText,
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
        'kind: $kind, '
        'stateText: $stateText, '
        'tone: $tone'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeActionRow';
  }
}
