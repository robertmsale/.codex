// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeOperationSurface {
  const AgentRuntimeOperationSurface({
    required this.surfaceId,
    required this.title,
    required this.subtitle,
    required this.rows,
    required this.actions,
  });

  static AgentRuntimeOperationSurface deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeOperationSurface(
      surfaceId: deserializer.deserializeString(),
      title: deserializer.deserializeString(),
      subtitle: deserializer.deserializeString(),
      rows: TraitHelpers.deserializeVectorAgentRuntimeFact(deserializer),
      actions: TraitHelpers.deserializeVectorAgentRuntimeActionRow(deserializer),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeOperationSurface bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeOperationSurface.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String surfaceId;
  final String title;
  final String subtitle;
  final List<AgentRuntimeFact> rows;
  final List<AgentRuntimeActionRow> actions;

  AgentRuntimeOperationSurface copyWith({
    String? surfaceId,
    String? title,
    String? subtitle,
    List<AgentRuntimeFact>? rows,
    List<AgentRuntimeActionRow>? actions,
  }) {
    return AgentRuntimeOperationSurface(
      surfaceId: surfaceId ?? this.surfaceId,
      title: title ?? this.title,
      subtitle: subtitle ?? this.subtitle,
      rows: rows ?? this.rows,
      actions: actions ?? this.actions,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(surfaceId);
    serializer.serializeString(title);
    serializer.serializeString(subtitle);
    TraitHelpers.serializeVectorAgentRuntimeFact(rows, serializer);
    TraitHelpers.serializeVectorAgentRuntimeActionRow(actions, serializer);
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

    return other is AgentRuntimeOperationSurface
      && surfaceId == other.surfaceId
      && title == other.title
      && subtitle == other.subtitle
      && listEquals(rows, other.rows)
      && listEquals(actions, other.actions);
  }

  @override
  int get hashCode => Object.hash(
        surfaceId,
        title,
        subtitle,
        rows,
        actions,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'surfaceId: $surfaceId, '
        'title: $title, '
        'subtitle: $subtitle, '
        'rows: $rows, '
        'actions: $actions'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeOperationSurface';
  }
}
