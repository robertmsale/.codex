// ignore_for_file: type=lint, type=warning
part of 'signals.dart';


@immutable
class AgentRuntimeRoleEditorModelDefaults {
  const AgentRuntimeRoleEditorModelDefaults({
    required this.model,
    required this.reasoningEffort,
  });

  static AgentRuntimeRoleEditorModelDefaults deserialize(BinaryDeserializer deserializer) {
    deserializer.increaseContainerDepth();
    final instance = AgentRuntimeRoleEditorModelDefaults(
      model: deserializer.deserializeString(),
      reasoningEffort: deserializer.deserializeString(),
    );
    deserializer.decreaseContainerDepth();
    return instance;
  }

  static AgentRuntimeRoleEditorModelDefaults bincodeDeserialize(Uint8List input) {
    final deserializer = BincodeDeserializer(input);
    final value = AgentRuntimeRoleEditorModelDefaults.deserialize(deserializer);
    if (deserializer.offset < input.length) {
      throw Exception('Some input bytes were not read');
    }
    return value;
  }

  final String model;
  final String reasoningEffort;

  AgentRuntimeRoleEditorModelDefaults copyWith({
    String? model,
    String? reasoningEffort,
  }) {
    return AgentRuntimeRoleEditorModelDefaults(
      model: model ?? this.model,
      reasoningEffort: reasoningEffort ?? this.reasoningEffort,
    );
  }

  void serialize(BinarySerializer serializer) {
    serializer.increaseContainerDepth();
    serializer.serializeString(model);
    serializer.serializeString(reasoningEffort);
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

    return other is AgentRuntimeRoleEditorModelDefaults
      && model == other.model
      && reasoningEffort == other.reasoningEffort;
  }

  @override
  int get hashCode => Object.hash(
        model,
        reasoningEffort,
      );

  @override
  String toString() {
    String? fullString;

    assert(() {
      fullString = '$runtimeType('
        'model: $model, '
        'reasoningEffort: $reasoningEffort'
        ')';
      return true;
    }());

    return fullString ?? 'AgentRuntimeRoleEditorModelDefaults';
  }
}
