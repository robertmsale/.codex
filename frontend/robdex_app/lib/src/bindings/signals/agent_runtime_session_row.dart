// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeSessionRow {
  const AgentRuntimeSessionRow({
    required this.id,
    required this.title,
    required this.status,
    required this.subtitle,
    required this.groupLabel,
    required this.tone,
  });

  static AgentRuntimeSessionRow deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeSessionRow(
      id: deserializer.deserializeString(),
      title: deserializer.deserializeString(),
      status: deserializer.deserializeString(),
      subtitle: deserializer.deserializeString(),
      groupLabel: deserializer.deserializeString(),
      tone: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeSessionRow bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeSessionRow.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String id;
  final String title;
  final String status;
  final String subtitle;
  final String groupLabel;
  final String tone;

  AgentRuntimeSessionRow copyWith({
    String? id,
    String? title,
    String? status,
    String? subtitle,
    String? groupLabel,
    String? tone,
  }) {
    return AgentRuntimeSessionRow(
      id: id ?? this.id,
      title: title ?? this.title,
      status: status ?? this.status,
      subtitle: subtitle ?? this.subtitle,
      groupLabel: groupLabel ?? this.groupLabel,
      tone: tone ?? this.tone,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(id);
    serializer.serializeString(title);
    serializer.serializeString(status);
    serializer.serializeString(subtitle);
    serializer.serializeString(groupLabel);
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

    return other is AgentRuntimeSessionRow
      && id == other.id
      && title == other.title
      && status == other.status
      && subtitle == other.subtitle
      && groupLabel == other.groupLabel
      && tone == other.tone;
  }

  @override
  int get hashCode => Object.hash(
        id,
        title,
        status,
        subtitle,
        groupLabel,
        tone,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'id: $id, '
        'title: $title, '
        'status: $status, '
        'subtitle: $subtitle, '
        'groupLabel: $groupLabel, '
        'tone: $tone'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeSessionRow';
  }
}
