// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeShellProjectRow {
  const AgentRuntimeShellProjectRow({
    required this.id,
    required this.title,
    required this.subtitle,
    required this.selectable,
    required this.defaultWorkdir,
    required this.defaultWorktreeRoot,
    required this.defaultRoleId,
    required this.defaultModel,
    required this.tracked,
    required this.listed,
    required this.archived,
  });

  static AgentRuntimeShellProjectRow deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeShellProjectRow(
      id: deserializer.deserializeString(),
      title: deserializer.deserializeString(),
      subtitle: deserializer.deserializeString(),
      selectable: deserializer.deserializeBool(),
      defaultWorkdir: deserializer.deserializeString(),
      defaultWorktreeRoot: deserializer.deserializeString(),
      defaultRoleId: deserializer.deserializeString(),
      defaultModel: deserializer.deserializeString(),
      tracked: deserializer.deserializeBool(),
      listed: deserializer.deserializeBool(),
      archived: deserializer.deserializeBool(),
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
  final String defaultWorkdir;
  final String defaultWorktreeRoot;
  final String defaultRoleId;
  final String defaultModel;
  final bool tracked;
  final bool listed;
  final bool archived;

  AgentRuntimeShellProjectRow copyWith({
    String? id,
    String? title,
    String? subtitle,
    bool? selectable,
    String? defaultWorkdir,
    String? defaultWorktreeRoot,
    String? defaultRoleId,
    String? defaultModel,
    bool? tracked,
    bool? listed,
    bool? archived,
  }) {
    return AgentRuntimeShellProjectRow(
      id: id ?? this.id,
      title: title ?? this.title,
      subtitle: subtitle ?? this.subtitle,
      selectable: selectable ?? this.selectable,
      defaultWorkdir: defaultWorkdir ?? this.defaultWorkdir,
      defaultWorktreeRoot: defaultWorktreeRoot ?? this.defaultWorktreeRoot,
      defaultRoleId: defaultRoleId ?? this.defaultRoleId,
      defaultModel: defaultModel ?? this.defaultModel,
      tracked: tracked ?? this.tracked,
      listed: listed ?? this.listed,
      archived: archived ?? this.archived,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(id);
    serializer.serializeString(title);
    serializer.serializeString(subtitle);
    serializer.serializeBool(selectable);
    serializer.serializeString(defaultWorkdir);
    serializer.serializeString(defaultWorktreeRoot);
    serializer.serializeString(defaultRoleId);
    serializer.serializeString(defaultModel);
    serializer.serializeBool(tracked);
    serializer.serializeBool(listed);
    serializer.serializeBool(archived);
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
      && selectable == other.selectable
      && defaultWorkdir == other.defaultWorkdir
      && defaultWorktreeRoot == other.defaultWorktreeRoot
      && defaultRoleId == other.defaultRoleId
      && defaultModel == other.defaultModel
      && tracked == other.tracked
      && listed == other.listed
      && archived == other.archived;
  }

  @override
  int get hashCode => Object.hash(
        id,
        title,
        subtitle,
        selectable,
        defaultWorkdir,
        defaultWorktreeRoot,
        defaultRoleId,
        defaultModel,
        tracked,
        listed,
        archived,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'id: $id, '
        'title: $title, '
        'subtitle: $subtitle, '
        'selectable: $selectable, '
        'defaultWorkdir: $defaultWorkdir, '
        'defaultWorktreeRoot: $defaultWorktreeRoot, '
        'defaultRoleId: $defaultRoleId, '
        'defaultModel: $defaultModel, '
        'tracked: $tracked, '
        'listed: $listed, '
        'archived: $archived'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeShellProjectRow';
  }
}
