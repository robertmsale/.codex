// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeModelOption {
  const AgentRuntimeModelOption({
    required this.id,
    required this.displayLabel,
    required this.source,
    required this.isDefault,
  });

  static AgentRuntimeModelOption deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeModelOption(
      id: deserializer.deserializeString(),
      displayLabel: deserializer.deserializeString(),
      source: deserializer.deserializeString(),
      isDefault: deserializer.deserializeBool(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeModelOption bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeModelOption.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String id;
  final String displayLabel;
  final String source;
  final bool isDefault;

  AgentRuntimeModelOption copyWith({
    String? id,
    String? displayLabel,
    String? source,
    bool? isDefault,
  }) {
    return AgentRuntimeModelOption(
      id: id ?? this.id,
      displayLabel: displayLabel ?? this.displayLabel,
      source: source ?? this.source,
      isDefault: isDefault ?? this.isDefault,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(id);
    serializer.serializeString(displayLabel);
    serializer.serializeString(source);
    serializer.serializeBool(isDefault);
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

    return other is AgentRuntimeModelOption
      && id == other.id
      && displayLabel == other.displayLabel
      && source == other.source
      && isDefault == other.isDefault;
  }

  @override
  int get hashCode => Object.hash(
        id,
        displayLabel,
        source,
        isDefault,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'id: $id, '
        'displayLabel: $displayLabel, '
        'source: $source, '
        'isDefault: $isDefault'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeModelOption';
  }
}
