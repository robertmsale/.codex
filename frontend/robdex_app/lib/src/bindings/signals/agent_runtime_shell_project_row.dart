// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeShellProjectRow {
  const AgentRuntimeShellProjectRow({
    required this.id,
    required this.title,
    required this.subtitle,
    required this.selectable,
  });

  static AgentRuntimeShellProjectRow deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeShellProjectRow(
      id: deserializer.deserializeString(),
      title: deserializer.deserializeString(),
      subtitle: deserializer.deserializeString(),
      selectable: deserializer.deserializeBool(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeShellProjectRow bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeShellProjectRow.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String id;
  final String title;
  final String subtitle;
  final bool selectable;

  AgentRuntimeShellProjectRow copyWith({
    String? id,
    String? title,
    String? subtitle,
    bool? selectable,
  }) {
    return AgentRuntimeShellProjectRow(
      id: id ?? this.id,
      title: title ?? this.title,
      subtitle: subtitle ?? this.subtitle,
      selectable: selectable ?? this.selectable,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(id);
    serializer.serializeString(title);
    serializer.serializeString(subtitle);
    serializer.serializeBool(selectable);
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

    return other is AgentRuntimeShellProjectRow
      && id == other.id
      && title == other.title
      && subtitle == other.subtitle
      && selectable == other.selectable;
  }

  @override
  int get hashCode => Object.hash(
        id,
        title,
        subtitle,
        selectable,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'id: $id, '
        'title: $title, '
        'subtitle: $subtitle, '
        'selectable: $selectable'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeShellProjectRow';
  }
}
