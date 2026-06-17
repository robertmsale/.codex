// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeRoleVersionRow {
  const AgentRuntimeRoleVersionRow({
    required this.versionId,
    required this.version,
    required this.status,
    required this.createdAt,
    required this.isCurrent,
    required this.canActivate,
  });

  static AgentRuntimeRoleVersionRow deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeRoleVersionRow(
      versionId: deserializer.deserializeString(),
      version: deserializer.deserializeString(),
      status: deserializer.deserializeString(),
      createdAt: deserializer.deserializeString(),
      isCurrent: deserializer.deserializeBool(),
      canActivate: deserializer.deserializeBool(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeRoleVersionRow bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeRoleVersionRow.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String versionId;
  final String version;
  final String status;
  final String createdAt;
  final bool isCurrent;
  final bool canActivate;

  AgentRuntimeRoleVersionRow copyWith({
    String? versionId,
    String? version,
    String? status,
    String? createdAt,
    bool? isCurrent,
    bool? canActivate,
  }) {
    return AgentRuntimeRoleVersionRow(
      versionId: versionId ?? this.versionId,
      version: version ?? this.version,
      status: status ?? this.status,
      createdAt: createdAt ?? this.createdAt,
      isCurrent: isCurrent ?? this.isCurrent,
      canActivate: canActivate ?? this.canActivate,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(versionId);
    serializer.serializeString(version);
    serializer.serializeString(status);
    serializer.serializeString(createdAt);
    serializer.serializeBool(isCurrent);
    serializer.serializeBool(canActivate);
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

    return other is AgentRuntimeRoleVersionRow
      && versionId == other.versionId
      && version == other.version
      && status == other.status
      && createdAt == other.createdAt
      && isCurrent == other.isCurrent
      && canActivate == other.canActivate;
  }

  @override
  int get hashCode => Object.hash(
        versionId,
        version,
        status,
        createdAt,
        isCurrent,
        canActivate,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'versionId: $versionId, '
        'version: $version, '
        'status: $status, '
        'createdAt: $createdAt, '
        'isCurrent: $isCurrent, '
        'canActivate: $canActivate'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeRoleVersionRow';
  }
}
