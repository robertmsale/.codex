// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeRoleRow {
  const AgentRuntimeRoleRow({
    required this.id,
    required this.title,
    required this.subtitle,
    required this.status,
    required this.tone,
    required this.currentVersion,
  });

  static AgentRuntimeRoleRow deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeRoleRow(
      id: deserializer.deserializeString(),
      title: deserializer.deserializeString(),
      subtitle: deserializer.deserializeString(),
      status: deserializer.deserializeString(),
      tone: deserializer.deserializeString(),
      currentVersion: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeRoleRow bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeRoleRow.deserialize(deserializer);
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
  final String currentVersion;

  AgentRuntimeRoleRow copyWith({
    String? id,
    String? title,
    String? subtitle,
    String? status,
    String? tone,
    String? currentVersion,
  }) {
    return AgentRuntimeRoleRow(
      id: id ?? this.id,
      title: title ?? this.title,
      subtitle: subtitle ?? this.subtitle,
      status: status ?? this.status,
      tone: tone ?? this.tone,
      currentVersion: currentVersion ?? this.currentVersion,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(id);
    serializer.serializeString(title);
    serializer.serializeString(subtitle);
    serializer.serializeString(status);
    serializer.serializeString(tone);
    serializer.serializeString(currentVersion);
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

    return other is AgentRuntimeRoleRow
      && id == other.id
      && title == other.title
      && subtitle == other.subtitle
      && status == other.status
      && tone == other.tone
      && currentVersion == other.currentVersion;
  }

  @override
  int get hashCode => Object.hash(
        id,
        title,
        subtitle,
        status,
        tone,
        currentVersion,
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
        'tone: $tone, '
        'currentVersion: $currentVersion'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeRoleRow';
  }
}
